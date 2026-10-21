//! The caption routes: import, list, remove, dry run and `captions.vtt`, read
//! through HTTP as the admin and a visitor read them.
//!
//! Expected values come from the contract (6.5) and the media UI plan (S5), not
//! from the handlers.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{Answer, Seed, append_key, get, header, router, seeded_pool, send};
use omeka_api::settings::ANNOTATIONS_PUBLIC;
use omeka_db::ids::{ItemId, MediaId};
use omeka_db::repos::{MediaRepo, SettingRepo};
use omeka_db::rows::{NewMedia, NewResource};
use serde_json::{Value, json};

const ANNO: &str = "application/ld+json; profile=\"http://www.w3.org/ns/anno.jsonld\"";
const VTT: &str = "text/vtt";
const SRT: &str = "application/x-subrip";

const ONE_CUE: &str = "WEBVTT\n\n00:00:00.000 --> 00:00:00.400\nA red square, then a blue bar.\n";
const TWO_CUES_SRT: &str = "1\n00:00:00,000 --> 00:00:00,400\nPremier plan.\n\n2\n00:00:01,000 --> 00:00:02,500\nDeuxieme plan.\n";

fn tracks_path(media: u64, lang: &str) -> String {
    format!("/annotations/media/{media}/tracks?lang={lang}")
}

