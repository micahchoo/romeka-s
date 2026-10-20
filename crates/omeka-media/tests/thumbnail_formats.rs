//! Thumbnails of archive masters: JPEG 2000 in memory, and TIFF and JPEG
//! originals read through [`ImageThumbnailer::derivatives_from_reader`],
//! which never holds them whole.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Cursor;

use omeka_media::ThumbnailType;
use omeka_media::thumbnailer::{ImageThumbnailer, ThumbnailSkip};

const JP2: &[u8] = include_bytes!("fixtures/thumbnail/gradient-96x64.jp2");

fn decoded(derivatives: &[(ThumbnailType, Vec<u8>)], kind: ThumbnailType) -> image::RgbImage {
    let bytes = &derivatives
        .iter()
        .find(|(k, _)| *k == kind)
        .expect("kind")
        .1;
    image::load_from_memory(bytes).expect("a JPEG").to_rgb8()
}

fn near(pixel: image::Rgb<u8>, expected: [u8; 3]) -> bool {
    pixel
        .0
        .iter()
        .zip(expected)
        .all(|(got, want)| got.abs_diff(want) <= 40)
}

/// How the hand-written TIFF holds its pixels.
#[derive(Clone, Copy)]
enum Chunks {
    Strips(u32),
    Tiles(u32),
}

/// An uncompressed TIFF, little-endian, written field by field so strips,
/// tiles, 1-bit and 16-bit samples can all be made. `pixel` gives each
/// pixel's samples (one byte each, or the bit for 1-bit).
fn tiff(
    width: u32,
    height: u32,
    bits: u16,
    samples: u16,
    photometric: u16,
    chunks: Chunks,
    pixel: impl Fn(u32, u32) -> Vec<u16>,
) -> Vec<u8> {
    let (chunk_width, chunk_height) = match chunks {
        Chunks::Strips(rows) => (width, rows),
        Chunks::Tiles(side) => (side, side),
    };
    let across = width.div_ceil(chunk_width);
    let down = height.div_ceil(chunk_height);
    let mut data = Vec::new();
    let mut offsets = Vec::new();
    let mut counts = Vec::new();
    for chunk_row in 0..down {
        for chunk_col in 0..across {
            let start = data.len();
            let rows = match chunks {
                Chunks::Strips(_) => chunk_height.min(height - chunk_row * chunk_height),
                Chunks::Tiles(_) => chunk_height,
            };
            for row in 0..rows {
                let y = chunk_row * chunk_height + row;
                let mut bitbuf = vec![0_u8; chunk_width.div_ceil(8) as usize];
                for col in 0..chunk_width {
                    let x = chunk_col * chunk_width + col;
                    let values = if x < width && y < height {
                        pixel(x, y)
                    } else {
                        vec![0; samples as usize]
                    };
                    for value in values {
                        match bits {
                            1 if value != 0 => bitbuf[col as usize / 8] |= 0x80 >> (col % 8),
                            1 => {}
                            8 => data.push(value as u8),
                            _ => data.extend(value.to_le_bytes()),
                        }
                    }
                }
                if bits == 1 {
                    data.extend(bitbuf);
                }
            }
            offsets.push(start);
            counts.push(data.len() - start);
        }
    }
    let n = offsets.len();
    let mut entries: Vec<(u16, u16, Vec<u32>)> = vec![
        (256, 4, vec![width]),
        (257, 4, vec![height]),
        (258, 3, vec![u32::from(bits); samples as usize]),
        (259, 3, vec![1]),
        (262, 3, vec![u32::from(photometric)]),
        (277, 3, vec![u32::from(samples)]),
        (284, 3, vec![1]),
    ];
    let (offset_tag, count_tag) = match chunks {
        Chunks::Strips(rows) => {
            entries.push((278, 4, vec![rows]));
            (273, 279)
        }
        Chunks::Tiles(side) => {
            entries.push((322, 4, vec![side]));
            entries.push((323, 4, vec![side]));
            (324, 325)
        }
    };
    let ifd_at = 8_usize;
    let ifd_len = 2 + 12 * (entries.len() + 2) + 4;
    let mut extra = Vec::new();
    let extra_at = ifd_at + ifd_len;
    let data_at = extra_at + 8 * n + 16;
    entries.push((
        offset_tag,
        4,
        offsets.iter().map(|o| (o + data_at) as u32).collect(),
    ));
    entries.push((count_tag, 4, counts.iter().map(|c| *c as u32).collect()));
    entries.sort_by_key(|entry| entry.0);
    let mut out = b"II*\0".to_vec();
    out.extend((ifd_at as u32).to_le_bytes());
    out.extend((entries.len() as u16).to_le_bytes());
    for (tag, kind, values) in &entries {
        out.extend(tag.to_le_bytes());
        out.extend(kind.to_le_bytes());
        out.extend((values.len() as u32).to_le_bytes());
        let width = if *kind == 3 { 2 } else { 4 };
        let bytes: Vec<u8> = values
            .iter()
            .flat_map(|v| {
                if width == 2 {
                    (*v as u16).to_le_bytes().to_vec()
                } else {
                    v.to_le_bytes().to_vec()
                }
            })
            .collect();
        if bytes.len() <= 4 {
            let mut field = bytes.clone();
            field.resize(4, 0);
            out.extend(field);
        } else {
            out.extend(((extra_at + extra.len()) as u32).to_le_bytes());
            extra.extend(bytes);
        }
    }
    out.extend(0_u32.to_le_bytes());
    assert_eq!(out.len(), extra_at);
    extra.resize(data_at - extra_at, 0);
    out.extend(extra);
    out.extend(data);
    out
}

