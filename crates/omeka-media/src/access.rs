//! What a stored original needs so a current browser shows it as it is, and
//! when it does not, which access copy a derivative makes.
//!
//! The rule is one-way: **the original is never changed**. It stays the
//! preservation master, downloadable and checksummed. An access copy is a
//! derivative like the three thumbnails, and the players, the Image API and
//! the IIIF canvas read it when the original is not something a browser shows.
//!
//! [`Access::of`] is the table. It answers for video by container and codec,
//! for audio by codec (and the size of a WAV), and for images by type and a
//! test of a TIFF's layout. `None` means this wave has no access rule for the
//! type, which is true of office documents, models and caption files: they get
//! no row rather than a guess.
//!
//! The facts come from [`facts`], which reads a stored original through
//! [`crate::sample`] and runs `ffprobe` for the codecs. The decision is made
//! once, when the media is created or by the backfill, and stored as an
//! `access_copy` row in `romeka_media_derivative` (see
//! `omeka_api::derivatives`): `none` with the words a reader sees, or
//! `pending` with the tool that will make the copy. The worker that runs those
//! tools lands with D3 (audio and video) and D4 (images), which read
//! [`Access::Copy`] and [`crate::tool`]'s argument lists.
//!
//! "A browser shows the original" covers the image path as well: a JPEG, PNG,
//! GIF, WebP, AVIF or SVG is shown by the browser, and a TIFF or JPEG 2000 is
//! shown through the server's own derivatives. So an image's test is whether
//! this server reads its layout, which is the same fact that decides whether
//! the thumbnails exist.

use crate::av_probe::{self, Streams};
use crate::media_type::{RAW_MEDIA_TYPES, extension_for_media_type, is_raw_media_type};
use crate::sample;
use crate::store::Store;
use crate::tool::{AccessTool, Tools};

/// The words a reader sees when the original needs no copy.
pub const ORIGINAL_REASON: &str = "a browser shows the original";

/// The size above which a WAV gets an AAC copy. About three minutes of CD
/// stereo: a reader waits on a download that long, and the same audio as AAC
/// is a tenth of it.
pub const LARGE_WAV_BYTES: u64 = 32 * 1024 * 1024;

/// Video containers no current browser opens, whatever streams they hold. An
/// H.264 track in an AVI still needs an MP4, because the player refuses the
/// container before it reads the codec.
const COPY_VIDEO_TYPES: [&str; 10] = [
    "video/3gpp",
    "video/divx",
    "video/dvd",
    "video/mpeg",
    "video/x-dv",
    "video/x-flv",
    "video/x-ms-asf",
    "video/x-ms-wmv",
    "video/x-ms-vob",
    "video/x-msvideo",
];

/// Video codecs a current browser plays in a container it opens. Every other
/// codec, including one the probe names but this table does not, gets a copy:
/// an unknown codec is not something to promise a reader.
const PLAY_VIDEO_CODECS: [&str; 4] = ["h264", "vp8", "vp9", "av1"];

/// Audio codecs a current browser plays. The `pcm_*le` family is a WAV, whose
/// size decides separately; big-endian PCM is an AIFF and needs a copy.
const PLAY_AUDIO_CODECS: [&str; 5] = ["aac", "mp3", "opus", "vorbis", "flac"];

/// Audio containers whose codec no browser plays, for a file the probe could
/// not read. The codec table catches the same files when the probe answers.
const COPY_AUDIO_TYPES: [&str; 7] = [
    "audio/amr",
    "audio/x-aiff",
    "audio/x-ape",
    "audio/x-monkeys-audio",
    "audio/x-ms-wma",
    "audio/x-ms-wax",
    "audio/x-realaudio",
];

/// Photoshop's two stored types.
const PHOTOSHOP_TYPES: [&str; 2] = ["image/vnd.adobe.photoshop", "image/x-photoshop"];

/// The extensions that name a Photoshop document, a high dynamic range image or
/// a PostScript page, for a type a byte read did not name.
const PHOTOSHOP_EXTENSIONS: [&str; 1] = ["psd"];
const DYNAMIC_RANGE_EXTENSIONS: [&str; 2] = ["exr", "hdr"];
const POSTSCRIPT_EXTENSIONS: [&str; 3] = ["ai", "eps", "ps"];

/// PostScript's stored types: an EPS, an Illustrator file and a plain PS page.
const POSTSCRIPT_TYPES: [&str; 3] = [
    "application/illustrator",
    "application/postscript",
    "image/x-eps",
];

/// What an access copy is, as a player or a page uses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessTarget {
    /// H.264 video with AAC audio in an MP4.
    Mp4,
    /// AAC audio in an M4A.
    M4a,
    /// A high-resolution JPEG access master.
    Jpeg,
}

impl AccessTarget {
    /// The extension a stored copy carries.
    #[must_use]
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::M4a => "m4a",
            Self::Jpeg => "jpg",
        }
    }

    /// The reason in words a reader sees, naming what the copy is.
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::Mp4 => {
                "a browser cannot play this video as stored; an H.264/AAC MP4 plays everywhere"
            }
            Self::M4a => "a browser cannot play this audio as stored; an AAC M4A plays everywhere",
            Self::Jpeg => {
                "a browser cannot show this image as stored; a JPEG access master shows it"
            }
        }
    }
}

