//! The notes a public page shows, read as a visitor reads them: the list, the
//! markers over the picture, the toggle, the paging link and the chip on a
//! cropped tile.
//!
//! Every expected value comes from the design (`briefs/media-ui/C-public.md`,
//! interactions 2 and 4), not from the renderer: a note at (x, y) of a W by H
//! canvas sits at x/W and y/H, a time note links to `?t=` at its start, and a
//! note's text is read as text.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{Seed, append_key, get, seeded_pool, send};
use omeka_db::DbPool;
use omeka_db::ids as db;
use omeka_db::repos::{ExtentRow, MediaExtentRepo, MediaRepo, SettingRepo};
use omeka_db::rows::{NewMedia, NewResource};
use omeka_db::with_db;
use omeka_media::store::{FileStore, Store};
use serde_json::{Value, json};

const SITE: &str = "/s/museum";
const ANNO: &str = "application/ld+json; profile=\"http://www.w3.org/ns/anno.jsonld\"";

fn canvas(item: u64, media: u64) -> String {
    format!("http://localhost/iiif/3/item/{item}/canvas/{media}")
}

/// One annotation document a client may POST.
fn note(canvas: &str, text: &str, selector: Option<Value>) -> String {
    let target = match selector {
        None => json!(canvas),
        Some(selector) => json!({
            "type": "SpecificResource",
            "source": { "id": canvas },
            "selector": selector,
        }),
    };
    json!({
        "@context": "http://www.w3.org/ns/anno.jsonld",
        "type": "Annotation",
        "motivation": "commenting",
        "body": { "type": "TextualBody", "value": text, "format": "text/plain" },
        "target": target,
    })
    .to_string()
}

fn rect(x: u32, y: u32, w: u32, h: u32) -> Value {
    json!({
        "type": "FragmentSelector",
        "conformsTo": "http://www.w3.org/TR/media-frags/",
        "value": format!("xywh=pixel:{x},{y},{w},{h}"),
    })
}

fn time(start: &str, end: &str) -> Value {
    json!({
        "type": "FragmentSelector",
        "conformsTo": "http://www.w3.org/TR/media-frags/",
        "value": format!("t={start},{end}"),
    })
}

/// Create one note through the Web Annotation Protocol and answer its row id.
async fn add_note(app: &Router, media: u64, body: &str) -> i64 {
    let request = Request::builder()
        .method("POST")
        .uri(append_key(&format!("/annotations/media/{media}/")))
        .header("content-type", ANNO)
        .body(Body::from(body.to_string()))
        .unwrap();
    let answer = send(app.clone(), request).await;
    assert_eq!(answer.status, StatusCode::CREATED, "{}", answer.body);
    answer.json()["id"]
        .as_str()
        .and_then(|id| id.rsplit('/').next())
        .and_then(|id| id.parse().ok())
        .expect("the annotation's row id")
}

async fn page_of(app: &Router, path: &str) -> String {
    let answer = send(app.clone(), get(path, false)).await;
    assert_eq!(answer.status, StatusCode::OK, "{path}: {}", answer.body);
    answer.body
}

/// A media row for a stored original the test writes, so the extent reader
/// finds a real image header.
async fn add_image(
    pool: &DbPool,
    store: &FileStore,
    seed: &Seed,
    storage: &str,
    position: u64,
) -> db::MediaId {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/media-kinds/image.png");
    let bytes = std::fs::read(path).expect("the image corpus");
    store
        .put(&bytes, &format!("original/{storage}.png"), None)
        .expect("the stored original");
    with_db!(pool, |dbp| MediaRepo::new(dbp)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: Some(seed.user),
                resource_class_id: None,
                resource_template_id: None,
                thumbnail_id: None,
                title: Some("An annotated picture".to_string()),
                is_public: true,
                created: None,
            },
            item_id: db::ItemId::from(seed.item.0),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            data: None,
            source: Some("picture.png".to_string()),
            media_type: Some("image/png".to_string()),
            storage_id: Some(storage.to_string()),
            extension: Some("png".to_string()),
            sha256: Some("abc123".to_string()),
            size: Some(i64::try_from(bytes.len()).unwrap_or(0)),
            has_original: true,
            has_thumbnails: true,
            position: Some(position as i64),
            lang: None,
            alt_text: None,
        })
        .await)
    .unwrap()
    .id()
}

