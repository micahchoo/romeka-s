//! A block setting that names a media of the site (`mediaRef`): the hero's
//! background and the IIIF image's source.
//!
//! The write resolves the media into the stored plain rendering. What it
//! stores is public, so a private media is left out of it, and the save still
//! succeeds; the public page renders the media's image only for a visitor who
//! may see it.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{append_key, get, json_request, router, seeded_pool, send};
use omeka_db::ids::{ItemId, MediaId, UserId};
use omeka_db::repos::MediaRepo;
use omeka_db::rows::{NewMedia, NewResource};
use serde_json::{Value, json};

/// The storage id of the seeded public media, which has renditions.
const SEEDED_STORAGE: &str = "4b1e5b9e0f9f4f5a8f6b2c3d4e5f6a7b8c9d0e1f";

/// A page with one block, `layout` with `data` and the Romeka `carrier`.
fn page_body(site: u64, slug: &str, layout: &str, data: Value, carrier: Value) -> String {
    json!({
        "o:site": { "o:id": site },
        "o:title": slug,
        "o:slug": slug,
        "o:block": [
            { "o:layout": layout, "o:data": data, "o:layout_data": { "romeka": carrier } }
        ],
    })
    .to_string()
}

/// A hero whose background is the site media `media`.
fn hero_with_media(site: u64, media: u64) -> String {
    page_body(
        site,
        "media-hero",
        "html",
        json!({}),
        json!({
            "block": "hero",
            "v": 2,
            "settings": {
                "heading": "Maps",
                "background_source": "media",
                "background_media": media,
                "focal_x": 30,
                "focal_y": 70
            }
        }),
    )
}

/// A media of `item`, public or private, with renditions.
async fn add_media(pool: &sqlx::SqlitePool, item: ItemId, owner: UserId, is_public: bool) -> u64 {
    MediaRepo::new(pool)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: Some(owner),
                title: Some("A private scan".to_string()),
                is_public,
                ..NewResource::default()
            },
            item_id: item,
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            data: None,
            source: Some("scan.jpg".to_string()),
            media_type: Some("image/jpeg".to_string()),
            storage_id: Some("privatescan".to_string()),
            extension: Some("jpg".to_string()),
            sha256: Some("privatescan".to_string()),
            size: Some(1),
            has_original: true,
            has_thumbnails: true,
            position: None,
            lang: None,
            alt_text: Some("A scan".to_string()),
        })
        .await
        .unwrap()
        .id()
        .0
}

/// A hero with a public site media saves its large rendition into the plain
/// rendering, and the public page shows that image at the focal point.
#[tokio::test]
async fn a_hero_with_site_media_stores_and_renders_its_image() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let created = send(
        app.clone(),
        json_request(
            "POST",
            &append_key("/api/site_pages"),
            &hero_with_media(seed.site, seed.media.0),
        ),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let stored = created.json()["o:block"][0]["o:data"]["html"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        stored,
        format!("<h2>Maps</h2><img src=\"/files/large/{SEEDED_STORAGE}.jpg\" alt=\"\">")
    );

    let page = send(app, get("/s/museum/page/media-hero", false)).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(
        page.body.contains(&format!(
            "<img class=\"romeka-hero-background\" src=\"http://localhost/files/large/{SEEDED_STORAGE}.jpg\" \
             alt=\"\" style=\"object-position: 30% 70%\">"
        )),
        "the media background did not render: {}",
        page.body
    );
}

/// A private media does not refuse the save: the plain rendering leaves its
/// image out, the reference stays in the settings, and the public page shows
/// no image.
#[tokio::test]
async fn a_private_media_saves_without_its_image() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let private = add_media(&pool, seed.item, seed.user, false).await;
    let created = send(
        app.clone(),
        json_request(
            "POST",
            &append_key("/api/site_pages"),
            &hero_with_media(seed.site, private),
        ),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let block = &created.json()["o:block"][0];
    assert_eq!(block["o:data"]["html"], "<h2>Maps</h2>");
    assert_eq!(
        block["o:layout_data"]["romeka"]["settings"]["background_media"],
        private
    );
    let page = send(app, get("/s/museum/page/media-hero", false)).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(page.body.contains("romeka-hero-no-image"), "{}", page.body);
    assert!(!page.body.contains("privatescan"), "{}", page.body);
}

/// The IIIF image with a private site media stores the empty `image_url`, on
/// which upstream renders nothing, and keeps the reference.
#[tokio::test]
async fn an_iiif_image_from_a_private_media_stores_no_address() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let private = add_media(&pool, seed.item, seed.user, false).await;
    let body = page_body(
        seed.site,
        "iiif-private",
        "iiifImage",
        json!({ "image_url": "", "title": "Scan" }),
        json!({ "block": "iiifImage", "v": 2, "settings": { "source": "media", "media": private } }),
    );
    let created = send(
        app.clone(),
        json_request("POST", &append_key("/api/site_pages"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let block = &created.json()["o:block"][0];
    assert_eq!(block["o:data"]["image_url"], "");
    assert_eq!(
        block["o:layout_data"]["romeka"]["settings"]["media"],
        private
    );
    let page = send(app, get("/s/museum/page/iiif-private", false)).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(
        !page.body.contains(&format!("/iiif/image/3/{private}/")),
        "{}",
        page.body
    );
}

/// The IIIF image with a site media source stores that media's Image API
/// address as `image_url`, so upstream opens the same image, and the public
/// page opens it through the same address on this origin.
#[tokio::test]
async fn an_iiif_image_from_site_media_stores_its_image_api_address() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let media = seed.media.0;
    let body = page_body(
        seed.site,
        "iiif-media",
        "iiifImage",
        json!({ "image_url": "", "title": "Scan" }),
        json!({ "block": "iiifImage", "v": 2, "settings": { "source": "media", "media": media } }),
    );
    let created = send(
        app.clone(),
        json_request("POST", &append_key("/api/site_pages"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    assert_eq!(
        created.json()["o:block"][0]["o:data"]["image_url"],
        format!("http://localhost/iiif/image/3/{media}/info.json")
    );
    let page = send(app, get("/s/museum/page/iiif-media", false)).await;
    assert!(
        page.body.contains(&format!(
            "\"tileSources\":\"/iiif/image/3/{media}/info.json\""
        )),
        "{}",
        page.body
    );
}

/// A media that turns private after the save is left out of the public page,
/// though the stored plain rendering still names it until the next save.
#[tokio::test]
async fn a_media_made_private_later_is_not_shown() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let media = add_media(&pool, seed.item, seed.user, true).await;
    let created = send(
        app.clone(),
        json_request(
            "POST",
            &append_key("/api/site_pages"),
            &hero_with_media(seed.site, media),
        ),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let repo = MediaRepo::new(&pool);
    let mut row = repo.get(MediaId(media)).await.unwrap().unwrap();
    row.resource.is_public = false;
    repo.update(&row).await.unwrap();
    let page = send(app, get("/s/museum/page/media-hero", false)).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(page.body.contains("romeka-hero-no-image"), "{}", page.body);
    assert!(!page.body.contains("privatescan"), "{}", page.body);
}