/// What a stored original needs for a browser to show it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// The original as stored is what the reader gets.
    Original,
    /// A derivative in `target`'s format, made by the first of `tools` this
    /// install has. A RAW file has two: `exiftool` reads the embedded preview,
    /// which is fast, and `libraw` decodes the sensor data when there is none.
    Copy {
        target: AccessTarget,
        tools: &'static [AccessTool],
    },
}

impl Access {
    /// The decision for one stored file, or `None` when this wave has no rule
    /// for its type.
    #[must_use]
    pub fn of(facts: &Facts) -> Option<Self> {
        let media_type = facts.media_type.as_deref()?;
        if media_type == "application/ogg" {
            let ogv = facts
                .extension
                .as_deref()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("ogv"));
            return Some(if ogv {
                Self::video(facts)
            } else {
                Self::audio(facts)
            });
        }
        if media_type.starts_with("video/") {
            return Some(Self::video(facts));
        }
        if media_type.starts_with("audio/") {
            return Some(Self::audio(facts));
        }
        if media_type == "application/pdf" {
            return Some(Self::Original);
        }
        Self::image(facts, media_type)
    }

    /// The reason in words for a reader, whether the original stands or a copy
    /// is coming.
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::Original => ORIGINAL_REASON,
            Self::Copy { target, .. } => target.reason(),
        }
    }

    /// What the derivative row records for this decision: nothing to do, or
    /// the copy queued with the command that will make it, or nothing to do
    /// because the command is not installed.
    #[must_use]
    pub fn queue(self, tools: &Tools) -> Queue {
        match self {
            Self::Original => Queue::None {
                reason: ORIGINAL_REASON.to_string(),
            },
            Self::Copy { tools: wanted, .. } => {
                match wanted.iter().find_map(|tool| tool.found(tools)) {
                    Some(tool) => Queue::Pending { tool },
                    None => Queue::None {
                        reason: format!("needs {}, not installed", wanted[0].command()),
                    },
                }
            }
        }
    }

    /// Video: a container a browser refuses is a copy whatever it holds, and a
    /// container one opens is a copy when the codec is not one every browser
    /// plays. A probe that answered nothing is a copy, because the codec is
    /// then unknown.
    fn video(facts: &Facts) -> Self {
        let media_type = facts.media_type.as_deref().unwrap_or_default();
        if names(&COPY_VIDEO_TYPES, media_type) {
            return Self::copy(AccessTarget::Mp4, &[AccessTool::Ffmpeg]);
        }
        let Some(codec) = facts.streams.video.as_deref() else {
            return Self::copy(AccessTarget::Mp4, &[AccessTool::Ffmpeg]);
        };
        if !names(&PLAY_VIDEO_CODECS, codec) {
            return Self::copy(AccessTarget::Mp4, &[AccessTool::Ffmpeg]);
        }
        if let Some(audio) = facts.streams.audio.as_deref()
            && !names(&PLAY_AUDIO_CODECS, audio)
        {
            return Self::copy(AccessTarget::Mp4, &[AccessTool::Ffmpeg]);
        }
        Self::Original
    }

    /// Audio: by codec, with the container as the fallback when the probe could
    /// not answer, plus the size rule for a WAV.
    fn audio(facts: &Facts) -> Self {
        let media_type = facts.media_type.as_deref().unwrap_or_default();
        match facts.streams.audio.as_deref() {
            Some(codec) if !audio_plays(codec) => {
                return Self::copy(AccessTarget::M4a, &[AccessTool::Ffmpeg]);
            }
            Some(_) => {}
            None if names(&COPY_AUDIO_TYPES, media_type) => {
                return Self::copy(AccessTarget::M4a, &[AccessTool::Ffmpeg]);
            }
            None => {}
        }
        if is_wav(media_type) && facts.size.is_some_and(|size| size > LARGE_WAV_BYTES) {
            return Self::copy(AccessTarget::M4a, &[AccessTool::Ffmpeg]);
        }
        Self::Original
    }

    /// Images: the types no decoder here reads get an access master from the
    /// one tool that reads them, and a TIFF is decided by its layout.
    fn image(facts: &Facts, media_type: &str) -> Option<Self> {
        let extension = facts.extension.as_deref();
        if is_raw_media_type(media_type) || names_raw_extension(extension) {
            return Some(Self::copy(
                AccessTarget::Jpeg,
                &[AccessTool::ExifTool, AccessTool::LibRaw],
            ));
        }
        if names(&PHOTOSHOP_TYPES, media_type) || names_any(&PHOTOSHOP_EXTENSIONS, extension) {
            return Some(Self::copy(AccessTarget::Jpeg, &[AccessTool::Vips]));
        }
        if media_type == "image/x-exr"
            || media_type == "image/vnd.radiance"
            || media_type == "image/heic"
            || media_type == "image/heif"
            || names_any(&DYNAMIC_RANGE_EXTENSIONS, extension)
        {
            return Some(Self::copy(AccessTarget::Jpeg, &[AccessTool::Vips]));
        }
        if names(&POSTSCRIPT_TYPES, media_type) || names_any(&POSTSCRIPT_EXTENSIONS, extension) {
            return Some(Self::copy(AccessTarget::Jpeg, &[AccessTool::Ghostscript]));
        }
        if media_type == "image/tiff" {
            return Some(match facts.tiff {
                Some(TiffTest::Decodes) => Self::Original,
                _ => Self::copy(AccessTarget::Jpeg, &[AccessTool::Vips]),
            });
        }
        media_type.starts_with("image/").then_some(Self::Original)
    }

    const fn copy(target: AccessTarget, tools: &'static [AccessTool]) -> Self {
        Self::Copy { target, tools }
    }
}

