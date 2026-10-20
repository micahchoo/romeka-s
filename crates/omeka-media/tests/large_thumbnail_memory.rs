//! A 70 MB TIFF master is thumbnailed with bounded memory.
//!
//! The check reads the process's peak resident memory, so this binary holds
//! this one test and nothing else. The TIFF is written strip by strip, so
//! making it does not raise the peak either.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{BufReader, BufWriter};

use omeka_media::ThumbnailType;
use omeka_media::thumbnailer::ImageThumbnailer;
use tiff::encoder::{TiffEncoder, colortype};

/// 4900 x 4900 RGB is 72,030,000 bytes of pixels: over the 64 MB a reader
/// may hold whole.
const SIDE: u32 = 4900;
/// The reduced copy's sums (1600 x 1600 x 3 x 4 bytes, 30.7 MB), the copy
/// itself (7.7 MB), one strip, and the resize's working image.
const MAX_GROWTH: u64 = 48 * 1024 * 1024;

fn peak_resident_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmHWM:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

#[test]
fn a_70_mb_tiff_is_thumbnailed_with_bounded_memory() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("master.tif");
    {
        let file = BufWriter::new(std::fs::File::create(&path).unwrap());
        let mut tiff = TiffEncoder::new(file).unwrap();
        let mut image = tiff.new_image::<colortype::RGB8>(SIDE, SIDE).unwrap();
        image.rows_per_strip(16).unwrap();
        let mut y = 0;
        while image.next_strip_sample_count() > 0 {
            let rows = (image.next_strip_sample_count() / u64::from(SIDE * 3)) as u32;
            let strip: Vec<u8> = (y..y + rows)
                .flat_map(|row| {
                    (0..SIDE).flat_map(move |x| [(x / 20) as u8, (row / 20) as u8, 128])
                })
                .collect();
            image.write_strip(&strip).unwrap();
            y += rows;
        }
        image.finish().unwrap();
    }
    let size = std::fs::metadata(&path).unwrap().len();
    assert!(size > 70_000_000, "{size} bytes");

    let before = peak_resident_bytes();
    let file = BufReader::new(std::fs::File::open(&path).unwrap());
    let derivatives = ImageThumbnailer
        .derivatives_from_reader(file, Some("image/tiff"))
        .expect("thumbnails");
    let after = peak_resident_bytes();

    let large = &derivatives
        .iter()
        .find(|(kind, _)| *kind == ThumbnailType::Large)
        .unwrap()
        .1;
    let large = image::load_from_memory(large).unwrap();
    assert_eq!((large.width(), large.height()), (800, 800));
    if let (Some(before), Some(after)) = (before, after) {
        let growth = after.saturating_sub(before);
        eprintln!("peak resident memory grew by {growth} bytes for a {size} byte TIFF");
        assert!(growth <= MAX_GROWTH, "grew by {growth} bytes");
    }
}
