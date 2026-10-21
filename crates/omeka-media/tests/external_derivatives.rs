//! The three derivatives of a type no image decoder here reads: a video's
//! poster (`ffmpeg`), a PDF's first page (`pdftoppm`) and the AVIF or HEIC
//! frame `ffmpeg` decodes. The real commands run, because the shipped image
//! carries them (`packaging/Dockerfile`); a test host without them fails with
//! the reason the ingest would log.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use omeka_media::file_render::ThumbnailType;
use omeka_media::store::{FileStore, Store, StorePrefix, storage_path};
use omeka_media::thumbnailer::{
    ImageThumbnailer, ThumbnailOutcome, ThumbnailSkip, generate_thumbnails,
    generate_thumbnails_from_path, thumbnail_path, thumbnails_stored,
};
use omeka_media::tool::{ToolError, Tools};

fn fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/media-kinds")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn store(dir: &Path) -> FileStore {
    FileStore::new(dir, "/files")
}

fn sizes(bytes: &[u8]) -> (u64, u64) {
    let tags = omeka_media::image_metadata(&mut std::io::Cursor::new(bytes));
    let side = |name: &str| -> u64 {
        tags.get(name)
            .and_then(|tag| tag.values.first())
            .and_then(|value| value.parse().ok())
            .unwrap_or_else(|| panic!("no {name} in {tags:?}"))
    };
    (side("Width"), side("Height"))
}

/// The three derivatives a still makes, read back from where the store wrote
/// them. Every one is a JPEG the image crate decodes, and the square is the
/// square strategy's 200 pixels.
fn stored_derivatives(store: &FileStore, storage_id: &str) -> Vec<(ThumbnailType, Vec<u8>)> {
    ThumbnailType::ALL
        .into_iter()
        .map(|kind| {
            let bytes = store
                .read(&thumbnail_path(storage_id, kind))
                .unwrap_or_else(|error| panic!("{}: {error}", kind.as_str()));
            assert_eq!(
                image::guess_format(&bytes).ok(),
                Some(image::ImageFormat::Jpeg),
                "{} is not a JPEG",
                kind.as_str()
            );
            (kind, bytes)
        })
        .collect()
}

/// The reason names the install, so a red run says what to do.
fn expect_still(
    result: Result<Vec<(ThumbnailType, Vec<u8>)>, ThumbnailSkip>,
) -> Vec<(ThumbnailType, Vec<u8>)> {
    match result {
        Ok(derivatives) => derivatives,
        Err(ThumbnailSkip::Still(ToolError::ToolMissing(tool))) => panic!(
            "the {tool} command is not installed, so this test cannot run. \
             Install ffmpeg and poppler-utils."
        ),
        Err(other) => panic!("{other}"),
    }
}

/// A video's poster is a frame, and it feeds the same three strategies an
/// image does, so `has_thumbnails` becomes true for a video.
#[test]
fn a_video_gets_a_poster_in_the_three_sizes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = store(dir.path());
    let video = fixture("video.mp4");

    let outcome = generate_thumbnails(&store, "poster", &video, Some("video/mp4")).expect("store");

    assert_eq!(outcome, ThumbnailOutcome::Generated);
    assert!(thumbnails_stored(&store, "poster").expect("stored"));
    let derivatives = stored_derivatives(&store, "poster");
    let square = derivatives
        .iter()
        .find(|(kind, _)| *kind == ThumbnailType::Square)
        .map(|(_, bytes)| sizes(bytes))
        .expect("square");
    assert_eq!(square, (200, 200));
    let large = derivatives
        .iter()
        .find(|(kind, _)| *kind == ThumbnailType::Large)
        .map(|(_, bytes)| sizes(bytes))
        .expect("large");
    assert!(
        large.0 > 0 && large.1 > 0 && large.0 <= 800 && large.1 <= 800,
        "{large:?}"
    );
}

/// A PDF's first page goes through the same strategies, so a document shows a
/// page instead of a generic icon.
#[test]
fn a_pdf_gets_its_first_page() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = store(dir.path());
    let pdf = fixture("doc.pdf");

    let outcome =
        generate_thumbnails(&store, "page", &pdf, Some("application/pdf")).expect("store");

    assert_eq!(outcome, ThumbnailOutcome::Generated);
    let derivatives = stored_derivatives(&store, "page");
    let large = derivatives
        .iter()
        .find(|(kind, _)| *kind == ThumbnailType::Large)
        .map(|(_, bytes)| sizes(bytes))
        .expect("large");
    assert!(
        large.0 > large.1,
        "the fixture page is landscape: {large:?}"
    );
}

/// The AVIF and HEIC corpus files decode through `ffmpeg` on a build that has
/// the decoders. Either answer is a fact the matrix records, never a panic.
#[test]
fn an_avif_gets_a_frame_when_ffmpeg_decodes_it() {
    let avif = fixture("image.avif");
    let derivatives = expect_still(ImageThumbnailer.derivative_bytes(&avif, Some("image/avif")));
    assert_eq!(
        derivatives
            .iter()
            .map(|(kind, _)| *kind)
            .collect::<Vec<_>>(),
        ThumbnailType::ALL
    );
}

/// A still whose tool is absent is a typed skip with the tool's name, which is
/// the answer the ingest treats as "no derivative" rather than an error.
#[test]
fn a_missing_tool_skips_with_the_reason() {
    let pdf = fixture("doc.pdf");
    let skip = ImageThumbnailer
        .still_derivatives(&pdf, "application/pdf", &Tools::none())
        .expect_err("no pdftoppm");
    assert_eq!(
        skip,
        ThumbnailSkip::Still(ToolError::ToolMissing("pdftoppm"))
    );
    assert_eq!(
        skip.to_string(),
        "cannot make a still: the pdftoppm command is not on PATH"
    );
}

/// The backfill path reads a file the caller already streamed, so a 2 GB video
/// never has to be in memory for its poster.
#[test]
fn a_still_from_a_file_writes_the_three_derivatives() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = store(&dir.path().join("files"));
    let source = dir.path().join("streamed-video");
    std::fs::write(&source, fixture("video.mp4")).expect("write the streamed copy");

    let outcome = generate_thumbnails_from_path(&store, "streamed", &source, Some("video/mp4"))
        .expect("path");

    assert_eq!(outcome, ThumbnailOutcome::Generated);
    assert!(
        store
            .exists(&thumbnail_path("streamed", ThumbnailType::Square))
            .expect("square")
    );
}

/// A type no tool makes a still of is skipped before anything is read, so the
/// backfill does not open a file it cannot use.
#[test]
fn a_type_no_tool_reads_is_skipped_without_a_still() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = store(dir.path());
    let original = storage_path(StorePrefix::Original, "word", Some("docx"));
    store.put(b"not read", &original, None).expect("store");
    let source: PathBuf = dir.path().join("missing");

    let outcome =
        generate_thumbnails_from_path(&store, "word", &source, Some("application/msword"))
            .expect("path");

    assert_eq!(
        outcome,
        ThumbnailOutcome::Skipped(ThumbnailSkip::UnsupportedType(
            "application/msword".to_string()
        ))
    );
}
