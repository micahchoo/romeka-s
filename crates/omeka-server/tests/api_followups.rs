//! The API follow-ups this unit closed, each driven over HTTP against the same
//! seeded database the other read and write tests use.
//!
//! 1. `ItemRepresentation::getResourceJsonLd` emits `o:site` for every site the
//!    item belongs to, whether or not the site is public.
//! 2. `AbstractEntityAdapter::hydrateThumbnail` resolves the `o:thumbnail`
//!    asset and answers 404 when it does not exist.
//! 3. `AbstractRepresentation::thumbnailDisplayUrl` resolves a value
//!    annotation's custom thumbnail.
//!
//! The upstream shapes and statuses were captured from the oracle at
//! `5e413f009c447f4b587803ab23a9cec7d228f6e8`; the matrix is in `REPORT.md`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{append_key, get, json_request, seed_system, seeded_pool, send};
use omeka_db::ids::{SiteId, UserId};
use omeka_db::repos::{PropertyRepo, SiteRepo};
use omeka_db::rows::{NewProperty, NewSite};
use serde_json::json;
use sqlx::SqlitePool;

/// A site owned by `owner`, with the theme upstream requires.
async fn add_site(pool: &SqlitePool, slug: &str, owner: Option<UserId>) -> SiteId {
    SiteRepo::new(pool)
        .create(&NewSite {
            owner_id: owner,
            slug: slug.to_string(),
            theme: "default".to_string(),
            title: slug.to_string(),
            ..NewSite::default()
        })
        .await
        .unwrap()
        .id
}

/// The `dcterms:description` property the annotation bodies name; the fixture
/// catalog carries only `dcterms:title`.
async fn add_description_property(pool: &SqlitePool) -> u64 {
    let vocabulary: i64 = sqlx::query_scalar("SELECT id FROM vocabulary WHERE prefix = 'dcterms'")
        .fetch_one(pool)
        .await
        .unwrap();
    PropertyRepo::new(pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: omeka_db::ids::VocabularyId(vocabulary as u64),
            local_name: "description".to_string(),
            label: "Description".to_string(),
            comment: None,
        })
        .await
        .unwrap()
        .id
        .0
}

fn title(text: &str) -> serde_json::Value {
    json!([{ "property_id": "auto", "type": "literal", "@value": text }])
}

/// `ItemRepresentation::getResourceJsonLd` (`ItemRepresentation.php:19-43`):
/// the read carries `o:site` for each membership as a `{@id, o:id}` reference,
/// ordered by site id. Upstream's `sites()` applies no visibility filter, so a
/// private site the item belongs to is still listed.
#[tokio::test]
async fn an_item_read_lists_every_site_it_belongs_to() {
    let (pool, seed) = seeded_pool().await;
    let app = common::router(pool.clone());
    let public = add_site(&pool, "followups-public", Some(seed.user)).await;
    let private = add_site(&pool, "followups-private", Some(seed.user)).await;
    let mut row = SiteRepo::new(&pool).get(private).await.unwrap().unwrap();
    row.is_public = false;
    SiteRepo::new(&pool).update(&row).await.unwrap();

    let created = send(
        app.clone(),
        json_request(
            "POST",
            &append_key("/api/items"),
            &json!({
                "o:is_public": true,
                "o:site": [{ "o:id": public.0 }, { "o:id": private.0 }],
                "dcterms:title": title("On two sites"),
            })
            .to_string(),
        ),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let id = created.json()["o:id"].as_u64().unwrap();

    let read = send(app, get(&format!("/api/items/{id}"), false)).await;
    assert_eq!(read.status, StatusCode::OK, "{}", read.body);
    assert_eq!(
        read.json()["o:site"],
        json!([
            { "@id": format!("http://localhost/api/sites/{}", public.0), "o:id": public.0 },
            { "@id": format!("http://localhost/api/sites/{}", private.0), "o:id": private.0 },
        ])
    );
}

/// `AbstractEntityAdapter::hydrateThumbnail` (`AbstractEntityAdapter.php:946`):
/// a numeric `o:thumbnail.o:id` naming a missing asset is a 404 with the
/// `findEntity` message, and nothing is written.
#[tokio::test]
async fn an_item_write_rejects_a_thumbnail_asset_that_does_not_exist() {
    let (pool, _seed) = seeded_pool().await;
    let app = common::router(pool);

    let answer = send(
        app,
        json_request(
            "POST",
            &append_key("/api/items"),
            &json!({
                "o:is_public": true,
                "o:thumbnail": { "o:id": 999999 },
                "dcterms:title": title("Missing asset"),
            })
            .to_string(),
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND, "{}", answer.body);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Omeka\\Entity\\Asset entity with criteria {\"id\":999999} not found"}}"#
    );
}

/// `AbstractEntityAdapter::hydrateThumbnail`: an `o:thumbnail` that names an
/// existing asset is stored and read back, and a full `PUT` without it clears
/// the reference.
#[tokio::test]
async fn an_item_write_stores_the_thumbnail_asset_it_names() {
    let (pool, seed) = seeded_pool().await;
    let system = seed_system(&pool, &seed).await;
    let app = common::router(pool);

    let created = send(
        app.clone(),
        json_request(
            "POST",
            &append_key("/api/items"),
            &json!({
                "o:is_public": true,
                "o:thumbnail": { "o:id": system.asset },
                "dcterms:title": title("Custom thumbnail"),
            })
            .to_string(),
        ),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let document = created.json();
    assert_eq!(document["o:thumbnail"]["o:id"], system.asset);
    let id = document["o:id"].as_u64().unwrap();

    let updated = send(
        app,
        json_request(
            "PUT",
            &append_key(&format!("/api/items/{id}")),
            &json!({
                "o:is_public": true,
                "dcterms:title": title("Custom thumbnail"),
            })
            .to_string(),
        ),
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    assert_eq!(updated.json()["o:thumbnail"], serde_json::Value::Null);
}

/// `AbstractRepresentation::thumbnailDisplayUrl` (`AbstractRepresentation.php:114`):
/// a value annotation whose `thumbnail_id` names an asset shows the asset URL
/// for every thumbnail type, the same way an item does.
#[tokio::test]
async fn a_value_annotation_thumbnail_resolves_to_the_asset_url() {
    let (pool, seed) = seeded_pool().await;
    add_description_property(&pool).await;
    let system = seed_system(&pool, &seed).await;
    let app = common::router(pool.clone());

    let body = json!({
        "o:is_public": true,
        "dcterms:title": [{
            "property_id": "auto",
            "type": "literal",
            "@value": "Annotated title",
            "@annotation": {
                "o:thumbnail": { "o:id": system.asset },
                "dcterms:description": [
                    { "property_id": "auto", "type": "literal", "@value": "a note" },
                ],
            },
        }],
    })
    .to_string();
    let created = send(app, json_request("POST", &append_key("/api/items"), &body)).await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);

    let annotation: i64 = sqlx::query_scalar(
        "SELECT value_annotation_id FROM value WHERE value_annotation_id IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let answer = send(
        common::router(pool),
        get(&format!("/api/value_annotations/{annotation}"), true),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let document = answer.json();
    assert_eq!(document["o:thumbnail"]["o:id"], system.asset);
    let url = "http://localhost/files/asset/a1b2c3d4e5f60718293a4b5c6d7e8f9012345678.png";
    for kind in ["large", "medium", "square"] {
        assert_eq!(document["thumbnail_display_urls"][kind], url, "{kind}");
    }
}
