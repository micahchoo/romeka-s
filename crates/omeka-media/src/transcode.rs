//! The playable copy a browser needs when it cannot open the original.
//!
//! One access-copy row of kind `access_copy` names a target and a tool
//! (`crate::access`). This module does the audio and video half: an H.264/AAC
//! MP4 for a video and an AAC M4A for audio, made by `ffmpeg` with
//! [`crate::tool::video_mp4_args`] or [`crate::tool::audio_m4a_args`]. The
//! image access master is the image unit's, and a target this module does not
//! run is a typed skip.
//!
//! The original is never changed. It is streamed out of the store in
//! [`crate::derivative::COPY_CHUNK`] pieces to a temporary file, which `ffmpeg`
//! opens, and the copy is written back through [`crate::store::Store`] under the
//! `access/` prefix. So an object store and a 2 GB video cost the same memory as
//! a local JPEG, and a reader who may not see the original is refused the copy
//! the same way.
//!
//! The work runs on its own thread, [`Transcode`]: a transcode lasts minutes,
//! the caller is an async worker that must keep checking for a stop request and
//! a starting import, and `ffmpeg` reports its progress on standard output while
//! it runs. The caller polls [`Transcode::try_progress`] and asks for a stop
//! with [`Transcode::request_stop`]; the command is killed within
//! [`crate::tool::TRANSCODE_POLL`] and the partial copy is removed.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::access::{AccessTarget, copy_path};
use crate::av_probe;
use crate::derivative::copy_to;
use crate::store::{Store, StorePrefix, storage_path};
use crate::tool::{self, AccessTool, ToolError, Tools, audio_m4a_args, video_mp4_args};

/// The facts of one stored original a copy is made from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscodeSource {
    pub storage_id: String,
    pub extension: Option<String>,
    pub media_type: Option<String>,
    pub has_original: bool,
}

/// What one copy's work did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscodeWork {
    /// The copy is in the store under `access/`.
    Ready {
        /// The command that made it.
        tool: &'static str,
        /// The copy's size in bytes.
        bytes: u64,
    },
    /// This copy is not this module's to make, or its command is not installed.
    Skipped {
        reason: String,
        tool: Option<&'static str>,
    },
    /// `ffmpeg` or the store refused.
    Failed {
        reason: String,
        tool: Option<&'static str>,
    },
    /// The caller asked the running copy to stop; nothing was stored.
    Stopped,
}

impl TranscodeWork {
    /// The reason a reader shows, when there is one.
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Ready { .. } | Self::Stopped => None,
            Self::Skipped { reason, .. } | Self::Failed { reason, .. } => Some(reason),
        }
    }

    /// The command the work used or needed.
    #[must_use]
    pub const fn tool(&self) -> Option<&'static str> {
        match self {
            Self::Ready { tool, .. } => Some(*tool),
            Self::Skipped { tool, .. } | Self::Failed { tool, .. } => *tool,
            Self::Stopped => Some("ffmpeg"),
        }
    }
}

/// One copy being made on its own thread.
pub struct Transcode {
    stop: Arc<AtomicBool>,
    progress: Receiver<u8>,
    worker: JoinHandle<TranscodeWork>,
}

impl Transcode {
    /// Start one copy. The thread copies the original out of the store, runs
    /// `ffmpeg` and stores the result; the caller polls it.
    ///
    /// # Errors
    /// A string when the worker thread cannot be started. Every other failure
    /// is the returned [`TranscodeWork`]'s.
    pub fn start(
        store: Arc<dyn Store>,
        tools: Tools,
        source: TranscodeSource,
        target: AccessTarget,
    ) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let (sender, progress) = std::sync::mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("romeka-access".to_string())
            .spawn(move || run(store.as_ref(), &tools, &source, target, &flag, &sender))
            .map_err(|error| format!("cannot start the copy's thread: {error}"))?;
        Ok(Self {
            stop,
            progress,
            worker,
        })
    }

    /// Ask the copy to stop. The command is killed at its next poll.
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// The last percentage `ffmpeg` reported, if it reported one since the last
    /// call.
    #[must_use]
    pub fn try_progress(&self) -> Option<u8> {
        self.progress.try_recv().ok()
    }

    /// Whether the worker thread has finished, so [`Self::finish`] returns at
    /// once.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.worker.is_finished()
    }

    /// Wait for the worker and answer what it did.
    #[must_use]
    pub fn finish(self) -> TranscodeWork {
        match self.worker.join() {
            Ok(work) => work,
            Err(_) => TranscodeWork::Failed {
                reason: "the copy's thread stopped before it answered".to_string(),
                tool: Some("ffmpeg"),
            },
        }
    }
}

