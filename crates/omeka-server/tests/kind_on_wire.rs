//! The server's media kind on the wire and in both filters.
//!
//! Every media document carries `o:kind`, one stable name per
//! `omeka_media::kind::MediaKind` variant. `/api/media` takes `romeka_kind`
//! (repeatable) and `/api/items` takes `has_media_kind`, and each answers
//! exactly the rows whose document names one of the requested kinds.
//!
//! The corpus test reads every file in `fixtures/media-kinds/`, stores it as a
//! file media with the type the server detects, and asserts that the filter's
//! answer equals the set the documents name. That is the check that keeps the
//! SQL mirror of `MediaKind::of` honest. The variant test seeds one media per
//! kind, including the four no corpus file produces (the two IIIF kinds, an
//! embed and a captions type).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use axum::Router;
use common::{
    Seed, append_key, get, multipart_asset_request, router, router_with_store, seeded_engine_pool,
    send,
};
use omeka_db::DbPool;
use omeka_db::repos::{ItemRepo, MediaRepo};
use omeka_db::rows::{NewItem, NewMedia, NewResource};
use omeka_db::with_db;
use omeka_media::kind::{DISPLAY_IMAGE_MEDIA_TYPES, KindInput, MediaKind};
use omeka_media::media_type::detected_media_type_of;
use omeka_media::store::FileStore;

/// One media row the test stored.
struct Stored {
    file: String,
    item: u64,
    media: u64,
    kind: MediaKind,
}

/// One row of the variant table: the stored fields that decide a kind, and the
/// kind they decide.
type VariantRow = (
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
    bool,
    MediaKind,
);

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/media-kinds")
}

/// Every corpus file name, sorted, without the corpus's own documents.
fn corpus_files() -> Vec<String> {
    let mut files: Vec<String> = std::fs::read_dir(corpus_dir())
        .expect("fixtures/media-kinds")
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            (!matches!(name.as_str(), "make.sh" | "README.md" | "BASELINE.md")
                && !name.starts_with('.'))
            .then_some(name)
        })
        .collect();
    files.sort();
    files
}

/// Store one item with one file media, and answer the ids the API will use.
async fn store_media(
    pool: &DbPool,
    seed: &Seed,
    name: &str,
    ingester: &str,
    renderer: &str,
    media_type: Option<&str>,
    has_thumbnails: bool,
) -> (u64, u64) {
    let item = with_db!(pool, |db| ItemRepo::new(db)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(seed.user),
                resource_class_id: None,
                resource_template_id: None,
                thumbnail_id: None,
                title: Some(name.to_string()),
                is_public: true,
                created: None,
            },
            primary_media_id: None,
        })
        .await)
    .unwrap();
    let media = with_db!(pool, |db| MediaRepo::new(db)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: Some(seed.user),
                resource_class_id: None,
                resource_template_id: None,
                thumbnail_id: None,
                title: Some(name.to_string()),
                is_public: true,
                created: None,
            },
            item_id: item.id(),
            ingester: ingester.to_string(),
            renderer: renderer.to_string(),
            data: None,
            source: Some(name.to_string()),
            media_type: media_type.map(str::to_string),
            storage_id: Some(format!("kind-{}", item.id().0)),
            extension: None,
            sha256: None,
            size: Some(1),
            has_original: true,
            has_thumbnails,
            position: Some(1),
            lang: None,
            alt_text: None,
        })
        .await)
    .unwrap();
    (item.id().0, media.id().0)
}

/// The kind `MediaKind::of` answers for the row `store_media` writes.
fn kind_of(
    ingester: &str,
    renderer: &str,
    media_type: Option<&str>,
    thumbnails: bool,
) -> MediaKind {
    MediaKind::of(&KindInput {
        renderer,
        ingester,
        media_type,
        has_thumbnails: thumbnails,
    })
}

/// One document's `o:kind`, as a string the test can compare.
fn kind_name(doc: &serde_json::Value) -> String {
    doc["o:kind"]
        .as_str()
        .unwrap_or_else(|| panic!("no o:kind in {doc}"))
        .to_string()
}

/// The `o:id` of every document in a list answer.
fn ids(body: &serde_json::Value) -> BTreeSet<u64> {
    body.as_array()
        .unwrap_or_else(|| panic!("not a list: {body}"))
        .iter()
        .map(|doc| doc["o:id"].as_u64().expect("o:id"))
        .collect()
}