/// A video whose run time is already recorded, so the page needs no probe.
async fn add_video(pool: &DbPool, seed: &Seed, position: u64) -> db::MediaId {
    let media = with_db!(pool, |dbp| MediaRepo::new(dbp)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: Some(seed.user),
                resource_class_id: None,
                resource_template_id: None,
                thumbnail_id: None,
                title: Some("A recording".to_string()),
                is_public: true,
                created: None,
            },
            item_id: db::ItemId::from(seed.item.0),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            data: None,
            source: Some("talk.mp4".to_string()),
            media_type: Some("video/mp4".to_string()),
            storage_id: Some("notes-video".to_string()),
            extension: Some("mp4".to_string()),
            sha256: Some("abc123".to_string()),
            size: Some(1024),
            has_original: true,
            has_thumbnails: true,
            position: Some(position as i64),
            lang: None,
            alt_text: None,
        })
        .await)
    .unwrap();
    let media = media.id();
    with_db!(pool, |dbp| MediaExtentRepo::new(dbp)
        .put(
            media,
            "original/notes-video.mp4",
            &ExtentRow {
                duration_ms: Some(90_000),
                width: Some(640),
                height: Some(360),
            },
        )
        .await)
    .unwrap();
    media
}

#[tokio::test]
async fn a_picture_page_draws_each_note_where_it_points_and_lists_it_in_place_order() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let pool = DbPool::from(pool);
    let app = common::router_with_store(pool.clone(), store.clone());
    let media = add_image(&pool, &store, &seed, "notes-image", 2).await.0;
    let target = canvas(seed.item.0, media);

    // Created bottom first, so the list's order is the place, not the write.
    add_note(
        &app,
        media,
        &note(&target, "Bottom right", Some(rect(80, 50, 20, 20))),
    )
    .await;
    add_note(&app, media, &note(&target, "Whole file", None)).await;
    add_note(
        &app,
        media,
        &note(&target, "Top left", Some(rect(10, 10, 20, 10))),
    )
    .await;

    let page = page_of(&app, &format!("{SITE}/media/{media}")).await;
    assert!(
        page.contains("viewBox=\"0 0 120 80\""),
        "the canvas is the stored picture's size: {page}"
    );
    // 10/120 and 10/80 for the top-left region, and 80/120 and 50/80 for the
    // other, as percentages of the picture.
    assert!(page.contains("style=\"left:8.33%;top:12.50%\""), "{page}");
    assert!(page.contains("style=\"left:66.67%;top:62.50%\""), "{page}");
    assert!(page.contains("href=\"#note-"), "{page}");

    let list = page
        .split("<ol class=\"annotation-items\">")
        .nth(1)
        .unwrap_or("");
    let whole = list.find("Whole file").expect("the whole-file note");
    let top = list.find("Top left").expect("the top note");
    let bottom = list.find("Bottom right").expect("the bottom note");
    assert!(
        whole < top && top < bottom,
        "whole file, then top to bottom: {list}"
    );
    assert!(
        page.contains("Whole image") && page.contains(">Region</a>"),
        "each entry names its place: {page}"
    );
    assert!(
        page.contains("3 notes · 2 on regions · 1 on the whole image"),
        "the facts line counts the page: {page}"
    );
    assert!(
        page.contains("Show notes on the picture"),
        "the toggle is visible with no script: {page}"
    );
    assert!(
        page.contains("js/annotations.js?v="),
        "the script is the theme's own, versioned: {page}"
    );
}

#[tokio::test]
async fn a_note_with_markup_is_read_as_text() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let pool = DbPool::from(pool);
    let app = common::router_with_store(pool.clone(), store.clone());
    let media = add_image(&pool, &store, &seed, "hostile-note", 2).await.0;
    add_note(
        &app,
        media,
        &note(
            &canvas(seed.item.0, media),
            "<script>alert(1)</script>",
            Some(rect(1, 2, 10, 20)),
        ),
    )
    .await;
    let page = page_of(&app, &format!("{SITE}/media/{media}")).await;
    assert!(!page.contains("<script>alert"), "{page}");
    assert!(
        page.contains("&lt;script&gt;alert(1)&lt;/script&gt;"),
        "{page}"
    );
}

#[tokio::test]
async fn the_item_page_chips_a_cropped_tile_and_draws_no_overlay_on_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let pool = DbPool::from(pool);
    let app = common::router_with_store(pool.clone(), store.clone());
    let media = add_image(&pool, &store, &seed, "strip-image", 2).await.0;
    add_note(
        &app,
        media,
        &note(
            &canvas(seed.item.0, media),
            "A note on the second file",
            None,
        ),
    )
    .await;

    let page = page_of(&app, &format!("{SITE}/item/{}", seed.item.0)).await;
    let strip = page
        .split("<ul class=\"media-files-list\">")
        .nth(1)
        .unwrap_or_default();
    let tile = strip
        .split("<li class=\"media-file\">")
        .nth(1)
        .unwrap_or_default();
    assert!(
        tile.contains(&format!("href=\"{SITE}/media/{media}\"")),
        "the tile opens its media page: {tile}"
    );
    assert!(
        tile.contains("1 note") && tile.contains("media-file-notes"),
        "the tile names its note count: {tile}"
    );
    assert!(
        !tile.contains("annotated-media"),
        "a cropped tile carries no overlay: {tile}"
    );
    assert!(
        !page.contains("annotation-overlay"),
        "the first file has no notes, so no overlay is drawn: {page}"
    );
}