/// The thread's work: copy out, transcode, store the copy.
fn run(
    store: &dyn Store,
    tools: &Tools,
    source: &TranscodeSource,
    target: AccessTarget,
    stop: &AtomicBool,
    progress: &Sender<u8>,
) -> TranscodeWork {
    let tool = Some(AccessTool::Ffmpeg.command());
    if target == AccessTarget::Jpeg {
        return TranscodeWork::Skipped {
            reason: "the image access master is not this worker's".to_string(),
            tool: None,
        };
    }
    let Some(media_type) = source.media_type.as_deref() else {
        return TranscodeWork::Failed {
            reason: "the row stored no media type".to_string(),
            tool,
        };
    };
    if !source.has_original {
        return TranscodeWork::Failed {
            reason: "the row stored no original".to_string(),
            tool,
        };
    }
    let original = storage_path(
        StorePrefix::Original,
        &source.storage_id,
        source.extension.as_deref(),
    );
    let Ok(dir) = tempfile::Builder::new().prefix("romeka-access").tempdir() else {
        return TranscodeWork::Failed {
            reason: "cannot open a temporary folder".to_string(),
            tool,
        };
    };
    // The input keeps its extension, and the output the target's, because
    // `ffmpeg` leans on both to pick a demuxer and a muxer.
    let extension = source
        .extension
        .as_deref()
        .filter(|extension| !extension.is_empty())
        .unwrap_or(target.suffix());
    let input = dir.path().join(format!("source.{extension}"));
    let output = dir.path().join(format!("copy.{}", target.suffix()));
    let mut file = match std::fs::File::create(&input) {
        Ok(file) => file,
        Err(error) => {
            return TranscodeWork::Failed {
                reason: format!("cannot open a temporary file: {error}"),
                tool,
            };
        }
    };
    if let Err(error) = copy_to(store, &original, &mut file) {
        return TranscodeWork::Failed {
            reason: format!("cannot read {original}: {error}"),
            tool,
        };
    }
    drop(file);
    let duration = av_probe::probe_with(tools, &input)
        .and_then(|extent| extent.duration_ms)
        .map(Duration::from_millis);
    let args = tool::with_progress(match target {
        AccessTarget::M4a => audio_m4a_args(&input, &output),
        _ => video_mp4_args(&input, &output),
    });
    let mut last = 0;
    let mut report = |percent: u8| {
        if percent > last {
            last = percent;
            let _ = progress.send(percent);
        }
    };
    match tools.transcode(&args, duration, stop, &mut report) {
        Ok(tool::TranscodeEnd::Stopped) => {
            remove(store, &copy_path(&source.storage_id, target));
            TranscodeWork::Stopped
        }
        Ok(tool::TranscodeEnd::Finished) => store_copy(store, &output, source, target, media_type),
        Err(ToolError::ToolMissing(name)) => TranscodeWork::Skipped {
            reason: format!("toolMissing: {name}"),
            tool: Some(name),
        },
        Err(error) => TranscodeWork::Failed {
            reason: error.to_string(),
            tool,
        },
    }
}

