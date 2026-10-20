//! Read-endpoint integration tests: a seeded SQLite database driven through the
//! axum router with `oneshot`, asserting the frozen JSON-LD shapes.
//!
//! One test per resource, each asserting `@context`, `o:id` and one value.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{
    assert_envelope, get, header as header_value, router, router_with_store, seeded_pool,
    seeded_router, send,
};
use omeka_db::ids::{ItemId, UserId};
use omeka_db::repos::{ItemRepo, MediaRepo};
use omeka_db::rows::{NewItem, NewResource};
use omeka_media::store::{FileStore, Store};

fn storage_id() -> &'static str {
    "4b1e5b9e0f9f4f5a8f6b2c3d4e5f6a7b8c9d0e1f"
}

#[tokio::test]
async fn items_search_returns_the_item_with_its_title_value() {
    let (app, seed) = seeded_router().await;
    let answer = send(app, get("/api/items", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    let documents = answer.json();
    let item = &documents[0];
    assert_envelope(item, seed.item.0);
    assert_eq!(item["@type"], serde_json::json!(["o:Item", "dctype:Image"]));
    assert_eq!(item["dcterms:title"][0]["@value"], "A picture of a cat");
    assert_eq!(item["o:item_set"][0]["o:id"], seed.item_set.0);
}

#[tokio::test]
async fn item_member_carries_media_and_thumbnail_urls() {
    let (app, seed) = seeded_router().await;
    let answer = send(app, get(&format!("/api/items/{}", seed.item.0), false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    let item = answer.json();
    assert_envelope(&item, seed.item.0);
    assert_eq!(item["o:media"][0]["o:id"], seed.media.0);
    assert_eq!(item["o:primary_media"]["o:id"], seed.media.0);
    assert_eq!(
        item["thumbnail_display_urls"]["large"],
        format!("http://localhost/files/large/{}.jpg", storage_id())
    );
}

/// `MediaRepresentation::thumbnailUrls` with `has_thumbnails` set: three keys,
/// one file URL per type, all under `<type>/<storage_id>.jpg`.
#[tokio::test]
async fn media_member_emits_the_three_thumbnail_urls_when_the_row_has_them() {
    let (app, seed) = seeded_router().await;
    let answer = send(app, get(&format!("/api/media/{}", seed.media.0), false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    let urls = &answer.json()["o:thumbnail_urls"];
    for kind in ["large", "medium", "square"] {
        assert_eq!(
            urls[kind],
            format!("http://localhost/files/{kind}/{}.jpg", storage_id())
        );
    }
}

/// The same document with no thumbnails: upstream answers the PHP empty array,
/// which serializes as `[]`, not `{}`.
#[tokio::test]
async fn media_member_emits_an_empty_thumbnail_urls_without_files() {
    let (pool, seed) = seeded_pool().await;
    let repo = MediaRepo::new(&pool);
    let mut row = repo.get(seed.media).await.unwrap().unwrap();
    row.has_thumbnails = false;
    repo.update(&row).await.unwrap();

    let root = temp_store_root("empty");
    let answer = send(
        router_with_store(pool, FileStore::new(&root, "/files")),
        get(&format!("/api/media/{}", seed.media.0), false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.json()["o:thumbnail_urls"], serde_json::json!([]));
    let _ = std::fs::remove_dir_all(&root);
}

/// The files in the store are proof of their own: a row whose flag is false but
/// whose three derivatives exist still serves them instead of the original.
#[tokio::test]
async fn media_member_emits_the_three_thumbnail_urls_from_the_store_files() {
    let (pool, seed) = seeded_pool().await;
    let repo = MediaRepo::new(&pool);
    let mut row = repo.get(seed.media).await.unwrap().unwrap();
    row.has_thumbnails = false;
    repo.update(&row).await.unwrap();

    let root = temp_store_root("present");
    let store = FileStore::new(&root, "/files");
    for kind in ["large", "medium", "square"] {
        store
            .put(b"jpeg", &format!("{kind}/{}.jpg", storage_id()))
            .unwrap();
    }
    let answer = send(
        router_with_store(pool, store),
        get(&format!("/api/media/{}", seed.media.0), false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    let urls = &answer.json()["o:thumbnail_urls"];
    for kind in ["large", "medium", "square"] {
        assert_eq!(
            urls[kind],
            format!("http://localhost/files/{kind}/{}.jpg", storage_id())
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// A temporary store root unique to one test process and tag.
fn temp_store_root(tag: &str) -> std::path::PathBuf {
    let root =
        std::env::temp_dir().join(format!("romeka-server-thumbs-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    root
}

#[tokio::test]
async fn items_total_results_respects_per_page() {
    let (pool, seed) = seeded_pool().await;
    let repo = ItemRepo::new(&pool);
    for title in ["Second item", "Third item"] {
        repo.create(&NewItem {
            resource: NewResource {
                owner_id: Some(UserId(seed.user.0)),
                title: Some(title.to_string()),
                is_public: true,
                ..Default::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap();
    }
    let answer = send(router(pool), get("/api/items?per_page=2", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.json().as_array().unwrap().len(), 2);
    assert_eq!(header_value(&answer, "omeka-s-total-results"), Some("3"));
    let link = header_value(&answer, "link").unwrap();
    assert!(link.contains("rel=\"next\""), "no next link: {link}");
    assert!(link.contains("per_page=2"), "per_page lost: {link}");
}

#[tokio::test]
async fn items_search_matches_the_title() {
    let (app, _seed) = seeded_router().await;
    let hit = send(app.clone(), get("/api/items?search=cat", false)).await;
    assert_eq!(hit.json().as_array().unwrap().len(), 1);
    let miss = send(app, get("/api/items?search=zzz", false)).await;
    assert_eq!(miss.json().as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn missing_item_is_a_404_with_the_find_message() {
    let (app, _seed) = seeded_router().await;
    let answer = send(app, get("/api/items/999", false)).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Omeka\\Entity\\Item entity with criteria {\"id\":\"999\"} not found"}}"#
    );
}

#[tokio::test]
async fn item_sets_search_carries_the_items_link() {
    let (app, seed) = seeded_router().await;
    let answer = send(app, get("/api/item_sets", false)).await;
    let document = &answer.json()[0];
    assert_envelope(document, seed.item_set.0);
    assert_eq!(document["@type"], "o:ItemSet");
    assert_eq!(document["o:is_open"], true);
    assert_eq!(
        document["o:items"]["@id"],
        format!("http://localhost/api/items?item_set_id={}", seed.item_set.0)
    );
    assert_eq!(
        document["dcterms:title"][0]["@value"],
        "Featured photographs"
    );
}

#[tokio::test]
async fn media_search_carries_the_ingester_and_filename() {
    let (app, seed) = seeded_router().await;
    let answer = send(app, get("/api/media", false)).await;
    let document = &answer.json()[0];
    assert_envelope(document, seed.media.0);
    assert_eq!(document["o:ingester"], "upload");
    assert_eq!(document["o:filename"], format!("{}.jpg", storage_id()));
    assert_eq!(document["dcterms:title"][0]["@value"], "A photo");
}

#[tokio::test]
async fn vocabularies_carry_the_namespace_and_prefix() {
    let (app, seed) = seeded_router().await;
    let answer = send(app, get("/api/vocabularies", false)).await;
    let document = &answer.json()[0];
    assert_envelope(document, seed.vocabulary.0);
    assert_eq!(document["o:prefix"], "dcterms");
    assert_eq!(document["o:namespace_uri"], "http://purl.org/dc/terms/");
    assert_eq!(document["o:label"], "Dublin Core");
}

#[tokio::test]
async fn resource_classes_carry_the_qualified_term() {
    let (app, seed) = seeded_router().await;
    let answer = send(app, get("/api/resource_classes", false)).await;
    let document = &answer.json()[0];
    assert_envelope(document, seed.class.0);
    assert_eq!(document["o:local_name"], "Image");
    assert_eq!(document["o:term"], "dctype:Image");
}

#[tokio::test]
async fn properties_carry_the_qualified_term() {
    let (app, _seed) = seeded_router().await;
    let answer = send(app, get("/api/properties", false)).await;
    let document = &answer.json()[0];
    assert_eq!(document["o:local_name"], "title");
    assert_eq!(document["o:term"], "dcterms:title");
    assert_eq!(document["o:label"], "Title");
}

#[tokio::test]
async fn resource_templates_carry_their_properties() {
    let (app, seed) = seeded_router().await;
    let answer = send(app, get("/api/resource_templates", false)).await;
    let document = &answer.json()[0];
    assert_envelope(document, seed.template.0);
    assert_eq!(document["o:label"], "Picture");
    assert_eq!(
        document["o:resource_template_property"][0]["o:data_type"],
        serde_json::json!(["literal"])
    );
}

#[tokio::test]
async fn users_search_requires_an_authenticated_key() {
    let (app, seed) = seeded_router().await;
    let anonymous = send(app.clone(), get("/api/users", false)).await;
    assert_eq!(anonymous.status, StatusCode::FORBIDDEN);
    let keyed = send(app, get("/api/users", true)).await;
    assert_eq!(keyed.status, StatusCode::OK);
    let document = &keyed.json()[0];
    assert_envelope(document, seed.user.0);
    assert_eq!(document["o:name"], "Admin");
    assert_eq!(document["o:email"], "admin@example.com");
}

#[tokio::test]
async fn sites_carry_the_slug_and_site_item_link() {
    let (app, seed) = seeded_router().await;
    let answer = send(app, get("/api/sites", false)).await;
    let document = &answer.json()[0];
    assert_envelope(document, seed.site);
    assert_eq!(document["o:slug"], "museum");
    assert_eq!(document["o:title"], "Museum");
    assert_eq!(
        document["o:site_item"]["@id"],
        format!("http://localhost/api/items?site_id={}", seed.site)
    );
}

#[tokio::test]
async fn value_annotations_member_returns_its_values() {
    let (app, seed) = seeded_router().await;
    let answer = send(
        app,
        get(&format!("/api/value_annotations/{}", seed.annotation), true),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    let document = answer.json();
    assert_envelope(&document, seed.annotation);
    assert_eq!(document["@type"], "o:ValueAnnotation");
    assert_eq!(document["dcterms:title"][0]["@value"], "Editorial note");
}

#[tokio::test]
async fn item_primary_media_falls_back_to_the_first_media() {
    use omeka_db::rows::{NewMedia, NewResource};
    let (pool, seed) = seeded_pool().await;
    let item = ItemRepo::new(&pool)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(UserId(seed.user.0)),
                title: Some("Item with media, no primary".to_string()),
                is_public: true,
                ..Default::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap();
    let media = omeka_db::repos::MediaRepo::new(&pool)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: Some(UserId(seed.user.0)),
                is_public: true,
                ..Default::default()
            },
            item_id: item.id(),
            ingester: "html".to_string(),
            renderer: "html".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    let answer = send(
        router(pool),
        get(&format!("/api/items/{}", item.id().0), false),
    )
    .await;
    // `ItemRepresentation::primaryMedia`: no stored primary means the first media.
    assert_eq!(answer.json()["o:primary_media"]["o:id"], media.id().0);
}

#[tokio::test]
async fn a_private_item_is_invisible_to_anonymous_requests() {
    let (pool, seed) = seeded_pool().await;
    let mut item = ItemRepo::new(&pool)
        .get(ItemId(seed.item.0))
        .await
        .unwrap()
        .unwrap();
    item.resource.is_public = false;
    ItemRepo::new(&pool).update(&item).await.unwrap();
    let app = router(pool);

    let anonymous = send(app.clone(), get("/api/items", false)).await;
    assert_eq!(anonymous.json().as_array().unwrap().len(), 0);
    let owner = send(app, get("/api/items", true)).await;
    assert_eq!(owner.json().as_array().unwrap().len(), 1);
}

// The three tests below keep their names from the wave that left these
// endpoints at 501: `omeka-db` still has no repository for `asset`, `job` or
// the value-annotation listing, and `read::system` queries the table directly.
// The field-level assertions live in `api_read_gaps.rs`.

#[tokio::test]
async fn jobs_search_needs_an_admin_key_and_has_no_repository_yet() {
    let (app, _seed) = seeded_router().await;
    let anonymous = send(app.clone(), get("/api/jobs", false)).await;
    assert_eq!(anonymous.status, StatusCode::FORBIDDEN);
    let keyed = send(app, get("/api/jobs", true)).await;
    assert_eq!(keyed.status, StatusCode::OK);
}

#[tokio::test]
async fn assets_search_has_no_repository_yet() {
    let (app, _seed) = seeded_router().await;
    let answer = send(app, get("/api/assets", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.json().as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn value_annotations_search_has_no_repository_yet() {
    let (app, _seed) = seeded_router().await;
    let anonymous = send(app.clone(), get("/api/value_annotations", false)).await;
    assert_eq!(anonymous.status, StatusCode::FORBIDDEN);
    let keyed = send(app, get("/api/value_annotations", true)).await;
    assert_eq!(keyed.status, StatusCode::OK);
    assert_eq!(keyed.json().as_array().unwrap().len(), 1);
}
