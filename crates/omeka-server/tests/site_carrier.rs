//! The Romeka carrier at the API and the public page: a write regenerates
//! `o:data.html` from the block's plain renderer and records `plain_hash`; a
//! stored hash that no longer matches is an outside edit and is kept.
//!
//! `docs/compat.md`, "Site pages: the one-way rule", is the rule under test.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{append_key, get, json_request, router, seeded_pool, send};
use omeka_db::ids::SiteId;
use omeka_db::repos::{AssetRepo, SiteRepo};
use omeka_db::rows::{NewAsset, NewSitePage};
use omeka_site::blocks::romeka::{byte_hash, plain_hash};
use serde_json::{Value, json};

/// One `o:block` for a hero carrier. `html` and `hash` are what the client
/// sends as the stored plain rendering; `None` stands for absent.
fn hero_body(site: u64, settings: Value, html: Option<&str>, hash: Option<&str>) -> String {
    let mut romeka = json!({ "block": "hero", "v": 1, "settings": settings });
    if let Some(hash) = hash {
        romeka["plain_hash"] = Value::String(hash.to_string());
    }
    let mut data = json!({});
    if let Some(html) = html {
        data["html"] = Value::String(html.to_string());
    }
    json!({
        "o:site": { "o:id": site },
        "o:title": "Hero page",
        "o:slug": "hero",
        "o:block": [
            { "o:layout": "html", "o:data": data, "o:layout_data": { "romeka": romeka } }
        ],
    })
    .to_string()
}

/// A public page in the seeded site, so the carrier can reference it.
async fn add_page(pool: &sqlx::SqlitePool, site: u64, slug: &str) -> u64 {
    SiteRepo::new(pool)
        .add_page(&NewSitePage {
            site_id: SiteId::from(site),
            slug: slug.to_string(),
            title: slug.to_string(),
            is_public: true,
            ..NewSitePage::default()
        })
        .await
        .unwrap()
        .id
        .0
}

/// An image asset, so the hero's background resolves to a file URL.
async fn add_asset(pool: &sqlx::SqlitePool) -> u64 {
    AssetRepo::new(pool)
        .create(&NewAsset {
            owner_id: None,
            name: "A shelf".to_string(),
            media_type: "image/png".to_string(),
            storage_id: "abc123".to_string(),
            extension: Some("png".to_string()),
            alt_text: Some("A shelf of books".to_string()),
        })
        .await
        .unwrap()
        .id
        .0
}

/// A write with no `plain_hash` regenerates the stored html from the block's
/// plain renderer and records the hash of what it stored.
#[tokio::test]
async fn a_write_regenerates_the_plain_rendering_and_sets_the_hash() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let body = hero_body(
        seed.site,
        json!({ "heading": "Welcome", "subheading": "A library", "body": "<p>Read <em>more</em>.</p>" }),
        None,
        None,
    );
    let created = send(
        app.clone(),
        json_request("POST", &append_key("/api/site_pages"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let document = created.json();
    let block = &document["o:block"][0];
    let stored = block["o:data"]["html"].as_str().unwrap();
    assert!(
        stored.contains("<h2>Welcome</h2>"),
        "plain heading missing: {stored}"
    );
    assert!(
        stored.contains("<p>A library</p>"),
        "plain subheading missing: {stored}"
    );
    assert!(
        stored.contains("<p>Read <em>more</em>.</p>"),
        "plain body missing: {stored}"
    );
    let recorded = block["o:layout_data"]["romeka"]["plain_hash"]
        .as_str()
        .unwrap();
    assert_eq!(
        recorded,
        plain_hash(stored),
        "the hash is of the stored html"
    );
    assert_eq!(block["o:layout_data"]["romeka"]["block"], "hero");
    assert_eq!(
        block["o:layout_data"]["romeka"]["v"], 2,
        "the write records the current carrier version"
    );
}

/// A carrier version 1 hash was the raw-byte hash. It vouches for its html, so
/// the write is not an outside edit, and the write migrates the carrier to the
/// current version with the canonical hash.
#[tokio::test]
async fn a_version_one_byte_hash_migrates_on_the_next_save() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let clean = send(
        app.clone(),
        json_request(
            "POST",
            &append_key("/api/site_pages"),
            &hero_body(seed.site, json!({ "heading": "Welcome" }), None, None),
        ),
    )
    .await;
    let page_id = clean.json()["o:id"].as_u64().unwrap();
    let stored = clean.json()["o:block"][0]["o:data"]["html"]
        .as_str()
        .unwrap()
        .to_string();

    let v1 = hero_body(
        seed.site,
        json!({ "heading": "Welcome" }),
        Some(&stored),
        Some(&byte_hash(&stored)),
    );
    let mut v1 = serde_json::from_str::<Value>(&v1).unwrap();
    v1["o:block"][0]["o:layout_data"]["romeka"]["v"] = json!(1);
    let migrated = send(
        app,
        json_request(
            "PUT",
            &append_key(&format!("/api/site_pages/{page_id}")),
            &v1.to_string(),
        ),
    )
    .await;

    assert_eq!(migrated.status, StatusCode::OK, "{}", migrated.body);
    let block = &migrated.json()["o:block"][0];
    assert_eq!(block["o:layout_data"]["romeka"]["v"], 2);
    assert_eq!(
        block["o:layout_data"]["romeka"]["plain_hash"],
        plain_hash(block["o:data"]["html"].as_str().unwrap())
    );
}

/// A stored hash that disagrees with the stored html is an outside edit: the
/// write keeps both the html and the hash.
#[tokio::test]
async fn a_write_keeps_an_outside_edit_and_its_old_hash() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let clean = send(
        app.clone(),
        json_request(
            "POST",
            &append_key("/api/site_pages"),
            &hero_body(seed.site, json!({ "heading": "Welcome" }), None, None),
        ),
    )
    .await;
    let page_id = clean.json()["o:id"].as_u64().unwrap();
    let first_hash = clean.json()["o:block"][0]["o:layout_data"]["romeka"]["plain_hash"]
        .as_str()
        .unwrap()
        .to_string();

    let edited = send(
        app.clone(),
        json_request(
            "PUT",
            &append_key(&format!("/api/site_pages/{page_id}")),
            &hero_body(
                seed.site,
                json!({ "heading": "Welcome" }),
                Some("<p>edited outside</p>"),
                Some(&first_hash),
            ),
        ),
    )
    .await;
    assert_eq!(edited.status, StatusCode::OK, "{}", edited.body);
    let block = &edited.json()["o:block"][0];
    assert_eq!(block["o:data"]["html"], "<p>edited outside</p>");
    assert_eq!(block["o:layout_data"]["romeka"]["plain_hash"], first_hash);
}

