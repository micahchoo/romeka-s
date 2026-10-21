//! `GET /api/romeka/media/{id}/render` and `/player`: the server's render of
//! one embed as a frame document, and the provider player URL that render is.
//!
//! The routes exist so the admin stage can show an oEmbed or HTML media the way
//! the public site shows it, in one sandboxed frame, without deciding for
//! itself what provider markup is safe, and so the stage can frame a provider's
//! own player directly instead of nesting it in a frame of ours. Every
//! assertion here is what a caller of a route observes: the status, the
//! document or URL, and the headers that make the answer a frame.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::{HeaderMap, StatusCode};
use common::{
    KEY_CREDENTIAL, KEY_IDENTITY, Seed, get, get_keyed, router, seeded_engine_pool, send,
};
use omeka_db::DbPool;
use omeka_db::repos::MediaRepo;
use omeka_db::rows::{NewMedia, NewResource};
use omeka_db::with_db;
use serde_json::{Value, json};

/// The fields a case varies. Everything else is a stored row on the seeded
/// public item, so only the case's own field differs between the answers.
struct Fields {
    ingester: &'static str,
    renderer: &'static str,
    is_public: bool,
    source: Option<&'static str>,
    media_type: Option<&'static str>,
    data: Option<Value>,
}

impl Fields {
    fn plain(ingester: &'static str, renderer: &'static str) -> Self {
        Self {
            ingester,
            renderer,
            is_public: true,
            source: None,
            media_type: None,
            data: None,
        }
    }
}

async fn store(pool: &DbPool, seed: &Seed, fields: Fields) -> u64 {
    let row = with_db!(pool, |db| MediaRepo::new(db)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: Some(seed.user),
                resource_class_id: None,
                resource_template_id: None,
                thumbnail_id: None,
                title: None,
                is_public: fields.is_public,
                created: None,
            },
            item_id: seed.item,
            ingester: fields.ingester.to_string(),
            renderer: fields.renderer.to_string(),
            data: fields.data,
            source: fields.source.map(str::to_string),
            media_type: fields.media_type.map(str::to_string),
            storage_id: None,
            extension: None,
            sha256: None,
            size: None,
            has_original: false,
            has_thumbnails: false,
            position: Some(9),
            lang: None,
            alt_text: None,
        })
        .await)
    .unwrap();
    row.id().0
}

