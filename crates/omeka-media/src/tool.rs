//! The external commands a still derivative needs: `ffmpeg` for a video's
//! poster and for an AVIF or HEIC frame, `pdftoppm` for a PDF's first page,
//! `ffprobe` for the duration and frame size a time-based canvas needs, and
//! `exiftool` for the JPEG preview a camera RAW file embeds.
//!
//! The same seam carries the commands an access copy needs, which D3 and D4
//! run: `ffmpeg` for the H.264/AAC MP4 and the AAC M4A, `exiftool` for a RAW's
//! embedded preview, `dcraw_emu` (libraw) for a RAW decode, `vips` for the
//! TIFF, PSD, EXR, HDR and HEIC layouts the image crate does not read, and `gs`
//! (ghostscript) for the first page of an EPS, AI or PS file. [`AccessTool`]
//! names them, and each argument list is a function below. None of those runs
//! here: this module only knows how to find the command and how to ask it.
//!
//! The image crate decodes the types GD decodes, plus TIFF, JPEG 2000 and SVG.
//! It has no video, PDF, AVIF, HEIC or RAW decoder, so those get their still
//! from a command. The commands are optional at runtime: the shipped image
//! carries all four (`packaging/Dockerfile`), and an installation without them
//! skips the derivative with a typed [`ToolError`] rather than failing the
//! ingest.
//!
//! One lookup per process ([`Tools::on_path`]), one bounded run per still. No
//! run passes through a shell, every run has a timeout and a cap on what it may
//! write, and the still is bounded before it reaches the decoder: `ffmpeg`
//! scales to at most 1600 pixels a side and `pdftoppm` rasterises at 150 DPI.
//! Nothing here reads the whole file: `ffmpeg` seeks before its input, so a
//! 2 GB video is opened at one point rather than read through.
//!
//! [`Still`] is the table of what each media type asks for. A caller asks
//! `Still::of(media_type)` and calls `make`; it never builds an argument list.

use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;

use image::metadata::Orientation;

/// How long one command may run before it is killed.
pub const TIMEOUT: Duration = Duration::from_secs(30);

/// How long `ffprobe` may run: it reads headers, not pixels.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// The longest still a command may write to standard output or to a file.
const STILL_LIMIT: usize = 32 * 1024 * 1024;

/// The largest `ffprobe` answer kept, in bytes.
const PROBE_LIMIT: usize = 1024 * 1024;

/// The most of a command's diagnostic output that is kept for the reason.
const STDERR_LIMIT: usize = 8 * 1024;

/// The longest side of a still, in pixels. The large derivative is 800, so
/// this leaves room to downscale without decoding a poster-sized frame whole.
const STILL_SIDE: u32 = 1600;

/// The resolution a PDF page is rasterised at.
const PDF_DPI: u32 = 150;

/// Why a command made no still. The ingest treats every one of these as a
/// skipped derivative, and the backfill reports it as the row's message.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ToolError {
    /// The command is not on `PATH`, so this installation makes no such still.
    #[error("the {0} command is not on PATH")]
    ToolMissing(&'static str),
    /// The command did not finish within the timeout and was killed.
    #[error("the {0} command did not finish within {1:?}")]
    Timeout(&'static str, Duration),
    /// The command exited non-zero, wrote too much, or wrote nothing.
    #[error("the {0} command failed: {1}")]
    Failed(&'static str, String),
}

/// A still a command made, and the orientation its container states for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MadeStill {
    /// The encoded still.
    pub bytes: Vec<u8>,
    /// The turn the container's EXIF tag asks for, when the still carries no
    /// tag of its own. A RAW's embedded preview is stored as the sensor saw
    /// it, and the RAW file's `Orientation` says how it goes upright.
    pub orientation: Option<Orientation>,
}

/// What a media type asks for: a first frame, a frame ten percent in, or a
/// PDF's first page. `None` from [`Still::of`] means no command makes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Still {
    /// The first frame of an AVIF or HEIC image.
    Frame,
    /// A frame about ten percent into a video, taken as its poster.
    Poster,
    /// The first page of a PDF.
    Page,
    /// The full-size JPEG a camera RAW file embeds.
    RawPreview,
}

impl Still {
    /// The still a media type gets from a command, when it gets one.
    #[must_use]
    pub fn of(media_type: &str) -> Option<Self> {
        match media_type {
            "image/avif" | "image/heic" => Some(Self::Frame),
            "application/pdf" => Some(Self::Page),
            media_type if media_type.starts_with("video/") => Some(Self::Poster),
            media_type if crate::media_type::is_raw_media_type(media_type) => {
                Some(Self::RawPreview)
            }
            _ => None,
        }
    }

    /// Make the still at `source`.
    ///
    /// # Errors
    /// A [`ToolError`] when the command is absent, runs past the timeout, or
    /// fails.
    pub fn make(self, tools: &Tools, source: &Path) -> Result<MadeStill, ToolError> {
        let still = match self {
            Self::Frame => tools.frame(source, None).map(MadeStill::unoriented),
            Self::Poster => tools.poster(source).map(MadeStill::unoriented),
            Self::Page => tools.page(source).map(MadeStill::unoriented),
            Self::RawPreview => tools.raw_preview(source),
        };
        if let Err(error) = &still {
            tracing::warn!(
                target: "romeka::media",
                tool = self.command(),
                %error,
                "no derivative: the external tool made no still"
            );
        }
        still
    }

    /// Make the still from bytes already in memory, through a temporary file
    /// the command can open.
    ///
    /// The file carries the media type's extension, because `ffmpeg`'s format
    /// probe leans on it: the Debian 12 build reads an `.avif` file and cannot
    /// read the same bytes named without an extension, where it guesses the
    /// `mov` demuxer and looks for a `moov` atom. `exiftool` reads a RAW by
    /// its maker's type, which the extension names.
    ///
    /// # Errors
    /// As [`Still::make`], plus a [`ToolError::Failed`] when the temporary file
    /// cannot be written.
    pub fn make_from_bytes(
        self,
        tools: &Tools,
        original: &[u8],
        media_type: &str,
    ) -> Result<MadeStill, ToolError> {
        let dir = tempfile::tempdir().map_err(|error| {
            ToolError::Failed(self.command(), format!("no temporary folder: {error}"))
        })?;
        let extension = crate::media_type::extension_for_media_type(media_type);
        let source = dir.path().join(match extension {
            Some(extension) => format!("source.{extension}"),
            None => "source".to_string(),
        });
        std::fs::write(&source, original).map_err(|error| {
            ToolError::Failed(self.command(), format!("no temporary file: {error}"))
        })?;
        self.make(tools, &source)
    }