#[tokio::test]
async fn the_item_page_fills_the_first_files_notes_list() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let pool = DbPool::from(pool);
    let app = common::router_with_store(pool.clone(), store.clone());
    // A second file, so the item page draws a strip after the notes list.
    add_image(&pool, &store, &seed, "item-second", 2).await;
    add_note(
        &app,
        seed.media.0,
        &note(
            &canvas(seed.item.0, seed.media.0),
            "A note on the first file",
            Some(rect(10, 10, 20, 10)),
        ),
    )
    .await;

    let page = page_of(&app, &format!("{SITE}/item/{}", seed.item.0)).await;
    assert!(
        page.contains("class=\"annotated-media"),
        "the first file carries its notes: {page}"
    );
    let list = page
        .split("<section class=\"annotation-list\"")
        .nth(1)
        .unwrap_or_default();
    assert!(
        list.contains("A note on the first file"),
        "the list fills where it is placed: {page}"
    );
    let filled = page
        .find("<section class=\"annotation-list\"")
        .expect("a filled list");
    let strip = page
        .find("<ul class=\"media-files-list\">")
        .expect("the strip");
    assert!(
        filled < strip,
        "the list sits after the first file and before the strip: {page}"
    );
    assert_eq!(
        page.matches("<div class=\"annotation-list\"></div>")
            .count(),
        1,
        "the places the list takes keep their element: {page}"
    );
}

#[tokio::test]
async fn the_switch_off_leaves_the_item_pages_first_file_without_notes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let pool = DbPool::from(pool);
    let app = common::router_with_store(pool.clone(), store.clone());
    add_note(
        &app,
        seed.media.0,
        &note(
            &canvas(seed.item.0, seed.media.0),
            "A hidden note on the first file",
            None,
        ),
    )
    .await;
    with_db!(&pool, |dbp| SettingRepo::new(dbp)
        .set(omeka_api::settings::ANNOTATIONS_PUBLIC, &json!(false))
        .await)
    .unwrap();

    let page = page_of(&app, &format!("{SITE}/item/{}", seed.item.0)).await;
    assert!(!page.contains("A hidden note on the first file"), "{page}");
    assert!(!page.contains("class=\"annotated-media"), "{page}");
    assert!(
        page.contains("<div class=\"annotation-list\"></div>"),
        "the place stays, empty: {page}"
    );
}

#[tokio::test]
async fn a_media_with_no_notes_is_the_page_without_the_list() {
    let (pool, seed) = seeded_pool().await;
    let pool = DbPool::from(pool);
    let app = common::router(pool);
    let page = page_of(&app, &format!("{SITE}/media/{}", seed.media.0)).await;
    assert!(!page.contains("annotated-media"), "{page}");
    assert!(
        !page.contains("<section class=\"annotation-list\""),
        "no notes list is rendered: {page}"
    );
    assert!(!page.contains("js/annotations.js"), "{page}");
}

#[tokio::test]
async fn a_time_note_cues_the_player_at_its_start() {
    let (pool, seed) = seeded_pool().await;
    let pool = DbPool::from(pool);
    let app = common::router(pool.clone());
    let media = add_video(&pool, &seed, 2).await.0;
    add_note(
        &app,
        media,
        &note(
            &canvas(seed.item.0, media),
            "The shield appears",
            Some(time("62", "70")),
        ),
    )
    .await;

    let page = page_of(&app, &format!("{SITE}/media/{media}")).await;
    assert!(
        page.contains(&format!("href=\"{SITE}/media/{media}?t=62#player\"")),
        "the place link cues the player with no script: {page}"
    );
    assert!(
        page.contains("data-start-ms=\"62000\"") && page.contains("data-end-ms=\"70000\""),
        "the entry carries its range for the script: {page}"
    );
    assert!(page.contains("id=\"player\""), "{page}");
    assert!(
        page.contains("class=\"note-rail\"")
            && page.contains("class=\"note-rail-mark\"")
            && page.contains("left:68.89%"),
        "62 s of 90 s is a mark 68.89% along the rail: {page}"
    );
    assert!(
        page.contains(">1:02 to 1:10</a>") && page.contains("aria-label=\"Play 1:02 to 1:10\""),
        "{page}"
    );
    assert!(
        page.contains(&format!("href=\"{SITE}/media/{media}?note=")),
        "the entry's link cites the note: {page}"
    );
}