/// What the derivative row records for one decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Queue {
    /// Nothing to make, and the words a reader sees.
    None { reason: String },
    /// The copy is waiting for `tool`.
    Pending { tool: &'static str },
}

/// What a TIFF's directory says about whether this server reads its pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TiffTest {
    /// 8-bit grey, RGB or palette samples in strips, one page: the decoder the
    /// thumbnails use reads it.
    Decodes,
    /// A layout the decoder here does not read, and why.
    Rejects(&'static str),
}

/// The facts of one stored original that decide its access copy. A field the
/// caller did not read stays `None`, and the table then decides from the type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    pub media_type: Option<String>,
    pub extension: Option<String>,
    pub size: Option<u64>,
    /// The container and codecs an audio or video file reports.
    pub streams: Streams,
    /// The layout of a TIFF, when the file is one.
    pub tiff: Option<TiffTest>,
}

impl Facts {
    /// The facts a test or a caller with no store can state, without a probe.
    #[must_use]
    pub fn new(media_type: Option<&str>, extension: Option<&str>, size: Option<u64>) -> Self {
        Self {
            media_type: media_type.map(str::to_string),
            extension: extension.map(str::to_string),
            size,
            streams: Streams::default(),
            tiff: None,
        }
    }
}

/// One stored original, as the decision needs to see it.
#[derive(Debug, Clone, Copy)]
pub struct StoredFile<'a> {
    /// `original/<storage id><extension>`, from [`crate::store::storage_path`].
    pub storage_path: &'a str,
    pub media_type: Option<&'a str>,
    pub extension: Option<&'a str>,
    pub size: Option<u64>,
}

/// Read the facts a stored original's decision needs. An audio or video file is
/// probed through a bounded sample of its ends, and a TIFF's directory is read;
/// nothing else touches the store.
#[must_use]
pub fn facts(store: &dyn Store, tools: &Tools, file: &StoredFile<'_>) -> Facts {
    let mut facts = Facts::new(file.media_type, file.extension, file.size);
    let Some(media_type) = facts.media_type.as_deref() else {
        return facts;
    };
    if media_type.starts_with("audio/")
        || media_type.starts_with("video/")
        || media_type == "application/ogg"
    {
        facts.streams =
            av_probe::probe_streams(store, tools, file.storage_path, file.extension, file.size);
    }
    if is_tiff(media_type, facts.extension.as_deref()) {
        facts.tiff = Some(tiff_test(store, file));
    }
    facts
}

/// Whether this server reads a TIFF's pixels: the directory's colour type, bit
/// depth, chunk type and page count are the four facts the decoder fails on.
fn tiff_test(store: &dyn Store, file: &StoredFile<'_>) -> TiffTest {
    use tiff::ColorType;
    use tiff::decoder::ChunkType;

    let Ok(sample) = sample::stored(store, file.storage_path, file.size, file.extension) else {
        return TiffTest::Rejects("unreadable");
    };
    let Ok(handle) = std::fs::File::open(sample.path()) else {
        return TiffTest::Rejects("unreadable");
    };
    let Ok(mut decoder) = tiff::decoder::Decoder::new(handle) else {
        return TiffTest::Rejects("unreadable");
    };
    match decoder.colortype() {
        Ok(ColorType::Gray(8) | ColorType::RGB(8) | ColorType::RGBA(8)) => {}
        Ok(ColorType::GrayA(8) | ColorType::Palette(8) | ColorType::YCbCr(8)) => {}
        Ok(ColorType::CMYK(_) | ColorType::CMYKA(_) | ColorType::Lab(_)) => {
            return TiffTest::Rejects("cmyk");
        }
        Ok(_) => return TiffTest::Rejects("not 8 bits per sample"),
        Err(_) => return TiffTest::Rejects("unreadable"),
    }
    if decoder.get_chunk_type() == ChunkType::Tile {
        return TiffTest::Rejects("tiled");
    }
    if decoder.more_images() {
        return TiffTest::Rejects("multipage");
    }
    TiffTest::Decodes
}

/// Whether a codec is one a browser plays, by the `ffprobe` name.
fn audio_plays(codec: &str) -> bool {
    names(&PLAY_AUDIO_CODECS, codec)
        || (codec.starts_with("pcm_") && (codec.ends_with("le") || codec == "pcm_u8"))
}

