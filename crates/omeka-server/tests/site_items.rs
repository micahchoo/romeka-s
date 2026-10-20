//! `POST /api/sites/{id}/items`: the admin's way to change which items a site holds, and the sync
//! an item write runs for a site that keeps in step with its item pool.
//!
//! Every test drives the router against a migrated in-memory database and reads `item_site` back,
//! so what it asserts is the membership the public site and upstream both read.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::StatusCode;
use common::{Answer, append_key, json_request, seeded_pool, send};
use omeka_db::ids::{SiteId, UserId};
use omeka_db::repos::{ApiKeyRepo, SiteRepo, UserRepo};
use omeka_db::rows::{NewApiKey, NewSite, NewUser};
use serde_json::{Value, json};
use sqlx::SqlitePool;

fn title(text: &str) -> Value {
    json!([{ "property_id": "auto", "type": "literal", "@value": text }])
}

async fn add_site(pool: &SqlitePool, slug: &str, owner: UserId) -> SiteId {
    SiteRepo::new(pool)
        .create(&NewSite {
            owner_id: Some(owner),
            slug: slug.to_string(),
            theme: "default".to_string(),
            title: slug.to_string(),
            ..NewSite::default()
        })
        .await
        .unwrap()
        .id
}

/// Create an item through the API as the seeded global admin and return its id.
async fn add_item(app: &Router, text: &str, public: bool, extra: Value) -> u64 {
    let mut body = json!({ "o:is_public": public, "dcterms:title": title(text) });
    if let (Some(body), Some(extra)) = (body.as_object_mut(), extra.as_object()) {
        body.extend(extra.clone());
    }
    let created = send(
        app.clone(),
        json_request("POST", &append_key("/api/items"), &body.to_string()),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    created.json()["o:id"].as_u64().unwrap()
}

/// The ids of the items attached to a site, as `item_site` stores them.
async fn site_items(pool: &SqlitePool, site: SiteId) -> Vec<u64> {
    sqlx::query_scalar::<_, i64>("SELECT item_id FROM item_site WHERE site_id = ? ORDER BY item_id")
        .bind(site.0 as i64)
        .fetch_all(pool)
        .await
        .unwrap()
        .into_iter()
        .map(|raw| u64::try_from(raw).unwrap())
        .collect()
}

async fn change(app: &Router, uri: &str, body: Value) -> Answer {
    send(app.clone(), json_request("POST", uri, &body.to_string())).await
}

fn site_uri(site: SiteId) -> String {
    append_key(&format!("/api/sites/{}/items", site.0))
}

/// Every item the seed made, so a test can name the items it adds on top.
async fn seeded_items(pool: &SqlitePool) -> Vec<u64> {
    sqlx::query_scalar::<_, i64>("SELECT id FROM item ORDER BY id")
        .fetch_all(pool)
        .await
        .unwrap()
        .into_iter()
        .map(|raw| u64::try_from(raw).unwrap())
        .collect()
}

#[tokio::test]
async fn a_query_adds_the_items_it_selects_and_a_dry_run_writes_nothing() {
    let (pool, seed) = seeded_pool().await;
    let app = common::router(pool.clone());
    let site = add_site(&pool, "gallery", seed.user).await;
    let before = seeded_items(&pool).await;
    let map = add_item(&app, "Harbour map", true, json!({})).await;
    let chart = add_item(&app, "Harbour chart", true, json!({})).await;
    let letter = add_item(&app, "Private letter", false, json!({})).await;

    let preview = change(
        &app,
        &site_uri(site),
        json!({ "action": "add", "query": { "fulltext_search": "harbour" }, "dry_run": true }),
    )
    .await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.body);
    assert_eq!(
        preview.json(),
        json!({ "matched": 2, "added": 2, "removed": 0, "total": 2 })
    );
    assert!(
        site_items(&pool, site).await.is_empty(),
        "a dry run counts the change and writes nothing"
    );

    let added = change(
        &app,
        &site_uri(site),
        json!({ "action": "add", "query": { "fulltext_search": "harbour" } }),
    )
    .await;
    assert_eq!(added.status, StatusCode::OK, "{}", added.body);
    assert_eq!(site_items(&pool, site).await, vec![map, chart]);

    let public = change(
        &app,
        &site_uri(site),
        json!({ "action": "add", "query": { "is_public": "1" } }),
    )
    .await;
    let mut expected: Vec<u64> = before.clone();
    expected.extend([map, chart]);
    assert_eq!(public.status, StatusCode::OK, "{}", public.body);
    assert_eq!(
        public.json()["added"].as_u64().unwrap() as usize,
        expected.len() - 2,
        "only the public items the site lacked are added"
    );
    assert_eq!(site_items(&pool, site).await, expected);
    assert!(!site_items(&pool, site).await.contains(&letter));
}