/// Write the command's output into the store and answer its size.
fn store_copy(
    store: &dyn Store,
    output: &Path,
    source: &TranscodeSource,
    target: AccessTarget,
    media_type: &str,
) -> TranscodeWork {
    let tool = Some(AccessTool::Ffmpeg.command());
    let path = copy_path(&source.storage_id, target);
    let Ok(file) = std::fs::File::open(output) else {
        return TranscodeWork::Failed {
            reason: format!("the {media_type} copy was not written"),
            tool,
        };
    };
    let size = match file.metadata() {
        Ok(metadata) => metadata.len(),
        Err(error) => {
            return TranscodeWork::Failed {
                reason: format!("cannot read the copy: {error}"),
                tool,
            };
        }
    };
    if size == 0 {
        return TranscodeWork::Failed {
            reason: "ffmpeg wrote an empty copy".to_string(),
            tool,
        };
    }
    let mut reader = std::io::BufReader::new(file);
    match store.put_stream(&mut reader, Some(size), &path, Some(target.media_type())) {
        Ok(bytes) => TranscodeWork::Ready {
            tool: "ffmpeg",
            bytes,
        },
        Err(error) => TranscodeWork::Failed {
            reason: format!("cannot store {path}: {error}"),
            tool,
        },
    }
}

/// Remove a copy a stopped or failed run left in the store. A missing file is
/// already the state wanted, so its answer is ignored.
fn remove(store: &dyn Store, path: &str) {
    if let Err(error) = store.delete(path) {
        tracing::warn!(%error, path, "a stopped copy could not be removed");
    }
}

