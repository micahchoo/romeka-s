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
use crate::store::Store;
use crate::tool::Tools;

/// The pieces a stored original is copied to a temporary file in: an object
/// store answers in bounded reads, and `ffprobe` opens a file.
const CHUNK: u64 = 8 * 1024 * 1024;

/// The largest original this copies out, the same 2 GiB the HTTP client will
/// fetch. A bigger file is not probed; its canvas is left out.
const MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;

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

    use std::os::unix::fs::PermissionsExt as _;
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
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write script");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("make it executable");
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
}