fn caption_request(media: u64, lang: &str, body: &str, content_type: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(append_key(&tracks_path(media, lang)))
        .header("content-type", content_type)
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn dry_run_request(media: u64, lang: &str, body: &str, content_type: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(append_key(&format!(
            "{}&dry_run=1",
            tracks_path(media, lang)
        )))
        .header("content-type", content_type)
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn delete_request(media: u64, lang: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(append_key(&tracks_path(media, lang)))
        .body(Body::empty())
        .unwrap()
}

async fn import(app: &Router, media: u64, lang: &str, body: &str, content_type: &str) -> Answer {
    send(
        app.clone(),
        caption_request(media, lang, body, content_type),
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

/// A recording on the seeded item: a time surface, owned by the seeded user.
async fn add_video(pool: &sqlx::SqlitePool, seed: &Seed, public: bool) -> MediaId {
    add_media(
        pool,
        seed,
        public,
        "A recording",
        "video/mp4",
        "mp4",
        "video/mp4",
    )
    .await
}

async fn add_media(
    pool: &sqlx::SqlitePool,
    seed: &Seed,
    public: bool,
    title: &str,
    source: &str,
    extension: &str,
    media_type: &str,
) -> MediaId {
    MediaRepo::new(pool)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: Some(seed.user),
                title: Some(title.to_string()),
                is_public: public,
                ..NewResource::default()
            },
            item_id: ItemId(seed.item.0),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            data: None,
            source: Some(source.to_string()),
            media_type: Some(media_type.to_string()),
            storage_id: Some(format!("{extension}-missing")),
            extension: Some(extension.to_string()),
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

/// One WAP create or replace, with the seeded key pair.
fn wap_request(method: &str, path: &str, body: &str, extra: &[(&str, &str)]) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(append_key(path))
        .header("content-type", ANNO);
    for (name, value) in extra {
        builder = builder.header(*name, *value);
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

fn note(canvas: &str, motivation: &str, text: &str, selector: Option<Value>) -> String {
    let target = match selector {
        None => json!(canvas),
        Some(selector) => json!({
            "type": "SpecificResource",
            "source": { "id": canvas, "type": "Canvas" },
            "selector": selector,
        }),
    };
    json!({
        "@context": "http://www.w3.org/ns/anno.jsonld",
        "type": "Annotation",
        "motivation": motivation,
        "body": { "type": "TextualBody", "value": text, "format": "text/plain", "language": "en" },
        "target": target,
    })
    .to_string()
}

fn times(start: &str, end: &str) -> Value {
    json!({
        "type": "FragmentSelector",
        "conformsTo": "http://www.w3.org/TR/media-frags/",
        "value": format!("t={start},{end}"),
    })
}

async fn create_note(app: &Router, media: u64, body: &str) -> Answer {
    send(
        app.clone(),
        wap_request("POST", &format!("/annotations/media/{media}/"), body, &[]),
    )
    .await
}

#[tokio::test]
async fn an_import_replaces_a_language_and_the_same_file_changes_nothing() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let media = add_video(&pool, &seed, true).await.0;

    let first = import(&app, media, "en", ONE_CUE, VTT).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.body);
    assert_eq!(
        first.json(),
        json!({"lang": "en", "cues": 1, "replaced": false})
    );
    let location = format!("http://localhost/annotations/media/{media}/captions.vtt?lang=en");
    assert_eq!(header(&first, "location"), Some(location.as_str()));
    let ids: Vec<u64> = header(&first, "romeka-track-ids")
        .expect("the track ids")
        .split(' ')
        .map(|id| id.parse().unwrap())
        .collect();
    assert_eq!(ids.len(), 1);
    let read = send(app.clone(), get(&format!("/annotations/{}", ids[0]), true)).await;
    assert_eq!(header(&read, "etag"), Some("\"r1\""));

    let again = import(&app, media, "en", ONE_CUE, VTT).await;
    assert_eq!(again.status, StatusCode::OK);
    assert_eq!(again.json()["replaced"], false);
    let read = send(app.clone(), get(&format!("/annotations/{}", ids[0]), true)).await;
    assert_eq!(
        header(&read, "etag"),
        Some("\"r1\""),
        "the same file bumped a revision"
    );
}

#[tokio::test]
async fn a_changed_file_replaces_the_track_and_keeps_hand_written_notes() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let media = add_video(&pool, &seed, true).await.0;
    let canvas = format!(
        "http://localhost/iiif/3/item/{}/canvas/{media}",
        seed.item.0
    );
    let comment = create_note(&app, media, &note(&canvas, "commenting", "A note", None)).await;
    assert_eq!(comment.status, StatusCode::CREATED, "{}", comment.body);
    let hand = create_note(
        &app,
        media,
        &note(&canvas, "supplementing", "Hand cue", Some(times("1", "2"))),
    )
    .await;
    assert_eq!(hand.status, StatusCode::CREATED, "{}", hand.body);

    assert_eq!(
        import(&app, media, "en", ONE_CUE, VTT).await.status,
        StatusCode::OK
    );
    let changed = ONE_CUE.replace("A red square", "A blue square");
    let answer = import(&app, media, "en", &changed, VTT).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.json()["replaced"], true);

    let container = send(
        app.clone(),
        get(&format!("/annotations/media/{media}/?origin=all"), true),
    )
    .await;
    assert_eq!(
        container.json()["total"],
        3,
        "the commenting note, the hand-written cue and the imported cue"
    );
    let vtt = send(
        app,
        get(
            &format!("/annotations/media/{media}/captions.vtt?lang=en"),
            true,
        ),
    )
    .await;
    assert!(vtt.body.contains("A blue square"), "{}", vtt.body);
    assert!(
        vtt.body.contains("Hand cue"),
        "the hand-written supplementing note left the language"
    );
}

#[tokio::test]
async fn a_kind_without_a_timeline_refuses_captions() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let answer = import(&app, seed.media.0, "en", ONE_CUE, VTT).await;
    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        answer.json()["errors"]["error"],
        "This media has no timeline."
    );
    let preview = send(app, dry_run_request(seed.media.0, "en", ONE_CUE, VTT)).await;
    assert_eq!(preview.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_dry_run_answers_the_effect_and_changes_nothing() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let media = add_video(&pool, &seed, true).await.0;
    let canvas = format!(
        "http://localhost/iiif/3/item/{}/canvas/{media}",
        seed.item.0
    );
    let first = import(&app, media, "en", ONE_CUE, VTT).await;
    let id = header(&first, "romeka-track-ids").unwrap().to_string();
    // A person fixes the imported cue, so the next file would lose that edit.
    let replaced = send(
        app.clone(),
        wap_request(
            "PUT",
            &format!("/annotations/{id}"),
            &note(
                &canvas,
                "supplementing",
                "A red square, corrected",
                Some(times("0", "0.4")),
            ),
            &[("if-match", "\"r1\"")],
        ),
    )
    .await;
    assert_eq!(replaced.status, StatusCode::OK, "{}", replaced.body);

    let before = list(&app, media).await;
    let preview = send(app.clone(), dry_run_request(media, "en", ONE_CUE, VTT)).await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.body);
    let document = preview.json();
    assert_eq!(document["cues"], 1);
    assert_eq!(document["start"], 0);
    assert_eq!(document["end"], 400);
    assert_eq!(document["replaces"], 1);
    assert_eq!(
        document["handEdited"], 1,
        "the corrected row's words would be lost"
    );
    assert_eq!(list(&app, media).await, before, "the dry run wrote a row");
    let read = send(app, get(&format!("/annotations/{id}"), true)).await;
    assert_eq!(header(&read, "etag"), Some("\"r2\""));
}

#[tokio::test]
async fn removing_a_language_leaves_the_other_and_the_notes() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let media = add_video(&pool, &seed, true).await.0;
    let canvas = format!(
        "http://localhost/iiif/3/item/{}/canvas/{media}",
        seed.item.0
    );
    create_note(&app, media, &note(&canvas, "commenting", "A note", None)).await;
    import(&app, media, "en", ONE_CUE, VTT).await;
    import(&app, media, "fr", TWO_CUES_SRT, SRT).await;

    let listed = list(&app, media).await;
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0]["lang"], "en");
    assert_eq!(listed[0]["cues"], 1);
    assert_eq!(listed[0]["handEdited"], 0);
    assert!(listed[0]["imported"].is_string(), "{}", listed[0]);
    assert_eq!(listed[1]["lang"], "fr");
    assert_eq!(listed[1]["cues"], 2);

    let removed = send(app.clone(), delete_request(media, "en")).await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT);
    let listed = list(&app, media).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["lang"], "fr");

    let gone = send(app.clone(), delete_request(media, "en")).await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);

    let container = send(
        app,
        get(&format!("/annotations/media/{media}/?origin=all"), true),
    )
    .await;
    assert_eq!(
        container.json()["total"],
        3,
        "the note and the French track's two cues"
    );
}

