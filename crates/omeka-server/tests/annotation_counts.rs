//! `GET /annotations/item/{id}/counts`, read through HTTP as the admin item
//! page reads it: one answer for every file of an item, and each count equal to
//! the total the file's own WAP container states.
//!
//! The counts are the item page's only source for a card's number and its
//! action, so these tests drive the route the way the page does: once per
//! visit, as a caller with and without the media-update grant, with the public
//! notes switch on and off.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::{Request, StatusCode};
use common::{Seed, get, header, seeded_pool, send, visibility_pool};
use omeka_api::settings::ANNOTATIONS_PUBLIC;
use omeka_db::ids::{ItemId, MediaId, UserId};
use omeka_db::repos::{Cue, MediaAnnotationRepo, MediaAnnotationWrite, MediaRepo, SettingRepo};
use omeka_db::rows::{NewMedia, NewResource};
use serde_json::{Value, json};

const ANNO: &str = "application/ld+json; profile=\"http://www.w3.org/ns/anno.jsonld\"";

fn counts_path(item: u64) -> String {
    format!("/annotations/item/{item}/counts")
}

fn container_path(media: MediaId) -> String {
    format!("/annotations/media/{}/", media.0)
}

/// The `media` member of a counts document, as `(count, writable)` by id.
fn counts_of(document: &Value) -> std::collections::HashMap<u64, (u64, bool)> {
    let media = document["media"].as_object().expect("a media member");
    media
        .iter()
        .map(|(id, entry)| {
            (
                id.parse::<u64>().expect("a numeric media id"),
                (
                    entry["count"].as_u64().expect("a count"),
                    entry["writable"].as_bool().expect("a writable flag"),
                ),
            )
        })
        .collect()
}

/// One media on the seeded item, written as the schema's own row.
async fn add_media(pool: &sqlx::SqlitePool, seed: &Seed, title: &str, position: i64) -> MediaId {
    MediaRepo::new(pool)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: Some(seed.user),
                title: Some(title.to_string()),
                is_public: true,
                ..NewResource::default()
            },
            item_id: seed.item,
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            source: Some(format!("{title}.jpg")),
            media_type: Some("image/jpeg".to_string()),
            storage_id: Some(format!("missing-{position}")),
            extension: Some("jpg".to_string()),
            sha256: Some("missing".to_string()),
            size: Some(1),
            has_original: true,
            position: Some(position),
            ..NewMedia::default()
        })
        .await
        .expect("a media row")
        .id()
}

/// One authored note, straight to the table: a count does not care how a note
/// was written, only whether its rows are visible.
async fn add_note(pool: &sqlx::SqlitePool, media: MediaId, owner: UserId, motivation: &str) {
    let timed = motivation == "supplementing";
    MediaAnnotationRepo::new(pool)
        .insert(
            media,
            Some(owner),
            None,
            &MediaAnnotationWrite {
                motivation,
                selector_kind: if timed { "time" } else { "whole" },
                selector: None,
                time_ms: timed.then_some((0, 1000)),
                body_json: r#"[{"type":"TextualBody","value":"n","format":"text/plain"}]"#,
                body_text: "n",
                lang: None,
                extensions: None,
            },
        )
        .await
        .expect("a note row");
}

/// A caption import's rows: the container lists them under `origin=all` only.
async fn add_track(pool: &sqlx::SqlitePool, media: MediaId) {
    MediaAnnotationRepo::new(pool)
        .replace_track(
            media,
            Some("en"),
            &[Cue {
                start_ms: 0,
                end_ms: 1000,
                body_json: r#"[{"type":"TextualBody","value":"A cue","format":"text/plain"}]"#,
                body_text: "A cue",
            }],
        )
        .await
        .expect("a caption row");
}

async fn set_switch(pool: &sqlx::SqlitePool, on: bool) {
    SettingRepo::new(pool)
        .set(ANNOTATIONS_PUBLIC, &json!(on))
        .await
        .expect("the switch row");
}

