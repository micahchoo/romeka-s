//! Write-surface fidelity tests: the five divergences from upstream at
//! 5e413f0 that the extended oracle probe (`x_*` steps) found on 2026-10-17.
//!
//! Every test drives the router, so what it asserts is what a caller sees
//! through HTTP. Each test is named after the upstream behaviour it freezes,
//! and the `x_*` probe step it matches is named in its doc comment.
//!
//! The oracle answers below were captured with `curl` against
//! `http://localhost:8899` (see `REPORT.md` for the commands).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{append_key, get, json_request, request, seeded_pool, seeded_router, send, site_pool};
use serde_json::{Value, json};
use sqlx::SqlitePool;

fn keyed(uri: &str) -> String {
    append_key(uri)
}

/// A `POST /api/users` body with one invalid email, for the validator message
/// test and for the resources that need a throwaway owner.
fn user_body(email: &str) -> String {
    format!(r#"{{"o:name":"Fidelity","o:email":"{email}","o:role":"researcher"}}"#)
}

/// `ValueHydrator` line 162: on a partial update the stored values are removed
/// only when the body named at least one valid value. A `PATCH` that names none
/// keeps them.
///
/// Matches the probe steps `x_item_set_patch_closed` and
/// `x_item_set_patch_show`, and the same fault on items and media.
#[tokio::test]
async fn a_patch_that_names_no_value_keeps_the_stored_values() {
    let (pool, seed) = seeded_pool().await;
    let description = add_property(&pool, "description", "Description").await;
    let app = common::router(pool);

    let patched = send(
        app.clone(),
        json_request(
            "PATCH",
            &keyed(&format!("/api/items/{}", seed.item.0)),
            r#"{"o:is_public":true}"#,
        ),
    )
    .await;
    assert_eq!(patched.status, StatusCode::OK, "{}", patched.body);
    let item = patched.json();
    assert!(
        item.get("dcterms:title").is_some(),
        "the item PATCH dropped its values: {}",
        patched.body
    );
    assert_eq!(item["o:title"], "A picture of a cat");

    let patched = send(
        app.clone(),
        json_request(
            "PATCH",
            &keyed(&format!("/api/item_sets/{}", seed.item_set.0)),
            r#"{"o:is_open":false}"#,
        ),
    )
    .await;
    assert_eq!(patched.status, StatusCode::OK, "{}", patched.body);
    let item_set = patched.json();
    assert_eq!(item_set["o:is_open"], false);
    assert!(
        item_set.get("dcterms:title").is_some(),
        "the item set PATCH dropped its values: {}",
        patched.body
    );

    let patched = send(
        app.clone(),
        json_request(
            "PATCH",
            &keyed(&format!("/api/media/{}", seed.media.0)),
            r#"{"o:alt_text":"alt"}"#,
        ),
    )
    .await;
    assert_eq!(patched.status, StatusCode::OK, "{}", patched.body);
    let media = patched.json();
    assert_eq!(media["o:alt_text"], "alt");
    assert!(
        media.get("dcterms:title").is_some(),
        "the media PATCH dropped its values: {}",
        patched.body
    );

    // A PATCH that names a value still replaces the stored set, so the rule is
    // a guard on "nothing named", not on "PATCH" (`x_item_patch`).
    let patched = send(
        app,
        json_request(
            "PATCH",
            &keyed(&format!("/api/items/{}", seed.item.0)),
            &format!(
                r#"{{"dcterms:description":[{{"property_id":{description},"type":"literal","@value":"patched"}}]}}"#
            ),
        ),
    )
    .await;
    assert_eq!(patched.status, StatusCode::OK, "{}", patched.body);
    let item = patched.json();
    assert_eq!(item["dcterms:description"][0]["@value"], "patched");
    assert!(
        item.get("dcterms:title").is_none(),
        "a PATCH that named a value kept the replaced title: {}",
        patched.body
    );
}

/// `DataType\Resource\AbstractResource::hydrate`: an unknown value resource
/// throws before the flush, so the answer is the 404 and no row is written.
///
/// Matches the probe step `x_item_bad_resource_value`.
#[tokio::test]
async fn an_unknown_value_resource_answers_404_and_writes_nothing() {
    let (pool, _seed) = seeded_pool().await;
    // The fixture catalog holds only `dcterms:title`; add the term the probe
    // body uses so the unknown-term check does not answer first.
    let relation = add_property(&pool, "relation", "Relation").await;
    let app = common::router(pool);

    let before = send(app.clone(), get("/api/items", false)).await;
    assert_eq!(before.status, StatusCode::OK);

    let body = format!(
        r#"{{"dcterms:relation":[{{"property_id":{relation},"type":"resource","value_resource_id":999999}}]}}"#
    );
    let answer = send(
        app.clone(),
        json_request("POST", &keyed("/api/items"), &body),
    )
    .await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND, "{}", answer.body);
    assert_eq!(
        answer.json()["errors"]["error"],
        "Resource not found with id 999999."
    );

    let after = send(app, get("/api/items", false)).await;
    assert_eq!(
        after.body, before.body,
        "a failed create changed the item listing"
    );
}

