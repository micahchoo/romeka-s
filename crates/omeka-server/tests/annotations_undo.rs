//! The undo of a caption replace (G6): the edition a replace keeps, the route
//! that brings it back, and every reader that must not see the kept words.
//!
//! Expected values come from the media UI design (interaction 4 and its tension)
//! and from the contract's caption route (6.5), not from the handlers.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{Answer, Seed, append_key, get, header, router_with_store, send, site_pool};
use omeka_db::ids::{ItemId, MediaId};
use omeka_db::repos::MediaRepo;
use omeka_db::rows::{NewMedia, NewResource};
use omeka_media::store::FileStore;
use serde_json::{Value, json};

const VTT: &str = "text/vtt";

const RED: &str = "WEBVTT\n\n00:00:00.000 --> 00:00:00.400\nA red square, then a blue bar.\n";
const BLUE: &str = "WEBVTT\n\n00:00:00.000 --> 00:00:00.400\nA blue square, then a blue bar.\n";
const GREEN: &str = "WEBVTT\n\n00:00:00.000 --> 00:00:00.400\nA green square, then a blue bar.\n";

const NOTHING_TO_UNDO: &str = "There is no caption edition to bring back. \
     An undo works once, until the next import of these captions.";

/// A router whose kept editions live in a tempdir, and the dir that holds them.
fn app_with_store(pool: &sqlx::SqlitePool) -> (Router, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    (router_with_store(pool.clone(), store), dir)
}

fn tracks_path(media: u64, lang: &str) -> String {
    format!("/annotations/media/{media}/tracks?lang={lang}")
}

async fn import(app: &Router, media: u64, lang: &str, body: &str) -> Answer {
    send(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(append_key(&tracks_path(media, lang)))
            .header("content-type", VTT)
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
}

async fn undo(app: &Router, media: u64, lang: &str) -> Answer {
    send(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(append_key(&format!(
                "/annotations/media/{media}/tracks/undo?lang={lang}"
            )))
            .body(Body::empty())
            .unwrap(),
    )
    .await
}

/// The caption languages, as the editor's list reads them (G3).
async fn list(app: &Router, media: u64) -> Vec<Value> {
    let answer = send(
        app.clone(),
        get(&format!("/annotations/media/{media}/tracks"), true),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    answer.json().as_array().cloned().unwrap_or_default()
}

async fn body_of(app: &Router, path: &str, keyed: bool) -> String {
    let answer = send(app.clone(), get(path, keyed)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    answer.body
}

/// Every way a reader meets a media's captions, as one string each: the WAP
/// container, its IIIF page, `captions.vtt`, the public media page, and the
/// item's Content Search answer.
async fn readers(app: &Router, media: u64, item: u64, query: &str) -> Vec<(String, String)> {
    vec![
        (
            "the container".to_string(),
            body_of(
                app,
                &format!("/annotations/media/{media}/?origin=all"),
                true,
            )
            .await,
        ),
        (
            "the annotation page".to_string(),
            body_of(
                app,
                &format!("/annotations/media/{media}/page?origin=all"),
                true,
            )
            .await,
        ),
        (
            "captions.vtt".to_string(),
            body_of(
                app,
                &format!("/annotations/media/{media}/captions.vtt?lang=en"),
                true,
            )
            .await,
        ),
        (
            "the public media page".to_string(),
            body_of(app, &format!("/s/museum/media/{media}"), false).await,
        ),
        (
            "content search".to_string(),
            body_of(app, &format!("/iiif/search/2/item/{item}?q={query}"), false).await,
        ),
    ]
}

/// A recording on the seeded item: a time surface, owned by the seeded user.
async fn add_video(pool: &sqlx::SqlitePool, seed: &Seed) -> MediaId {
    MediaRepo::new(pool)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: Some(seed.user),
                title: Some("A recording".to_string()),
                is_public: true,
                ..NewResource::default()
            },
            item_id: ItemId(seed.item.0),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            data: None,
            source: Some("clip.mp4".to_string()),
            media_type: Some("video/mp4".to_string()),
            storage_id: Some("mp4-missing".to_string()),
            extension: Some("mp4".to_string()),
            sha256: Some("missing".to_string()),
            size: Some(1),
            has_original: true,
            has_thumbnails: false,
            position: None,
            lang: None,
            alt_text: None,
        })
        .await
        .expect("create a media row")
        .id()
}