    /// The command name, as the reason and the log report it.
    #[must_use]
    pub const fn command(self) -> &'static str {
        match self {
            Self::Frame | Self::Poster => "ffmpeg",
            Self::Page => "pdftoppm",
            Self::RawPreview => "exiftool",
        }
    }
}

impl MadeStill {
    /// A still whose own pixels are already upright, as `ffmpeg` and
    /// `pdftoppm` write them.
    fn unoriented(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            orientation: None,
        }
    }
}

/// The longest side an access copy's video is scaled to. A 1080p television is
/// the largest screen a collection streams to, and a transcode that keeps the
/// source's 4K would cost more disk than the original.
pub const ACCESS_SIDE: u32 = 1080;

/// The H.264 quality an access copy uses.
pub const ACCESS_CRF: u8 = 23;

/// The AAC bitrate of a video or audio access copy, in kbit/s.
pub const ACCESS_AUDIO_KBPS: u16 = 128;

/// The JPEG quality of an image access master.
pub const ACCESS_JPEG_QUALITY: u8 = 90;

/// The resolution an EPS, AI or PS page is rasterised at.
const POSTSCRIPT_DPI: u32 = 150;

/// The external command an access copy comes from. Each is optional at
/// runtime, like `ffmpeg` and `pdftoppm`: a command this install does not have
/// is a typed skip, never an error, and the media keeps its original.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessTool {
    /// H.264/AAC MP4 from a video, AAC M4A from audio.
    Ffmpeg,
    /// The full-size JPEG a camera RAW file embeds.
    ExifTool,
    /// The sensor data of a camera RAW file, through `dcraw_emu` or `dcraw`.
    LibRaw,
    /// The TIFF, PSD, EXR, HDR and HEIC layouts the image crate does not read.
    Vips,
    /// The first page of an EPS, AI or PS file.
    Ghostscript,
}

impl AccessTool {
    /// Every tool, in the order the access table prefers them.
    pub const ALL: [Self; 5] = [
        Self::Ffmpeg,
        Self::ExifTool,
        Self::LibRaw,
        Self::Vips,
        Self::Ghostscript,
    ];

    /// The command name this tool is preferred under, as a reason reports it.
    /// A RAW decode may be found as `dcraw`, which
    /// [`Tools::libraw_name`] reports instead.
    #[must_use]
    pub const fn command(self) -> &'static str {
        match self {
            Self::Ffmpeg => "ffmpeg",
            Self::ExifTool => "exiftool",
            Self::LibRaw => "dcraw_emu",
            Self::Vips => "vips",
            Self::Ghostscript => "gs",
        }
    }

    /// The name of the command this install found, or `None` when it has none.
    #[must_use]
    pub fn found(self, tools: &Tools) -> Option<&'static str> {
        tools.access_program(self).ok().map(|_| match self {
            Self::LibRaw => tools.libraw_name(),
            other => other.command(),
        })
    }
}

/// The arguments an H.264/AAC MP4 access copy runs, at most [`ACCESS_SIDE`]
/// pixels a side, with its index at the front so a player can seek before the
/// download finishes.
#[must_use]
pub fn video_mp4_args(source: &Path, output: &Path) -> Vec<OsString> {
    let scale = format!(
        "scale=w='min(iw,{ACCESS_SIDE})':h='min(ih,{ACCESS_SIDE})':force_original_aspect_ratio=decrease"
    );
    let crf = ACCESS_CRF.to_string();
    let bitrate = format!("{ACCESS_AUDIO_KBPS}k");
    into_args([
        OsStr::new("-nostdin"),
        OsStr::new("-y"),
        OsStr::new("-v"),
        OsStr::new("error"),
        OsStr::new("-i"),
        source.as_os_str(),
        OsStr::new("-map"),
        OsStr::new("0:v:0"),
        OsStr::new("-map"),
        OsStr::new("0:a?"),
        OsStr::new("-vf"),
        OsStr::new(&scale),
        OsStr::new("-c:v"),
        OsStr::new("libx264"),
        OsStr::new("-preset"),
        OsStr::new("medium"),
        OsStr::new("-crf"),
        OsStr::new(&crf),
        OsStr::new("-pix_fmt"),
        OsStr::new("yuv420p"),
        OsStr::new("-c:a"),
        OsStr::new("aac"),
        OsStr::new("-b:a"),
        OsStr::new(&bitrate),
        OsStr::new("-movflags"),
        OsStr::new("+faststart"),
        output.as_os_str(),
    ])
}

/// The arguments an AAC M4A access copy runs, at the same bitrate and with the
/// same fast-start index.
#[must_use]
pub fn audio_m4a_args(source: &Path, output: &Path) -> Vec<OsString> {
    let bitrate = format!("{ACCESS_AUDIO_KBPS}k");
    into_args([
        OsStr::new("-nostdin"),
        OsStr::new("-y"),
        OsStr::new("-v"),
        OsStr::new("error"),
        OsStr::new("-i"),
        source.as_os_str(),
        OsStr::new("-vn"),
        OsStr::new("-c:a"),
        OsStr::new("aac"),
        OsStr::new("-b:a"),
        OsStr::new(&bitrate),
        OsStr::new("-movflags"),
        OsStr::new("+faststart"),
        output.as_os_str(),
    ])
}

