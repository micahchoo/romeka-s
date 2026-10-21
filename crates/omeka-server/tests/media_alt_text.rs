//! `MediaRepresentation::altTextResolved` (`MediaRepresentation.php:271`): a
//! media whose `alt_text` is empty takes its alt text from the first value of
//! the property the global `media_alt_text_property` setting names, and the
//! setting travels through `GET` and `PATCH /api/settings`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{append_key, get, json_request, router, seeded_pool, send};
use omeka_db::ids::VocabularyId;
use omeka_db::repos::{MediaRepo, PropertyRepo, SettingRepo, ValueRepo};
use serde_json::json;

const ALT_PROPERTY: &str = "media_alt_text_property";
const STORAGE_ID: &str = "4b1e5b9e0f9f4f5a8f6b2c3d4e5f6a7b8c9d0e1f";

/// Store `media_alt_text_property` = `dcterms:title`, and set the seeded
/// media's title value to `text`.
async fn name_the_media(pool: &sqlx::SqlitePool, vocabulary: VocabularyId, media: u64, text: &str) {
    SettingRepo::new(pool)
        .set(ALT_PROPERTY, &json!("dcterms:title"))
        .await
        .unwrap();
    let title = PropertyRepo::new(pool)
        .find(vocabulary, "title")
        .await
        .unwrap()
        .expect("dcterms:title is seeded");
    let mut value = ValueRepo::new(pool)
        .list_for_resource(omeka_db::ids::MediaId(media).as_resource())
        .await
        .unwrap()
        .into_iter()
        .find(|value| value.property_id == title.id)
        .expect("the media's title value is seeded");
    value.value = Some(text.to_string());
    ValueRepo::new(pool).update(&value).await.unwrap();
}

/// The media page's renderer markup, whose alt is the resolved alt text.
fn rendered_media_alt(body: &str) -> Option<&str> {
    let start = body.find("<div class=\"media-render file\">")?;
    let render = &body[start..];
    let alt = render.find("alt=\"")? + "alt=\"".len();
    let rest = &render[alt..];
    Some(&rest[..rest.find('"')?])
}

/// A media with no alt text takes the property value the setting names.
#[tokio::test]
async fn a_media_without_alt_text_uses_the_setting_property() {
    let (pool, seed) = seeded_pool().await;
    name_the_media(&pool, seed.vocabulary, seed.media.0, "A photo of a cat").await;

    let path = format!("/s/museum/media/{}", seed.media.0);
    let answer = send(router(pool), get(&path, false)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(
        rendered_media_alt(&answer.body),
        Some("A photo of a cat"),
        "{}",
        answer.body
    );
}

/// An explicit alt text wins over the setting's property.
#[tokio::test]
async fn a_stored_alt_text_wins_over_the_setting_property() {
    let (pool, seed) = seeded_pool().await;
    name_the_media(&pool, seed.vocabulary, seed.media.0, "A photo of a cat").await;
    let mut media = MediaRepo::new(&pool)
        .get(seed.media)
        .await
        .unwrap()
        .expect("seeded media");
    media.alt_text = Some("The curator alt".to_string());
    MediaRepo::new(&pool).update(&media).await.unwrap();

    let path = format!("/s/museum/media/{}", seed.media.0);
    let answer = send(router(pool), get(&path, false)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(rendered_media_alt(&answer.body), Some("The curator alt"));
}

/// The carrier's plain rendering stores the resolved alt text, so an upstream
/// installation shows the same frame text here.
#[tokio::test]
async fn the_plain_rendering_stores_the_resolved_alt_text() {
    let (pool, seed) = seeded_pool().await;
    name_the_media(&pool, seed.vocabulary, seed.media.0, "A photo of a cat").await;

    let body = json!({
        "o:site": { "o:id": seed.site },
        "o:title": "Alt text",
        "o:slug": "alt-text",
        "o:block": [{
            "o:layout": "html",
            "o:data": {},
            "o:layout_data": { "romeka": {
                "block": "hero",
                "v": 2,
                "settings": { "heading": "Maps", "background_source": "media", "background_media": seed.media.0 },
            }},
        }],
    });
    let created = send(
        router(pool),
        json_request("POST", &append_key("/api/site_pages"), &body.to_string()),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let stored = created.json()["o:block"][0]["o:data"]["html"]
        .as_str()
        .expect("stored plain html")
        .to_string();
    assert!(
        stored.contains(&format!(
            "<img src=\"/files/large/{STORAGE_ID}.jpg\" alt=\"A photo of a cat\">"
        )),
        "{stored}"
    );
}

/// `GET` and `PATCH /api/settings` carry the property term; an empty string
/// removes the row and turns the fallback off.
#[tokio::test]
async fn the_setting_travels_through_the_settings_route() {
    let (pool, _seed) = seeded_pool().await;
    let app = router(pool);

    let patched = send(
        app.clone(),
        json_request(
            "PATCH",
            &append_key("/api/settings"),
            &json!({ ALT_PROPERTY: "dcterms:title" }).to_string(),
        ),
    )
    .await;
    assert_eq!(patched.status, StatusCode::OK, "{}", patched.body);
    assert_eq!(patched.json()[ALT_PROPERTY], json!("dcterms:title"));

    let read = send(app.clone(), get("/api/settings", true)).await;
    assert_eq!(read.status, StatusCode::OK, "{}", read.body);
    assert_eq!(read.json()[ALT_PROPERTY], json!("dcterms:title"));

    let removed = send(
        app,
        json_request(
            "PATCH",
            &append_key("/api/settings"),
            &json!({ ALT_PROPERTY: "" }).to_string(),
        ),
    )
    .await;
    assert_eq!(removed.status, StatusCode::OK, "{}", removed.body);
    assert!(
        removed.json().get(ALT_PROPERTY).is_none(),
        "{}",
        removed.body
    );
}