/// One file's container `total`: the count the file's own page states.
async fn container_total(app: &Router, uri: &str) -> u64 {
    let answer = send(
        app.clone(),
        Request::builder()
            .uri(uri)
            .header("accept", ANNO)
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    answer.json()["total"].as_u64().expect("a total")
}

#[tokio::test]
async fn the_item_counts_answer_every_file_in_one_read() {
    let (pool, seed) = seeded_pool().await;
    let second = add_media(&pool, &seed, "Second", 2).await;
    let third = add_media(&pool, &seed, "Third", 3).await;
    add_note(&pool, seed.media, seed.user, "commenting").await;
    add_note(&pool, seed.media, seed.user, "commenting").await;
    add_note(&pool, second, seed.user, "commenting").await;
    add_track(&pool, second).await;
    let app = common::router(pool);

    let answer = send(app.clone(), get(&counts_path(seed.item.0), true)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(header(&answer, "content-type"), Some("application/json"));
    assert_eq!(
        header(&answer, "cache-control"),
        Some("no-store"),
        "a count follows every write"
    );
    let counts = counts_of(&answer.json());
    assert_eq!(counts.len(), 3, "every file of the item, in one answer");
    assert_eq!(counts.get(&seed.media.0), Some(&(2, true)));
    assert_eq!(
        counts.get(&second.0),
        Some(&(1, true)),
        "the imported cue is not a note"
    );
    assert_eq!(counts.get(&third.0), Some(&(0, true)));

    // The contract: each count is what that file's container states.
    for media in [seed.media, second, third] {
        let total = container_total(&app, &container_path(media)).await;
        assert_eq!(
            counts.get(&media.0).map(|(count, _)| *count),
            Some(total),
            "media {}",
            media.0
        );
    }

    // An item with no file the caller may read names nothing, without a 404:
    // the media decide, exactly as they do for a container.
    let empty = send(app, get(&counts_path(999_999), true)).await;
    assert_eq!(empty.status, StatusCode::OK);
    assert_eq!(empty.json(), json!({ "media": {} }));
}

#[tokio::test]
async fn a_reader_counts_what_the_containers_show_and_the_switch_hides_the_rest() {
    let (pool, seed) = visibility_pool().await;
    let item = ItemId(seed.item.0);
    add_note(&pool, seed.media_public, seed.owner, "commenting").await;
    add_note(&pool, seed.media_public, seed.owner, "commenting").await;
    add_note(&pool, seed.media_public, seed.owner, "supplementing").await;
    add_note(&pool, seed.media_private, seed.owner, "commenting").await;
    let app = common::router(pool.clone());
    let reader = format!(
        "{}?key_identity={}&key_credential={}",
        counts_path(item.0),
        common::VIEWER_KEY_IDENTITY,
        common::VIEWER_KEY_CREDENTIAL
    );

    let answer = send(app.clone(), get(&reader, false)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let counts = counts_of(&answer.json());
    assert_eq!(
        counts.get(&seed.media_public.0),
        Some(&(3, false)),
        "a researcher reads the notes and may not write them"
    );
    assert_eq!(
        counts.get(&seed.media_private.0),
        None,
        "a private file is absent, as its container is a 404"
    );
    let public_total = container_total(
        &app,
        &format!(
            "{}?key_identity={}&key_credential={}",
            container_path(seed.media_public),
            common::VIEWER_KEY_IDENTITY,
            common::VIEWER_KEY_CREDENTIAL
        ),
    )
    .await;
    assert_eq!(counts.get(&seed.media_public.0).unwrap().0, public_total);

    set_switch(&pool, false).await;
    let hidden = send(app.clone(), get(&reader, false)).await;
    assert_eq!(hidden.status, StatusCode::OK, "{}", hidden.body);
    assert_eq!(
        counts_of(&hidden.json()).get(&seed.media_public.0),
        Some(&(1, false)),
        "the switch keeps the hand-written caption and hides the two comments"
    );
    let hidden_total = container_total(
        &app,
        &format!(
            "{}?key_identity={}&key_credential={}",
            container_path(seed.media_public),
            common::VIEWER_KEY_IDENTITY,
            common::VIEWER_KEY_CREDENTIAL
        ),
    )
    .await;
    assert_eq!(
        counts_of(&hidden.json())
            .get(&seed.media_public.0)
            .unwrap()
            .0,
        hidden_total
    );

    // The owner may write, and the switch does not hide their own notes.
    let owner = format!(
        "{}?key_identity={}&key_credential={}",
        counts_path(item.0),
        common::OWNER_KEY_IDENTITY,
        common::OWNER_KEY_CREDENTIAL
    );
    let own = send(app, get(&owner, false)).await;
    assert_eq!(own.status, StatusCode::OK, "{}", own.body);
    assert_eq!(
        counts_of(&own.json()).get(&seed.media_public.0),
        Some(&(3, true))
    );
}