/// The arguments that make a JPEG access master from `source`, or `None` when
/// `tool` cannot make one.
///
/// `exiftool` and `dcraw_emu` write their image to standard output, so the
/// caller captures it. `dcraw_emu` writes a TIFF: `vips copy` encodes the JPEG
/// from it, so D4's RAW decode is two runs. An EPS, AI or PS file's first page
/// is rasterised, the way a PDF's first page is, and a multi-page TIFF, PSD or
/// JP2 gives its first page, which is each loader's default.
#[must_use]
pub fn image_jpeg_args(tool: AccessTool, source: &Path, output: &Path) -> Option<Vec<OsString>> {
    match tool {
        AccessTool::ExifTool => Some(into_args([
            OsStr::new("-q"),
            OsStr::new("-q"),
            OsStr::new("-b"),
            OsStr::new("-JpgFromRaw"),
            source.as_os_str(),
        ])),
        AccessTool::LibRaw => Some(into_args([
            OsStr::new("-q"),
            OsStr::new("3"),
            OsStr::new("-w"),
            OsStr::new("-o"),
            OsStr::new("1"),
            OsStr::new("-T"),
            // libraw's `-c` is an exposure threshold, not dcraw's stdout
            // switch; `-Z -` is how `dcraw_emu` writes the decoded TIFF to
            // standard output.
            OsStr::new("-Z"),
            OsStr::new("-"),
            source.as_os_str(),
        ])),
        AccessTool::Vips => {
            let quality = ACCESS_JPEG_QUALITY.to_string();
            let mut target = output.as_os_str().to_owned();
            target.push(format!("[Q={quality}]"));
            Some(into_args([
                OsStr::new("copy"),
                source.as_os_str(),
                target.as_os_str(),
            ]))
        }
        AccessTool::Ghostscript => {
            let mut destination = OsString::from("-sOutputFile=");
            destination.push(output);
            let quality = format!("-dJPEGQ={ACCESS_JPEG_QUALITY}");
            let dpi = format!("-r{POSTSCRIPT_DPI}");
            Some(into_args([
                OsStr::new("-q"),
                OsStr::new("-dNOPAUSE"),
                OsStr::new("-dBATCH"),
                OsStr::new("-dSAFER"),
                OsStr::new("-sDEVICE=jpeg"),
                OsStr::new(&quality),
                OsStr::new(&dpi),
                OsStr::new("-dFirstPage=1"),
                OsStr::new("-dLastPage=1"),
                destination.as_os_str(),
                source.as_os_str(),
            ]))
        }
        AccessTool::Ffmpeg => None,
    }
}

/// Collect a borrowed argument slice into the owned list a command takes.
fn into_args<const N: usize>(arguments: [&OsStr; N]) -> Vec<OsString> {
    arguments.into_iter().map(OsStr::to_owned).collect()
}

/// The commands this process may run, found once.
///
/// A test, or a caller that turns derivatives off, builds one with [`Tools::none`]
/// and names a command with [`Tools::with_ffmpeg`], [`Tools::with_pdftoppm`] or
/// [`Tools::with_exiftool`].
#[derive(Debug, Clone)]
pub struct Tools {
    ffmpeg: Option<PathBuf>,
    ffprobe: Option<PathBuf>,
    pdftoppm: Option<PathBuf>,
    exiftool: Option<PathBuf>,
    /// The libraw decoder, found as `dcraw_emu` or, when that is absent, as
    /// `dcraw`.
    libraw: Option<PathBuf>,
    /// The name [`Tools::libraw`] was found under.
    libraw_name: &'static str,
    vips: Option<PathBuf>,
    ghostscript: Option<PathBuf>,
    timeout: Duration,
}