#[tokio::test]
async fn an_item_set_adds_its_items_and_remove_detaches_named_items() {
    let (pool, seed) = seeded_pool().await;
    let app = common::router(pool.clone());
    let site = add_site(&pool, "sets", seed.user).await;
    let set = send(
        app.clone(),
        json_request(
            "POST",
            &append_key("/api/item_sets"),
            &json!({ "dcterms:title": title("Maps") }).to_string(),
        ),
    )
    .await;
    assert_eq!(set.status, StatusCode::OK, "{}", set.body);
    let set = set.json()["o:id"].as_u64().unwrap();
    let in_set = json!({ "o:item_set": [{ "o:id": set }] });
    let first = add_item(&app, "First map", true, in_set.clone()).await;
    let second = add_item(&app, "Second map", false, in_set).await;
    add_item(&app, "Not a map", true, json!({})).await;

    let added = change(
        &app,
        &site_uri(site),
        json!({ "action": "add", "item_sets": [set] }),
    )
    .await;
    assert_eq!(added.status, StatusCode::OK, "{}", added.body);
    assert_eq!(
        added.json(),
        json!({ "matched": 2, "added": 2, "removed": 0, "total": 2 })
    );
    assert_eq!(site_items(&pool, site).await, vec![first, second]);

    let removed = change(
        &app,
        &site_uri(site),
        json!({ "action": "remove", "items": [first] }),
    )
    .await;
    assert_eq!(removed.status, StatusCode::OK, "{}", removed.body);
    assert_eq!(
        removed.json(),
        json!({ "matched": 1, "added": 0, "removed": 1, "total": 1 })
    );
    assert_eq!(site_items(&pool, site).await, vec![second]);

    let none = change(
        &app,
        &site_uri(site),
        json!({ "action": "remove", "items": [] }),
    )
    .await;
    assert_eq!(none.json()["removed"], 0, "an empty list names no item");
    assert_eq!(site_items(&pool, site).await, vec![second]);
}

/// Create an item set with one title through the API and return its id.
async fn add_item_set(app: &Router, text: &str) -> u64 {
    let set = send(
        app.clone(),
        json_request(
            "POST",
            &append_key("/api/item_sets"),
            &json!({ "dcterms:title": title(text) }).to_string(),
        ),
    )
    .await;
    assert_eq!(set.status, StatusCode::OK, "{}", set.body);
    set.json()["o:id"].as_u64().unwrap()
}

async fn patch_item(app: &Router, item: u64, body: Value) {
    let patched = send(
        app.clone(),
        json_request(
            "PATCH",
            &append_key(&format!("/api/items/{item}")),
            &body.to_string(),
        ),
    )
    .await;
    assert_eq!(patched.status, StatusCode::OK, "{}", patched.body);
}

#[tokio::test]
async fn a_synced_site_takes_an_item_only_when_it_is_created() {
    let (pool, seed) = seeded_pool().await;
    let app = common::router(pool.clone());
    let maps = add_item_set(&app, "Maps").await;
    let site = add_site(&pool, "synced", seed.user).await;
    let mut row = SiteRepo::new(&pool).get(site).await.unwrap().unwrap();
    row.assign_new_items = true;
    // The pool the admin writes from the picked sets: `item_set_id[]=<maps>`.
    row.item_pool = json!({ "item_set_id": [maps.to_string()] });
    SiteRepo::new(&pool).update(&row).await.unwrap();

    let in_set = add_item(
        &app,
        "A map",
        true,
        json!({ "o:item_set": [{ "o:id": maps }] }),
    )
    .await;
    let outside = add_item(&app, "A letter", true, json!({})).await;
    assert_eq!(
        site_items(&pool, site).await,
        vec![in_set],
        "a new item joins when the site's pool selects it"
    );

    patch_item(&app, outside, json!({ "o:item_set": [{ "o:id": maps }] })).await;
    assert_eq!(
        site_items(&pool, site).await,
        vec![in_set],
        "an edit that makes an item match does not join it"
    );

    change(
        &app,
        &site_uri(site),
        json!({ "action": "remove", "items": [in_set] }),
    )
    .await;
    patch_item(
        &app,
        in_set,
        json!({ "dcterms:title": title("A map, retitled") }),
    )
    .await;
    assert!(
        site_items(&pool, site).await.is_empty(),
        "an item removed by hand stays removed after an edit"
    );
}

