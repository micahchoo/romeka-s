//! A TIFF master over 64 MB is streamed into the store and still gets its
//! three thumbnails, made by a bounded read (`docs/import-v2.md`, "Run
//! details").
//!
//! The check reads the process's peak resident memory, so this binary holds
//! this one test and nothing else.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod import_site;

use std::io::{BufWriter, Write};

use import_site::site;
use omeka_db::rows::ImportStatus;
use serde_json::json;

/// 4900 x 4900 RGB: 72,030,000 bytes of pixels.
const SIDE: u32 = 4900;
const ROWS_PER_STRIP: u32 = 16;
const MAX_GROWTH: u64 = 64 * 1024 * 1024;

fn peak_resident_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmHWM:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

/// An uncompressed little-endian RGB TIFF, pixels first and the directory
/// after them, written row by row.
fn write_tiff(path: &std::path::Path) {
    let row_bytes = SIDE * 3;
    let strips = SIDE.div_ceil(ROWS_PER_STRIP);
    let data = u64::from(row_bytes) * u64::from(SIDE);
    let ifd_at = 8 + data;
    let entries = 10_u32;
    let arrays_at = ifd_at + 2 + 12 * u64::from(entries) + 4;
    let counts_at = arrays_at + 4 * u64::from(strips);
    let bits_at = counts_at + 4 * u64::from(strips);
    let mut out = BufWriter::new(std::fs::File::create(path).unwrap());
    out.write_all(b"II*\0").unwrap();
    out.write_all(&u32::try_from(ifd_at).unwrap().to_le_bytes())
        .unwrap();
    for y in 0..SIDE {
        let row: Vec<u8> = (0..SIDE)
            .flat_map(|x| [(x / 20) as u8, (y / 20) as u8, 128])
            .collect();
        out.write_all(&row).unwrap();
    }
    let entry =
        |out: &mut BufWriter<std::fs::File>, tag: u16, kind: u16, count: u32, value: u32| {
            out.write_all(&tag.to_le_bytes()).unwrap();
            out.write_all(&kind.to_le_bytes()).unwrap();
            out.write_all(&count.to_le_bytes()).unwrap();
            out.write_all(&value.to_le_bytes()).unwrap();
        };
    let at = |offset: u64| u32::try_from(offset).unwrap();
    out.write_all(&u16::try_from(entries).unwrap().to_le_bytes())
        .unwrap();
    entry(&mut out, 256, 4, 1, SIDE);
    entry(&mut out, 257, 4, 1, SIDE);
    entry(&mut out, 258, 3, 3, at(bits_at));
    entry(&mut out, 259, 3, 1, 1);
    entry(&mut out, 262, 3, 1, 2);
    entry(&mut out, 273, 4, strips, at(arrays_at));
    entry(&mut out, 277, 3, 1, 3);
    entry(&mut out, 278, 4, 1, ROWS_PER_STRIP);
    entry(&mut out, 279, 4, strips, at(counts_at));
    entry(&mut out, 284, 3, 1, 1);
    out.write_all(&0_u32.to_le_bytes()).unwrap();
    for strip in 0..strips {
        let offset = 8 + u64::from(strip * ROWS_PER_STRIP) * u64::from(row_bytes);
        out.write_all(&at(offset).to_le_bytes()).unwrap();
    }
    for strip in 0..strips {
        let rows = ROWS_PER_STRIP.min(SIDE - strip * ROWS_PER_STRIP);
        out.write_all(&(rows * row_bytes).to_le_bytes()).unwrap();
    }
    for _ in 0..3 {
        out.write_all(&8_u16.to_le_bytes()).unwrap();
    }
    out.flush().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tiff_over_64_mb_is_streamed_and_gets_its_thumbnails() {
    let site = site().await;
    let path = site.archive.join("scans/BIG_002.tif");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    write_tiff(&path);
    let size = std::fs::metadata(&path).unwrap().len();
    assert!(size > 64 * 1024 * 1024, "{size}");
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
    assert!(media[0].has_thumbnails, "the bounded read made them");
    let files = site.stored_files();
    for prefix in ["large/", "medium/", "square/"] {
        let thumbnail = files
            .iter()
            .find(|path| path.starts_with(prefix))
            .expect(prefix);
        let bytes = std::fs::read(site.files.join(thumbnail)).unwrap();
        let tags = omeka_media::image_metadata(&mut std::io::Cursor::new(bytes));
        let side = match prefix {
            "large/" => "800",
            "medium/" => "400",
            _ => "200",
        };
        assert_eq!(tags["Width"].values, [side], "{prefix}");
        assert_eq!(tags["Height"].values, [side], "{prefix}");
    }
    if let (Some(before), Some(after)) = (before, after) {
        let growth = after.saturating_sub(before);
        eprintln!("peak resident memory grew by {growth} bytes for a {size} byte TIFF");
        assert!(growth <= MAX_GROWTH, "grew by {growth} bytes");
    }
}
