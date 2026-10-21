//! The external commands a still derivative needs: `ffmpeg` for a video's
//! poster and for an AVIF or HEIC frame, `pdftoppm` for a PDF's first page, and
//! `ffprobe` for the duration and frame size a time-based canvas needs.
//!
//! The image crate decodes the types GD decodes, plus TIFF, JPEG 2000 and SVG.
//! It has no video, PDF, AVIF or HEIC decoder, so those get their still from a
//! command. The commands are optional at runtime: the shipped image carries
//! both (`packaging/Dockerfile`), and an installation without them skips the
//! derivative with a typed [`ToolError`] rather than failing the ingest.
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

use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;

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
}

impl Still {
    /// The still a media type gets from a command, when it gets one.
    #[must_use]
    pub fn of(media_type: &str) -> Option<Self> {
        match media_type {
            "image/avif" | "image/heic" => Some(Self::Frame),
            "application/pdf" => Some(Self::Page),
            media_type if media_type.starts_with("video/") => Some(Self::Poster),
            _ => None,
        }
    }

    /// Make the still at `source`.
    ///
    /// # Errors
    /// A [`ToolError`] when the command is absent, runs past the timeout, or
    /// fails.
    pub fn make(self, tools: &Tools, source: &Path) -> Result<Vec<u8>, ToolError> {
        let still = match self {
            Self::Frame => tools.frame(source, None),
            Self::Poster => tools.poster(source),
            Self::Page => tools.page(source),
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
    /// `mov` demuxer and looks for a `moov` atom.
    ///
    /// # Errors
    /// As [`Still::make`], plus a [`ToolError::Failed`] when the temporary file
    /// cannot be written.
    pub fn make_from_bytes(
        self,
        tools: &Tools,
        original: &[u8],
        media_type: &str,
    ) -> Result<Vec<u8>, ToolError> {
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
        }
    }
}

/// The commands this process may run, found once.
///
/// A test, or a caller that turns derivatives off, builds one with [`Tools::none`]
/// and names a command with [`Tools::with_ffmpeg`] or [`Tools::with_pdftoppm`].
#[derive(Debug, Clone)]
pub struct Tools {
    ffmpeg: Option<PathBuf>,
    ffprobe: Option<PathBuf>,
    pdftoppm: Option<PathBuf>,
    timeout: Duration,
}

impl Tools {
    /// The tools this process finds on `PATH`, looked up once.
    #[must_use]
    pub fn on_path() -> &'static Self {
        static FOUND: OnceLock<Tools> = OnceLock::new();
        FOUND.get_or_init(|| Self {
            ffmpeg: find_on_path("ffmpeg"),
            ffprobe: find_on_path("ffprobe"),
            pdftoppm: find_on_path("pdftoppm"),
            timeout: TIMEOUT,
        })
    }

    /// No tools: every still is [`ToolError::ToolMissing`].
    #[must_use]
    pub fn none() -> Self {
        Self {
            ffmpeg: None,
            ffprobe: None,
            pdftoppm: None,
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

    /// The timeout a command runs under.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
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
                .expect("still"),
            b"still-bytes"
        );
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
}
