//! `ffprobe`'s answer for an audio or video original: the duration a canvas
//! needs and, for a video, the frame size it paints.
//!
//! The command runs through [`crate::tool`], the one process seam, so a missing
//! `ffprobe` is a typed [`ToolError`] and never an error of its own. A file the
//! probe cannot read has no extent; the canvas is then left out, because a
//! time-based canvas needs a duration.
//!
//! The probe is blocking: it runs a command. A caller on an async runtime owns
//! a blocking thread for it.

use std::io::Write as _;
use std::path::Path;

use serde_json::Value;

use crate::kind::Extent;
use crate::sample;
use crate::store::Store;
use crate::tool::Tools;

/// The pieces a stored original is copied to a temporary file in: an object
/// store answers in bounded reads, and `ffprobe` opens a file.
const CHUNK: u64 = 8 * 1024 * 1024;

/// The largest original this copies out, the same 2 GiB the HTTP client will
/// fetch. A bigger file is not probed; its canvas is left out.
const MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// The container and the first codec of each stream type: what an access rule
/// reads of an audio or video original. Every member is `None` when the probe
/// could not answer, which the access table treats as a file whose streams it
/// does not know.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Streams {
    /// `ffprobe`'s first `format_name`: `mov` for an MP4 or a QuickTime file,
    /// `matroska` for a WebM or an MKV, `ogg`, `avi`, `asf`, `mpeg`, `flv`.
    pub format: Option<String>,
    /// The first video stream's `codec_name`, such as `h264` or `hevc`.
    pub video: Option<String>,
    /// The first audio stream's `codec_name`, such as `aac` or `wmav2`.
    pub audio: Option<String>,
}

/// The extent of the file at `path`, probed with the tools this process found
/// on `PATH`. `None` when `ffprobe` is missing, fails, or reports no positive
/// duration.
#[must_use]
pub fn probe(path: &Path) -> Option<Extent> {
    probe_with(Tools::on_path(), path)
}

/// [`probe`] with the tools a caller names.
#[must_use]
pub fn probe_with(tools: &Tools, path: &Path) -> Option<Extent> {
    parse(&tools.probe_json(path).ok()?)
}

/// The extent of the original at `storage_path`, copied to a temporary file
/// first. The copy carries `extension`, because `ffprobe` leans on it to pick
/// the demuxer.
///
/// `None` when the file cannot be read, is larger than [`MAX_BYTES`], or
/// [`probe`] answers nothing.
#[must_use]
pub fn probe_stored(
    store: &dyn Store,
    tools: &Tools,
    storage_path: &str,
    extension: Option<&str>,
) -> Option<Extent> {
    let suffix = extension
        .filter(|extension| !extension.is_empty())
        .map(|extension| format!(".{extension}"))
        .unwrap_or_default();
    let mut file = tempfile::Builder::new()
        .prefix("romeka-probe")
        .suffix(&suffix)
        .tempfile()
        .ok()?;
    copy(store, storage_path, file.as_file_mut()).ok()?;
    probe_with(tools, file.path())
}

/// The container and stream codecs of the stored original at `storage_path`,
/// read from a bounded sample of its ends: see [`crate::sample`]. A file an
/// object store holds is never copied whole to answer this, because a video's
/// metadata sits at one end and a create pays this cost.
///
/// Every member is `None` when the file cannot be read, is larger than
/// [`MAX_BYTES`], or the probe fails, so a caller's rule decides what an
/// unknown codec means.
#[must_use]
pub fn probe_streams(
    store: &dyn Store,
    tools: &Tools,
    storage_path: &str,
    extension: Option<&str>,
    size: Option<u64>,
) -> Streams {
    if size.is_some_and(|size| size > MAX_BYTES) {
        return Streams::default();
    }
    let Ok(file) = sample::stored(store, storage_path, size, extension) else {
        return Streams::default();
    };
    match tools.probe_codecs_json(file.path()) {
        Ok(answer) => parse_streams(&answer),
        Err(_) => Streams::default(),
    }
}

/// The streams in one `ffprobe` answer. The first codec of each stream type is
/// the one a browser's player is given.
fn parse_streams(answer: &str) -> Streams {
    let Ok(answer) = serde_json::from_str::<Value>(answer) else {
        return Streams::default();
    };
    let format = answer
        .get("format")
        .and_then(|format| format.get("format_name"))
        .and_then(Value::as_str)
        .and_then(|names| names.split(',').next())
        .filter(|name| !name.is_empty())
        .map(str::to_string);
    let mut streams = Streams {
        format,
        video: None,
        audio: None,
    };
    for stream in answer
        .get("streams")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(codec) = stream.get("codec_name").and_then(Value::as_str) else {
            continue;
        };
        match stream.get("codec_type").and_then(Value::as_str) {
            Some("video") if streams.video.is_none() => streams.video = Some(codec.to_string()),
            Some("audio") if streams.audio.is_none() => streams.audio = Some(codec.to_string()),
            _ => {}
        }
    }
    streams
}