#[tokio::test]
async fn items_and_item_sets_together_select_their_union() {
    let (pool, seed) = seeded_pool().await;
    let app = common::router(pool.clone());
    let site = add_site(&pool, "union", seed.user).await;
    let maps = add_item_set(&app, "Maps").await;
    let in_set = json!({ "o:item_set": [{ "o:id": maps }] });
    let first = add_item(&app, "First map", true, in_set.clone()).await;
    let second = add_item(&app, "Second map", true, in_set).await;
    let letter = add_item(&app, "A letter", true, json!({})).await;
    add_item(&app, "Another letter", true, json!({})).await;

    let body = json!({ "action": "add", "items": [letter, first], "item_sets": [maps] });
    let mut dry = body.clone();
    dry["dry_run"] = json!(true);
    let preview = change(&app, &site_uri(site), dry).await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.body);
    assert_eq!(
        preview.json(),
        json!({ "matched": 3, "added": 3, "removed": 0, "total": 3 }),
        "an item that is also in a picked set counts once"
    );
    assert!(
        site_items(&pool, site).await.is_empty(),
        "a dry run writes nothing"
    );

    let added = change(&app, &site_uri(site), body).await;
    assert_eq!(added.status, StatusCode::OK, "{}", added.body);
    assert_eq!(site_items(&pool, site).await, vec![first, second, letter]);
}

#[tokio::test]
async fn a_new_site_holds_no_items_and_does_not_sync() {
    let (pool, _seed) = seeded_pool().await;
    let app = common::router(pool.clone());
    add_item(&app, "A public print", true, json!({})).await;
    let created = send(
        app.clone(),
        json_request(
            "POST",
            &append_key("/api/sites"),
            &json!({ "o:title": "Fresh", "o:slug": "fresh", "o:theme": "default" }).to_string(),
        ),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let doc = created.json();
    assert_eq!(doc["o:assign_new_items"], json!(false));
    assert_eq!(doc["o:item_pool"], json!([]));
    let site = SiteId(doc["o:id"].as_u64().unwrap());
    assert!(site_items(&pool, site).await.is_empty());

    add_item(&app, "Made after the site", true, json!({})).await;
    assert!(
        site_items(&pool, site).await.is_empty(),
        "a site that does not sync takes no new item"
    );
}

#[tokio::test]
async fn only_a_site_editor_may_change_its_items() {
    let (pool, seed) = seeded_pool().await;
    let app = common::router(pool.clone());
    let site = add_site(&pool, "guarded", seed.user).await;
    let author = UserRepo::new(&pool)
        .create(&NewUser {
            email: "author@example.com".to_string(),
            name: "Author".to_string(),
            password_hash: None,
            role: "author".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    let identity = "authorkey0000000000000000000000a";
    ApiKeyRepo::new(&pool)
        .create(&NewApiKey {
            id: identity.to_string(),
            owner_id: author.id,
            label: "author".to_string(),
            credential_hash: bcrypt::hash("author-credential", 4).unwrap(),
            last_ip: None,
            last_accessed: None,
            created: None,
        })
        .await
        .unwrap();
    let body = json!({ "action": "add", "query": {} });

    let keyed = format!(
        "/api/sites/{}/items?key_identity={identity}&key_credential=author-credential",
        site.0
    );
    let denied = change(&app, &keyed, body.clone()).await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN, "{}", denied.body);

    let anonymous = change(&app, &format!("/api/sites/{}/items", site.0), body.clone()).await;
    assert_eq!(
        anonymous.status,
        StatusCode::FORBIDDEN,
        "{}",
        anonymous.body
    );
    assert!(site_items(&pool, site).await.is_empty());

    let missing = change(&app, &append_key("/api/sites/999999/items"), body).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND, "{}", missing.body);
}

/// `SiteAdapter::validateEntity` checks `is_array($itemPool)`, which a keyed array (a JSON object,
/// as `parse_str` stores a search) passes. A scalar pool is still refused.
#[tokio::test]
async fn a_saved_search_is_valid_item_pool_data() {
    let (pool, seed) = seeded_pool().await;
    let app = common::router(pool.clone());
    let site = add_site(&pool, "pooled", seed.user).await;
    let uri = append_key(&format!("/api/sites/{}", site.0));

    let saved = change_site(&app, &uri, json!({ "o:item_pool": { "is_public": "1" } })).await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
    assert_eq!(saved.json()["o:item_pool"], json!({ "is_public": "1" }));

    let scalar = change_site(&app, &uri, json!({ "o:item_pool": "all" })).await;
    assert_eq!(
        scalar.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        scalar.body
    );
}

async fn change_site(app: &Router, uri: &str, body: Value) -> Answer {
    send(app.clone(), json_request("PATCH", uri, &body.to_string())).await
}
