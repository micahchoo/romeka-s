//! Micro-benches for the thumbnailer: decode, resize, encode, and the store path.
//!
//! Covers `crates/omeka-media/src/thumbnailer.rs` (`File/Thumbnailer/Gd.php`).
//! `derivative_bytes` is the whole transform; `decode_only` times the same
//! source with the `image` crate alone, so the decode share of the transform is
//! visible. `generate_thumbnails` adds the three file writes.
//!
//! Run with `cargo bench -p omeka-media --bench thumbnail`. The numbers, their
//! limiters and the checks that did not apply are in `crates/omeka-media/README.md`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use image::imageops::FilterType;
use omeka_media::file_render::ThumbnailType;
use omeka_media::store::{FileStore, Thumbnailer};
use omeka_media::thumbnailer::{
    ImageThumbnailer, ThumbnailOutcome, ThumbnailSkip, generate_thumbnails, thumbnail_path,
};

/// A per-pixel gradient, so the JPEG encoder has real work to do.
fn gradient(width: u32, height: u32) -> image::RgbImage {
    image::RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
    })
}

fn png_bytes(image: image::RgbImage) -> Vec<u8> {
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgb8(image)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}

fn jpeg_bytes(image: &image::RgbImage, quality: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality)
        .encode_image(image)
        .unwrap();
    bytes
}

fn thumbnail(c: &mut Criterion) {
    let small = png_bytes(gradient(100, 60));
    let medium = png_bytes(gradient(1000, 500));
    let large_source = gradient(4000, 3000);
    let large = jpeg_bytes(&large_source, 85);
    let directory = tempfile::Builder::new()
        .prefix("thumbnail-")
        .tempdir_in(support::scratch_dir())
        .unwrap();
    let store = FileStore::new(directory.path(), "/files");
    let serial = AtomicUsize::new(0);

    // Run once, outside the timed region: a case that stopped decoding or
    // encoding would otherwise be timed as a skip.
    assert_eq!(
        ImageThumbnailer
            .derivative_bytes(&medium, Some("image/png"))
            .unwrap()
            .len(),
        ThumbnailType::ALL.len()
    );
    assert!(matches!(
        ImageThumbnailer.derivative_bytes(&medium, Some("application/pdf")),
        Err(ThumbnailSkip::UnsupportedType(_))
    ));
    assert!(image::load_from_memory(&large).is_ok());

    let mut group = c.benchmark_group("media/thumbnail");
    group
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(30);

    for (name, bytes, media_type) in [
        ("png_100x60", &small, "image/png"),
        ("png_1000x500", &medium, "image/png"),
        ("jpeg_4000x3000", &large, "image/jpeg"),
    ] {
        group.bench_function(format!("derivative_bytes/{name}"), |b| {
            b.iter(|| {
                ImageThumbnailer.derivative_bytes(black_box(bytes), black_box(Some(media_type)))
            })
        });
    }
    group.bench_function("derivative_bytes/unsupported_type", |b| {
        b.iter(|| {
            ImageThumbnailer
                .derivative_bytes(black_box(&medium), black_box(Some("application/pdf")))
        })
    });

    for (name, bytes) in [("png_1000x500", &medium), ("jpeg_4000x3000", &large)] {
        group.bench_function(format!("decode_only/{name}"), |b| {
            b.iter(|| image::load_from_memory(black_box(bytes)))
        });
    }

    // The two stages a derivative is made of, measured on their own so the
    // transform's split between decode, resize and encode is arithmetic rather
    // than a guess. `resize_exact` and `crop_imm` are the calls
    // `thumbnailer::scale_to_fit` and `thumbnailer::square` make.
    let decoded = image::load_from_memory(&large).unwrap();
    let square_source = decoded.crop_imm(500, 0, 3000, 3000);
    let eight_hundred = decoded.resize_exact(800, 600, FilterType::Triangle);
    group.bench_function("resize_only/large_to_800x600", |b| {
        b.iter(|| decoded.resize_exact(800, 600, FilterType::Triangle))
    });
    group.bench_function("resize_only/medium_to_400x300", |b| {
        b.iter(|| decoded.resize_exact(400, 300, FilterType::Triangle))
    });
    group.bench_function("resize_only/square_to_200x200", |b| {
        b.iter(|| square_source.resize_exact(200, 200, FilterType::Triangle))
    });
    group.bench_function("jpeg_encode_only/800x600", |b| {
        b.iter(|| {
            let mut bytes = Vec::new();
            image::codecs::jpeg::JpegEncoder::new(&mut bytes)
                .encode_image(black_box(&eight_hundred))
                .unwrap();
            black_box(bytes)
        })
    });

    group.bench_function("ingest_seam/thumbnails_png_1000x500", |b| {
        b.iter(|| ImageThumbnailer.thumbnails(black_box(&medium), black_box(Some("image/png"))))
    });

    group.bench_function("thumbnail_path/square", |b| {
        b.iter(|| thumbnail_path(black_box("abc123"), black_box(ThumbnailType::Square)))
    });

    group.bench_function("generate_thumbnails/fresh_store", |b| {
        b.iter_batched(
            || {
                format!(
                    "{}-{}",
                    std::process::id(),
                    serial.fetch_add(1, Ordering::Relaxed)
                )
            },
            |storage_id| {
                let outcome =
                    generate_thumbnails(&store, &storage_id, black_box(&medium), Some("image/png"));
                black_box(outcome)
            },
            BatchSize::SmallInput,
        )
    });
    let existing = format!("{}-existing", std::process::id());
    let outcome = generate_thumbnails(&store, &existing, &medium, Some("image/png")).unwrap();
    assert!(
        matches!(outcome, ThumbnailOutcome::Generated),
        "the guard fixture must generate: {outcome:?}"
    );
    group.bench_function("generate_thumbnails/existing", |b| {
        b.iter(|| {
            generate_thumbnails(
                &store,
                black_box(&existing),
                black_box(&medium),
                Some("image/png"),
            )
        })
    });
    group.bench_function("generate_thumbnails/unsupported", |b| {
        b.iter_batched(
            || {
                format!(
                    "{}-{}",
                    std::process::id(),
                    serial.fetch_add(1, Ordering::Relaxed)
                )
            },
            |storage_id| {
                let outcome = generate_thumbnails(
                    &store,
                    &storage_id,
                    black_box(&medium),
                    Some("application/pdf"),
                );
                black_box(outcome)
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

criterion_group!(benches, thumbnail);
criterion_main!(benches);