/// `GET` one list answer, keyed, with a large page.
async fn list(app: &Router, path: &str) -> serde_json::Value {
    let answer = send(app.clone(), get(path, true)).await;
    assert_eq!(answer.status, axum::http::StatusCode::OK, "{}", answer.body);
    answer.json()
}

async fn store_corpus(pool: &DbPool, seed: &Seed) -> Vec<Stored> {
    let mut stored = Vec::new();
    for file in corpus_files() {
        let bytes = std::fs::read(corpus_dir().join(&file)).expect(&file);
        let extension = file.rsplit_once('.').map(|(_, extension)| extension);
        let media_type = detected_media_type_of(&bytes, extension);
        let thumbnails = DISPLAY_IMAGE_MEDIA_TYPES.contains(&media_type.as_str());
        let (item, media) = store_media(
            pool,
            seed,
            &format!("corpus/{file}"),
            "upload",
            "file",
            Some(&media_type),
            thumbnails,
        )
        .await;
        stored.push(Stored {
            file,
            item,
            media,
            kind: kind_of("upload", "file", Some(&media_type), thumbnails),
        });
    }
    stored
}

/// The seeded fixture's own media: a JPEG whose thumbnails exist.
fn seeded_kind() -> MediaKind {
    kind_of("upload", "file", Some("image/jpeg"), true)
}

#[tokio::test]
async fn every_corpus_file_carries_its_kind_and_the_filters_find_exactly_it() {
    let (pool, seed) = seeded_engine_pool().await;
    let app = router(pool.clone());
    let stored = store_corpus(&pool, &seed).await;

    // The document names the kind the table gives its stored inputs.
    for row in &stored {
        let doc = list(&app, &format!("/api/media/{}", row.media)).await;
        assert_eq!(
            kind_name(&doc),
            row.kind.name(),
            "{} ({:?}): {doc}",
            row.file,
            row.kind
        );
    }

    // The filter's answer is the set the documents name, for every name the
    // server knows, so a name with no corpus row answers nothing.
    let mut media_by_kind: BTreeMap<&str, BTreeSet<u64>> = BTreeMap::new();
    let mut items_by_kind: BTreeMap<&str, BTreeSet<u64>> = BTreeMap::new();
    media_by_kind
        .entry(seeded_kind().name())
        .or_default()
        .insert(seed.media.0);
    items_by_kind
        .entry(seeded_kind().name())
        .or_default()
        .insert(seed.item.0);
    for row in &stored {
        media_by_kind
            .entry(row.kind.name())
            .or_default()
            .insert(row.media);
        items_by_kind
            .entry(row.kind.name())
            .or_default()
            .insert(row.item);
    }
    for name in "local-image|iiif-image|audio|video|pdf|model-3d|captions|embed|iiif-presentation|other-file"
        .split('|')
    {
        let media = list(&app, &format!("/api/media?per_page=1000&romeka_kind={name}")).await;
        assert_eq!(
            ids(&media),
            media_by_kind.get(name).cloned().unwrap_or_default(),
            "media kind {name}"
        );
        let items = list(&app, &format!("/api/items?per_page=1000&has_media_kind={name}")).await;
        assert_eq!(
            ids(&items),
            items_by_kind.get(name).cloned().unwrap_or_default(),
            "item kind {name}"
        );
    }
}

