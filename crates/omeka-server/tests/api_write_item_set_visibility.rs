//! `ItemAdapter::hydrate`'s `o:item_set` (`Api/Adapter/ItemAdapter.php:307-337`)
//! under `ResourceVisibilityFilter`.
//!
//! A set the item does not hold yet is looked up with `findEntity`, whose query
//! the visibility filter narrows to public sets, the caller's own, or every set
//! for a role with `view-all`. A private set someone else owns is therefore the
//! same 404 as a missing one. The filter also narrows the item's loaded set
//! collection, so a full update cannot see, and so cannot drop, a link to a set
//! the caller cannot see. `o:is_open` plays no part: a closed set takes items.
//!
//! Every answer here was first read from the upstream oracle at `5e413f0`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::StatusCode;
use common::{append_key, caller, json_request, seeded_pool, send};
use serde_json::{Value, json};

async fn write(app: &Router, method: &str, uri: &str, body: Value) -> (StatusCode, Value) {
    let answer = send(app.clone(), json_request(method, uri, &body.to_string())).await;
    (answer.status, answer.json())
}

/// The admin's new item set, as its id.
async fn admin_set(app: &Router, body: Value) -> u64 {
    let (status, set) = write(app, "POST", &append_key("/api/item_sets"), body).await;
    assert_eq!(status, StatusCode::OK, "{set}");
    set["o:id"].as_u64().unwrap()
}

fn not_found(set: u64) -> Value {
    json!({"errors": {"error": format!(
        "Omeka\\Entity\\ItemSet entity with criteria {{\"id\":{set}}} not found"
    )}})
}

fn set_ids(item: &Value) -> Vec<u64> {
    item["o:item_set"]
        .as_array()
        .unwrap()
        .iter()
        .map(|set| set["o:id"].as_u64().unwrap())
        .collect()
}

#[tokio::test]
async fn item_adapter_finds_a_new_item_set_through_the_visibility_filter() {
    let (pool, _seed) = seeded_pool().await;
    let author = caller(&pool, "author").await;
    let editor = caller(&pool, "editor").await;
    let app = common::router(pool);
    let private = admin_set(&app, json!({"o:is_public": false})).await;
    let public = admin_set(&app, json!({"o:is_public": true})).await;
    let closed = admin_set(&app, json!({"o:is_public": true, "o:is_open": false})).await;
    let items = author.uri("/api/items");

    for named in [json!([{"o:id": private}]), json!([private])] {
        let (status, body) = write(&app, "POST", &items, json!({"o:item_set": named})).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{named}: {body}");
        assert_eq!(body, not_found(private), "{named}");
    }
    for open_or_not in [public, closed] {
        let (status, body) = write(
            &app,
            "POST",
            &items,
            json!({"o:item_set": [{"o:id": open_or_not}]}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(set_ids(&body), vec![open_or_not]);
    }

    let (status, body) = write(
        &app,
        "POST",
        &editor.uri("/api/items"),
        json!({"o:item_set": [{"o:id": private}]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "view-all sees every set: {body}");
    assert_eq!(set_ids(&body), vec![private]);
}

#[tokio::test]
async fn item_adapter_keeps_a_link_to_a_set_the_caller_cannot_see() {
    let (pool, _seed) = seeded_pool().await;
    let author = caller(&pool, "author").await;
    let app = common::router(pool);
    let private = admin_set(&app, json!({"o:is_public": false})).await;
    let public = admin_set(&app, json!({"o:is_public": true})).await;
    let (status, item) = write(
        &app,
        "POST",
        &append_key("/api/items"),
        json!({
            "o:owner": {"o:id": author.id.0},
            "o:item_set": [{"o:id": private}, {"o:id": public}],
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{item}");
    let path = format!("/api/items/{}", item["o:id"]);
    let as_admin = || async {
        let read = send(app.clone(), common::get(&append_key(&path), false)).await;
        set_ids(&read.json())
    };

    let (status, body) = write(
        &app,
        "PUT",
        &author.uri(&path),
        json!({"o:item_set": [{"o:id": public}]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        set_ids(&body),
        vec![public],
        "the author sees only the public set"
    );
    assert_eq!(as_admin().await, vec![private, public]);

    let (status, body) = write(
        &app,
        "PUT",
        &author.uri(&path),
        json!({"o:item_set": [{"o:id": private}, {"o:id": public}]}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body, not_found(private));

    let (status, body) = write(&app, "PUT", &author.uri(&path), json!({"o:item_set": []})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(set_ids(&body), Vec::<u64>::new());
    assert_eq!(as_admin().await, vec![private]);
}

/// `ItemRepresentation::itemSets` reads the same filtered collection, so a
/// reader who cannot see a set does not see the item's link to it either.
#[tokio::test]
async fn item_representation_lists_only_the_sets_the_reader_can_see() {
    let (pool, _seed) = seeded_pool().await;
    let author = caller(&pool, "author").await;
    let app = common::router(pool);
    let private = admin_set(&app, json!({"o:is_public": false})).await;
    let public = admin_set(&app, json!({"o:is_public": true})).await;
    let (_, item) = write(
        &app,
        "POST",
        &append_key("/api/items"),
        json!({"o:item_set": [{"o:id": private}, {"o:id": public}]}),
    )
    .await;
    let path = format!("/api/items/{}", item["o:id"]);

    let anonymous = send(app.clone(), common::get(&path, false)).await;
    assert_eq!(set_ids(&anonymous.json()), vec![public]);
    let listed = send(
        app.clone(),
        common::get(&format!("/api/items?id={}", item["o:id"]), false),
    )
    .await;
    assert_eq!(set_ids(&listed.json()[0]), vec![public]);
    let as_author = send(app.clone(), json_request("GET", &author.uri(&path), "")).await;
    assert_eq!(set_ids(&as_author.json()), vec![public]);
    let as_admin = send(app, common::get(&append_key(&path), false)).await;
    assert_eq!(set_ids(&as_admin.json()), vec![private, public]);
}
