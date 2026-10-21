//! The backfill streams an original that an external tool must open, so a large
//! video is never held in memory on its way to a poster.
//!
//! The check reads the process's peak resident memory, so this binary holds this
//! one test and nothing else.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Write as _;

use omeka_cli::commands::install::{InstallOptions, run as install};
use omeka_cli::commands::thumbnails::{ThumbnailOptions, ThumbnailStatus, run};
use omeka_db::repos::{ItemRepo, MediaRepo};
use omeka_db::rows::{NewItem, NewMedia, NewResource};

/// A sparse original this size: the holes cost no disk and read as zeros.
const VIDEO_BYTES: u64 = 256 * 1024 * 1024;

/// The read of the whole original would add 256 MB; the chunked copy adds 8 MB.
const MAX_GROWTH: u64 = 96 * 1024 * 1024;

fn peak_resident_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmHWM:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

/// A file that starts like an mp4 and is 256 MB of holes after it. The row's
/// media type is what decides the path, so the bytes only have to make `ffmpeg`
/// try and fail.
fn write_large_video(path: &std::path::Path) {
    let mut file = std::fs::File::create(path).unwrap();
    file.set_len(VIDEO_BYTES).unwrap();
    file.write_all(&[
        0u8, 0, 0, 0x18, b'f', b't', b'y', b'p', b'm', b'p', b'4', b'2',
    ])
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_large_video_is_streamed_not_read_whole() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("romeka.db");
    install(&InstallOptions {
        database: database.clone().into(),
        email: "admin@example.org".to_string(),
        name: "Admin".to_string(),
        password: "a long test password".to_string(),
        title: "Test".to_string(),
        time_zone: "UTC".to_string(),
        locale: "en".to_string(),
    })
    .await
    .expect("install");
    let pool = omeka_db::open_file(&database).await.expect("pool");
    let files = dir.path().join("files");
    std::fs::create_dir_all(files.join("original")).unwrap();
    write_large_video(&files.join("original/big.mp4"));
    let item = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .expect("item");
    MediaRepo::new(&pool)
        .create(&NewMedia {
            resource: NewResource::default(),
            item_id: item.id(),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            data: None,
            source: Some("big.mp4".to_string()),
            media_type: Some("video/mp4".to_string()),
            storage_id: Some("big".to_string()),
            extension: Some("mp4".to_string()),
            sha256: Some(String::new()),
            size: Some(VIDEO_BYTES as i64),
            has_original: true,
            has_thumbnails: false,
            position: None,
            lang: None,
            alt_text: None,
        })
        .await
        .expect("media row");

    let before = peak_resident_bytes();
    let report = run(&ThumbnailOptions {
        database: database.into(),
        store: Some(files),
        force: false,
    })
    .await
    .expect("backfill");
    let after = peak_resident_bytes();

    // The bytes are not a real video, so `ffmpeg` reads the whole streamed file
    // and refuses it. That is a failure with the command's reason, not a skip:
    // a missing tool is the skip (`make_preview` tells the two apart).
    assert_eq!(report.failed, 1, "{report:?}");
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ThumbnailStatus::Failed);
    let message = report.results[0].message.as_deref().unwrap_or_default();
    assert!(message.contains("ffmpeg"), "{message}");
    if let (Some(before), Some(after)) = (before, after) {
        let growth = after.saturating_sub(before);
        eprintln!("peak resident memory grew by {growth} bytes for a {VIDEO_BYTES} byte video");
        assert!(growth <= MAX_GROWTH, "grew by {growth} bytes");
    }
}