/// The poster a copy makes when the original's own still did not: the copy is
/// streamed out of the store and its frame becomes the three thumbnails.
///
/// `true` when this run wrote them. A tool that cannot read the copy, or a
/// store that cannot hand it over, answers `false`: the media keeps whatever
/// preview state it had, and the copy still plays.
#[must_use]
pub fn poster_from_copy(store: &dyn Store, storage_id: &str, copy: &str) -> bool {
    let Ok(dir) = tempfile::Builder::new()
        .prefix("romeka-poster")
        .suffix(".mp4")
        .tempdir()
    else {
        return false;
    };
    let input = dir.path().join("copy.mp4");
    let Ok(mut file) = std::fs::File::create(&input) else {
        return false;
    };
    if copy_to(store, copy, &mut file).is_err() {
        return false;
    }
    drop(file);
    match crate::thumbnailer::generate_thumbnails_from_path(
        store,
        storage_id,
        &input,
        Some("video/mp4"),
    ) {
        Ok(crate::thumbnailer::ThumbnailOutcome::Generated) => true,
        Ok(_) => false,
        Err(error) => {
            tracing::warn!(%error, storage_id, "no poster from the access copy");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::path::{Path, PathBuf};

    use super::*;
    use crate::store::FileStore;

    fn corpus(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/media-kinds")
            .join(name)
    }

    fn source(name: &str, media_type: &str, extension: &str) -> TranscodeSource {
        TranscodeSource {
            storage_id: name.replace('.', "-"),
            extension: Some(extension.to_string()),
            media_type: Some(media_type.to_string()),
            has_original: true,
        }
    }

    /// Wait for the worker, polling as the async caller does, and answer what it
    /// did with the percentages it reported.
    fn poll(run: Transcode) -> (TranscodeWork, Vec<u8>) {
        let mut seen = Vec::new();
        let started = std::time::Instant::now();
        while !run.is_finished() {
            while let Some(percent) = run.try_progress() {
                seen.push(percent);
            }
            assert!(
                started.elapsed() < Duration::from_secs(60),
                "the copy did not finish"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        while let Some(percent) = run.try_progress() {
            seen.push(percent);
        }
        (run.finish(), seen)
    }

    /// A copy keeps 1080 lines on the short side: a 720p source keeps its frame,
    /// a 4K one becomes 1920x1080, and a portrait 4K one 1080x1920. (A first
    /// cut capped both sides at 1080, so a 1280x720 AVI became 1080x608.)
    #[test]
    fn a_copy_keeps_up_to_1080_lines_on_the_short_side() {
        let tools = Tools::on_path();
        let Some(ffmpeg) = tools.ffmpeg() else {
            eprintln!("skipping: no ffmpeg on PATH");
            return;
        };
        if tools.ffprobe().is_none() {
            eprintln!("skipping: no ffprobe on PATH");
            return;
        }
        for (made, kept) in [
            ((1280, 720), (1280, 720)),
            ((3840, 2160), (1920, 1080)),
            ((2160, 3840), (1080, 1920)),
        ] {
            let dir = tempfile::tempdir().expect("tempdir");
            let sample = dir.path().join("sample.avi");
            let status = std::process::Command::new(ffmpeg)
                .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
                .arg(format!(
                    "testsrc=duration=0.4:size={}x{}:rate=5",
                    made.0, made.1
                ))
                .args(["-c:v", "mjpeg"])
                .arg(&sample)
                .status()
                .expect("ffmpeg runs");
            assert!(status.success(), "the sample was made");
            let store = FileStore::new(dir.path(), "/files");
            let storage_id = "scale-sample";
            let original = storage_path(StorePrefix::Original, storage_id, Some("avi"));
            store
                .put(
                    &std::fs::read(&sample).expect("the sample"),
                    &original,
                    Some("video/x-msvideo"),
                )
                .expect("store");
            let run = Transcode::start(
                Arc::new(store),
                tools.clone(),
                TranscodeSource {
                    storage_id: storage_id.to_string(),
                    extension: Some("avi".to_string()),
                    media_type: Some("video/x-msvideo".to_string()),
                    has_original: true,
                },
                AccessTarget::Mp4,
            )
            .expect("a worker");
            let (work, _) = poll(run);
            assert!(matches!(work, TranscodeWork::Ready { .. }), "{work:?}");
            let copy = dir.path().join(copy_path(storage_id, AccessTarget::Mp4));
            let copied = av_probe::probe_with(tools, &copy).expect("the copy");
            assert_eq!(copied.size, Some(kept), "a {made:?} source");
        }
    }

    /// An MJPEG AVI becomes a playable MP4 under `access/`, at the same
    /// duration, and the trailer records the progress it reported.
    #[test]
    fn an_avi_becomes_an_mp4_and_reports_its_progress() {
        if Tools::on_path().ffmpeg().is_none() || Tools::on_path().ffprobe().is_none() {
            eprintln!("skipping: no ffmpeg or ffprobe on PATH");
            return;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileStore::new(dir.path(), "/files");
        let bytes = std::fs::read(corpus("mjpeg.avi")).expect("the fixture");
        let storage_id = "mjpeg-avi";
        let original = storage_path(StorePrefix::Original, storage_id, Some("avi"));
        store
            .put(&bytes, &original, Some("video/x-msvideo"))
            .expect("store");
        let run = Transcode::start(
            Arc::new(store),
            Tools::on_path().clone(),
            TranscodeSource {
                storage_id: storage_id.to_string(),
                extension: Some("avi".to_string()),
                media_type: Some("video/x-msvideo".to_string()),
                has_original: true,
            },
            AccessTarget::Mp4,
        )
        .expect("a worker");
        let (work, progress) = poll(run);
        match &work {
            TranscodeWork::Ready { tool, bytes } => {
                assert_eq!(*tool, "ffmpeg");
                assert!(*bytes > 1024, "the copy is a real file");
            }
            other => panic!("{other:?}"),
        }
        assert!(!progress.is_empty(), "ffmpeg's progress reached the caller");
        assert!(
            progress.iter().all(|percent| *percent < 100),
            "100 is the row settling ready: {progress:?}"
        );
        let copy = copy_path(storage_id, AccessTarget::Mp4);
        assert_eq!(copy, "access/mjpeg-avi.mp4");
        let stored = std::fs::read(dir.path().join(&copy)).expect("the copy");
        assert!(stored.len() > 1024, "the copy is a real file");
        // The duration is the source's, so a time note written against the
        // original still names a moment inside the copy.
        let original_extent = av_probe::probe(&corpus("mjpeg.avi")).expect("the original");
        let copied =
            av_probe::probe_with(Tools::on_path(), &dir.path().join(&copy)).expect("the copy");
        let original_ms = original_extent.duration_ms.expect("a duration");
        let copied_ms = copied.duration_ms.expect("a duration");
        assert!(
            copied_ms.abs_diff(original_ms) <= 100,
            "original {original_ms} ms, copy {copied_ms} ms"
        );
        assert_eq!(copied.size, Some((96, 64)), "the frame size is kept");

        // The copy's frame is the poster a media whose original yielded none
        // gets: the three thumbnails are made from it.
        assert!(
            poster_from_copy(&FileStore::new(dir.path(), "/files"), storage_id, &copy),
            "the copy makes a poster"
        );
        for kind in crate::file_render::ThumbnailType::ALL {
            let thumb = crate::thumbnailer::thumbnail_path(storage_id, kind);
            assert!(dir.path().join(&thumb).is_file(), "{thumb} was not written");
        }
    }

    /// A stop request kills the command, leaves no copy in the store, and
    /// answers `Stopped` so the caller can put the row back in the queue.
    #[test]
    fn a_stop_request_ends_the_copy_with_nothing_stored() {
        if Tools::on_path().ffmpeg().is_none() {
            eprintln!("skipping: no ffmpeg on PATH");
            return;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileStore::new(dir.path(), "/files");
        let bytes = std::fs::read(corpus("video.ogv")).expect("the fixture");
        let storage_id = "stopped";
        store
            .put(
                &bytes,
                &storage_path(StorePrefix::Original, storage_id, Some("ogv")),
                Some("video/ogg"),
            )
            .expect("store");
        let run = Transcode::start(
            Arc::new(store),
            Tools::on_path().clone(),
            TranscodeSource {
                storage_id: storage_id.to_string(),
                extension: Some("ogv".to_string()),
                media_type: Some("video/ogg".to_string()),
                has_original: true,
            },
            AccessTarget::Mp4,
        )
        .expect("a worker");
        run.request_stop();
        let work = run.finish();
        assert_eq!(work, TranscodeWork::Stopped);
        assert!(
            !dir.path()
                .join(copy_path(storage_id, AccessTarget::Mp4))
                .exists(),
            "nothing is stored"
        );
    }

    /// A row whose type this worker does not copy is a typed skip, and the store
    /// is never touched.
    #[test]
    fn an_image_master_is_not_this_workers_copy() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store: Arc<dyn Store> = Arc::new(FileStore::new(dir.path(), "/files"));
        let run = Transcode::start(
            store,
            Tools::none(),
            source("image.tif", "image/tiff", "tif"),
            AccessTarget::Jpeg,
        )
        .expect("a worker");
        let work = run.finish();
        assert!(matches!(work, TranscodeWork::Skipped { .. }), "{work:?}");
        assert!(work.tool().is_none(), "{work:?}");
    }

    /// Without `ffmpeg` the copy is skipped with the reason D2's table uses, not
    /// failed: the install still stores and plays what it can.
    #[test]
    fn a_missing_ffmpeg_is_a_typed_skip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileStore::new(dir.path(), "/files");
        let bytes = std::fs::read(corpus("wma.wma")).expect("the fixture");
        let storage_id = "no-fmpeg";
        store
            .put(
                &bytes,
                &storage_path(StorePrefix::Original, storage_id, Some("wma")),
                Some("audio/x-ms-wma"),
            )
            .expect("store");
        let run = Transcode::start(
            Arc::new(store),
            Tools::none(),
            TranscodeSource {
                storage_id: storage_id.to_string(),
                extension: Some("wma".to_string()),
                media_type: Some("audio/x-ms-wma".to_string()),
                has_original: true,
            },
            AccessTarget::M4a,
        )
        .expect("a worker");
        let work = run.finish();
        assert_eq!(work.reason(), Some("toolMissing: ffmpeg"), "{work:?}");
        assert_eq!(work.tool(), Some("ffmpeg"));
    }
}