impl Tools {
    /// The tools this process finds on `PATH`, looked up once.
    #[must_use]
    pub fn on_path() -> &'static Self {
        static FOUND: OnceLock<Tools> = OnceLock::new();
        FOUND.get_or_init(|| {
            let (libraw, libraw_name) = match find_on_path("dcraw_emu") {
                Some(path) => (Some(path), "dcraw_emu"),
                None => match find_on_path("dcraw") {
                    Some(path) => (Some(path), "dcraw"),
                    None => (None, "dcraw_emu"),
                },
            };
            Self {
                ffmpeg: find_on_path("ffmpeg"),
                ffprobe: find_on_path("ffprobe"),
                pdftoppm: find_on_path("pdftoppm"),
                exiftool: find_on_path("exiftool"),
                libraw,
                libraw_name,
                vips: find_on_path("vips"),
                ghostscript: find_on_path("gs"),
                timeout: TIMEOUT,
            }
        })
    }

    /// No tools: every still is [`ToolError::ToolMissing`].
    #[must_use]
    pub fn none() -> Self {
        Self {
            ffmpeg: None,
            ffprobe: None,
            pdftoppm: None,
            exiftool: None,
            libraw: None,
            libraw_name: "dcraw_emu",
            vips: None,
            ghostscript: None,
            timeout: TIMEOUT,
        }
    }

    /// Run a command for at most `timeout`.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Name the `ffmpeg` to run, instead of the one on `PATH`.
    #[must_use]
    pub fn with_ffmpeg(mut self, path: impl Into<PathBuf>) -> Self {
        self.ffmpeg = Some(path.into());
        self
    }

    /// Name the `pdftoppm` to run, instead of the one on `PATH`.
    #[must_use]
    pub fn with_pdftoppm(mut self, path: impl Into<PathBuf>) -> Self {
        self.pdftoppm = Some(path.into());
        self
    }

    /// Name the `ffprobe` to run, instead of the one on `PATH`.
    #[must_use]
    pub fn with_ffprobe(mut self, path: impl Into<PathBuf>) -> Self {
        self.ffprobe = Some(path.into());
        self
    }

    /// Name the `exiftool` to run, instead of the one on `PATH`.
    #[must_use]
    pub fn with_exiftool(mut self, path: impl Into<PathBuf>) -> Self {
        self.exiftool = Some(path.into());
        self
    }

    /// Name the libraw decoder to run, instead of the one on `PATH`. It is
    /// reported under the `dcraw_emu` name.
    #[must_use]
    pub fn with_libraw(mut self, path: impl Into<PathBuf>) -> Self {
        self.libraw = Some(path.into());
        self.libraw_name = "dcraw_emu";
        self
    }

    /// Name the `vips` to run, instead of the one on `PATH`.
    #[must_use]
    pub fn with_vips(mut self, path: impl Into<PathBuf>) -> Self {
        self.vips = Some(path.into());
        self
    }

    /// Name the ghostscript (`gs`) to run, instead of the one on `PATH`.
    #[must_use]
    pub fn with_ghostscript(mut self, path: impl Into<PathBuf>) -> Self {
        self.ghostscript = Some(path.into());
        self
    }

    /// The `ffmpeg` this process found, when it found one.
    #[must_use]
    pub fn ffmpeg(&self) -> Option<&Path> {
        self.ffmpeg.as_deref()
    }

    /// The `ffprobe` this process found, when it found one.
    #[must_use]
    pub fn ffprobe(&self) -> Option<&Path> {
        self.ffprobe.as_deref()
    }

    /// The `pdftoppm` this process found, when it found one.
    #[must_use]
    pub fn pdftoppm(&self) -> Option<&Path> {
        self.pdftoppm.as_deref()
    }

    /// The `exiftool` this process found, when it found one.
    #[must_use]
    pub fn exiftool(&self) -> Option<&Path> {
        self.exiftool.as_deref()
    }

    /// The libraw decoder this process found, when it found one.
    #[must_use]
    pub fn libraw(&self) -> Option<&Path> {
        self.libraw.as_deref()
    }

    /// The name the libraw decoder was found under, `dcraw_emu` when none.
    #[must_use]
    pub fn libraw_name(&self) -> &'static str {
        self.libraw_name
    }

    /// The `vips` this process found, when it found one.
    #[must_use]
    pub fn vips(&self) -> Option<&Path> {
        self.vips.as_deref()
    }

    /// The ghostscript (`gs`) this process found, when it found one.
    #[must_use]
    pub fn ghostscript(&self) -> Option<&Path> {
        self.ghostscript.as_deref()
    }

    /// The command that would run `tool`, or the typed skip when this install
    /// does not have it. An access worker runs its copy through this and
    /// reports [`ToolError::ToolMissing`] the way a still does.
    ///
    /// # Errors
    /// [`ToolError::ToolMissing`] when the command is not on this install.
    pub fn access_program(&self, tool: AccessTool) -> Result<&Path, ToolError> {
        match tool {
            AccessTool::Ffmpeg => self.program(self.ffmpeg.as_deref(), "ffmpeg"),
            AccessTool::ExifTool => self.program(self.exiftool.as_deref(), "exiftool"),
            AccessTool::LibRaw => self.program(self.libraw.as_deref(), self.libraw_name),
            AccessTool::Vips => self.program(self.vips.as_deref(), "vips"),
            AccessTool::Ghostscript => self.program(self.ghostscript.as_deref(), "gs"),
        }
    }

    /// The timeout a command runs under.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Run an access tool with `args`, bounded in time, and answer what it
    /// wrote to standard output. A command that writes its image to a file
    /// answers nothing here; its caller reads the file.
    ///
    /// # Errors
    /// A [`ToolError`] when the command is absent, runs past the timeout, exits
    /// non-zero, or writes more than `limit` bytes.
    pub fn run_access(
        &self,
        tool: AccessTool,
        args: &[OsString],
        limit: usize,
    ) -> Result<Vec<u8>, ToolError> {
        let program = self.access_program(tool)?;
        let name = match tool {
            AccessTool::LibRaw => self.libraw_name,
            other => other.command(),
        };
        let borrowed: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
        let output = capture(program, name, &borrowed, self.timeout, limit)?;
        if !output.status.success() {
            return Err(ToolError::Failed(name, reason(&output.stderr)));
        }
        Ok(output.stdout)
    }

    /// `ffprobe`'s JSON answer for the file at `source`: the container's
    /// duration and each stream's type and frame size.
    ///
    /// # Errors
    /// As [`Still::make`]: the tool is absent, runs past [`PROBE_TIMEOUT`], or
    /// fails.
    pub fn probe_json(&self, source: &Path) -> Result<String, ToolError> {
        let ffprobe = self.program(self.ffprobe.as_deref(), "ffprobe")?;
        let args: Vec<&OsStr> = vec![
            OsStr::new("-v"),
            OsStr::new("error"),
            OsStr::new("-show_entries"),
            OsStr::new("format=duration:stream=codec_type,width,height"),
            OsStr::new("-of"),
            OsStr::new("json"),
            source.as_os_str(),
        ];
        let timeout = self.timeout.min(PROBE_TIMEOUT);
        let output = capture(ffprobe, "ffprobe", &args, timeout, PROBE_LIMIT)?;
        if !output.status.success() {
            return Err(ToolError::Failed("ffprobe", reason(&output.stderr)));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// `ffprobe`'s JSON answer for an original's container and stream codecs:
    /// what an access decision reads. A separate answer from
    /// [`Tools::probe_json`], because a canvas needs the duration and frame
    /// size and an access rule needs neither.
    ///
    /// # Errors
    /// As [`Tools::probe_json`].
    pub fn probe_codecs_json(&self, source: &Path) -> Result<String, ToolError> {
        let ffprobe = self.program(self.ffprobe.as_deref(), "ffprobe")?;
        let args: Vec<&OsStr> = vec![
            OsStr::new("-v"),
            OsStr::new("error"),
            OsStr::new("-show_entries"),
            OsStr::new("format=format_name:stream=codec_type,codec_name"),
            OsStr::new("-of"),
            OsStr::new("json"),
            source.as_os_str(),
        ];
        let timeout = self.timeout.min(PROBE_TIMEOUT);
        let output = capture(ffprobe, "ffprobe", &args, timeout, PROBE_LIMIT)?;
        if !output.status.success() {
            return Err(ToolError::Failed("ffprobe", reason(&output.stderr)));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// A video's poster: a frame about ten percent in, so a recording that
    /// opens on a title card shows something. A video whose duration the
    /// container does not report, or whose seek lands past the last frame,
    /// falls back to its first frame.
    fn poster(&self, source: &Path) -> Result<Vec<u8>, ToolError> {
        let ffmpeg = self.program(self.ffmpeg.as_deref(), "ffmpeg")?;
        let at = self
            .duration(ffmpeg, source)
            .map(|duration| duration.mul_f64(0.1))
            .filter(|at| !at.is_zero());
        match at {
            Some(at) => self
                .frame_with(ffmpeg, source, Some(at))
                .or_else(|_| self.frame_with(ffmpeg, source, None)),
            None => self.frame_with(ffmpeg, source, None),
        }
    }

    /// A still image's frame: AVIF and HEIC have no decoder here.
    fn frame(&self, source: &Path, at: Option<Duration>) -> Result<Vec<u8>, ToolError> {
        let ffmpeg = self.program(self.ffmpeg.as_deref(), "ffmpeg")?;
        self.frame_with(ffmpeg, source, at)
    }

    /// Run `ffmpeg` for one frame, scaled to [`STILL_SIDE`] at most.
    ///
    /// `-ss` comes before `-i`, so the demuxer seeks instead of decoding from
    /// the start: the cost is one seek, not the length of the file.
    fn frame_with(
        &self,
        ffmpeg: &Path,
        source: &Path,
        at: Option<Duration>,
    ) -> Result<Vec<u8>, ToolError> {
        let scale = format!(
            "scale=w='min(iw,{STILL_SIDE})':h='min(ih,{STILL_SIDE})':force_original_aspect_ratio=decrease"
        );
        let mut args: Vec<&OsStr> = vec![
            OsStr::new("-nostdin"),
            OsStr::new("-hide_banner"),
            OsStr::new("-v"),
            OsStr::new("error"),
        ];
        let seconds;
        if let Some(at) = at {
            seconds = format!("{:.3}", at.as_secs_f64());
            args.push(OsStr::new("-ss"));
            args.push(OsStr::new(&seconds));
        }
        args.extend([
            OsStr::new("-i"),
            source.as_os_str(),
            OsStr::new("-frames:v"),
            OsStr::new("1"),
            OsStr::new("-vf"),
            OsStr::new(&scale),
            OsStr::new("-f"),
            OsStr::new("image2pipe"),
            OsStr::new("-vcodec"),
            OsStr::new("png"),
            OsStr::new("-"),
        ]);
        let output = capture(ffmpeg, "ffmpeg", &args, self.timeout, STILL_LIMIT)?;
        successful("ffmpeg", output)
    }

    /// A PDF's first page at [`PDF_DPI`], written to a file beside the input.
    fn page(&self, source: &Path) -> Result<Vec<u8>, ToolError> {
        let pdftoppm = self.program(self.pdftoppm.as_deref(), "pdftoppm")?;
        let dir = tempfile::tempdir().map_err(|error| {
            ToolError::Failed("pdftoppm", format!("no temporary folder: {error}"))
        })?;
        let root = dir.path().join("page");
        let dpi = PDF_DPI.to_string();
        let args: Vec<&OsStr> = vec![
            OsStr::new("-f"),
            OsStr::new("1"),
            OsStr::new("-l"),
            OsStr::new("1"),
            OsStr::new("-r"),
            OsStr::new(&dpi),
            OsStr::new("-png"),
            OsStr::new("-singlefile"),
            source.as_os_str(),
            root.as_os_str(),
        ];
        let output = capture(pdftoppm, "pdftoppm", &args, self.timeout, STILL_LIMIT)?;
        if !output.status.success() {
            return Err(ToolError::Failed("pdftoppm", reason(&output.stderr)));
        }
        let mut file = std::fs::File::open(root.with_extension("png")).map_err(|error| {
            ToolError::Failed("pdftoppm", format!("no page was written: {error}"))
        })?;
        let size = file
            .metadata()
            .map_err(|error| ToolError::Failed("pdftoppm", error.to_string()))?
            .len();
        if size as usize > STILL_LIMIT {
            return Err(ToolError::Failed(
                "pdftoppm",
                format!("the page is {size} bytes, over the {STILL_LIMIT} byte limit"),
            ));
        }
        let mut bytes = Vec::with_capacity(size as usize);
        file.read_to_end(&mut bytes)
            .map_err(|error| ToolError::Failed("pdftoppm", error.to_string()))?;
        if bytes.is_empty() {
            return Err(ToolError::Failed(
                "pdftoppm",
                "the page is empty".to_string(),
            ));
        }
        Ok(bytes)
    }

    /// The full-size JPEG a camera RAW file embeds, and the orientation its
    /// container states for those pixels. `JpgFromRaw` is the full-size
    /// preview a maker writes beside the sensor data; `PreviewImage` is the
    /// smaller one. A file with neither is a typed failure, not an empty
    /// still.
    fn raw_preview(&self, source: &Path) -> Result<MadeStill, ToolError> {
        let exiftool = self.program(self.exiftool.as_deref(), "exiftool")?;
        let bytes = match self.binary_tag(exiftool, source, "JpgFromRaw") {
            Ok(bytes) => bytes,
            Err(_) => self
                .binary_tag(exiftool, source, "PreviewImage")
                .map_err(|_| {
                    ToolError::Failed(
                        "exiftool",
                        "the file has no embedded JPEG preview".to_string(),
                    )
                })?,
        };
        Ok(MadeStill {
            bytes,
            orientation: self.orientation(exiftool, source),
        })
    }

    /// One binary tag `exiftool` prints to standard output.
    fn binary_tag(&self, exiftool: &Path, source: &Path, tag: &str) -> Result<Vec<u8>, ToolError> {
        let argument = format!("-{tag}");
        let args: Vec<&OsStr> = vec![
            OsStr::new("-q"),
            OsStr::new("-q"),
            OsStr::new("-b"),
            OsStr::new(&argument),
            source.as_os_str(),
        ];
        let output = capture(exiftool, "exiftool", &args, self.timeout, STILL_LIMIT)?;
        if !output.status.success() {
            return Err(ToolError::Failed("exiftool", reason(&output.stderr)));
        }
        if output.stdout.is_empty() {
            return Err(ToolError::Failed("exiftool", format!("no {tag}")));
        }
        Ok(output.stdout)
    }

    /// The image master commands this install has, as a queued row names them.
    /// The claim is filtered by this list, so two workers on one queue (a main
    /// image with `exiftool` and a tools image with `vips`) each claim the rows
    /// they can make and neither settles the other's as a skip.
    #[must_use]
    pub fn image_master_names(&self) -> Vec<&'static str> {
        AccessTool::ALL
            .into_iter()
            .filter(|tool| *tool != AccessTool::Ffmpeg)
            .filter_map(|tool| tool.found(self))
            .collect()
    }

    /// The numeric EXIF orientation a file's container states, when it states
    /// one that turns the pixels. A camera writes its pixels as the sensor saw
    /// them and this tag says how they go upright; an embedded preview usually
    /// carries no tag of its own, so its container's value is the one to apply.
    fn orientation(&self, exiftool: &Path, source: &Path) -> Option<Orientation> {
        let args: Vec<&OsStr> = vec![
            OsStr::new("-q"),
            OsStr::new("-q"),
            OsStr::new("-s3"),
            OsStr::new("-n"),
            OsStr::new("-Orientation#"),
            source.as_os_str(),
        ];
        let output = capture(exiftool, "exiftool", &args, PROBE_TIMEOUT, STDERR_LIMIT).ok()?;
        if !output.status.success() {
            return None;
        }
        let value: u8 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .ok()?;
        Orientation::from_exif(value)
            .filter(|orientation| *orientation != Orientation::NoTransforms)
    }

    /// The duration `ffmpeg` reports in its input header, when it reports one.
    fn duration(&self, ffmpeg: &Path, source: &Path) -> Option<Duration> {
        let args = [
            OsStr::new("-nostdin"),
            OsStr::new("-hide_banner"),
            OsStr::new("-i"),
            source.as_os_str(),
        ];
        let output = capture(ffmpeg, "ffmpeg", &args, self.timeout, STILL_LIMIT).ok()?;
        parse_duration(&String::from_utf8_lossy(&output.stderr))
    }

    /// The program to run, or the missing-tool reason.
    fn program<'a>(
        &self,
        program: Option<&'a Path>,
        name: &'static str,
    ) -> Result<&'a Path, ToolError> {
        program
            .filter(|path| path.is_file())
            .ok_or(ToolError::ToolMissing(name))
    }
}