/// Clearing the hash (the editor's "Use the Romeka version") makes the next
/// write regenerate the plain rendering and re-set the hash.
#[tokio::test]
async fn clearing_the_hash_restores_the_romeka_plain_rendering() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let body = hero_body(seed.site, json!({ "heading": "Welcome" }), None, None);
    let created = send(
        app.clone(),
        json_request("POST", &append_key("/api/site_pages"), &body),
    )
    .await;
    let page_id = created.json()["o:id"].as_u64().unwrap();
    let edited = hero_body(
        seed.site,
        json!({ "heading": "Welcome" }),
        Some("<p>edited outside</p>"),
        Some("0000000000000000"),
    );
    let kept = send(
        app.clone(),
        json_request(
            "PUT",
            &append_key(&format!("/api/site_pages/{page_id}")),
            &edited,
        ),
    )
    .await;
    let stored_edited = kept.json()["o:block"][0]["o:data"]["html"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(stored_edited, "<p>edited outside</p>");

    let restored = send(
        app.clone(),
        json_request(
            "PUT",
            &append_key(&format!("/api/site_pages/{page_id}")),
            &hero_body(
                seed.site,
                json!({ "heading": "Hello again" }),
                Some("<p>edited outside</p>"),
                None,
            ),
        ),
    )
    .await;
    let block = &restored.json()["o:block"][0];
    let stored = block["o:data"]["html"].as_str().unwrap();
    assert!(
        stored.contains("<h2>Hello again</h2>"),
        "the plain rendering was not regenerated: {stored}"
    );
    assert_eq!(
        block["o:layout_data"]["romeka"]["plain_hash"],
        plain_hash(stored)
    );
}