/// Copy a stored original into `out` in [`CHUNK`] pieces, so a large video is
/// never held whole.
fn copy(store: &dyn Store, storage_path: &str, out: &mut std::fs::File) -> Result<u64, CopyError> {
    let mut copied = 0;
    loop {
        if copied > MAX_BYTES {
            return Err(CopyError::TooLarge);
        }
        let bytes = store
            .read_range(storage_path, copied..copied + CHUNK)
            .map_err(|_| CopyError::Unreadable)?;
        if bytes.is_empty() {
            return Ok(copied);
        }
        copied += bytes.len() as u64;
        out.write_all(&bytes).map_err(|_| CopyError::Unwritable)?;
    }
}

/// Why an original was not copied out. The caller leaves the canvas out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CopyError {
    Unreadable,
    Unwritable,
    TooLarge,
}

/// The extent in one `ffprobe` answer. A positive duration is required; the
/// first video stream's frame size is taken when it names one.
fn parse(answer: &str) -> Option<Extent> {
    let answer: Value = serde_json::from_str(answer).ok()?;
    let seconds = answer
        .get("format")?
        .get("duration")?
        .as_str()?
        .parse::<f64>()
        .ok()?;
    let milliseconds = (seconds * 1000.0).round();
    if !milliseconds.is_finite() || milliseconds < 1.0 || milliseconds > u64::MAX as f64 {
        return None;
    }
    let size = answer
        .get("streams")?
        .as_array()?
        .iter()
        .filter(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("video"))
        .find_map(|stream| {
            let width = u32::try_from(stream.get("width")?.as_u64()?).ok()?;
            let height = u32::try_from(stream.get("height")?.as_u64()?).ok()?;
            (width > 0 && height > 0).then_some((width, height))
        });
    Some(Extent {
        size,
        duration_ms: Some(milliseconds as u64),
    })
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::path::PathBuf;

    use super::*;

    fn dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    /// The fixture is read from the corpus: its own answers are the contract.
    fn corpus(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/media-kinds")
            .join(name)
    }

    /// A stand-in command, so a test never depends on what `ffprobe` prints.
    fn script(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("ffprobe");
        crate::testing::write_script(&path, &format!("#!/bin/sh\n{body}\n")).expect("write script");
        path
    }

    fn tools_for(dir: &Path, body: &str) -> Tools {
        Tools::none().with_ffprobe(script(dir, body))
    }

    /// The real corpus file, through the real `ffprobe`: 350 milliseconds of
    /// audio and no frame size.
    #[test]
    fn an_audio_file_has_a_duration_and_no_frame_size() {
        if Tools::on_path().ffprobe().is_none() {
            return;
        }
        assert_eq!(
            probe(&corpus("audio.mp3")),
            Some(Extent {
                size: None,
                duration_ms: Some(350),
            })
        );
    }

    /// The real corpus video adds its frame size.
    #[test]
    fn a_video_file_adds_its_frame_size() {
        if Tools::on_path().ffprobe().is_none() {
            return;
        }
        assert_eq!(
            probe(&corpus("video.mp4")),
            Some(Extent {
                size: Some((96, 64)),
                duration_ms: Some(500),
            })
        );
    }

    /// A file the command cannot read has no extent, which leaves the canvas
    /// out rather than failing anything.
    #[test]
    fn an_unreadable_file_has_no_extent() {
        let dir = dir();
        let tools = tools_for(dir.path(), "echo 'Invalid data' >&2; exit 1");
        assert_eq!(probe_with(&tools, &corpus("audio.mp3")), None);
    }

    /// A container that reports no duration (`N/A`) has no extent either.
    #[test]
    fn no_duration_means_no_extent() {
        let dir = dir();
        let tools = tools_for(dir.path(), r#"printf '%s' '{"streams":[],"format":{}}'"#);
        assert_eq!(probe_with(&tools, &corpus("audio.mp3")), None);
    }

    /// A copy out of the store is the whole original, probed in a file with
    /// its extension.
    #[test]
    fn a_stored_original_is_probed_through_a_copy() {
        let dir = dir();
        let store = crate::store::FileStore::new(dir.path(), "/files");
        let bytes = std::fs::read(corpus("audio.ogg")).expect("the fixture");
        store
            .put(&bytes, "original/abc.ogg", Some("audio/ogg"))
            .expect("store");
        let log = dir.path().join("args");
        let tools = tools_for(
            dir.path(),
            &format!(
                "printf '%s\\n' \"$@\" > {log}\n\
                 printf '%s' '{{\"streams\":[{{\"codec_type\":\"audio\"}}],\
                 \"format\":{{\"duration\":\"0.350000\"}}}}'",
                log = log.display()
            ),
        );
        let extent =
            probe_stored(&store, &tools, "original/abc.ogg", Some("ogg")).expect("an extent");
        assert_eq!(extent.duration_ms, Some(350));
        let recorded = std::fs::read_to_string(&log).expect("args");
        assert!(
            recorded.contains(".ogg"),
            "the copy keeps the extension: {recorded}"
        );
    }

    /// A path the store does not hold is no extent, not an error.
    #[test]
    fn a_missing_original_has_no_extent() {
        let dir = dir();
        let store = crate::store::FileStore::new(dir.path(), "/files");
        let tools = tools_for(dir.path(), "exit 0");
        assert_eq!(
            probe_stored(&store, &tools, "original/gone.mp3", Some("mp3")),
            None
        );
    }

    /// Milliseconds are rounded, a zero is no duration, and the first video
    /// stream names the frame size.
    #[test]
    fn the_answer_is_read_as_milliseconds_and_a_frame_size() {
        let answer = r#"{"streams":[
            {"codec_type":"audio"},
            {"codec_type":"video","width":1920,"height":1080}
        ],"format":{"duration":"62.3456"}}"#;
        assert_eq!(
            parse(answer),
            Some(Extent {
                size: Some((1920, 1080)),
                duration_ms: Some(62_346),
            })
        );
        assert_eq!(parse(r#"{"streams":[],"format":{"duration":"0.0"}}"#), None);
        assert_eq!(parse(r#"{"streams":[],"format":{"duration":"N/A"}}"#), None);
        assert_eq!(parse("not json"), None);
    }

    /// The first name of the container list and the first codec of each stream
    /// type are what an access rule reads.
    #[test]
    fn the_streams_name_the_container_and_the_first_codecs() {
        let answer = r#"{"streams":[
            {"codec_type":"video","codec_name":"hevc"},
            {"codec_type":"video","codec_name":"mjpeg"},
            {"codec_type":"audio","codec_name":"aac"},
            {"codec_type":"audio","codec_name":"wmav2"},
            {"codec_type":"data"}
        ],"format":{"format_name":"mov,mp4,m4a,3gp,3g2,mj2"}}"#;
        assert_eq!(
            parse_streams(answer),
            Streams {
                format: Some("mov".to_string()),
                video: Some("hevc".to_string()),
                audio: Some("aac".to_string()),
            }
        );
        assert_eq!(parse_streams("not json"), Streams::default());
        assert_eq!(
            parse_streams(r#"{"streams":[],"format":{}}"#),
            Streams::default()
        );
    }

    /// A stored corpus video is probed through a sample of its ends and names
    /// its container and both codecs, without a local path from the store.
    #[test]
    fn a_stored_video_names_its_container_and_codecs() {
        if Tools::on_path().ffprobe().is_none() {
            eprintln!("skipping: no ffprobe on PATH");
            return;
        }
        let dir = dir();
        let store = crate::store::FileStore::new(dir.path(), "/files");
        let bytes = std::fs::read(corpus("video.mov")).expect("the fixture");
        store
            .put(&bytes, "original/mov.mov", Some("video/quicktime"))
            .expect("store");
        assert_eq!(
            probe_streams(
                &store,
                Tools::on_path(),
                "original/mov.mov",
                Some("mov"),
                Some(bytes.len() as u64),
            ),
            Streams {
                format: Some("mov".to_string()),
                video: Some("h264".to_string()),
                audio: Some("aac".to_string()),
            }
        );
    }

    /// A file the store does not hold answers no streams, which leaves the
    /// decision to the media type.
    #[test]
    fn a_missing_original_has_no_streams() {
        let dir = dir();
        let store = crate::store::FileStore::new(dir.path(), "/files");
        assert_eq!(
            probe_streams(
                &store,
                Tools::on_path(),
                "original/gone.mov",
                Some("mov"),
                None
            ),
            Streams::default()
        );
    }
}