/// A box on a video's frames is drawn over the player in the frame's pixels,
/// carries its range for the script that shows it while it plays, and its entry
/// cues the player like a time note.
#[tokio::test]
async fn a_box_on_a_video_is_drawn_over_the_player_for_its_range() {
    let (pool, seed) = seeded_pool().await;
    let pool = DbPool::from(pool);
    let app = common::router(pool.clone());
    let media = add_video(&pool, &seed, 2).await.0;
    add_note(
        &app,
        media,
        &note(
            &canvas(seed.item.0, media),
            "The speaker's hands",
            Some(json!({
                "type": "FragmentSelector",
                "conformsTo": "http://www.w3.org/TR/media-frags/",
                "value": "xywh=pixel:64,36,320,180&t=62,70",
            })),
        ),
    )
    .await;

    let page = page_of(&app, &format!("{SITE}/media/{media}")).await;
    assert!(
        page.contains("class=\"annotated-media annotated-video\""),
        "a video with a box is marked so the box shows only while it plays: {page}"
    );
    assert!(
        page.contains("viewBox=\"0 0 640 360\"")
            && page.contains("x=\"64\" y=\"36\" width=\"320\" height=\"180\""),
        "the box is drawn in the frame's pixels: {page}"
    );
    assert!(
        page.contains("style=\"left:10.00%;top:10.00%\""),
        "the marker sits on the box's corner: {page}"
    );
    assert!(
        page.contains("data-start-ms=\"62000\"")
            && page.contains(&format!("href=\"{SITE}/media/{media}?t=62#player\""))
            && page.contains("note-place note-time note-box"),
        "the entry cues the player at the box's start: {page}"
    );
}

#[tokio::test]
async fn a_hundred_notes_page_and_a_cited_note_opens_its_page() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let pool = DbPool::from(pool);
    let app = common::router_with_store(pool.clone(), store.clone());
    let media = add_image(&pool, &store, &seed, "paged-image", 2).await.0;
    let target = canvas(seed.item.0, media);
    let mut ids = Vec::new();
    for index in 0..101 {
        ids.push(
            add_note(
                &app,
                media,
                &note(&target, &format!("Note {index}"), Some(rect(1, 1, 2, 2))),
            )
            .await,
        );
    }

    let first = page_of(&app, &format!("{SITE}/media/{media}")).await;
    assert_eq!(first.matches("class=\"annotation-item\"").count(), 100);
    assert!(first.contains("101 notes"), "{first}");
    assert!(first.contains("Next 100 notes"), "{first}");
    assert!(!first.contains("Note 100"), "the last note waits: {first}");
    assert!(
        !first.contains("facts"),
        "a paged list states no split: {first}"
    );
    let next_url = first
        .split("class=\"notes-next\" rel=\"next\" href=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("the next page's link")
        .to_string();
    let second = page_of(&app, &next_url).await;
    assert!(second.contains("Note 100"), "{second}");
    assert!(!second.contains("notes-next"), "{second}");

    // A note on the first page opens with the page a reader was reading, and
    // one on a later page opens the page that starts at it, marked.
    let early = ids[90];
    let page = page_of(&app, &format!("{SITE}/media/{media}?note={early}")).await;
    assert!(
        page.contains("Note 0"),
        "the first page keeps its context: {page}"
    );
    assert!(page.contains(&format!("id=\"note-{early}\"")), "{page}");
    let cited = ids[100];
    let page = page_of(&app, &format!("{SITE}/media/{media}?note={cited}")).await;
    assert!(
        !page.contains("Note 0"),
        "the later page starts at the note: {page}"
    );
    assert!(
        page.contains(&format!("id=\"note-{cited}\"")),
        "the cited note is on the page: {page}"
    );
    assert!(page.contains(&format!("data-cited=\"{cited}\"")), "{page}");
}

#[tokio::test]
async fn the_switch_off_hides_notes_from_a_visitor() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let pool = DbPool::from(pool);
    let app = common::router_with_store(pool.clone(), store.clone());
    let media = add_image(&pool, &store, &seed, "switch-image", 2).await.0;
    add_note(
        &app,
        media,
        &note(&canvas(seed.item.0, media), "A hidden note", None),
    )
    .await;
    with_db!(&pool, |dbp| SettingRepo::new(dbp)
        .set(omeka_api::settings::ANNOTATIONS_PUBLIC, &json!(false))
        .await)
    .unwrap();

    let page = page_of(&app, &format!("{SITE}/media/{media}")).await;
    assert!(!page.contains("A hidden note"), "{page}");
    assert!(!page.contains("annotated-media"), "{page}");
}