#[tokio::test]
async fn a_replace_keeps_the_edition_where_no_reader_serves_it_and_an_undo_restores_it() {
    let (pool, seed) = site_pool().await;
    let (app, _dir) = app_with_store(&pool);
    let media = add_video(&pool, &seed).await.0;
    let item = seed.item.0;

    let first = import(&app, media, "en", RED).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.body);
    let ids = header(&first, "romeka-track-ids")
        .expect("the track ids of the first import")
        .to_string();
    let imported = list(&app, media).await[0]["imported"]
        .as_str()
        .expect("the first import's time")
        .to_string();
    assert_eq!(
        list(&app, media).await[0]["undo"],
        Value::Null,
        "a first import replaces nothing, so there is nothing to undo"
    );

    let replaced = import(&app, media, "en", BLUE).await;
    assert_eq!(replaced.status, StatusCode::OK, "{}", replaced.body);
    assert_eq!(replaced.json()["replaced"], true);

    let listed = list(&app, media).await;
    assert_eq!(listed[0]["cues"], 1);
    assert_eq!(
        listed[0]["undo"],
        json!({"cues": 1, "imported": imported}),
        "the row names the edition an undo brings back, with its import time"
    );

    // The kept words are not in anything a reader reads.
    for (reader, body) in readers(&app, media, item, "red").await {
        assert!(
            !body.contains("A red square"),
            "the kept edition reached {reader}: {body}"
        );
    }
    let live = readers(&app, media, item, "blue").await;
    assert!(
        live[4].1.contains("A blue square"),
        "the live cue is what content search finds: {}",
        live[4].1
    );
    assert!(
        live[3].1.contains("A blue square"),
        "the site page shows the live cue: {}",
        live[3].1
    );

    let undone = undo(&app, media, "en").await;
    assert_eq!(undone.status, StatusCode::OK, "{}", undone.body);
    assert_eq!(
        undone.json(),
        json!({"lang": "en", "cues": 1, "imported": imported}),
        "the answer is the edition now live, and when it was imported"
    );

    // Every reader reads the brought-back words again, and the newer words are
    // gone from all of them.
    let restored = readers(&app, media, item, "red").await;
    for (reader, body) in &restored {
        if reader == "content search" {
            continue;
        }
        assert!(
            body.contains("A red square"),
            "{reader} does not show the edition the undo restored: {body}"
        );
        assert!(
            !body.contains("A blue square"),
            "{reader} still shows the replaced edition: {body}"
        );
    }
    assert!(
        restored[4].1.contains("A red square"),
        "content search reads the restored cue: {}",
        restored[4].1
    );
    let cleared = body_of(&app, &format!("/iiif/search/2/item/{item}?q=blue"), false).await;
    assert!(
        !cleared.contains("A blue square"),
        "the replaced cue is still indexed: {cleared}"
    );

    let listed = list(&app, media).await;
    assert_eq!(listed[0]["undo"], Value::Null, "an undo works once");
    assert_eq!(
        listed[0]["imported"], imported,
        "the edition keeps the time it was imported"
    );
    let container = body_of(
        &app,
        &format!("/annotations/media/{media}/?origin=all"),
        true,
    )
    .await;
    assert!(
        container.contains(&ids),
        "the cue ids came back with the edition, so an open editor still names it: {container}"
    );
}

#[tokio::test]
async fn an_undo_brings_back_one_step_and_then_has_nothing_left() {
    let (pool, seed) = site_pool().await;
    let (app, _dir) = app_with_store(&pool);
    let media = add_video(&pool, &seed).await.0;

    import(&app, media, "en", RED).await;
    import(&app, media, "en", BLUE).await;
    import(&app, media, "en", GREEN).await;
    let listed = list(&app, media).await;
    assert_eq!(
        listed[0]["undo"]["cues"], 1,
        "the kept edition is the one the last import replaced"
    );

    let undone = undo(&app, media, "en").await;
    assert_eq!(undone.status, StatusCode::OK, "{}", undone.body);
    let captions = body_of(
        &app,
        &format!("/annotations/media/{media}/captions.vtt?lang=en"),
        true,
    )
    .await;
    assert!(
        captions.contains("A blue square"),
        "the step back is the middle edition: {captions}"
    );

    let again = undo(&app, media, "en").await;
    assert_eq!(again.status, StatusCode::CONFLICT, "{}", again.body);
    assert_eq!(again.json()["errors"]["error"], NOTHING_TO_UNDO);
}