fn header(headers: &HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

/// A public oEmbed media answers the provider's frame as a whole document, and
/// the policy allows that frame's host and nothing wider.
#[tokio::test]
async fn an_oembed_media_answers_the_providers_frame() {
    let (pool, seed) = seeded_engine_pool().await;
    let media = store(
        &pool,
        &seed,
        Fields {
            source: Some("https://www.youtube.com/watch?v=abc123"),
            data: Some(json!({
                "type": "video",
                "title": "A talk",
                "html": "<iframe src=\"https://www.youtube.com/embed/abc123\"></iframe>",
            })),
            ..Fields::plain("oembed", "oembed")
        },
    )
    .await;

    let answer = send(
        router(pool),
        get(&format!("/api/romeka/media/{media}/render"), false),
    )
    .await;

    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(
        header(&answer.headers, "content-type"),
        "text/html; charset=utf-8"
    );
    assert!(
        answer.body.starts_with("<!doctype html>"),
        "{}",
        answer.body
    );
    assert!(
        answer.body.trim_end().ends_with("</html>"),
        "{}",
        answer.body
    );
    assert!(
        answer.body.contains(
            "<iframe src=\"https://www.youtube.com/embed/abc123\" sandbox=\"allow-scripts \
             allow-same-origin allow-popups allow-presentation\"></iframe>"
        ),
        "{}",
        answer.body
    );
    let policy = header(&answer.headers, "content-security-policy");
    assert!(policy.contains("default-src 'none'"), "{policy}");
    assert!(policy.contains("script-src 'none'"), "{policy}");
    assert!(
        policy.contains("frame-src https://www.youtube.com"),
        "{policy}"
    );
    assert!(policy.contains("frame-ancestors 'self'"), "{policy}");
    assert!(!policy.contains('*'), "{policy}");
    assert_eq!(header(&answer.headers, "x-frame-options"), "SAMEORIGIN");
    assert!(header(&answer.headers, "vary").contains("Cookie"));
    assert!(
        answer.headers.get("set-cookie").is_none(),
        "the frame document sets a cookie"
    );
}

/// A one-frame oEmbed render names the provider player URL, which the stage
/// frames directly, and the answer is private and carries no cookie.
#[tokio::test]
async fn a_one_frame_oembed_names_the_provider_player() {
    let (pool, seed) = seeded_engine_pool().await;
    let media = store(
        &pool,
        &seed,
        Fields {
            source: Some("https://www.youtube.com/watch?v=abc123"),
            data: Some(json!({
                "type": "video",
                "title": "A talk",
                "html": "<iframe src=\"https://www.youtube.com/embed/abc123?feature=oembed\" allowfullscreen></iframe>",
            })),
            ..Fields::plain("oembed", "oembed")
        },
    )
    .await;

    let answer = send(
        router(pool),
        get(&format!("/api/romeka/media/{media}/player"), false),
    )
    .await;

    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(header(&answer.headers, "content-type"), "application/json");
    assert_eq!(
        answer.json()["url"],
        json!("https://www.youtube.com/embed/abc123?feature=oembed")
    );
    assert!(header(&answer.headers, "cache-control").contains("private"));
    assert!(
        answer.headers.get("set-cookie").is_none(),
        "the player answer sets a cookie"
    );
}

/// A render with more than the frame, a plain `http` frame, a frame from a host
/// outside the oEmbed list, and an HTML media all name no provider player.
#[tokio::test]
async fn a_render_that_is_not_one_allow_listed_frame_names_no_player() {
    let (pool, seed) = seeded_engine_pool().await;
    let cases = [
        (
            "a caption beside the frame",
            "oembed",
            Some("https://www.youtube.com/watch?v=abc123"),
            json!({"html": "<p>Watch</p><iframe src=\"https://www.youtube.com/embed/abc123\"></iframe>"}),
        ),
        (
            "a plain http frame",
            "oembed",
            Some("https://www.youtube.com/watch?v=abc123"),
            json!({"html": "<iframe src=\"http://www.youtube.com/embed/abc123\"></iframe>"}),
        ),
        (
            "a frame from a host outside the list",
            "oembed",
            Some("https://example.org/page"),
            json!({"html": "<iframe src=\"https://example.org/embed\"></iframe>"}),
        ),
        (
            "an HTML media",
            "html",
            None,
            json!({"html": "<pre>Hello</pre>"}),
        ),
    ];
    for (what, renderer, source, data) in cases {
        let media = store(
            &pool,
            &seed,
            Fields {
                source,
                data: Some(data),
                ..Fields::plain(renderer, renderer)
            },
        )
        .await;
        let answer = send(
            router(pool.clone()),
            get(&format!("/api/romeka/media/{media}/player"), false),
        )
        .await;
        assert_eq!(answer.status, StatusCode::OK, "{what}: {}", answer.body);
        assert_eq!(answer.json()["url"], Value::Null, "{what}: {}", answer.body);
    }
}

/// An HTML media answers the stored fragment, purified at render, so a row
/// written before the write path purified it (or copied by the migrator) cannot
/// execute. The policy names the image host the fragment uses, port and all.
#[tokio::test]
async fn an_html_media_answers_purified_markup_and_stays_inert() {
    let (pool, seed) = seeded_engine_pool().await;
    let media = store(
        &pool,
        &seed,
        Fields {
            data: Some(json!({
                "html": "<p>Hello</p><script>alert(1)</script><img src=x onerror=alert(2)>\
                         <img src=\"https://cdn.example.org:8443/a.png\">",
            })),
            ..Fields::plain("html", "html")
        },
    )
    .await;

    let answer = send(
        router(pool),
        get(&format!("/api/romeka/media/{media}/render"), false),
    )
    .await;

    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(answer.body.contains("<p>Hello</p>"), "{}", answer.body);
    assert!(!answer.body.contains("<script"), "{}", answer.body);
    assert!(!answer.body.contains("onerror"), "{}", answer.body);
    let policy = header(&answer.headers, "content-security-policy");
    assert!(
        policy.contains("img-src 'self' https://cdn.example.org:8443"),
        "{policy}"
    );
    assert!(!policy.contains("frame-src"), "{policy}");
    assert!(!policy.contains('*'), "{policy}");
}

/// A private media is absent for an anonymous caller, exactly as it is from the
/// API, and present for a caller who may read it.
#[tokio::test]
async fn a_private_media_is_a_404_to_an_anonymous_caller() {
    let (pool, seed) = seeded_engine_pool().await;
    let media = store(
        &pool,
        &seed,
        Fields {
            is_public: false,
            data: Some(json!({"html": "<p>Draft</p>"})),
            ..Fields::plain("html", "html")
        },
    )
    .await;
    let uri = format!("/api/romeka/media/{media}/render");

    let anonymous = send(router(pool.clone()), get(&uri, false)).await;
    assert_eq!(
        anonymous.status,
        StatusCode::NOT_FOUND,
        "{}",
        anonymous.body
    );
    assert!(!anonymous.body.contains("Draft"), "{}", anonymous.body);

    let player = send(
        router(pool.clone()),
        get(&format!("/api/romeka/media/{media}/player"), false),
    )
    .await;
    assert_eq!(player.status, StatusCode::NOT_FOUND, "{}", player.body);

    let owner = send(router(pool), get_keyed(&uri, KEY_IDENTITY, KEY_CREDENTIAL)).await;
    assert_eq!(owner.status, StatusCode::OK, "{}", owner.body);
    assert!(owner.body.contains("<p>Draft</p>"), "{}", owner.body);
}

/// A kind that is not an embed names itself in a 415, so a caller can tell a
/// wrong media from a missing one.
#[tokio::test]
async fn an_image_media_answers_415_and_names_its_renderer() {
    let (pool, _seed) = seeded_engine_pool().await;
    let image = store(
        &pool,
        &_seed,
        Fields {
            source: Some("photo.jpg"),
            media_type: Some("image/jpeg"),
            ..Fields::plain("upload", "file")
        },
    )
    .await;

    let answer = send(
        router(pool),
        get(&format!("/api/romeka/media/{image}/render"), false),
    )
    .await;

    assert_eq!(
        answer.status,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "{}",
        answer.body
    );
    let document = answer.json();
    let message = document["errors"]["error"].as_str().unwrap_or_default();
    assert!(message.contains("oEmbed and HTML"), "{message}");
    assert!(message.contains("\"file\""), "{message}");
}

/// An id that names no media is the same 404.
#[tokio::test]
async fn an_unknown_media_is_a_404() {
    let (pool, _seed) = seeded_engine_pool().await;

    for id in ["9999", "not-a-number"] {
        let answer = send(
            router(pool.clone()),
            get(&format!("/api/romeka/media/{id}/render"), false),
        )
        .await;
        assert_eq!(
            answer.status,
            StatusCode::NOT_FOUND,
            "{id}: {}",
            answer.body
        );
    }
}
