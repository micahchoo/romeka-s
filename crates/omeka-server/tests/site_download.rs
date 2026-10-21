//! `GET /s/{site}/media/{id}/download`: the original under its source name.
//!
//! The route is Romeka's own: upstream serves `/files/original/<storage id>`
//! with no `Content-Disposition`, so a "save as" offers the storage id. The
//! page offers the route under every file that has an original.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{get, header, router_with_store, seeded_pool, send};
use omeka_db::ids::{ItemId, UserId};
use omeka_db::repos::MediaRepo;
use omeka_db::rows::{NewMedia, NewResource};
use omeka_media::store::{FileStore, Store};

/// The seeded media's stored name and bytes.
const STORAGE: &str = "4b1e5b9e0f9f4f5a8f6b2c3d4e5f6a7b8c9d0e1f";
const BYTES: &[u8] = b"the stored original";

/// One stored original, as a test writes it before asking for it.
struct StoredFile<'a> {
    source: &'a str,
    storage: &'a str,
    extension: &'a str,
    media_type: &'a str,
    is_public: bool,
    bytes: &'a [u8],
}

/// Add one media row with an original stored at `original/<storage>.<ext>`.
async fn add_media(
    pool: &sqlx::SqlitePool,
    store: &FileStore,
    item: ItemId,
    owner: UserId,
    file: StoredFile<'_>,
) -> u64 {
    store
        .put(
            file.bytes,
            &format!("original/{}.{}", file.storage, file.extension),
            Some(file.media_type),
        )
        .unwrap();
    MediaRepo::new(pool)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: Some(owner),
                title: Some("A file".to_string()),
                is_public: file.is_public,
                ..NewResource::default()
            },
            item_id: item,
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            data: None,
            source: Some(file.source.to_string()),
            media_type: Some(file.media_type.to_string()),
            storage_id: Some(file.storage.to_string()),
            extension: Some(file.extension.to_string()),
            sha256: Some(file.storage.to_string()),
            size: Some(file.bytes.len() as i64),
            has_original: true,
            has_thumbnails: false,
            position: None,
            lang: None,
            alt_text: None,
        })
        .await
        .unwrap()
        .id()
        .0
}

#[tokio::test]
async fn a_public_media_downloads_the_original_as_an_attachment_named_by_its_source() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let app = router_with_store(pool.clone(), store.clone());
    let media = add_media(
        &pool,
        &store,
        seed.item,
        seed.user,
        StoredFile {
            source: "A Talk.pdf",
            storage: "talk",
            extension: "pdf",
            media_type: "application/pdf",
            is_public: true,
            bytes: BYTES,
        },
    )
    .await;

    let answer = send(
        app,
        get(&format!("/s/museum/media/{media}/download"), false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(header(&answer, "content-type"), Some("application/pdf"));
    assert_eq!(
        header(&answer, "content-disposition"),
        Some("attachment; filename=\"A Talk.pdf\"; filename*=UTF-8''A%20Talk.pdf")
    );
    assert_eq!(header(&answer, "content-length"), Some("19"));
    assert_eq!(answer.body, "the stored original");
}

#[tokio::test]
async fn a_windows_source_name_keeps_only_its_last_segment() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let app = router_with_store(pool.clone(), store.clone());
    let media = add_media(
        &pool,
        &store,
        seed.item,
        seed.user,
        StoredFile {
            source: "C:\\Users\\a\\scan 1.pdf",
            storage: "scan",
            extension: "pdf",
            media_type: "application/pdf",
            is_public: true,
            bytes: BYTES,
        },
    )
    .await;

    let answer = send(
        app,
        get(&format!("/s/museum/media/{media}/download"), false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(
        header(&answer, "content-disposition"),
        Some("attachment; filename=\"scan 1.pdf\"; filename*=UTF-8''scan%201.pdf")
    );
}

#[tokio::test]
async fn a_private_media_is_a_404_to_an_anonymous_caller() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let app = router_with_store(pool.clone(), store.clone());
    let media = add_media(
        &pool,
        &store,
        seed.item,
        seed.user,
        StoredFile {
            source: "private.pdf",
            storage: "private",
            extension: "pdf",
            media_type: "application/pdf",
            is_public: false,
            bytes: BYTES,
        },
    )
    .await;

    let anonymous = send(
        app.clone(),
        get(&format!("/s/museum/media/{media}/download"), false),
    )
    .await;
    assert_eq!(anonymous.status, StatusCode::NOT_FOUND);
    assert!(!anonymous.body.contains("the stored original"));
}

#[tokio::test]
async fn a_media_with_no_original_has_no_download() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let app = router_with_store(pool.clone(), store.clone());
    let media = MediaRepo::new(&pool)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: Some(seed.user),
                title: Some("An embed".to_string()),
                is_public: true,
                ..NewResource::default()
            },
            item_id: seed.item,
            ingester: "oembed".to_string(),
            renderer: "oembed".to_string(),
            data: None,
            source: Some("https://example.org/film".to_string()),
            media_type: None,
            storage_id: None,
            extension: None,
            sha256: None,
            size: None,
            has_original: false,
            has_thumbnails: false,
            position: None,
            lang: None,
            alt_text: None,
        })
        .await
        .unwrap()
        .id()
        .0;

    let answer = send(
        app,
        get(&format!("/s/museum/media/{media}/download"), false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
}

/// The seeded media has an original at the storage path the seed wrote, so the
/// route answers it too.
#[tokio::test]
async fn the_seeded_media_keeps_its_source_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    store
        .put(
            BYTES,
            &format!("original/{STORAGE}.jpg"),
            Some("image/jpeg"),
        )
        .unwrap();
    let (pool, seed) = seeded_pool().await;
    let app = router_with_store(pool, store.clone());
    let answer = send(
        app,
        get(&format!("/s/museum/media/{}/download", seed.media.0), false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(
        header(&answer, "content-disposition"),
        Some("attachment; filename=\"source.jpg\"; filename*=UTF-8''source.jpg")
    );
}

#[tokio::test]
async fn a_source_with_no_name_uses_the_display_title_and_the_extension() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let app = router_with_store(pool.clone(), store.clone());
    let media = add_media(
        &pool,
        &store,
        seed.item,
        seed.user,
        StoredFile {
            source: "   ",
            storage: "untitled",
            extension: "pdf",
            media_type: "application/pdf",
            is_public: true,
            bytes: BYTES,
        },
    )
    .await;

    let answer = send(
        app,
        get(&format!("/s/museum/media/{media}/download"), false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(
        header(&answer, "content-disposition"),
        Some("attachment; filename=\"A file.pdf\"; filename*=UTF-8''A%20file.pdf")
    );
}

#[tokio::test]
async fn a_large_original_answers_whole_across_chunk_boundaries() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let app = router_with_store(pool.clone(), store.clone());
    // More than one 256 KiB chunk, which is the read size.
    let big = vec![b'x'; 700_000];
    let media = add_media(
        &pool,
        &store,
        seed.item,
        seed.user,
        StoredFile {
            source: "big.bin",
            storage: "big",
            extension: "bin",
            media_type: "application/octet-stream",
            is_public: true,
            bytes: &big,
        },
    )
    .await;

    let answer = send(
        app,
        get(&format!("/s/museum/media/{media}/download"), false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.body.len(), big.len());
    assert_eq!(answer.body.as_bytes(), big.as_slice());
    let length = big.len().to_string();
    assert_eq!(header(&answer, "content-length"), Some(length.as_str()));
}