impl Default for Tools {
    fn default() -> Self {
        Self::on_path().clone()
    }
}

/// `ffmpeg`'s `Duration: 00:01:02.34` line, from a header probe.
fn parse_duration(stderr: &str) -> Option<Duration> {
    let rest = stderr.split("Duration: ").nth(1)?;
    let clock = rest.split(',').next()?.trim();
    let mut parts = clock.split(':');
    let hours: u64 = parts.next()?.trim().parse().ok()?;
    let minutes: u64 = parts.next()?.trim().parse().ok()?;
    let seconds: f64 = parts.next()?.trim().parse().ok()?;
    if seconds < 0.0 {
        return None;
    }
    Some(Duration::from_secs(hours * 3600 + minutes * 60) + Duration::from_secs_f64(seconds))
}

/// The first `name` on `PATH` that is an executable file.
fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    path.metadata()
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// One finished run. A non-zero exit is the caller's to read, because
/// `ffmpeg` exits 1 when it prints an input header and has no output to write.
struct Captured {
    status: std::process::ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// Run `program` with `args`, bounded in time and in what it may write.
fn capture(
    program: &Path,
    name: &'static str,
    args: &[&OsStr],
    timeout: Duration,
    stdout_limit: usize,
) -> Result<Captured, ToolError> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => ToolError::ToolMissing(name),
            _ => ToolError::Failed(name, error.to_string()),
        })?;
    let stdout = read_apart(child.stdout.take(), stdout_limit);
    let stderr = read_apart(child.stderr.take(), STDERR_LIMIT);
    let status = wait(&mut child, name, timeout)?;
    Ok(Captured {
        status,
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

/// Wait for the child, killing it at `timeout`.
fn wait(
    child: &mut Child,
    name: &'static str,
    timeout: Duration,
) -> Result<std::process::ExitStatus, ToolError> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ToolError::Timeout(name, timeout));
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => return Err(ToolError::Failed(name, error.to_string())),
        }
    }
}