/// Red, green, blue and white quarters.
fn quarter(width: u32, height: u32) -> impl Fn(u32, u32) -> [u8; 3] {
    move |x, y| match (x < width / 2, y < height / 2) {
        (true, true) => [220, 20, 20],
        (false, true) => [20, 200, 20],
        (true, false) => [20, 20, 220],
        (false, false) => [250, 250, 250],
    }
}

fn assert_quarters(large: &image::RgbImage) {
    let (width, height) = large.dimensions();
    let at = |x: u32, y: u32| *large.get_pixel(x, y);
    assert!(
        near(at(width / 4, height / 4), [220, 20, 20]),
        "{:?}",
        at(width / 4, height / 4)
    );
    assert!(near(at(3 * width / 4, height / 4), [20, 200, 20]));
    assert!(near(at(width / 4, 3 * height / 4), [20, 20, 220]));
    assert!(near(at(3 * width / 4, 3 * height / 4), [250, 250, 250]));
}

#[test]
fn a_jp2_master_gets_three_thumbnails() {
    let derivatives = ImageThumbnailer
        .derivative_bytes(JP2, Some("image/jp2"))
        .expect("jp2");
    assert_eq!(
        decoded(&derivatives, ThumbnailType::Large).dimensions(),
        (96, 64)
    );
    assert_eq!(
        decoded(&derivatives, ThumbnailType::Square).dimensions(),
        (200, 200)
    );
    let large = decoded(&derivatives, ThumbnailType::Large);
    assert!(
        near(*large.get_pixel(48, 2), [0x20, 0x60, 0xa0]),
        "the gradient's top"
    );
    assert!(
        near(*large.get_pixel(48, 61), [0xe0, 0xc0, 0x40]),
        "the gradient's foot"
    );
    assert!(matches!(
        ImageThumbnailer.derivative_bytes(b"\0\0\0\x0CjP  broken", Some("image/jp2")),
        Err(ThumbnailSkip::Decode(_))
    ));
}

#[test]
fn a_striped_and_a_tiled_tiff_read_by_chunk_keep_their_quarters() {
    let (width, height) = (2100, 1700);
    let colour = quarter(width, height);
    let rgb = |x, y| colour(x, y).map(u16::from).to_vec();
    for chunks in [Chunks::Strips(7), Chunks::Tiles(256)] {
        let bytes = tiff(width, height, 8, 3, 2, chunks, rgb);
        let derivatives = ImageThumbnailer
            .derivatives_from_reader(Cursor::new(bytes), Some("image/tiff"))
            .expect("tiff");
        let large = decoded(&derivatives, ThumbnailType::Large);
        assert_eq!(large.dimensions(), (800, 648));
        assert_quarters(&large);
        assert_eq!(
            decoded(&derivatives, ThumbnailType::Medium).dimensions(),
            (400, 324)
        );
        assert_eq!(
            decoded(&derivatives, ThumbnailType::Square).dimensions(),
            (200, 200)
        );
    }
}

#[test]
fn a_16_bit_a_grey_and_a_1_bit_tiff_read_by_chunk() {
    let colour = quarter(64, 48);
    let deep = tiff(64, 48, 16, 3, 2, Chunks::Strips(5), |x, y| {
        colour(x, y).map(|v| u16::from(v) * 257).to_vec()
    });
    let derivatives = ImageThumbnailer
        .derivatives_from_reader(Cursor::new(deep), Some("image/tiff"))
        .expect("16-bit");
    assert_quarters(&decoded(&derivatives, ThumbnailType::Large));

    let grey = tiff(64, 48, 8, 1, 1, Chunks::Strips(48), |x, _| {
        vec![if x < 32 { 10 } else { 240 }]
    });
    let large = decoded(
        &ImageThumbnailer
            .derivatives_from_reader(Cursor::new(grey), Some("image/tiff"))
            .expect("grey"),
        ThumbnailType::Large,
    );
    assert!(near(*large.get_pixel(8, 24), [10, 10, 10]));
    assert!(near(*large.get_pixel(56, 24), [240, 240, 240]));

    let bilevel = tiff(64, 48, 1, 1, 1, Chunks::Strips(16), |x, _| {
        vec![u16::from(x >= 32)]
    });
    let large = decoded(
        &ImageThumbnailer
            .derivatives_from_reader(Cursor::new(bilevel), Some("image/tiff"))
            .expect("1-bit"),
        ThumbnailType::Large,
    );
    assert!(near(*large.get_pixel(8, 24), [0, 0, 0]), "black is zero");
    assert!(near(*large.get_pixel(56, 24), [255, 255, 255]));
}

#[test]
fn a_jpeg_read_by_reader_is_decoded_at_a_reduced_scale() {
    let (width, height) = (3400, 2000);
    let colour = quarter(width, height);
    let pixels = image::RgbImage::from_fn(width, height, |x, y| image::Rgb(colour(x, y)));
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgb8(pixels)
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Jpeg)
        .unwrap();
    let derivatives = ImageThumbnailer
        .derivatives_from_reader(Cursor::new(bytes), Some("image/jpeg"))
        .expect("jpeg");
    let large = decoded(&derivatives, ThumbnailType::Large);
    assert_eq!(large.dimensions(), (800, 471));
    assert_quarters(&large);
}

#[test]
fn the_reader_path_takes_only_tiff_and_jpeg() {
    assert_eq!(
        ImageThumbnailer.derivatives_from_reader(Cursor::new(JP2), Some("image/jp2")),
        Err(ThumbnailSkip::UnsupportedType("image/jp2".to_string()))
    );
    assert!(matches!(
        ImageThumbnailer.derivatives_from_reader(Cursor::new(b"II*\0junk"), Some("image/tiff")),
        Err(ThumbnailSkip::Decode(_))
    ));
}
