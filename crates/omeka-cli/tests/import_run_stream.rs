//! A file over 64 MB reaches the store as a stream (`docs/import-v2.md`,
//! "Evidence", "Bytes"): the run's peak resident memory grows by far less
//! than the file.
//!
//! The check reads the process's peak resident memory, so this binary holds
//! this one test and nothing else.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod import_site;

use std::io::Write;

use import_site::site;
use omeka_db::rows::ImportStatus;
use serde_json::json;
use sha2::{Digest, Sha256};

const LEN: usize = 66 * 1024 * 1024;
const MAX_GROWTH: u64 = 24 * 1024 * 1024;

/// `VmHWM`, the process's peak resident set, in bytes.
fn peak_resident_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmHWM:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_over_64_mb_streams_into_the_store() {
    let site = site().await;
    let path = site.archive.join("scans/BIG_001.tif");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut file = std::fs::File::create(&path).unwrap();
    let mut hasher = Sha256::new();
    let mut chunk = vec![0u8; 1024 * 1024];
    chunk[..8].copy_from_slice(b"II*\0\x08\0\0\0");
    for n in 0..LEN / chunk.len() {
        chunk[8] = n as u8;
        file.write_all(&chunk).unwrap();
        hasher.update(&chunk);
        chunk[..8].fill(7);
    }
    drop(file);
    let digest: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let id = site
        .draft(|plan| {
            plan["grouping"]["pattern"] = json!(r"^scans/(?<identifier>BIG_\d+)\.tif$");
        })
        .await;
    let before = peak_resident_bytes();

    let row = site.run(id).await;

    let after = peak_resident_bytes();
    assert_eq!(row.status, ImportStatus::Done, "{:?}", row.summary);
    let media = site.media().await;
    assert_eq!(media.len(), 1);
    assert_eq!(media[0].size, Some(LEN as i64));
    assert_eq!(media[0].sha256.as_deref(), Some(digest.as_str()));
    assert_eq!(media[0].media_type.as_deref(), Some("image/tiff"));
    assert!(
        !media[0].has_thumbnails,
        "no thumbnail from a file over 64 MB"
    );
    let original = site
        .stored_files()
        .into_iter()
        .find(|path| path.starts_with("original/"))
        .unwrap();
    assert_eq!(
        std::fs::metadata(site.files.join(original)).unwrap().len(),
        LEN as u64
    );
    if let (Some(before), Some(after)) = (before, after) {
        let growth = after.saturating_sub(before);
        assert!(
            growth <= MAX_GROWTH,
            "peak resident memory grew by {growth} bytes for a {LEN} byte file"
        );
    }
}
