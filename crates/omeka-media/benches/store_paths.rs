//! Micro-benches for the file store path rules and the small store operations.
//!
//! Covers `crates/omeka-media/src/store/`: the storage path rule from
//! `TempFile::store`, the URL and local path resolution, the extension rule,
//! the storage id, the checksum, and the four filesystem calls against a
//! temporary root.
//!
//! Run with `cargo bench -p omeka-media --bench store_paths`. The numbers, their
//! limiters and the checks that did not apply are in `crates/omeka-media/README.md`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use omeka_media::file_render::ThumbnailType;
use omeka_media::store::{
    Access, FileStore, Store, StorePrefix, extension_from_source_name, new_storage_id, sha256_hex,
    storage_path,
};

/// How many paths the write benches overwrite in turn.
///
/// An ingest writes a fresh path every time, but a bench that did that would
/// create a file per iteration and fill the disk before the run ended. Cycling a
/// window of paths keeps the syscall cost of one write identical and bounds the
/// tree at 64 files.
const PATH_WINDOW: usize = 64;

/// A 1 KiB payload, the size of a small uploaded file.
fn kib(len: usize) -> Vec<u8> {
    (0..len).map(|index| (index % 251) as u8).collect()
}

fn store_paths(c: &mut Criterion) {
    let directory = tempfile::Builder::new()
        .prefix("store-")
        .tempdir_in(support::scratch_dir())
        .unwrap();
    let store = FileStore::new(directory.path(), "/files");
    let payload = kib(1024);
    let megabyte = kib(1024 * 1024);
    let serial = AtomicUsize::new(0);

    // Run once, outside the timed region: a case that started failing would
    // otherwise be timed as an error path.
    assert_eq!(
        storage_path(StorePrefix::Original, "abc", Some("jpg")),
        "original/abc.jpg"
    );
    assert_eq!(
        storage_path(StorePrefix::Thumbnail(ThumbnailType::Square), "abc", None),
        "square/abc"
    );
    assert_eq!(
        store.url("original/abc.jpg", Access::Public),
        "/files/original/abc.jpg"
    );
    assert!(store.local_path("original/../etc/passwd").is_err());
    assert_eq!(
        extension_from_source_name("Photo.JPG").as_deref(),
        Some("jpg")
    );
    assert_eq!(extension_from_source_name("no_extension"), None);
    assert_eq!(new_storage_id().unwrap().len(), 40);
    assert_eq!(sha256_hex(b"abc").len(), 64);
    assert!(store.put(&payload, "original/guard.jpg").is_ok());
    assert_eq!(store.read("original/guard.jpg").unwrap(), payload);
    assert!(store.exists("original/guard.jpg").unwrap());
    assert!(!store.exists("original/absent.jpg").unwrap());
    assert!(store.delete("original/guard.jpg").unwrap());

    let mut group = c.benchmark_group("media/store_paths");
    group
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(50);

    group.bench_function("storage_path/original_with_extension", |b| {
        b.iter(|| {
            storage_path(
                black_box(StorePrefix::Original),
                black_box("a1b2c3"),
                black_box(Some("jpg")),
            )
        })
    });
    group.bench_function("storage_path/thumbnail_without_extension", |b| {
        b.iter(|| {
            storage_path(
                black_box(StorePrefix::Thumbnail(ThumbnailType::Square)),
                black_box("a1b2c3"),
                black_box(None),
            )
        })
    });
    group.bench_function("url", |b| {
        b.iter(|| store.url(black_box("original/a1b2c3.jpg"), Access::Public))
    });
    group.bench_function("local_path/ok", |b| {
        b.iter(|| store.local_path(black_box("original/a1b2c3.jpg")))
    });
    group.bench_function("local_path/parent_component", |b| {
        b.iter(|| store.local_path(black_box("original/../etc/passwd")))
    });

    for (name, source_name) in [
        ("upper_case", "Photo.JPG"),
        ("no_dot", "no_extension"),
        ("two_dots", "archive.tar.gz"),
        ("long_extension", "file.abcdefghijklm"),
        ("trailing_dot", "file."),
    ] {
        group.bench_function(format!("extension_from_source_name/{name}"), |b| {
            b.iter(|| extension_from_source_name(black_box(source_name)))
        });
    }

    group.bench_function("new_storage_id", |b| b.iter(new_storage_id));
    group.bench_function("sha256_hex/1KiB", |b| {
        b.iter(|| sha256_hex(black_box(&payload)))
    });
    group.bench_function("sha256_hex/1MiB", |b| {
        b.iter(|| sha256_hex(black_box(&megabyte)))
    });

    group.bench_function("put/1KiB", |b| {
        b.iter_batched(
            || {
                format!(
                    "original/w{}.jpg",
                    serial.fetch_add(1, Ordering::Relaxed) % PATH_WINDOW
                )
            },
            |path| {
                let result = store.put(black_box(&payload), &path);
                black_box(result)
            },
            BatchSize::SmallInput,
        )
    });
    group.bench_function("read/1KiB", |b| {
        store.put(&payload, "original/read-target.jpg").unwrap();
        b.iter(|| store.read(black_box("original/read-target.jpg")))
    });
    group.bench_function("exists/present", |b| {
        b.iter(|| store.exists(black_box("original/read-target.jpg")))
    });
    group.bench_function("exists/missing", |b| {
        b.iter(|| store.exists(black_box("original/absent.jpg")))
    });
    group.bench_function("delete/1KiB", |b| {
        b.iter_batched(
            || {
                let path = format!("original/d{}.jpg", serial.fetch_add(1, Ordering::Relaxed));
                store.put(&payload, &path).unwrap();
                path
            },
            |path| {
                let result = store.delete(black_box(&path));
                black_box(result)
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

criterion_group!(benches, store_paths);
criterion_main!(benches);
