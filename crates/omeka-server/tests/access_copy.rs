//! The access copy end to end: a video no browser plays gets an MP4, and the
//! media page, the IIIF canvas and a time note all read that copy while the
//! original stays the preservation master.
//!
//! The server under test is the real router over a seeded site and a real file
//! store. The copy is made by the derivative worker's own queue (`settle_next`
//! through `common::settle_derivatives`), so nothing here stands in for the
//! transcode.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{
    Seed, append_key, get, header, json_request, multipart_asset_request, router_with_store,
    seeded_pool, send, settle_derivatives,
};
use omeka_db::repos::{MediaDerivativeRepo, MediaRepo};
use omeka_db::rows::{DerivativeKind, DerivativeState};
use omeka_db::with_db;
use omeka_media::store::FileStore;
use omeka_media::tool::Tools;
use serde_json::{Value, json};
use std::path::Path;

/// The seeded site the fixture item is attached to.
const SITE: &str = "/s/museum";

fn corpus(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/media-kinds")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The URL inside a `<video …>` or `<audio …>` open tag, up to any fragment.
fn player_source(page: &str, tag: &str) -> Option<String> {
    let open = page.split(&format!("<{tag}")).nth(1)?;
    let tag_text = open.split('>').next()?;
    let src = tag_text.split("src=\"").nth(1)?.split('"').next()?;
    Some(src.to_string())
}

#[tokio::test(flavor = "multi_thread")]
async fn an_avi_plays_its_copy_and_a_time_note_still_lands_inside_it() {
    if Tools::on_path().ffmpeg().is_none() || Tools::on_path().ffprobe().is_none() {
        eprintln!("skipping: no ffmpeg or ffprobe on PATH");
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let app = router_with_store(pool.clone(), store.clone());
    let (item, media) = upload_avi(&app, &seed).await;
    let media_row = with_db!(&omeka_db::DbPool::from(pool.clone()), |db| MediaRepo::new(
        db
    )
    .get(omeka_db::ids::MediaId(media))
    .await)
    .expect("the media")
    .expect("the row");
    let storage_id = media_row.storage_id.clone().expect("a storage id");

    // The worker's queue: a preview and the access copy the create planned.
    settle_derivatives(pool.clone(), std::sync::Arc::new(store.clone())).await;
    let pool = omeka_db::DbPool::from(pool);
    let copy_row = with_db!(&pool, |db| MediaDerivativeRepo::new(db)
        .get(omeka_db::ids::MediaId(media), DerivativeKind::AccessCopy)
        .await)
    .expect("the row")
    .expect("the access row");
    assert_eq!(copy_row.state, DerivativeState::Ready, "{copy_row:?}");
    let copy = format!("access/{storage_id}.mp4");
    assert!(dir.path().join(&copy).is_file(), "the copy is in the store");
    assert!(
        dir.path()
            .join(format!("original/{storage_id}.avi"))
            .is_file(),
        "the original is untouched"
    );

    // The API document names the copy and keeps the original.
    let document = send(app.clone(), get(&format!("/api/media/{media}"), true))
        .await
        .json();
    let access = &document["o-module-romeka:access"];
    assert_eq!(access["state"], "ready", "{document}");
    assert_eq!(access["media_type"], "video/mp4", "{document}");
    let access_url = access["url"].as_str().expect("the copy's URL");
    assert!(access_url.ends_with(&copy), "{access_url}");
    let original_url = document["o:original_url"]
        .as_str()
        .expect("the original's URL");
    assert!(
        original_url.ends_with(&format!("original/{storage_id}.avi")),
        "{original_url}"
    );

    // The media page's player opens the copy, and the link inside it is the
    // original: the one file is playable, the other is the master.
    let page = send(app.clone(), get(&format!("{SITE}/media/{media}"), false))
        .await
        .body;
    let source = player_source(&page, "video").expect("a video element");
    assert!(
        source.ends_with(&copy),
        "the player opens the copy: {source}"
    );
    assert!(
        !source.contains("original/"),
        "the player never opens the AVI: {source}"
    );
    assert!(
        page.contains(&format!("href=\"{original_url}\"")),
        "the original stays linked: {page}"
    );
    assert!(
        page.contains("poster=\""),
        "the poster came from the copy's frame: {page}"
    );

    // The IIIF canvas paints the copy and links the original through
    // `rendering`, with the duration the probe measured.
    let manifest = send(
        app.clone(),
        get(&format!("/iiif/3/item/{item}/manifest"), false),
    )
    .await
    .json();
    let canvas = manifest["items"]
        .as_array()
        .and_then(|items| {
            items.iter().find(|canvas| {
                canvas["id"]
                    .as_str()
                    .is_some_and(|id| id.ends_with(&format!("/canvas/{media}")))
            })
        })
        .expect("the media's canvas");
    let body = &canvas["items"][0]["items"][0]["body"];
    assert_eq!(body["type"], "Video", "{canvas}");
    assert_eq!(body["format"], "video/mp4", "{canvas}");
    assert!(
        body["id"].as_str().is_some_and(|id| id.ends_with(&copy)),
        "the canvas body is the copy: {canvas}"
    );
    assert!(
        body["duration"]
            .as_f64()
            .is_some_and(|seconds| seconds > 0.0),
        "{canvas}"
    );
    assert_eq!(
        canvas["rendering"][0]["id"].as_str(),
        Some(original_url),
        "the original is a rendering link: {canvas}"
    );

    // The copy's run time is the original's, so a note written against the AVI
    // still names a moment inside the copy.
    let original_extent = omeka_media::av_probe::probe_with(
        Tools::on_path(),
        &dir.path().join(format!("original/{storage_id}.avi")),
    )
    .expect("the original's extent");
    let copy_extent = omeka_media::av_probe::probe_with(Tools::on_path(), &dir.path().join(&copy))
        .expect("the copy's extent");
    let original_ms = original_extent.duration_ms.expect("a duration");
    let copy_ms = copy_extent.duration_ms.expect("a duration");
    assert!(
        copy_ms.abs_diff(original_ms) <= 100,
        "original {original_ms} ms, copy {copy_ms} ms"
    );

    // A time range is a note the server accepts on this surface, and the page
    // cueing that moment cues the copy.
    let canvas_url = format!("http://localhost/iiif/3/item/{item}/canvas/{media}");
    let note = json!({
        "@context": "http://www.w3.org/ns/anno.jsonld",
        "type": "Annotation",
        "motivation": "commenting",
        "body": {"type": "TextualBody", "value": "The opening frame", "format": "text/plain"},
        "target": {
            "type": "SpecificResource",
            "source": {"id": canvas_url},
            "selector": {
                "type": "FragmentSelector",
                "conformsTo": "http://www.w3.org/TR/media-frags/",
                "value": "t=0,0.2"
            }
        }
    })
    .to_string();
    let created = send(
        app.clone(),
        json_request(
            "POST",
            &append_key(&format!("/annotations/media/{media}/")),
            &note,
        ),
    )
    .await;
    assert_eq!(created.status, 201, "{}", created.body);
    let container = send(
        app.clone(),
        get(&format!("/annotations/media/{media}/"), true),
    )
    .await;
    assert!(
        header(&container, "romeka-surface").is_some_and(|surface| surface.starts_with("time")),
        "{}",
        container.body
    );

    let cued = send(app, get(&format!("{SITE}/media/{media}?t=0.2"), false))
        .await
        .body;
    let source = player_source(&cued, "video").expect("a video element");
    assert!(
        source.ends_with(&format!("{copy}#t=0.2")),
        "the cue lands on the copy: {source}"
    );
}

/// Upload the MJPEG AVI as an item's only media and attach the item to the
/// seeded site. Answers the item and the media.
async fn upload_avi(app: &axum::Router, seed: &Seed) -> (u64, u64) {
    let data = json!({
        "o:is_public": true,
        "dcterms:title": [{"type": "literal", "@value": "Reel"}],
        "o:media": [{
            "o:ingester": "upload",
            "o:is_public": true,
            "file_index": 0,
        }],
    })
    .to_string();
    let answer = send(
        app.clone(),
        multipart_asset_request(
            &append_key("/api/items"),
            "reel.avi",
            &corpus("mjpeg.avi"),
            &data,
        ),
    )
    .await;
    assert!(answer.status.is_success(), "{}", answer.body);
    let created: Value = answer.json();
    let item = created["o:id"].as_u64().expect("the item");
    let media = created["o:media"][0]["o:id"].as_u64().expect("the media");
    let add = json!({"action": "add", "items": [item]}).to_string();
    let attached = send(
        app.clone(),
        json_request(
            "POST",
            &append_key(&format!("/api/sites/{}/items", seed.site)),
            &add,
        ),
    )
    .await;
    assert!(attached.status.is_success(), "{}", attached.body);
    (item, media)
}

/// A copy this install cannot make is no copy: the media page states the wait
/// instead of opening a video element on an AVI the browser cannot play, and
/// the original stays the download.
#[tokio::test(flavor = "multi_thread")]
async fn a_copy_this_install_cannot_make_states_the_wait_instead_of_a_player() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let app = router_with_store(pool.clone(), store);
    let (_, media) = upload_avi(&app, &seed).await;
    let pool = omeka_db::DbPool::from(pool);
    let claimed = with_db!(&pool, |db| MediaDerivativeRepo::new(db)
        .claim_next(&["ffmpeg"])
        .await)
    .expect("a claim")
    .expect("the access row a create queued");
    assert_eq!(claimed.kind, DerivativeKind::AccessCopy, "{claimed:?}");
    with_db!(&pool, |db| MediaDerivativeRepo::new(db)
        .finish(
            claimed.media_id,
            claimed.kind,
            DerivativeState::None,
            Some("needs vips, not installed".to_string()),
            Some("vips".to_string()),
        )
        .await)
    .expect("the settled row");

    let page = send(app.clone(), get(&format!("{SITE}/media/{media}"), false))
        .await
        .body;
    assert!(
        !page.contains("<video"),
        "no player opens an original the browser cannot play: {page}"
    );
    assert!(
        page.contains("A copy this browser can play or show could not be made."),
        "the page states the wait: {page}"
    );
    assert!(
        page.contains("/download"),
        "the original stays the download: {page}"
    );
}