/// An `html` block with no carrier is stored exactly as before: the write does
/// not touch it.
#[tokio::test]
async fn an_html_block_without_a_carrier_is_untouched() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let body = json!({
        "o:site": { "o:id": seed.site },
        "o:title": "Plain page",
        "o:slug": "plain",
        "o:block": [
            { "o:layout": "html", "o:data": { "html": "<p>hand written</p>" } }
        ],
    })
    .to_string();
    let created = send(
        app,
        json_request("POST", &append_key("/api/site_pages"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    assert_eq!(
        created.json()["o:block"][0]["o:data"]["html"],
        "<p>hand written</p>"
    );
}

/// The plain rendering resolves the background asset to a relative file URL and
/// the call to action page to the site's page URL.
#[tokio::test]
async fn the_plain_rendering_resolves_the_asset_and_the_page() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let asset = add_asset(&pool).await;
    let about = add_page(&pool, seed.site, "about").await;
    let body = hero_body(
        seed.site,
        json!({
            "heading": "Welcome",
            "background": asset,
            "cta_label": "Read on",
            "cta_page": about,
        }),
        None,
        None,
    );
    let created = send(
        app,
        json_request("POST", &append_key("/api/site_pages"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let stored = created.json()["o:block"][0]["o:data"]["html"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        stored.contains("<img src=\"/files/asset/abc123.png\" alt=\"A shelf of books\">"),
        "the background asset did not resolve: {stored}"
    );
    assert!(
        stored.contains("<a href=\"/s/museum/page/about\""),
        "the call to action page did not resolve: {stored}"
    );
}

/// A saved carrier whose hash still matches renders the rich banner on the
/// public page, with the background image and the call to action.
#[tokio::test]
async fn a_clean_carrier_renders_the_rich_banner() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let asset = add_asset(&pool).await;
    let body = hero_body(
        seed.site,
        json!({
            "heading": "Welcome",
            "subheading": "A library",
            "background": asset,
            "overlay": "strong",
            "alignment": "center",
            "cta_label": "Browse",
            "cta_url": "https://example.org/browse",
        }),
        None,
        None,
    );
    let created = send(
        app.clone(),
        json_request("POST", &append_key("/api/site_pages"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);

    let page = send(app.clone(), get("/s/museum/page/hero", false)).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(
        page.body
            .contains("class=\"romeka-hero romeka-hero-align-center romeka-hero-overlay-strong\""),
        "the rich banner did not render: {}",
        page.body
    );
    assert!(
        page.body
            .contains("<h2 class=\"romeka-hero-heading\">Welcome</h2>"),
        "{}",
        page.body
    );
    assert!(
        page.body.contains("class=\"romeka-hero-background\""),
        "the background image did not render: {}",
        page.body
    );
    assert!(
        page.body.contains(
            "<a href=\"https://example.org/browse\" class=\"romeka-hero-cta\">Browse</a>"
        ),
        "{}",
        page.body
    );
}

/// A carrier whose hash disagrees renders the stored html, not the banner.
#[tokio::test]
async fn an_edited_carrier_renders_the_edited_html() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let body = hero_body(
        seed.site,
        json!({ "heading": "Welcome" }),
        Some("<p>edited outside</p>"),
        Some("0000000000000000"),
    );
    let created = send(
        app.clone(),
        json_request("POST", &append_key("/api/site_pages"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let answer = send(app, get("/s/museum/page/hero", false)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(
        answer.body.contains("<p>edited outside</p>"),
        "the edited html did not render: {}",
        answer.body
    );
    assert!(
        !answer.body.contains("romeka-hero-heading"),
        "the rich banner rendered over the edit: {}",
        answer.body
    );
}

/// Keys the write does not know survive: a `romeka` key beyond the carrier's
/// own and any other `o:layout_data` member.
#[tokio::test]
async fn the_write_keeps_unknown_carrier_keys() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let body = json!({
        "o:site": { "o:id": seed.site },
        "o:title": "Extras",
        "o:slug": "extras",
        "o:block": [{
            "o:layout": "html",
            "o:data": { "html": "" },
            "o:layout_data": {
                "class": "hero",
                "romeka": {
                    "block": "hero",
                    "v": 1,
                    "settings": { "heading": "Welcome" },
                    "future": { "keep": true }
                }
            }
        }],
    })
    .to_string();
    let created = send(
        app,
        json_request("POST", &append_key("/api/site_pages"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let layout_data = &created.json()["o:block"][0]["o:layout_data"];
    assert_eq!(layout_data["class"], "hero");
    assert_eq!(layout_data["romeka"]["future"]["keep"], true);
    assert!(
        layout_data["romeka"]["plain_hash"].is_string(),
        "the write recorded no hash: {layout_data}"
    );
}

/// A carrier that names no registered block is an ordinary html block: the
/// write leaves its stored html alone.
#[tokio::test]
async fn a_carrier_naming_an_unknown_block_is_untouched() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let body = json!({
        "o:site": { "o:id": seed.site },
        "o:title": "Unknown",
        "o:slug": "unknown",
        "o:block": [{
            "o:layout": "html",
            "o:data": { "html": "<p>from another product</p>" },
            "o:layout_data": { "romeka": { "block": "someFutureBlock", "v": 1 } }
        }],
    })
    .to_string();
    let created = send(
        app,
        json_request("POST", &append_key("/api/site_pages"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let block = &created.json()["o:block"][0];
    assert_eq!(block["o:data"]["html"], "<p>from another product</p>");
    assert!(
        block["o:layout_data"]["romeka"]["plain_hash"].is_null(),
        "{}",
        block["o:layout_data"]
    );
}