/// Whether a stored type is a WAV container.
fn is_wav(media_type: &str) -> bool {
    matches!(media_type, "audio/wav" | "audio/x-wav" | "audio/wave")
}

/// Whether a stored type or its extension names a TIFF.
fn is_tiff(media_type: &str, extension: Option<&str>) -> bool {
    media_type == "image/tiff" || names_any(&["tif", "tiff"], extension)
}

/// Whether a stored extension names a camera RAW file.
fn names_raw_extension(extension: Option<&str>) -> bool {
    let Some(extension) = extension else {
        return false;
    };
    RAW_MEDIA_TYPES.iter().any(|media_type| {
        extension_for_media_type(media_type)
            .is_some_and(|known| known.eq_ignore_ascii_case(extension))
    })
}

/// Whether `value` is one of `known`, ignoring ASCII case.
fn names(known: &[&'static str], value: &str) -> bool {
    known.iter().any(|known| known.eq_ignore_ascii_case(value))
}

/// [`names`] for a value that may be absent.
fn names_any(known: &[&'static str], value: Option<&str>) -> bool {
    value.is_some_and(|value| names(known, value))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::*;
    use crate::store::FileStore;

    /// The five copies the corpus and the samples compare against.
    const MP4: Access = Access::Copy {
        target: AccessTarget::Mp4,
        tools: &[AccessTool::Ffmpeg],
    };
    const M4A: Access = Access::Copy {
        target: AccessTarget::M4a,
        tools: &[AccessTool::Ffmpeg],
    };
    const RAW_JPEG: Access = Access::Copy {
        target: AccessTarget::Jpeg,
        tools: &[AccessTool::ExifTool, AccessTool::LibRaw],
    };
    const VIPS_JPEG: Access = Access::Copy {
        target: AccessTarget::Jpeg,
        tools: &[AccessTool::Vips],
    };
    const GS_JPEG: Access = Access::Copy {
        target: AccessTarget::Jpeg,
        tools: &[AccessTool::Ghostscript],
    };

    fn corpus(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/media-kinds")
            .join(name)
    }

    /// Store one corpus file and answer its storage path and size.
    fn stored(store: &FileStore, name: &str) -> (String, u64) {
        let bytes = std::fs::read(corpus(name)).expect("the fixture");
        let path = format!("original/{name}");
        store.put(&bytes, &path, None).expect("store");
        (path, bytes.len() as u64)
    }

    /// The corpus, and what each file needs. The decision on a video or audio
    /// file comes from the codecs `ffprobe` reports, so those rows are skipped
    /// when the command is absent.
    const CORPUS: [(&str, &str, &str, Option<Access>); 27] = [
        ("image.jpg", "image/jpeg", "jpg", Some(Access::Original)),
        ("image.png", "image/png", "png", Some(Access::Original)),
        ("image.gif", "image/gif", "gif", Some(Access::Original)),
        ("image.webp", "image/webp", "webp", Some(Access::Original)),
        ("image.tif", "image/tiff", "tif", Some(VIPS_JPEG)),
        ("image.nef", "image/x-nikon-nef", "nef", Some(RAW_JPEG)),
        ("image.avif", "image/avif", "avif", Some(Access::Original)),
        ("image.heic", "image/heic", "heic", Some(VIPS_JPEG)),
        ("image.svg", "image/svg+xml", "svg", Some(Access::Original)),
        (
            "hostile.svg",
            "image/svg+xml",
            "svg",
            Some(Access::Original),
        ),
        ("audio.mp3", "audio/mpeg", "mp3", Some(Access::Original)),
        ("audio.m4a", "audio/mp4", "m4a", Some(Access::Original)),
        ("audio.ogg", "audio/ogg", "ogg", Some(Access::Original)),
        ("audio.opus", "audio/ogg", "opus", Some(Access::Original)),
        ("audio.flac", "audio/flac", "flac", Some(Access::Original)),
        ("audio.wav", "audio/x-wav", "wav", Some(Access::Original)),
        ("video.mp4", "video/mp4", "mp4", Some(Access::Original)),
        ("video.webm", "video/webm", "webm", Some(Access::Original)),
        (
            "video.mov",
            "video/quicktime",
            "mov",
            Some(Access::Original),
        ),
        (
            "video.mkv",
            "video/x-matroska",
            "mkv",
            Some(Access::Original),
        ),
        ("video.ogv", "video/ogg", "ogv", Some(MP4)),
        ("doc.pdf", "application/pdf", "pdf", Some(Access::Original)),
        ("doc.docx", "DOCX", "docx", None),
        ("model.glb", "model/gltf-binary", "glb", None),
        ("model.gltf", "model/gltf+json", "gltf", None),
        ("captions.vtt", "text/vtt", "vtt", None),
        ("captions.srt", "application/x-subrip", "srt", None),
    ];

    /// Every corpus file through the real store and the real tools: an image
    /// by its type and layout, audio and video by what `ffprobe` names, and no
    /// row for a kind this wave has no rule for.
    #[test]
    fn the_corpus_files_get_their_decision() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileStore::new(dir.path(), "/files");
        let tools = Tools::on_path();
        let mut skipped = 0;
        for (name, media_type, extension, expected) in CORPUS {
            let (path, size) = stored(&store, name);
            let file = StoredFile {
                storage_path: &path,
                media_type: Some(media_type),
                extension: Some(extension),
                size: Some(size),
            };
            let facts = facts(&store, tools, &file);
            if tools.ffprobe().is_none()
                && (media_type.starts_with("video/") || media_type.starts_with("audio/"))
            {
                skipped += 1;
                continue;
            }
            assert_eq!(
                Access::of(&facts),
                expected,
                "{name}, codecs {:?}",
                facts.streams
            );
        }
        if skipped > 0 {
            eprintln!("skipped {skipped} audio and video rows: no ffprobe on PATH");
        }
    }

    /// The corpus TIFF is 16 bits a sample, which the brief names as a layout
    /// that needs an access master even though the reduced thumbnail path
    /// reads it.
    #[test]
    fn the_corpus_tiff_is_16_bit_and_gets_a_master() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileStore::new(dir.path(), "/files");
        let (path, size) = stored(&store, "image.tif");
        let facts = facts(
            &store,
            Tools::on_path(),
            &StoredFile {
                storage_path: &path,
                media_type: Some("image/tiff"),
                extension: Some("tif"),
                size: Some(size),
            },
        );
        assert_eq!(facts.tiff, Some(TiffTest::Rejects("not 8 bits per sample")));
        assert_eq!(Access::of(&facts), Some(VIPS_JPEG));
    }

    /// A video container a browser refuses is copied whatever it holds, and a
    /// container one opens is copied when the codec is not one every browser
    /// plays. A probe that answered nothing is a copy.
    #[test]
    fn the_video_table_follows_the_container_and_the_codecs() {
        let video = |media_type: &str, video_codec: Option<&str>, audio: Option<&str>| {
            let mut facts = Facts::new(Some(media_type), None, None);
            facts.streams.video = video_codec.map(str::to_string);
            facts.streams.audio = audio.map(str::to_string);
            Access::of(&facts)
        };
        for media_type in COPY_VIDEO_TYPES {
            assert_eq!(video(media_type, Some("h264"), Some("aac")), Some(MP4));
        }
        assert_eq!(
            video("video/mp4", Some("h264"), Some("aac")),
            Some(Access::Original)
        );
        assert_eq!(
            video("video/quicktime", Some("h264"), None),
            Some(Access::Original)
        );
        assert_eq!(
            video("video/webm", Some("vp9"), Some("opus")),
            Some(Access::Original)
        );
        assert_eq!(
            video("video/webm", Some("av1"), Some("opus")),
            Some(Access::Original)
        );
        assert_eq!(video("video/mp4", Some("hevc"), Some("aac")), Some(MP4));
        assert_eq!(
            video("video/quicktime", Some("prores"), Some("pcm_s16le")),
            Some(MP4)
        );
        assert_eq!(video("video/x-matroska", Some("mjpeg"), None), Some(MP4));
        assert_eq!(video("video/avi", Some("mpeg4"), Some("mp3")), Some(MP4));
        assert_eq!(
            video("video/mp4", Some("h264"), Some("ac3")),
            Some(MP4),
            "the video is fine and the audio track is not"
        );
        assert_eq!(video("video/mp4", None, None), Some(MP4), "unknown streams");
    }

    /// Audio by codec: the containers a browser refuses, the codecs it plays,
    /// and the WAV whose size is the reason for a copy.
    #[test]
    fn the_audio_table_follows_the_codec_and_the_wav_size() {
        let audio = |media_type: &str, codec: Option<&str>, size: Option<u64>| {
            let mut facts = Facts::new(Some(media_type), None, size);
            facts.streams.audio = codec.map(str::to_string);
            Access::of(&facts)
        };
        for codec in [
            "wmav1", "wmav2", "wmapro", "ape", "amr_nb", "amr_wb", "real_144",
        ] {
            assert_eq!(
                audio("audio/x-ms-wma", Some(codec), None),
                Some(M4A),
                "{codec}"
            );
        }
        for codec in ["mp3", "aac", "opus", "vorbis", "flac", "pcm_s16le"] {
            assert_eq!(
                audio("audio/mpeg", Some(codec), None),
                Some(Access::Original),
                "{codec}"
            );
        }
        assert_eq!(
            audio("audio/x-aiff", Some("pcm_s16be"), None),
            Some(M4A),
            "big-endian PCM is an AIFF"
        );
        assert_eq!(
            audio("audio/x-wav", Some("pcm_s16le"), Some(LARGE_WAV_BYTES)),
            Some(Access::Original),
            "the threshold itself is small"
        );
        assert_eq!(
            audio("audio/x-wav", Some("pcm_s16le"), Some(LARGE_WAV_BYTES + 1)),
            Some(M4A)
        );
        assert_eq!(
            audio("audio/x-ms-wma", None, None),
            Some(M4A),
            "no probe: the container decides"
        );
        assert_eq!(audio("audio/mpeg", None, None), Some(Access::Original));
    }

    /// Images: the types no decoder here reads get a master from the one tool
    /// that reads them, and an 8-bit TIFF the decoder reads does not.
    #[test]
    fn the_image_table_follows_the_type_and_the_tiff_layout() {
        let image = |media_type: &str, extension: &str, tiff: Option<TiffTest>| {
            let mut facts = Facts::new(Some(media_type), Some(extension), None);
            facts.tiff = tiff;
            Access::of(&facts)
        };
        for media_type in RAW_MEDIA_TYPES {
            assert_eq!(
                image(media_type, "raw", None),
                Some(RAW_JPEG),
                "{media_type}"
            );
        }
        assert_eq!(
            image("image/vnd.adobe.photoshop", "psd", None),
            Some(VIPS_JPEG)
        );
        assert_eq!(image("image/x-exr", "exr", None), Some(VIPS_JPEG));
        assert_eq!(image("image/vnd.radiance", "hdr", None), Some(VIPS_JPEG));
        assert_eq!(image("image/heic", "heic", None), Some(VIPS_JPEG));
        assert_eq!(image("application/postscript", "eps", None), Some(GS_JPEG));
        assert_eq!(image("application/illustrator", "ai", None), Some(GS_JPEG));
        assert_eq!(image("image/jpeg", "jpg", None), Some(Access::Original));
        assert_eq!(image("image/png", "png", None), Some(Access::Original));
        assert_eq!(image("image/gif", "gif", None), Some(Access::Original));
        assert_eq!(image("image/webp", "webp", None), Some(Access::Original));
        assert_eq!(image("image/avif", "avif", None), Some(Access::Original));
        assert_eq!(image("image/svg+xml", "svg", None), Some(Access::Original));
        assert_eq!(
            image("image/tiff", "tif", Some(TiffTest::Decodes)),
            Some(Access::Original)
        );
        for reason in ["cmyk", "not 8 bits per sample", "tiled", "multipage"] {
            assert_eq!(
                image("image/tiff", "tif", Some(TiffTest::Rejects(reason))),
                Some(VIPS_JPEG),
                "{reason}"
            );
        }
        assert_eq!(
            image("image/tiff", "tif", None),
            Some(VIPS_JPEG),
            "a TIFF whose layout was not read is not promised"
        );
    }

    /// This wave has no access rule for a document, a model or a caption file,
    /// so it writes no row rather than a guess.
    #[test]
    fn a_kind_with_no_rule_has_no_decision() {
        for (media_type, extension) in [
            ("application/msword", "doc"),
            (
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                "docx",
            ),
            ("model/gltf-binary", "glb"),
            ("text/vtt", "vtt"),
            ("application/x-subrip", "srt"),
        ] {
            assert_eq!(
                Access::of(&Facts::new(Some(media_type), Some(extension), None)),
                None,
                "{media_type}"
            );
        }
    }

    /// The derivative row's state: nothing to make, or the copy waiting for a
    /// command, or nothing to make because the command is not installed.
    #[test]
    fn a_decision_names_the_command_or_why_it_cannot_run() {
        let dir = tempfile::tempdir().expect("tempdir");
        let present = |name: &str| dir.path().join(name);
        for tool in AccessTool::ALL {
            std::fs::write(present(tool.command()), "").expect("a stand-in command");
        }
        let all = Tools::none()
            .with_ffmpeg(present("ffmpeg"))
            .with_exiftool(present("exiftool"))
            .with_libraw(present("dcraw_emu"))
            .with_vips(present("vips"))
            .with_ghostscript(present("gs"));
        assert_eq!(
            Access::Original.queue(&all),
            Queue::None {
                reason: "a browser shows the original".to_string()
            }
        );
        assert_eq!(MP4.queue(&all), Queue::Pending { tool: "ffmpeg" });
        assert_eq!(M4A.queue(&all), Queue::Pending { tool: "ffmpeg" });
        assert_eq!(VIPS_JPEG.queue(&all), Queue::Pending { tool: "vips" });
        assert_eq!(GS_JPEG.queue(&all), Queue::Pending { tool: "gs" });
        assert_eq!(RAW_JPEG.queue(&all), Queue::Pending { tool: "exiftool" });

        let none = Tools::none();
        assert_eq!(
            RAW_JPEG.queue(&none),
            Queue::None {
                reason: "needs exiftool, not installed".to_string()
            }
        );
        assert_eq!(
            VIPS_JPEG.queue(&none),
            Queue::None {
                reason: "needs vips, not installed".to_string()
            }
        );
        assert_eq!(
            MP4.queue(&none),
            Queue::None {
                reason: "needs ffmpeg, not installed".to_string()
            }
        );
        let raw_only = Tools::none().with_libraw(present("dcraw_emu"));
        assert_eq!(
            RAW_JPEG.queue(&raw_only),
            Queue::Pending { tool: "dcraw_emu" },
            "the second tool makes the copy when the first is absent"
        );
    }

    /// A TIFF the decoder cannot read gets a copy: a CMYK one, a 16-bit one and
    /// a second page are the layouts the plan names.
    #[test]
    fn a_tiff_layout_the_decoder_does_not_read_gets_a_copy() {
        use tiff::encoder::TiffEncoder;
        use tiff::encoder::colortype::{CMYK8, RGB8, RGB16};

        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileStore::new(dir.path(), "/files");
        let encode = |name: &str, write: &dyn Fn(&mut TiffEncoder<std::fs::File>)| {
            let path = dir.path().join(name);
            let mut encoder =
                TiffEncoder::new(std::fs::File::create(&path).expect("create")).expect("encoder");
            write(&mut encoder);
            let bytes = std::fs::read(&path).expect("read");
            let stored = format!("original/{name}");
            store.put(&bytes, &stored, None).expect("store");
            let facts = facts(
                &store,
                Tools::on_path(),
                &StoredFile {
                    storage_path: &stored,
                    media_type: Some("image/tiff"),
                    extension: Some("tif"),
                    size: Some(bytes.len() as u64),
                },
            );
            let access = Access::of(&facts);
            (facts, access)
        };

        let (facts, access) = encode("eight.tif", &|encoder| {
            encoder
                .write_image::<RGB8>(2, 2, &[0u8; 12])
                .expect("8-bit");
        });
        assert_eq!(facts.tiff, Some(TiffTest::Decodes));
        assert_eq!(access, Some(Access::Original));

        let (facts, access) = encode("cmyk.tif", &|encoder| {
            encoder
                .write_image::<CMYK8>(2, 2, &[0u8; 16])
                .expect("cmyk");
        });
        assert_eq!(facts.tiff, Some(TiffTest::Rejects("cmyk")));
        assert_eq!(access, Some(VIPS_JPEG));

        let (facts, access) = encode("deep.tif", &|encoder| {
            encoder
                .write_image::<RGB16>(2, 2, &[0u16; 12])
                .expect("16-bit");
        });
        assert_eq!(facts.tiff, Some(TiffTest::Rejects("not 8 bits per sample")));
        assert_eq!(access, Some(VIPS_JPEG));

        let (facts, access) = encode("pages.tif", &|encoder| {
            encoder
                .write_image::<RGB8>(2, 2, &[0u8; 12])
                .expect("page 1");
            encoder
                .write_image::<RGB8>(2, 2, &[0u8; 12])
                .expect("page 2");
        });
        assert_eq!(facts.tiff, Some(TiffTest::Rejects("multipage")));
        assert_eq!(access, Some(VIPS_JPEG));
    }

    /// One generated sample per format the brief names, made with `ffmpeg` in
    /// the test rather than committed, so the corpus stays small.
    const VIDEO_SOURCE: [&str; 9] = [
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=96x64:rate=25:duration=0.3",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:duration=0.3:sample_rate=8000",
        "-shortest",
    ];
    const AUDIO_SOURCE: [&str; 4] = [
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:duration=0.3:sample_rate=8000",
    ];
    const DV_SOURCE: [&str; 9] = [
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=720x480:rate=30000/1001:duration=0.3",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:duration=0.3:sample_rate=48000",
        "-shortest",
    ];

    struct Sample {
        name: &'static str,
        media_type: &'static str,
        extension: &'static str,
        source: &'static [&'static str],
        codec_args: &'static [&'static str],
        expected: Option<Access>,
    }

    const SAMPLES: [Sample; 16] = [
        Sample {
            name: "mjpeg.avi",
            media_type: "video/x-msvideo",
            extension: "avi",
            source: &VIDEO_SOURCE,
            codec_args: &["-c:v", "mjpeg", "-c:a", "pcm_s16le", "-f", "avi"],
            expected: Some(MP4),
        },
        Sample {
            name: "wmv.asf",
            media_type: "video/x-ms-asf",
            extension: "asf",
            source: &VIDEO_SOURCE,
            codec_args: &["-c:v", "wmv2", "-c:a", "wmav2", "-f", "asf"],
            expected: Some(MP4),
        },
        Sample {
            name: "mpeg1.mpg",
            media_type: "video/mpeg",
            extension: "mpg",
            source: &VIDEO_SOURCE,
            codec_args: &["-c:v", "mpeg1video", "-c:a", "mp2", "-f", "mpeg"],
            expected: Some(MP4),
        },
        Sample {
            name: "vob.vob",
            media_type: "video/mpeg",
            extension: "vob",
            source: &VIDEO_SOURCE,
            codec_args: &["-c:v", "mpeg2video", "-c:a", "ac3", "-f", "vob"],
            expected: Some(MP4),
        },
        Sample {
            name: "flv.flv",
            media_type: "video/x-flv",
            extension: "flv",
            source: &VIDEO_SOURCE,
            codec_args: &["-c:v", "flv", "-c:a", "aac", "-f", "flv"],
            expected: Some(MP4),
        },
        Sample {
            name: "dv.dv",
            media_type: "video/x-dv",
            extension: "dv",
            source: &DV_SOURCE,
            codec_args: &[
                "-c:v",
                "dvvideo",
                "-pix_fmt",
                "yuv411p",
                "-c:a",
                "pcm_s16le",
                "-ac",
                "2",
                "-f",
                "dv",
            ],
            expected: Some(MP4),
        },
        Sample {
            name: "h263.3gp",
            media_type: "video/3gpp",
            extension: "3gp",
            source: &VIDEO_SOURCE,
            codec_args: &[
                "-vf",
                "scale=176:144",
                "-c:v",
                "h263",
                "-c:a",
                "aac",
                "-f",
                "3gp",
            ],
            expected: Some(MP4),
        },
        Sample {
            name: "hevc.mov",
            media_type: "video/quicktime",
            extension: "mov",
            source: &VIDEO_SOURCE,
            codec_args: &[
                "-c:v",
                "libx265",
                "-preset",
                "ultrafast",
                "-crf",
                "40",
                "-x265-params",
                "log-level=error",
                "-c:a",
                "aac",
                "-f",
                "mov",
            ],
            expected: Some(MP4),
        },
        Sample {
            name: "prores.mov",
            media_type: "video/quicktime",
            extension: "mov",
            source: &VIDEO_SOURCE,
            codec_args: &[
                "-vf",
                "scale=1280:720",
                "-c:v",
                "prores_ks",
                "-profile:v",
                "0",
                "-c:a",
                "pcm_s16le",
                "-f",
                "mov",
            ],
            expected: Some(MP4),
        },
        Sample {
            name: "h264.mp4",
            media_type: "video/mp4",
            extension: "mp4",
            source: &VIDEO_SOURCE,
            codec_args: &[
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-crf",
                "40",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-movflags",
                "+faststart",
                "-f",
                "mp4",
            ],
            expected: Some(Access::Original),
        },
        Sample {
            name: "theora.ogv",
            media_type: "video/ogg",
            extension: "ogv",
            source: &VIDEO_SOURCE,
            codec_args: &["-c:v", "libtheora", "-c:a", "libvorbis", "-f", "ogg"],
            expected: Some(MP4),
        },
        Sample {
            name: "wma.wma",
            media_type: "audio/x-ms-wma",
            extension: "wma",
            source: &AUDIO_SOURCE,
            codec_args: &["-c:a", "wmav2", "-f", "asf"],
            expected: Some(M4A),
        },
        Sample {
            name: "aiff.aiff",
            media_type: "audio/x-aiff",
            extension: "aiff",
            source: &AUDIO_SOURCE,
            codec_args: &["-c:a", "pcm_s16be", "-f", "aiff"],
            expected: Some(M4A),
        },
        Sample {
            name: "wav.wav",
            media_type: "audio/x-wav",
            extension: "wav",
            source: &AUDIO_SOURCE,
            codec_args: &["-c:a", "pcm_s16le", "-f", "wav"],
            expected: Some(Access::Original),
        },
        Sample {
            name: "aac.m4a",
            media_type: "audio/mp4",
            extension: "m4a",
            source: &AUDIO_SOURCE,
            codec_args: &["-c:a", "aac", "-f", "ipod"],
            expected: Some(Access::Original),
        },
        Sample {
            name: "flac.flac",
            media_type: "audio/flac",
            extension: "flac",
            source: &AUDIO_SOURCE,
            codec_args: &["-c:a", "flac", "-f", "flac"],
            expected: Some(Access::Original),
        },
    ];

    /// Make one sample with `ffmpeg`, or `None` when the build lacks its
    /// encoder (the test then states what it skipped).
    fn encoded(dir: &Path, sample: &Sample) -> Option<PathBuf> {
        let mut args: Vec<&str> = sample.source.to_vec();
        args.extend_from_slice(sample.codec_args);
        let output = dir.join(sample.name);
        let status = Command::new("ffmpeg")
            .args(["-nostdin", "-y", "-v", "error"])
            .args(&args)
            .arg(&output)
            .status()
            .ok()?;
        status.success().then_some(output)
    }

    /// Each generated sample goes through the real probe and gets the copy the
    /// table names, which covers MJPEG AVI, WMV, MPEG-1/2, VOB, FLV, DV, 3GP,
    /// HEVC and ProRes video and WMA and AIFF audio.
    #[test]
    fn the_generated_samples_get_the_copy_the_table_names() {
        if Tools::on_path().ffmpeg().is_none() || Tools::on_path().ffprobe().is_none() {
            eprintln!("skipping the generated samples: no ffmpeg or ffprobe on PATH");
            return;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileStore::new(dir.path(), "/files");
        let tools = Tools::on_path();
        let mut made = 0;
        for sample in &SAMPLES {
            let Some(path) = encoded(dir.path(), sample) else {
                eprintln!("skipping {}: ffmpeg has no encoder for it", sample.name);
                continue;
            };
            let bytes = std::fs::read(&path).expect("the sample");
            let stored_path = format!("original/{}", sample.name);
            store.put(&bytes, &stored_path, None).expect("store");
            let facts = facts(
                &store,
                tools,
                &StoredFile {
                    storage_path: &stored_path,
                    media_type: Some(sample.media_type),
                    extension: Some(sample.extension),
                    size: Some(bytes.len() as u64),
                },
            );
            assert_eq!(
                Access::of(&facts),
                sample.expected,
                "{}: container {:?}, codecs {:?}",
                sample.name,
                facts.streams.format,
                facts.streams
            );
            made += 1;
        }
        assert!(made >= 12, "only {made} samples were made");
    }
}