/// Read one pipe on its own thread, keeping at most `limit` bytes and draining
/// the rest, so a command that writes more than the limit still finishes
/// instead of blocking on a full pipe.
fn read_apart<R: Read + Send + 'static>(
    reader: Option<R>,
    limit: usize,
) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let Some(mut reader) = reader else {
            return Vec::new();
        };
        let mut kept = Vec::new();
        let mut buffer = [0u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    let room = limit.saturating_sub(kept.len());
                    kept.extend_from_slice(&buffer[..read.min(room)]);
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
        kept
    })
}

/// A successful run's standard output, or the reason it is not usable.
fn successful(name: &'static str, output: Captured) -> Result<Vec<u8>, ToolError> {
    if !output.status.success() {
        return Err(ToolError::Failed(name, reason(&output.stderr)));
    }
    if output.stdout.is_empty() {
        return Err(ToolError::Failed(name, "no still was written".to_string()));
    }
    if output.stdout.len() >= STILL_LIMIT {
        return Err(ToolError::Failed(
            name,
            format!("the still is at least {STILL_LIMIT} bytes, over the limit"),
        ));
    }
    Ok(output.stdout)
}

/// The end of a command's diagnostic output, which is what names a failure.
fn reason(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let text = text.trim();
    if text.is_empty() {
        return "it exited non-zero and said nothing".to_string();
    }
    let kept: String = text
        .chars()
        .skip(text.chars().count().saturating_sub(400))
        .collect();
    kept.trim_start().to_string()
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    /// A program that does what `body` says, under the name a tool has.
    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write script");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("make it executable");
        path
    }

    fn dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    /// A script written in one test can be `Text file busy` for a fork another
    /// test thread makes while the write is open. One lock across create and
    /// run keeps the fixture as calm as a real installed binary.
    fn scripted() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Without the command, the reason names it and nothing runs.
    #[test]
    fn a_missing_tool_is_a_typed_reason() {
        let dir = dir();
        let error = Still::Page
            .make(&Tools::none(), &dir.path().join("doc.pdf"))
            .expect_err("no pdftoppm");
        assert_eq!(error, ToolError::ToolMissing("pdftoppm"));
        assert_eq!(error.to_string(), "the pdftoppm command is not on PATH");

        let error = Still::Poster
            .make(&Tools::none(), &dir.path().join("clip.mp4"))
            .expect_err("no ffmpeg");
        assert_eq!(error, ToolError::ToolMissing("ffmpeg"));
    }

    /// A command that runs past the timeout is killed and answers `Timeout`.
    #[test]
    fn a_slow_command_times_out() {
        let _guard = scripted();
        let dir = dir();
        let slow = script(dir.path(), "ffmpeg", "sleep 5");
        let tools = Tools::none()
            .with_ffmpeg(&slow)
            .with_timeout(Duration::from_millis(120));
        let started = Instant::now();
        let error = Still::Frame
            .make(&tools, &dir.path().join("image.avif"))
            .expect_err("too slow");
        assert_eq!(
            error,
            ToolError::Timeout("ffmpeg", Duration::from_millis(120))
        );
        assert!(started.elapsed() < Duration::from_secs(3), "killed early");
    }

    /// A command that exits non-zero answers `Failed` with what it said.
    #[test]
    fn a_failing_command_keeps_its_reason() {
        let _guard = scripted();
        let dir = dir();
        let failing = script(
            dir.path(),
            "pdftoppm",
            "echo 'Syntax Error: bad page' >&2; exit 2",
        );
        let tools = Tools::none().with_pdftoppm(&failing);
        let error = Still::Page
            .make(&tools, &dir.path().join("doc.pdf"))
            .expect_err("fails");
        assert_eq!(
            error,
            ToolError::Failed("pdftoppm", "Syntax Error: bad page".to_string())
        );
    }

    /// A command that writes a still is read back byte for byte.
    #[test]
    fn a_still_is_the_commands_output() {
        let _guard = scripted();
        let dir = dir();
        let ffmpeg = script(dir.path(), "ffmpeg", "printf 'still-bytes'");
        let tools = Tools::none().with_ffmpeg(&ffmpeg);
        assert_eq!(
            Still::Frame
                .make(&tools, &dir.path().join("image.heic"))
                .expect("still")
                .bytes,
            b"still-bytes"
        );
    }

    /// A RAW's full-size preview is preferred, the smaller one is the fallback,
    /// and the container's orientation is read beside it.
    #[test]
    fn a_raw_preview_prefers_jpgfromraw_and_reads_the_orientation() {
        let _guard = scripted();
        let dir = dir();
        let exiftool = script(
            dir.path(),
            "exiftool",
            "case \"$*\" in \
             *-Orientation#*) printf '6\\n';; \
             *-JpgFromRaw*) printf 'full-preview';; \
             *-PreviewImage*) printf 'small-preview';; \
             *) exit 9;; esac",
        );
        let tools = Tools::none().with_exiftool(&exiftool);
        let made = Still::RawPreview
            .make(&tools, &dir.path().join("photo.nef"))
            .expect("a preview");
        assert_eq!(made.bytes, b"full-preview");
        assert_eq!(made.orientation, Some(Orientation::Rotate90));

        let fallback = script(
            dir.path(),
            "exiftool-fallback",
            "case \"$*\" in \
             *-Orientation#*) printf '1\\n';; \
             *-JpgFromRaw*) : ;; \
             *-PreviewImage*) printf 'small-preview';; \
             *) exit 9;; esac",
        );
        let tools = Tools::none().with_exiftool(&fallback);
        let made = Still::RawPreview
            .make(&tools, &dir.path().join("photo.nef"))
            .expect("a preview");
        assert_eq!(made.bytes, b"small-preview");
        assert_eq!(made.orientation, None);
    }

    /// Without `exiftool`, a RAW preview is the same typed reason as any other
    /// missing tool.
    #[test]
    fn a_missing_exiftool_is_a_typed_reason() {
        let dir = dir();
        let error = Still::RawPreview
            .make(&Tools::none(), &dir.path().join("photo.nef"))
            .expect_err("no exiftool");
        assert_eq!(error, ToolError::ToolMissing("exiftool"));
    }

    /// A video's poster is taken ten percent in, which needs the duration from
    /// the header probe. The probe exits non-zero on purpose, as `ffmpeg` does
    /// when it has an input and no output.
    #[test]
    fn a_poster_seeks_ten_percent_in() {
        let _guard = scripted();
        let dir = dir();
        let log = dir.path().join("args");
        let ffmpeg = script(
            dir.path(),
            "ffmpeg",
            &format!(
                "printf '%s\\n' \"$@\" >> {log}\n\
                 case \"$*\" in *-frames:v*) printf 'still-bytes';; \
                 *) echo '  Duration: 00:00:20.00, start: 0.0' >&2; exit 1;; esac",
                log = log.display()
            ),
        );
        let tools = Tools::none().with_ffmpeg(&ffmpeg);
        Still::Poster
            .make(&tools, &dir.path().join("clip.mp4"))
            .expect("poster");
        let recorded = std::fs::read_to_string(&log).expect("args");
        let lines: Vec<&str> = recorded.lines().collect();
        // The probe ran first; the frame run is the last invocation recorded.
        let start = lines
            .iter()
            .rposition(|line| *line == "-nostdin")
            .expect("an invocation");
        let frame = &lines[start..];
        let at = frame
            .iter()
            .position(|line| *line == "-ss")
            .expect("a seek before the input");
        assert_eq!(frame[at + 1], "2.000", "{recorded}");
        assert!(
            frame.iter().position(|line| *line == "-i").expect("input") > at,
            "the seek comes before the input, so nothing is decoded from the start"
        );
    }

    /// The table: only the types no decoder here reads ask for a command.
    #[test]
    fn the_still_table_names_the_tool_types() {
        assert_eq!(Still::of("video/mp4"), Some(Still::Poster));
        assert_eq!(Still::of("video/x-matroska"), Some(Still::Poster));
        assert_eq!(Still::of("application/pdf"), Some(Still::Page));
        assert_eq!(Still::of("image/avif"), Some(Still::Frame));
        assert_eq!(Still::of("image/heic"), Some(Still::Frame));
        assert_eq!(Still::of("image/x-nikon-nef"), Some(Still::RawPreview));
        assert_eq!(Still::of("image/x-adobe-dng"), Some(Still::RawPreview));
        assert_eq!(Still::of("image/jpeg"), None);
        assert_eq!(Still::of("text/vtt"), None);
    }

    /// Without `ffprobe`, a probe is the same typed reason as a missing still
    /// command, and nothing runs.
    #[test]
    fn a_missing_ffprobe_is_a_typed_reason() {
        let dir = dir();
        let error = Tools::none()
            .probe_json(&dir.path().join("clip.mp4"))
            .expect_err("no ffprobe");
        assert_eq!(error, ToolError::ToolMissing("ffprobe"));
    }

    /// The probe asks for the duration and the streams' types and frame sizes,
    /// in JSON, and answers what the command wrote.
    #[test]
    fn the_probe_asks_for_the_duration_and_the_frame_size() {
        let _guard = scripted();
        let dir = dir();
        let log = dir.path().join("args");
        let ffprobe = script(
            dir.path(),
            "ffprobe",
            &format!(
                "printf '%s\\n' \"$@\" > {log}\n\
                 printf '%s' '{{\"streams\":[{{\"codec_type\":\"audio\"}}],\
                 \"format\":{{\"duration\":\"0.350000\"}}}}'",
                log = log.display()
            ),
        );
        let tools = Tools::none().with_ffprobe(&ffprobe);
        let json = tools
            .probe_json(&dir.path().join("clip.mp4"))
            .expect("a probe");
        assert_eq!(
            json,
            r#"{"streams":[{"codec_type":"audio"}],"format":{"duration":"0.350000"}}"#
        );
        let recorded = std::fs::read_to_string(&log).expect("args");
        assert!(
            recorded.contains("format=duration:stream=codec_type,width,height"),
            "{recorded}"
        );
        assert!(recorded.contains("json"), "{recorded}");
        assert!(recorded.contains("clip.mp4"), "{recorded}");
    }

    /// A probe past the timeout is killed like any other command.
    #[test]
    fn a_slow_probe_times_out() {
        let _guard = scripted();
        let dir = dir();
        let slow = script(dir.path(), "ffprobe", "sleep 5");
        let tools = Tools::none()
            .with_ffprobe(&slow)
            .with_timeout(Duration::from_millis(120));
        let started = Instant::now();
        let error = tools
            .probe_json(&dir.path().join("clip.mp4"))
            .expect_err("too slow");
        assert_eq!(
            error,
            ToolError::Timeout("ffprobe", Duration::from_millis(120))
        );
        assert!(started.elapsed() < Duration::from_secs(3), "killed early");
    }

    /// The header clock is read, and `N/A` is not a duration.
    #[test]
    fn the_header_duration_is_parsed() {
        assert_eq!(
            parse_duration("  Duration: 00:02:03.50, start: 0.000000, bitrate: 1 kb/s"),
            Some(Duration::from_millis(123_500))
        );
        assert_eq!(parse_duration("  Duration: N/A, start: 0.000000"), None);
        assert_eq!(parse_duration("no header"), None);
    }

    /// An access tool the install has reports the name it runs under; one it
    /// does not have is the typed skip D3 and D4 turn into a reason.
    #[test]
    fn an_access_tool_is_found_or_a_typed_skip() {
        let _guard = scripted();
        let dir = dir();
        let tools = Tools::none().with_vips(script(dir.path(), "vips", "exit 0"));
        assert_eq!(AccessTool::Vips.found(&tools), Some("vips"));
        assert!(tools.access_program(AccessTool::Vips).is_ok());
        assert_eq!(AccessTool::Vips.found(&Tools::none()), None);
        assert_eq!(
            Tools::none()
                .access_program(AccessTool::Vips)
                .expect_err("no vips"),
            ToolError::ToolMissing("vips")
        );
        assert_eq!(
            Tools::none()
                .access_program(AccessTool::LibRaw)
                .expect_err("no libraw"),
            ToolError::ToolMissing("dcraw_emu")
        );
        assert_eq!(
            Tools::none()
                .access_program(AccessTool::Ghostscript)
                .expect_err("no gs"),
            ToolError::ToolMissing("gs")
        );

        let libraw = Tools::none().with_libraw(script(dir.path(), "dcraw_emu", "exit 0"));
        assert_eq!(AccessTool::LibRaw.found(&libraw), Some("dcraw_emu"));
        assert_eq!(libraw.libraw_name(), "dcraw_emu");

        let ghostscript = Tools::none().with_ghostscript(script(dir.path(), "gs", "exit 0"));
        assert_eq!(AccessTool::Ghostscript.found(&ghostscript), Some("gs"));
    }

    /// An image worker claims only the commands this install has, so a tools
    /// image and a main image can share one queue.
    #[test]
    fn the_image_master_commands_are_the_ones_installed() {
        let _guard = scripted();
        let dir = dir();
        assert!(
            Tools::none().image_master_names().is_empty(),
            "an install with no master command claims nothing"
        );
        let tools = Tools::none()
            .with_exiftool(script(dir.path(), "exiftool", "exit 0"))
            .with_vips(script(dir.path(), "vips", "exit 0"));
        let names = tools.image_master_names();
        assert!(names.contains(&"exiftool"), "{names:?}");
        assert!(names.contains(&"vips"), "{names:?}");
        assert!(!names.contains(&"ffmpeg"), "{names:?}");
    }

    /// The video copy is H.264 and AAC in an MP4 whose index comes first, with
    /// the source's own size kept and each side capped at 1080.
    #[test]
    fn the_video_copy_is_h264_aac_and_at_most_1080p() {
        let args = text(&video_mp4_args(
            Path::new("reel.avi"),
            Path::new("copy.mp4"),
        ));
        assert_eq!(
            args,
            [
                "-nostdin",
                "-y",
                "-v",
                "error",
                "-i",
                "reel.avi",
                "-map",
                "0:v:0",
                "-map",
                "0:a?",
                "-vf",
                "scale=w='min(iw,1080)':h='min(ih,1080)':force_original_aspect_ratio=decrease",
                "-c:v",
                "libx264",
                "-preset",
                "medium",
                "-crf",
                "23",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-b:a",
                "128k",
                "-movflags",
                "+faststart",
                "copy.mp4",
            ]
        );
    }

    /// The audio copy drops the video stream and keeps the same bitrate.
    #[test]
    fn the_audio_copy_is_aac_in_an_m4a() {
        let args = text(&audio_m4a_args(
            Path::new("talk.wma"),
            Path::new("copy.m4a"),
        ));
        assert_eq!(
            args,
            [
                "-nostdin",
                "-y",
                "-v",
                "error",
                "-i",
                "talk.wma",
                "-vn",
                "-c:a",
                "aac",
                "-b:a",
                "128k",
                "-movflags",
                "+faststart",
                "copy.m4a",
            ]
        );
    }

    /// Each image tool's argument list, and the tool that makes no image.
    #[test]
    fn the_image_master_asks_each_tool_its_own_way() {
        let source = Path::new("master.psd");
        let output = Path::new("copy.jpg");
        assert_eq!(
            text(&image_jpeg_args(AccessTool::Vips, source, output).expect("vips")),
            ["copy", "master.psd", "copy.jpg[Q=90]"]
        );
        assert_eq!(
            text(
                &image_jpeg_args(AccessTool::ExifTool, Path::new("photo.nef"), output)
                    .expect("exif")
            ),
            ["-q", "-q", "-b", "-JpgFromRaw", "photo.nef"],
            "the embedded preview goes to standard output"
        );
        assert_eq!(
            text(
                &image_jpeg_args(AccessTool::LibRaw, Path::new("photo.nef"), output).expect("raw")
            ),
            ["-q", "3", "-w", "-o", "1", "-T", "-Z", "-", "photo.nef"],
            "libraw decodes to a TIFF on standard output, which vips then encodes"
        );
        let page = text(
            &image_jpeg_args(AccessTool::Ghostscript, Path::new("poster.eps"), output).expect("gs"),
        );
        assert_eq!(page[0], "-q");
        assert!(page.contains(&"-sDEVICE=jpeg".to_string()), "{page:?}");
        assert!(page.contains(&"-r150".to_string()), "{page:?}");
        assert!(
            page.contains(&"-sOutputFile=copy.jpg".to_string()),
            "{page:?}"
        );
        assert_eq!(page.last().map(String::as_str), Some("poster.eps"));
        assert_eq!(image_jpeg_args(AccessTool::Ffmpeg, source, output), None);
    }

    /// The access probe asks for the container and the stream codecs, not the
    /// duration a canvas needs.
    #[test]
    fn the_codec_probe_asks_for_the_container_and_codecs() {
        let _guard = scripted();
        let dir = dir();
        let log = dir.path().join("args");
        let ffprobe = script(
            dir.path(),
            "ffprobe",
            &format!(
                "printf '%s\\n' \"$@\" > {log}\n\
                 printf '%s' '{{\"streams\":[],\"format\":{{\"format_name\":\"ogg\"}}}}'",
                log = log.display()
            ),
        );
        let tools = Tools::none().with_ffprobe(ffprobe);
        let json = tools
            .probe_codecs_json(&dir.path().join("clip.ogv"))
            .expect("a probe");
        assert!(json.contains("ogg"), "{json}");
        let recorded = std::fs::read_to_string(&log).expect("args");
        assert!(
            recorded.contains("format=format_name:stream=codec_type,codec_name"),
            "{recorded}"
        );
    }

    /// The owned argument list as text, for comparison.
    fn text(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect()
    }
}