#[tokio::test]
async fn captions_vtt_defaults_to_the_language_with_the_most_cues() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let media = add_video(&pool, &seed, true).await.0;
    import(&app, media, "en", ONE_CUE, VTT).await;
    import(&app, media, "fr", TWO_CUES_SRT, SRT).await;

    let answer = send(
        app.clone(),
        get(&format!("/annotations/media/{media}/captions.vtt"), true),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    let content_type = header(&answer, "content-type").unwrap();
    assert!(content_type.starts_with(VTT), "{content_type}");
    assert!(answer.body.starts_with("WEBVTT\n"), "{}", answer.body);
    assert_eq!(answer.body.matches("-->").count(), 2, "{}", answer.body);
    assert!(answer.body.contains("00:00:02.500"), "{}", answer.body);

    let english = send(
        app,
        get(
            &format!("/annotations/media/{media}/captions.vtt?lang=en"),
            true,
        ),
    )
    .await;
    assert_eq!(english.body.matches("-->").count(), 1);
    assert!(english.body.contains("A red square, then a blue bar."));
    assert!(
        english.body.contains("00:00:00.400"),
        "SRT commas render as WebVTT dots: {}",
        english.body
    );
}

#[tokio::test]
async fn with_the_switch_off_captions_stay_and_notes_go() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let media = add_video(&pool, &seed, true).await.0;
    let canvas = format!(
        "http://localhost/iiif/3/item/{}/canvas/{media}",
        seed.item.0
    );
    let created = create_note(&app, media, &note(&canvas, "commenting", "A note", None)).await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    import(&app, media, "en", ONE_CUE, VTT).await;
    SettingRepo::new(&pool)
        .set(ANNOTATIONS_PUBLIC, &json!(false))
        .await
        .expect("the switch");

    let captions = send(
        app.clone(),
        get(&format!("/annotations/media/{media}/captions.vtt"), false),
    )
    .await;
    assert_eq!(captions.status, StatusCode::OK);
    assert!(captions.body.contains("A red square"), "{}", captions.body);

    let container = send(app, get(&format!("/annotations/media/{media}/"), false)).await;
    assert_eq!(container.status, StatusCode::OK);
    assert_eq!(
        container.json()["total"],
        0,
        "the switch hid the commenting note"
    );
}

#[tokio::test]
async fn a_private_medias_captions_are_not_found_to_a_visitor() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let media = add_video(&pool, &seed, false).await.0;
    let imported = import(&app, media, "en", ONE_CUE, VTT).await;
    assert_eq!(imported.status, StatusCode::OK, "{}", imported.body);

    let answer = send(
        app,
        get(&format!("/annotations/media/{media}/captions.vtt"), false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_bad_cue_file_names_its_line_and_a_big_one_is_413() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let media = add_video(&pool, &seed, true).await.0;

    let not_vtt = import(
        &app,
        media,
        "en",
        "1\n00:00:00,000 --> 00:00:00,400\nx\n",
        VTT,
    )
    .await;
    assert_eq!(not_vtt.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        not_vtt.json()["errors"]["error"],
        "The file does not begin with WEBVTT."
    );

    let bad_line = import(
        &app,
        media,
        "en",
        "WEBVTT\n\n00:00:00.000 --> 00:00:00.400\nOne\n\nnonsense\n",
        VTT,
    )
    .await;
    assert_eq!(bad_line.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        bad_line.json()["errors"]["error"],
        "Line 6 is not a cue time."
    );

    let huge = "x".repeat(1024 * 1024 + 1);
    let too_large = import(&app, media, "en", &huge, VTT).await;
    assert_eq!(too_large.status, StatusCode::PAYLOAD_TOO_LARGE);

    let mut many = String::from("WEBVTT\n\n");
    for index in 0..=5000 {
        many.push_str(&format!(
            "00:00:{:02}.000 --> 00:00:{:02}.500\nCue {index}\n\n",
            index % 60,
            index % 60
        ));
    }
    let too_many = import(&app, media, "en", &many, VTT).await;
    assert_eq!(too_many.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        too_many.json()["errors"]["error"],
        "A caption file may hold at most 5,000 cues."
    );

    let wrong_type = import(&app, media, "en", ONE_CUE, "text/plain").await;
    assert_eq!(wrong_type.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn only_a_reader_with_the_write_grant_imports() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let media = add_video(&pool, &seed, true).await.0;
    import(&app, media, "en", ONE_CUE, VTT).await;

    // The caller may read the media but holds no update grant, so the write is
    // a 403 rather than the 404 a private media gives.
    let anonymous = Request::builder()
        .method("POST")
        .uri(format!("/annotations/media/{media}/tracks?lang=en"))
        .header("content-type", VTT)
        .body(Body::from(ONE_CUE.to_string()))
        .unwrap();
    let answer = send(app.clone(), anonymous).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);

    let listed = send(
        app,
        get(&format!("/annotations/media/{media}/tracks"), true),
    )
    .await;
    assert_eq!(listed.status, StatusCode::OK, "a reader lists languages");
}