#[tokio::test]
async fn each_variant_is_named_and_both_filters_answer_it() {
    let (pool, seed) = seeded_engine_pool().await;
    let app = router(pool.clone());

    // A row that produces each variant, in the table's own order.
    let rows: [VariantRow; 10] = [
        (
            "local-image.jpg",
            "upload",
            "file",
            Some("image/jpeg"),
            true,
            MediaKind::LocalImage,
        ),
        (
            "iiif-image.jpg",
            "iiif",
            "iiif",
            Some("image/jpeg"),
            false,
            MediaKind::IiifImage,
        ),
        (
            "tone.mp3",
            "upload",
            "file",
            Some("audio/mpeg"),
            false,
            MediaKind::Audio,
        ),
        (
            "clip.mp4",
            "upload",
            "file",
            Some("video/mp4"),
            false,
            MediaKind::Video,
        ),
        (
            "scan.pdf",
            "upload",
            "file",
            Some("application/pdf"),
            false,
            MediaKind::Pdf { first_page: false },
        ),
        (
            "model.glb",
            "upload",
            "file",
            Some("model/gltf-binary"),
            false,
            MediaKind::Model3d,
        ),
        (
            "captions.vtt",
            "upload",
            "file",
            Some("text/vtt"),
            false,
            MediaKind::Captions,
        ),
        (
            "youtube",
            "upload",
            "youtube",
            Some("image/jpeg"),
            false,
            MediaKind::Embed,
        ),
        (
            "manifest",
            "iiif_presentation",
            "iiif_presentation",
            None,
            false,
            MediaKind::IiifPresentation,
        ),
        (
            "archive.zip",
            "upload",
            "file",
            Some("application/zip"),
            false,
            MediaKind::OtherFile,
        ),
    ];

    for (name, ingester, renderer, media_type, thumbnails, kind) in rows {
        let (item, media) = store_media(
            &pool, &seed, name, ingester, renderer, media_type, thumbnails,
        )
        .await;
        let doc = list(&app, &format!("/api/media/{media}")).await;
        assert_eq!(kind_name(&doc), kind.name(), "{name}");

        let by_kind = list(
            &app,
            &format!("/api/media?per_page=1000&romeka_kind={}", kind.name()),
        )
        .await;
        assert!(
            ids(&by_kind).contains(&media),
            "{} is not in {name}: {by_kind}",
            media
        );
        let items = list(
            &app,
            &format!("/api/items?per_page=1000&has_media_kind={}", kind.name()),
        )
        .await;
        assert!(
            ids(&items).contains(&item),
            "item {item} for {name}: {items}"
        );
    }
}

#[tokio::test]
async fn a_repeated_kind_filter_unions_the_names() {
    let (pool, seed) = seeded_engine_pool().await;
    let app = router(pool.clone());
    let (audio_item, audio) = store_media(
        &pool,
        &seed,
        "tone.mp3",
        "upload",
        "file",
        Some("audio/mpeg"),
        false,
    )
    .await;
    let (_, video) = store_media(
        &pool,
        &seed,
        "clip.mp4",
        "upload",
        "file",
        Some("video/mp4"),
        false,
    )
    .await;
    let (_, pdf) = store_media(
        &pool,
        &seed,
        "scan.pdf",
        "upload",
        "file",
        Some("application/pdf"),
        false,
    )
    .await;

    let media = list(
        &app,
        "/api/media?per_page=1000&romeka_kind=audio&romeka_kind=video",
    )
    .await;
    let found = ids(&media);
    assert!(found.contains(&audio), "audio: {media}");
    assert!(found.contains(&video), "video: {media}");
    assert!(!found.contains(&pdf), "pdf: {media}");

    let items = list(
        &app,
        "/api/items?per_page=1000&has_media_kind=audio&has_media_kind=video",
    )
    .await;
    let found = ids(&items);
    assert!(found.contains(&audio_item), "audio item: {items}");
}

#[tokio::test]
async fn a_name_the_server_does_not_know_matches_nothing() {
    let (pool, _seed) = seeded_engine_pool().await;
    let app = router(pool.clone());
    let media = list(&app, "/api/media?per_page=1000&romeka_kind=document").await;
    assert_eq!(ids(&media), BTreeSet::new(), "{media}");
    let items = list(&app, "/api/items?per_page=1000&has_media_kind=document").await;
    assert_eq!(ids(&items), BTreeSet::new(), "{items}");
}

#[tokio::test]
async fn an_uploaded_file_carries_its_kind_in_the_list_and_the_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, _seed) = seeded_engine_pool().await;
    let app = router_with_store(pool.clone(), store.clone());
    let bytes = std::fs::read(corpus_dir().join("image.png")).expect("image.png");
    let data = serde_json::json!({
        "o:is_public": true,
        "o:media": [{
            "o:ingester": "upload",
            "o:is_public": true,
            "file_index": 0,
            "o:alt_text": "A corpus PNG",
        }],
    })
    .to_string();
    let answer = send(
        app.clone(),
        multipart_asset_request(&append_key("/api/items"), "image.png", &bytes, &data),
    )
    .await;
    assert!(answer.status.is_success(), "{}", answer.body);
    common::settle_derivatives(pool.clone(), std::sync::Arc::new(store)).await;
    let created = answer.json();
    let media = created["o:media"][0]["o:id"].as_u64().expect("media id");

    let doc = list(&app, &format!("/api/media/{media}")).await;
    assert_eq!(kind_name(&doc), MediaKind::LocalImage.name());

    let page = list(&app, "/api/media?per_page=1000&romeka_kind=local-image").await;
    assert!(ids(&page).contains(&media), "{page}");
}