/// `AbstractEntityAdapter::findEntity`: an unknown id in `o:item_set`,
/// `o:owner`, `o:resource_class`, `o:resource_template` or a media's `o:item`
/// answers the 404 `findEntity` throws, naming the class and the numeric id.
#[tokio::test]
async fn an_unknown_reference_member_answers_the_upstream_404() {
    let (pool, _seed) = seeded_pool().await;
    let app = common::router(pool);

    let cases = [
        (
            "/api/items",
            r#"{"o:item_set":[{"o:id":999999}]}"#,
            "Omeka\\Entity\\ItemSet entity with criteria {\"id\":999999} not found",
        ),
        (
            "/api/items",
            r#"{"o:owner":{"o:id":999999}}"#,
            "Omeka\\Entity\\User entity with criteria {\"id\":999999} not found",
        ),
        (
            "/api/items",
            r#"{"o:resource_class":{"o:id":999999}}"#,
            "Omeka\\Entity\\ResourceClass entity with criteria {\"id\":999999} not found",
        ),
        (
            "/api/items",
            r#"{"o:resource_template":{"o:id":999999}}"#,
            "Omeka\\Entity\\ResourceTemplate entity with criteria {\"id\":999999} not found",
        ),
        (
            "/api/media",
            r#"{"o:ingester":"html","o:renderer":"html","o:item":{"o:id":999999},"data":{"html":"x"}}"#,
            "Omeka\\Entity\\Item entity with criteria {\"id\":999999} not found",
        ),
    ];
    for (path, body, message) in cases {
        let answer = send(app.clone(), json_request("POST", &keyed(path), body)).await;
        assert_eq!(
            answer.status,
            StatusCode::NOT_FOUND,
            "{path}: {}",
            answer.body
        );
        assert_eq!(answer.json()["errors"]["error"], message, "{path}");
    }
}

/// Upstream hydrates the whole entity before its one flush, so an update whose
/// body names an unknown reference changes nothing. The port resolves the
/// references before the first write for the same reason.
#[tokio::test]
async fn a_failed_update_leaves_the_stored_row_unchanged() {
    let (pool, seed) = seeded_pool().await;
    let app = common::router(pool);

    let before = send(
        app.clone(),
        get(&format!("/api/items/{}", seed.item.0), false),
    )
    .await;
    assert_eq!(before.status, StatusCode::OK);
    let changed = r#"{"o:item_set":[{"o:id":999999}],
        "dcterms:title":[{"property_id":"auto","type":"literal","@value":"changed"}]}"#;
    let answer = send(
        app.clone(),
        json_request(
            "PUT",
            &keyed(&format!("/api/items/{}", seed.item.0)),
            changed,
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND, "{}", answer.body);
    let after = send(
        app.clone(),
        get(&format!("/api/items/{}", seed.item.0), false),
    )
    .await;
    assert_eq!(
        after.body, before.body,
        "a failed item update changed the row"
    );

    // A site PUT names its members in turn: an unknown user, item set or page
    // must each stop the write before it starts.
    let site_before = send(
        app.clone(),
        get(&format!("/api/sites/{}", seed.site), false),
    )
    .await;
    assert_eq!(site_before.status, StatusCode::OK);
    let cases = [
        (
            r#"{"o:slug":"museum","o:title":"changed","o:theme":"default",
                "o:site_permission":[{"o:user":{"o:id":999999},"o:role":"editor"}]}"#,
            "Omeka\\Entity\\User entity with criteria {\"id\":999999} not found",
        ),
        (
            r#"{"o:slug":"museum","o:title":"changed","o:theme":"default",
                "o:site_item_set":[{"o:item_set":{"o:id":999999}}]}"#,
            "Omeka\\Entity\\ItemSet entity with criteria {\"id\":999999} not found",
        ),
        (
            r#"{"o:slug":"museum","o:title":"changed","o:theme":"default",
                "o:page":[{"o:id":999999}]}"#,
            "Omeka\\Entity\\SitePage entity with criteria {\"id\":999999} not found",
        ),
    ];
    for (body, message) in cases {
        let answer = send(
            app.clone(),
            json_request("PUT", &keyed(&format!("/api/sites/{}", seed.site)), body),
        )
        .await;
        assert_eq!(answer.status, StatusCode::NOT_FOUND, "{}", answer.body);
        assert_eq!(answer.json()["errors"]["error"], message);
        let after = send(
            app.clone(),
            get(&format!("/api/sites/{}", seed.site), false),
        )
        .await;
        assert_eq!(
            after.body, site_before.body,
            "a failed site update changed the row"
        );
    }
}

