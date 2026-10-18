//! Behavioural tests for the image thumbnailer against a real file store.
//!
//! `crates/omeka-media/src/thumbnailer.rs` freezes the resize math; these tests
//! drive [`generate_thumbnails`] against a temporary [`FileStore`] and read the
//! bytes back, so they observe what a caller does: three files, at the paths
//! upstream writes, with the dimensions upstream computes.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use omeka_media::ThumbnailType;
use omeka_media::store::FileStore;
use omeka_media::thumbnailer::{ThumbnailOutcome, ThumbnailSkip, generate_thumbnails};

/// A temporary store that removes its tree when the test ends.
struct TempStore {
    store: FileStore,
    root: PathBuf,
}

impl TempStore {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!("romeka-thumb-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        Self {
            store: FileStore::new(&root, "/files"),
            root,
        }
    }
}

impl Drop for TempStore {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A PNG of the given size.
fn png(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
    });
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgb8(image)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .expect("encode png");
    bytes
}

fn dimensions(bytes: &[u8]) -> (u32, u32) {
    let image = image::load_from_memory(bytes).expect("decode");
    (image.width(), image.height())
}

/// `File/TempFile.php:287`: each derivative lands at
/// `<type>/<storage_id>.jpg` with the size `Gd::createDefault` and
/// `Gd::createSquare` compute for `thumbnails.types`.
#[test]
fn generated_image_produces_three_files_with_the_expected_dimensions() {
    let temp = TempStore::new("sizes");
    let outcome = generate_thumbnails(
        &temp.store,
        "4b1e5b9e0f9f4f5a8f6b2c3d4e5f6a7b8c9d0e1f",
        &png(1000, 500),
        Some("image/png"),
    )
    .expect("generate");
    assert_eq!(outcome, ThumbnailOutcome::Generated);

    let expected = [
        (ThumbnailType::Large, (800, 400)),
        (ThumbnailType::Medium, (400, 200)),
        (ThumbnailType::Square, (200, 200)),
    ];
    for (kind, size) in expected {
        let path = format!(
            "{}/4b1e5b9e0f9f4f5a8f6b2c3d4e5f6a7b8c9d0e1f.jpg",
            kind.as_str()
        );
        assert!(temp.store.exists(&path), "{path} is missing");
        assert_eq!(dimensions(&temp.store.read(&path).expect("read")), size);
    }
    assert!(!temp.store.exists("thumbnailer-junk"));
}

/// A second run finds every file present and writes none of them. The proof
/// overwrites one file with a sentinel after the first run: a rewrite would
/// replace the sentinel with a real JPEG.
#[test]
fn a_second_run_does_not_rewrite_the_files() {
    let temp = TempStore::new("idempotent");
    let storage_id = "abc123def456";
    let first = generate_thumbnails(&temp.store, storage_id, &png(1000, 500), Some("image/png"))
        .expect("first generate");
    assert_eq!(first, ThumbnailOutcome::Generated);

    let paths: Vec<String> = ThumbnailType::ALL
        .iter()
        .map(|kind| format!("{}/{storage_id}.jpg", kind.as_str()))
        .collect();
    let sentinel = b"sentinel".to_vec();
    temp.store
        .put(&sentinel, &paths[0])
        .expect("write sentinel");
    let modified: Vec<_> = paths
        .iter()
        .map(|path| {
            temp.store
                .local_path(path)
                .expect("path")
                .metadata()
                .expect("metadata")
                .modified()
                .expect("mtime")
        })
        .collect();

    let second = generate_thumbnails(&temp.store, storage_id, &png(1000, 500), Some("image/png"))
        .expect("second generate");
    assert_eq!(second, ThumbnailOutcome::Existing);
    assert_eq!(temp.store.read(&paths[0]).expect("read"), sentinel);
    for (path, before) in paths.iter().zip(modified) {
        assert_eq!(
            temp.store
                .local_path(path)
                .expect("path")
                .metadata()
                .expect("metadata")
                .modified()
                .expect("mtime"),
            before,
            "{path} was rewritten"
        );
    }
}

/// A partial tree is completed: a run that stopped after the first file leaves
/// the other two missing, and the next run writes them.
#[test]
fn a_partial_tree_is_completed() {
    let temp = TempStore::new("partial");
    let storage_id = "partial123";
    let paths: Vec<String> = ThumbnailType::ALL
        .iter()
        .map(|kind| format!("{}/{storage_id}.jpg", kind.as_str()))
        .collect();
    temp.store.put(b"half", &paths[0]).expect("seed");
    assert!(temp.store.exists(&paths[0]));
    assert!(!temp.store.exists(&paths[1]));

    let outcome = generate_thumbnails(&temp.store, storage_id, &png(600, 300), Some("image/png"))
        .expect("generate");
    assert_eq!(outcome, ThumbnailOutcome::Generated);
    let expected = [
        (ThumbnailType::Large, (600, 300)),
        (ThumbnailType::Medium, (400, 200)),
        (ThumbnailType::Square, (200, 200)),
    ];
    for (kind, size) in expected {
        let path = format!("{}/{storage_id}.jpg", kind.as_str());
        assert!(temp.store.exists(&path), "{path} is missing");
        assert_eq!(dimensions(&temp.store.read(&path).expect("read")), size);
    }
}

/// `Gd::setSource`: a declared type outside gif, jpeg, png and webp is a typed
/// skip, and nothing is written.
#[test]
fn an_unsupported_type_is_a_typed_skip() {
    let temp = TempStore::new("unsupported");
    let outcome = generate_thumbnails(&temp.store, "skipme", &png(20, 20), Some("application/pdf"))
        .expect("generate");
    assert_eq!(
        outcome,
        ThumbnailOutcome::Skipped(ThumbnailSkip::UnsupportedType(
            "application/pdf".to_string()
        ))
    );
    assert!(!temp.store.exists("large/skipme.jpg"));
}

/// A listed type whose bytes are not an image is a typed skip, not a panic, and
/// nothing is written.
#[test]
fn an_undecodable_source_is_a_typed_skip() {
    let temp = TempStore::new("undecodable");
    let outcome = generate_thumbnails(
        &temp.store,
        "nope",
        b"definitely not an image",
        Some("image/jpeg"),
    )
    .expect("generate");
    assert!(
        matches!(outcome, ThumbnailOutcome::Skipped(_)),
        "{outcome:?}"
    );
    assert!(!temp.store.exists("large/nope.jpg"));
}
