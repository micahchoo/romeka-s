//! The read endpoints this unit closed: `assets`, `jobs`, `modules`,
//! `data_types`, `site_pages`, `value_annotations` and the `api_resources`
//! member, plus the two `resources` routes the upstream adapter refuses.
//!
//! Every test drives the router over a seeded temp database, the same way
//! `api_read.rs` does. The upstream shapes are the frozen fixtures under
//! `fixtures/api/`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{
    Seed, assert_envelope, get, header, router, seed_system, seeded_pool, send, site_pool,
};

const ASSET_STORAGE: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678";

fn asset_filename() -> String {
    format!("{ASSET_STORAGE}.png")
}

// ----------------------------------------------------------------- assets

#[tokio::test]
async fn assets_search_lists_the_asset_with_its_file_url() {
    let (pool, seed) = seeded_pool().await;
    let system = seed_system(&pool, &seed).await;
    let answer = send(router(pool), get("/api/assets", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(header(&answer, "Omeka-S-Total-Results"), Some("1"));
    let document = &answer.json()[0];
    assert_envelope(document, system.asset);
    assert_eq!(document["@type"], "o:Asset");
    assert_eq!(document["o:name"], "omeka-logo.png");
    assert_eq!(document["o:filename"], asset_filename());
    assert_eq!(document["o:media_type"], "image/png");
    assert_eq!(
        document["o:asset_url"],
        format!("http://localhost/files/asset/{}", asset_filename())
    );
    assert_eq!(document["o:alt_text"], serde_json::Value::Null);
    assert_eq!(document["o:owner"]["o:id"], seed.user.0);
}

#[tokio::test]
async fn asset_member_returns_the_same_document() {
    let (pool, seed) = seeded_pool().await;
    let system = seed_system(&pool, &seed).await;
    let answer = send(
        router(pool),
        get(&format!("/api/assets/{}", system.asset), false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    let document = answer.json();
    assert_envelope(&document, system.asset);
    assert_eq!(document["o:filename"], asset_filename());
}

#[tokio::test]
async fn a_missing_asset_is_a_404_with_the_find_message() {
    let (pool, seed) = seeded_pool().await;
    seed_system(&pool, &seed).await;
    let answer = send(router(pool), get("/api/assets/999", false)).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Omeka\\Entity\\Asset entity with criteria {\"id\":\"999\"} not found"}}"#
    );
}

#[tokio::test]
async fn assets_filter_by_owner_id() {
    let (pool, seed) = seeded_pool().await;
    seed_system(&pool, &seed).await;
    let app = router(pool);
    let mine = send(
        app.clone(),
        get(&format!("/api/assets?owner_id={}", seed.user.0), false),
    )
    .await;
    assert_eq!(mine.json().as_array().unwrap().len(), 1);
    let nobody = send(app, get("/api/assets?owner_id=0", false)).await;
    assert_eq!(nobody.json().as_array().unwrap().len(), 0);
}

/// `AbstractRepresentation::thumbnailDisplayUrl`: a resource whose
/// `resource.thumbnail_id` names an asset shows the asset's `assetUrl` for
/// every thumbnail type.
#[tokio::test]
async fn a_resource_custom_thumbnail_shows_the_asset_url() {
    let (pool, seed) = seeded_pool().await;
    let system = seed_system(&pool, &seed).await;
    let items = omeka_db::repos::ItemRepo::new(&pool);
    let mut item = items.get(seed.item).await.unwrap().unwrap();
    item.resource.thumbnail_id = Some(omeka_db::ids::AssetId::from(system.asset));
    items.update(&item).await.unwrap();

    let answer = send(
        router(pool),
        get(&format!("/api/items/{}", seed.item.0), false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    let document = answer.json();
    assert_eq!(document["o:thumbnail"]["o:id"], system.asset);
    let url = format!("http://localhost/files/asset/{}", asset_filename());
    for kind in ["large", "medium", "square"] {
        assert_eq!(document["thumbnail_display_urls"][kind], url, "{kind}");
    }
}

// ------------------------------------------------------------------- jobs

#[tokio::test]
async fn jobs_search_requires_a_key_and_carries_the_job_fields() {
    let (pool, seed) = seeded_pool().await;
    let system = seed_system(&pool, &seed).await;
    let app = router(pool);
    let anonymous = send(app.clone(), get("/api/jobs", false)).await;
    assert_eq!(anonymous.status, StatusCode::FORBIDDEN);
    let answer = send(app, get("/api/jobs", true)).await;
    assert_eq!(answer.status, StatusCode::OK);
    let document = &answer.json()[0];
    assert_envelope(document, system.job);
    assert_eq!(document["@type"], "o:Job");
    assert_eq!(document["o:status"], "completed");
    assert_eq!(document["o:job_class"], "Omeka\\Job\\UpdateSiteItems");
    assert_eq!(
        document["o:args"],
        serde_json::json!({"action": "add", "sites": []})
    );
    assert_eq!(document["o:owner"]["o:id"], seed.user.0);
    assert_eq!(document["o:started"]["@value"], "2024-01-03T11:00:00+00:00");
    assert_eq!(document["o:ended"]["@value"], "2024-01-03T11:00:05+00:00");
}

#[tokio::test]
async fn jobs_member_requires_a_key() {
    let (pool, seed) = seeded_pool().await;
    let system = seed_system(&pool, &seed).await;
    let app = router(pool);
    let anonymous = send(
        app.clone(),
        get(&format!("/api/jobs/{}", system.job), false),
    )
    .await;
    assert_eq!(anonymous.status, StatusCode::FORBIDDEN);
    let keyed = send(app, get(&format!("/api/jobs/{}", system.job), true)).await;
    assert_eq!(keyed.status, StatusCode::OK);
    assert_eq!(keyed.json()["o:id"], system.job);
}

// ---------------------------------------------------------------- modules

#[tokio::test]
async fn modules_search_requires_a_key_and_reports_the_module_state() {
    let (pool, seed) = seeded_pool().await;
    let system = seed_system(&pool, &seed).await;
    let app = router(pool);
    let anonymous = send(app.clone(), get("/api/modules", false)).await;
    assert_eq!(anonymous.status, StatusCode::FORBIDDEN);
    let answer = send(app, get("/api/modules", true)).await;
    assert_eq!(answer.status, StatusCode::OK);
    let document = &answer.json()[0];
    assert_eq!(document["@context"], "http://localhost/api-context");
    assert_eq!(document["@type"], "o:Module");
    assert_eq!(document["o:id"], system.module);
    assert_eq!(document["o:state"], "active");
    assert_eq!(
        document["o:ini"],
        serde_json::json!({"name": "Common", "version": "3.4.0"})
    );
    assert_eq!(document["@id"], "http://localhost/api/modules/Common");
}

#[tokio::test]
async fn module_member_uses_the_directory_name_as_the_id() {
    let (pool, seed) = seeded_pool().await;
    let system = seed_system(&pool, &seed).await;
    let answer = send(
        router(pool),
        get(&format!("/api/modules/{}", system.module), true),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.json()["o:id"], "Common");
}

#[tokio::test]
async fn a_missing_module_is_a_404() {
    let (pool, seed) = seeded_pool().await;
    seed_system(&pool, &seed).await;
    let answer = send(router(pool), get("/api/modules/Nope", true)).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
}

// ------------------------------------------------------------- data_types

#[tokio::test]
async fn data_types_search_is_open_and_lists_the_registry_in_order() {
    let (pool, _seed) = seeded_pool().await;
    let answer = send(router(pool), get("/api/data_types", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    let documents = answer.json();
    let ids: Vec<&str> = documents
        .as_array()
        .unwrap()
        .iter()
        .map(|document| document["o:id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "literal",
            "uri",
            "resource",
            "resource:item",
            "resource:itemset",
            "resource:media",
        ]
    );
    assert_eq!(documents[0]["o:label"], "Text");
    assert_eq!(
        documents[0]["@id"],
        "http://localhost/api/data_types/literal"
    );
}

#[tokio::test]
async fn data_type_member_carries_the_label() {
    let (pool, _seed) = seeded_pool().await;
    let answer = send(router(pool), get("/api/data_types/resource:item", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.json()["o:id"], "resource:item");
    assert_eq!(answer.json()["o:label"], "Item");
}

#[tokio::test]
async fn an_unregistered_data_type_is_a_404() {
    let (pool, _seed) = seeded_pool().await;
    let answer = send(router(pool), get("/api/data_types/nonsense", false)).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------- api_resources

#[tokio::test]
async fn api_resource_member_echoes_any_id() {
    let (pool, _seed) = seeded_pool().await;
    let app = router(pool);
    let answer = send(app.clone(), get("/api/api_resources/items", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.json()["o:id"], "items");
    assert_eq!(
        answer.json()["@id"],
        "http://localhost/api/api_resources/items"
    );
    // `ApiResourceAdapter::read` validates nothing, so an unknown name answers
    // 200 the same way.
    let unknown = send(app, get("/api/api_resources/nonsense", false)).await;
    assert_eq!(unknown.status, StatusCode::OK);
    assert_eq!(unknown.json()["o:id"], "nonsense");
}

// -------------------------------------------------------------- resources

/// `ResourceAdapter::search` runs on the abstract `Resource` entity, so the
/// collection carries a document of every kind the seed created.
#[tokio::test]
async fn resources_collection_lists_every_kind() {
    let (pool, seed) = seeded_pool().await;
    let answer = send(router(pool), get("/api/resources", false)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let documents = answer.json();
    let ids: Vec<u64> = documents
        .as_array()
        .unwrap()
        .iter()
        .map(|document| document["o:id"].as_u64().unwrap())
        .collect();
    for id in [seed.item.0, seed.item_set.0, seed.media.0, seed.annotation] {
        assert!(ids.contains(&id), "resource {id} is missing from {ids:?}");
    }
}

/// `ResourceAdapter::read` proxies to the row's own adapter, so the member
/// carries that kind's `@id`.
#[tokio::test]
async fn resources_member_reads_the_row_its_kind_names() {
    let (pool, seed) = seeded_pool().await;
    let answer = send(
        router(pool),
        get(&format!("/api/resources/{}", seed.item.0), false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(
        answer.json()["@id"],
        format!("http://localhost/api/items/{}", seed.item.0)
    );
    assert_eq!(answer.json()["@type"][0], "o:Item");
}

// ------------------------------------------------------------- site_pages

async fn site_pages_seed() -> (axum::Router, Seed) {
    let (pool, seed) = site_pool().await;
    (router(pool), seed)
}

#[tokio::test]
async fn site_pages_search_hides_a_private_page_from_anonymous_callers() {
    let (app, seed) = site_pages_seed().await;
    let anonymous = send(app.clone(), get("/api/site_pages", false)).await;
    assert_eq!(anonymous.status, StatusCode::OK);
    let document = anonymous.json();
    let titles: Vec<&str> = document
        .as_array()
        .unwrap()
        .iter()
        .map(|document| document["o:title"].as_str().unwrap())
        .collect();
    assert_eq!(
        titles,
        ["Welcome", "Preview", "<script>alert('x')</script>"]
    );
    assert_eq!(header(&anonymous, "Omeka-S-Total-Results"), Some("3"));

    // The seeded actor is a global admin, which `sees` lets past the public
    // flag, so the private page appears.
    let keyed = send(app, get("/api/site_pages", true)).await;
    assert_eq!(keyed.json().as_array().unwrap().len(), 4);
    let _ = seed;
}

#[tokio::test]
async fn site_pages_search_filters_by_site_slug_and_public_flag() {
    let (app, _seed) = site_pages_seed().await;
    let by_slug = send(app.clone(), get("/api/site_pages?site_slug=museum", false)).await;
    assert_eq!(by_slug.json().as_array().unwrap().len(), 3);
    let no_site = send(app.clone(), get("/api/site_pages?site_slug=nope", false)).await;
    assert_eq!(no_site.json().as_array().unwrap().len(), 0);
    let public_only = send(app.clone(), get("/api/site_pages?is_public=1", true)).await;
    assert_eq!(public_only.json().as_array().unwrap().len(), 3);
    let private_only = send(app, get("/api/site_pages?is_public=0", true)).await;
    assert_eq!(private_only.json().as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn site_page_member_carries_its_blocks() {
    let (app, seed) = site_pages_seed().await;
    let _ = seed;
    let listing = send(app.clone(), get("/api/site_pages", false)).await;
    let first = listing.json()[0]["o:id"].as_u64().unwrap();
    let answer = send(app, get(&format!("/api/site_pages/{first}"), false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    let document = answer.json();
    assert_eq!(document["@type"], "o:SitePage");
    assert_eq!(document["o:id"], first);
    assert_eq!(document["o:slug"], "welcome");
    assert_eq!(document["o:block"][0]["o:layout"], "pageTitle");
}

// ------------------------------------------------------- value_annotations

#[tokio::test]
async fn value_annotations_search_lists_the_annotation() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool);
    let anonymous = send(app.clone(), get("/api/value_annotations", false)).await;
    assert_eq!(anonymous.status, StatusCode::FORBIDDEN);
    let answer = send(app, get("/api/value_annotations", true)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(header(&answer, "Omeka-S-Total-Results"), Some("1"));
    let document = &answer.json()[0];
    assert_envelope(document, seed.annotation);
    assert_eq!(document["@type"], "o:ValueAnnotation");
    assert_eq!(document["o:title"], "Editorial note");
    assert_eq!(document["dcterms:title"][0]["@value"], "Editorial note");
}