/// `SiteAdapter::hydrate` for `o:page`: `shouldHydrate` is true on a `PUT`, the
/// default is `[]`, and every page the body does not name is removed. A
/// `PATCH` that omits the member keeps them.
///
/// Matches the probe steps `x_site_put_navigation` and `x_site_show_after_put`.
#[tokio::test]
async fn a_site_put_without_o_page_removes_its_pages() {
    let (pool, seed) = site_pool().await;
    let app = common::router(pool);
    let site = format!("/api/sites/{}", seed.site);

    let patched = send(
        app.clone(),
        json_request("PATCH", &keyed(&site), r#"{"o:title":"Renamed"}"#),
    )
    .await;
    assert_eq!(patched.status, StatusCode::OK, "{}", patched.body);
    let pages = patched.json()["o:page"].as_array().unwrap().len();
    assert!(
        pages > 1,
        "the fixture site should have pages: {}",
        patched.body
    );

    // A PUT naming one page keeps that page and removes the rest.
    let kept = patched.json()["o:page"][0]["o:id"].clone();
    let body = format!(
        r#"{{"o:slug":"museum","o:title":"Renamed again","o:theme":"default","o:page":[{{"o:id":{kept}}}]}}"#
    );
    let put = send(app.clone(), json_request("PUT", &keyed(&site), &body)).await;
    assert_eq!(put.status, StatusCode::OK, "{}", put.body);
    let named = put.json();
    let named_pages = named["o:page"].as_array().unwrap();
    assert_eq!(named_pages.len(), 1, "{}", put.body);
    assert_eq!(named_pages[0]["o:id"], kept);

    // A PUT without `o:page` removes every page.
    let put = send(
        app.clone(),
        json_request(
            "PUT",
            &keyed(&site),
            r#"{"o:slug":"museum","o:title":"Renamed again","o:theme":"default"}"#,
        ),
    )
    .await;
    assert_eq!(put.status, StatusCode::OK, "{}", put.body);
    assert_eq!(
        put.json()["o:page"],
        json!([]),
        "the site PUT kept its pages: {}",
        put.body
    );

    let read = send(app, get(&site, false)).await;
    assert_eq!(read.status, StatusCode::OK, "{}", read.body);
    assert_eq!(read.json()["o:page"], json!([]));
}

/// `Api\Manager::delete` renders the entity after the flush has detached it:
/// `o:id` null, `@id` the collection URL, and every association the delete
/// cascaded away as a detached reference.
///
/// Matches the probe step `x_media_after_item_delete`.
#[tokio::test]
async fn a_delete_answers_the_detached_item() {
    let (app, seed) = seeded_router().await;

    let deleted = send(
        app,
        request("DELETE", &keyed(&format!("/api/items/{}", seed.item.0))),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.body);
    let item = deleted.json();
    assert_eq!(item["o:id"], Value::Null, "{}", deleted.body);
    assert_eq!(item["@id"], "http://localhost/api/items");
    assert_eq!(item["o:item_set"], json!([]));
    assert_eq!(
        item["o:media"],
        json!([{ "@id": "http://localhost/api/media", "o:id": null }])
    );
    assert_eq!(
        item["o:primary_media"],
        json!({ "@id": "http://localhost/api/media", "o:id": null })
    );
    assert!(
        item.get("dcterms:title").is_some(),
        "the detached item lost its values: {}",
        deleted.body
    );
}

/// The same rule for the other deletes the probe covers: an item set's
/// `o:items` link drops its query, a user has no `@reverse`, a media keeps its
/// live `o:item`, and a site's pages become detached references.
///
/// Matches the probe steps `x_item_set_delete`, `x_user_delete` and
/// `x_item_delete`.
#[tokio::test]
async fn a_delete_answers_the_detached_item_set_user_media_and_site() {
    let (pool, seed) = seeded_pool().await;
    let app = common::router(pool);

    let deleted = send(
        app.clone(),
        request(
            "DELETE",
            &keyed(&format!("/api/item_sets/{}", seed.item_set.0)),
        ),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.body);
    let item_set = deleted.json();
    assert_eq!(item_set["o:id"], Value::Null);
    assert_eq!(item_set["@id"], "http://localhost/api/item_sets");
    assert_eq!(
        item_set["o:items"],
        json!({ "@id": "http://localhost/api/items" })
    );

    let deleted = send(
        app.clone(),
        request("DELETE", &keyed(&format!("/api/media/{}", seed.media.0))),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.body);
    let media = deleted.json();
    assert_eq!(media["o:id"], Value::Null);
    assert_eq!(media["@id"], "http://localhost/api/media");
    assert_eq!(
        media["o:item"],
        json!({ "@id": format!("http://localhost/api/items/{}", seed.item.0), "o:id": seed.item.0 })
    );

    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/users"), &user_body("gone@example.com")),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let user = created.json()["o:id"].as_u64().unwrap();
    let deleted = send(
        app.clone(),
        request("DELETE", &keyed(&format!("/api/users/{user}"))),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.body);
    let document = deleted.json();
    assert_eq!(document["o:id"], Value::Null);
    assert_eq!(document["@id"], "http://localhost/api/users");
    assert_eq!(document["o:email"], "gone@example.com");
}

/// `SiteRepresentation::pages()` for a site the delete cascaded: each page keeps
/// its place as a detached reference, and `o:site_item` drops its `site_id`.
#[tokio::test]
async fn a_delete_answers_a_site_with_detached_pages() {
    let (pool, seed) = site_pool().await;
    let app = common::router(pool);

    let before = send(
        app.clone(),
        request("GET", &keyed(&format!("/api/sites/{}", seed.site))),
    )
    .await;
    let pages = before.json()["o:page"].as_array().unwrap().len();
    assert!(
        pages > 1,
        "the fixture site should have pages: {}",
        before.body
    );

    let deleted = send(
        app,
        request("DELETE", &keyed(&format!("/api/sites/{}", seed.site))),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.body);
    let site = deleted.json();
    assert_eq!(site["o:id"], Value::Null);
    assert_eq!(site["@id"], "http://localhost/api/sites");
    let detached = site["o:page"].as_array().unwrap();
    assert_eq!(detached.len(), pages, "{}", deleted.body);
    for page in detached {
        assert_eq!(
            page,
            &json!({ "@id": "http://localhost/api/site_pages", "o:id": null })
        );
    }
    assert_eq!(
        site["o:site_item"],
        json!({ "@id": "http://localhost/api/items" })
    );
}

/// `Laminas\Validator\EmailAddress` at `INVALID_FORMAT`: the message
/// `UserAdapter::validateEntity` reports for `not-an-email`.
///
/// Matches the probe step `x_user_bad_email`.
#[tokio::test]
async fn an_invalid_email_answers_the_upstream_message() {
    let (app, _seed) = seeded_router().await;
    let answer = send(
        app,
        json_request("POST", &keyed("/api/users"), &user_body("not-an-email")),
    )
    .await;
    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        answer.json()["errors"]["o:email"][0],
        "The input is not a valid email address. Use the basic format local-part@hostname"
    );
}

/// One property on the seeded `dcterms` vocabulary, for a body that names a
/// term the fixture catalog does not carry.
async fn add_property(pool: &SqlitePool, local_name: &str, label: &str) -> u64 {
    let vocabulary: i64 = sqlx::query_scalar("SELECT id FROM vocabulary WHERE prefix = 'dcterms'")
        .fetch_one(pool)
        .await
        .unwrap();
    let property = omeka_db::repos::PropertyRepo::new(pool)
        .create(&omeka_db::rows::NewProperty {
            owner_id: None,
            vocabulary_id: omeka_db::ids::VocabularyId(vocabulary as u64),
            local_name: local_name.to_string(),
            label: label.to_string(),
            comment: None,
        })
        .await
        .unwrap();
    property.id.0
}
