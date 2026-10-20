//! The map block and the item page's map section, from the API write to the
//! public page: the write stores the public places as the plain rendering, the
//! page draws the rich map, and only a page with a map loads Leaflet.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{Seed, append_key, get, json_request, router, send, site_pool};
use omeka_db::ids::{ItemId, PropertyId, SiteId};
use omeka_db::repos::{ItemRepo, PropertyRepo, SiteSettingRepo, ValueRepo};
use omeka_db::rows::{NewItem, NewProperty, NewResource, NewValue};
use serde_json::{Value, json};
use sqlx::SqlitePool;

/// `dcterms:spatial`, named as the site's location field.
async fn location_field(pool: &SqlitePool, seed: &Seed) -> PropertyId {
    let spatial = PropertyRepo::new(pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: seed.vocabulary,
            local_name: "spatial".to_string(),
            label: "Spatial Coverage".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    SiteSettingRepo::new(pool)
        .set(
            SiteId::from(seed.site),
            "romeka_location_property",
            &json!("dcterms:spatial"),
        )
        .await
        .unwrap();
    spatial.id
}

async fn place(pool: &SqlitePool, item: ItemId, property: PropertyId, text: &str) {
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: item.as_resource(),
            property_id: property,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some(text.to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
}

/// A private item with a place: the map's plain rendering must not name it.
async fn private_item(pool: &SqlitePool, seed: &Seed) -> ItemId {
    ItemRepo::new(pool)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(seed.user),
                resource_class_id: None,
                resource_template_id: None,
                thumbnail_id: None,
                title: Some("A private letter".to_string()),
                is_public: false,
                created: None,
            },
            primary_media_id: None,
        })
        .await
        .unwrap()
        .id()
}

fn map_page(site: u64, slug: &str, settings: &Value) -> String {
    json!({
        "o:site": { "o:id": site },
        "o:title": "Places",
        "o:slug": slug,
        "o:block": [{
            "o:layout": "html",
            "o:data": {},
            "o:layout_data": { "romeka": { "block": "map", "v": 2, "settings": settings } }
        }],
    })
    .to_string()
}

/// The write stores each public item's places as a linked list, which is what
/// upstream shows, and leaves the private item out because the html is public.
#[tokio::test]
async fn the_write_stores_the_public_places_as_the_plain_rendering() {
    let (pool, seed) = site_pool().await;
    let spatial = location_field(&pool, &seed).await;
    place(&pool, seed.item, spatial, "51.5055, -0.0754").await;
    let hidden = private_item(&pool, &seed).await;
    place(&pool, hidden, spatial, "48.8584, 2.2945").await;
    let app = router(pool);

    let body = map_page(
        seed.site,
        "places",
        &json!({ "source": "items", "items": [seed.item.0, hidden.0] }),
    );
    let created = send(
        app.clone(),
        json_request("POST", &append_key("/api/site_pages"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let stored = created.json()["o:block"][0]["o:data"]["html"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        stored,
        format!(
            "<ul><li><a href=\"/s/museum/item/{}\" rel=\"noreferrer noopener\">A picture of a cat</a> \
             51.5055, -0.0754</li></ul>",
            seed.item.0
        )
    );
}

/// A query selection resolves through the items search at write time too.
#[tokio::test]
async fn a_query_selection_lists_what_the_search_finds() {
    let (pool, seed) = site_pool().await;
    let spatial = location_field(&pool, &seed).await;
    place(&pool, seed.item, spatial, "51.5055, -0.0754").await;
    let app = router(pool);

    let body = map_page(
        seed.site,
        "places",
        &json!({ "source": "query", "query": format!("item_set_id={}", seed.item_set.0) }),
    );
    let created = send(
        app.clone(),
        json_request("POST", &append_key("/api/site_pages"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let stored = created.json()["o:block"][0]["o:data"]["html"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        stored.contains("A picture of a cat</a> 51.5055, -0.0754"),
        "{stored}"
    );
}

/// The public page draws the map and loads Leaflet for it; a page with no map
/// loads none.
#[tokio::test]
async fn only_a_page_with_a_map_loads_leaflet() {
    let (pool, seed) = site_pool().await;
    let spatial = location_field(&pool, &seed).await;
    place(&pool, seed.item, spatial, "51.5055, -0.0754").await;
    let app = router(pool);
    let body = map_page(
        seed.site,
        "places",
        &json!({ "source": "items", "items": [seed.item.0], "cluster": true }),
    );
    let created = send(
        app.clone(),
        json_request("POST", &append_key("/api/site_pages"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);

    let page = send(app.clone(), get("/s/museum/page/places", false)).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(page.body.contains("data-romeka-map=\""), "{}", page.body);
    assert!(
        page.body.contains("&quot;cluster&quot;:true"),
        "{}",
        page.body
    );
    assert!(
        page.body.contains("/assets/vendor/leaflet/leaflet.js?v="),
        "{}",
        page.body
    );
    assert!(page.body.contains("/assets/js/map.js?v="), "{}", page.body);

    let welcome = send(app.clone(), get("/s/museum/page/welcome", false)).await;
    assert_eq!(welcome.status, StatusCode::OK, "{}", welcome.body);
    assert!(!welcome.body.contains("leaflet"), "{}", welcome.body);

    let script = send(app, get("/assets/vendor/leaflet/leaflet.js", false)).await;
    assert_eq!(script.status, StatusCode::OK);
    assert!(script.body.contains("1.9.4"), "the vendored release");
}

/// The item page's "Map" section shows the item's own place when the theme
/// lists it, and loads Leaflet for it.
#[tokio::test]
async fn the_item_page_map_section_shows_the_items_place() {
    let (pool, seed) = site_pool().await;
    let db = omeka_db::DbPool::from(pool.clone());
    let spatial = location_field(&pool, &seed).await;
    place(&pool, seed.item, spatial, "51.5055, -0.0754").await;
    omeka_api::theme_settings::store(
        &db,
        SiteId::from(seed.site),
        "default",
        Some(&json!({ "resource_page_blocks": { "items": { "main": ["values", "map"] } } })),
    )
    .await
    .unwrap();
    let app = router(pool);

    let page = send(app, get(&format!("/s/museum/item/{}", seed.item.0), false)).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    let section = page
        .body
        .split("<div class=\"romeka-map-section\">")
        .nth(1)
        .expect("the map section");
    assert!(section.contains("&quot;lat&quot;:51.5055"), "{section}");
    assert!(
        page.body.contains("/assets/vendor/leaflet/leaflet.js?v="),
        "{}",
        page.body
    );
}