#[tokio::test]
async fn an_import_that_changes_nothing_keeps_the_undo_it_had() {
    let (pool, seed) = site_pool().await;
    let (app, _dir) = app_with_store(&pool);
    let media = add_video(&pool, &seed).await.0;

    import(&app, media, "en", RED).await;
    import(&app, media, "en", BLUE).await;
    let unchanged = import(&app, media, "en", BLUE).await;
    assert_eq!(unchanged.json()["replaced"], false);
    assert_eq!(
        list(&app, media).await[0]["undo"]["cues"],
        1,
        "a file that changed nothing did not replace an edition"
    );

    assert_eq!(
        undo(&app, media, "en").await.status,
        StatusCode::OK,
        "the step back is still there"
    );
}

#[tokio::test]
async fn an_undo_with_no_kept_edition_is_a_conflict() {
    let (pool, seed) = site_pool().await;
    let (app, _dir) = app_with_store(&pool);
    let media = add_video(&pool, &seed).await.0;

    let answer = undo(&app, media, "en").await;
    assert_eq!(answer.status, StatusCode::CONFLICT, "{}", answer.body);
    assert_eq!(answer.json()["errors"]["error"], NOTHING_TO_UNDO);

    import(&app, media, "en", RED).await;
    let after_first = undo(&app, media, "en").await;
    assert_eq!(
        after_first.status,
        StatusCode::CONFLICT,
        "a first import replaced no edition: {}",
        after_first.body
    );
}

#[tokio::test]
async fn a_removal_drops_the_kept_edition_with_the_language() {
    let (pool, seed) = site_pool().await;
    let (app, _dir) = app_with_store(&pool);
    let media = add_video(&pool, &seed).await.0;

    import(&app, media, "en", RED).await;
    import(&app, media, "en", BLUE).await;
    assert_eq!(list(&app, media).await[0]["undo"]["cues"], 1);

    let removed = send(
        app.clone(),
        Request::builder()
            .method("DELETE")
            .uri(append_key(&tracks_path(media, "en")))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT);
    assert!(list(&app, media).await.is_empty(), "the language is gone");
    let answer = undo(&app, media, "en").await;
    assert_eq!(answer.status, StatusCode::CONFLICT, "{}", answer.body);
}

#[tokio::test]
async fn a_kept_edition_is_not_a_served_file() {
    let (pool, seed) = site_pool().await;
    let (app, _dir) = app_with_store(&pool);
    let media = add_video(&pool, &seed).await.0;
    import(&app, media, "en", RED).await;
    import(&app, media, "en", BLUE).await;

    let answer = send(
        app,
        Request::builder()
            .uri("/api/romeka/files/allow")
            .header(
                "x-forwarded-uri",
                format!("/files/caption-undo/{media}/en.json"),
            )
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(
        answer.status,
        StatusCode::FORBIDDEN,
        "the proxy never serves a kept edition: {}",
        answer.body
    );
}

#[tokio::test]
async fn only_a_reader_with_the_write_grant_undoes() {
    let (pool, seed) = site_pool().await;
    let (app, _dir) = app_with_store(&pool);
    let media = add_video(&pool, &seed).await.0;
    import(&app, media, "en", RED).await;
    import(&app, media, "en", BLUE).await;

    let anonymous = send(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/annotations/media/{media}/tracks/undo?lang=en"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(
        anonymous.status,
        StatusCode::FORBIDDEN,
        "{}",
        anonymous.body
    );
    assert_eq!(
        list(&app, media).await[0]["undo"]["cues"],
        1,
        "the refused call changed nothing"
    );
}
