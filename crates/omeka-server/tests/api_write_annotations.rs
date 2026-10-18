//! Value annotation write tests: a posted `@annotation` becomes a
//! `value_annotation` resource, is returned on every read, is replaced on
//! update, and goes with its value on delete.
//!
//! `ValueHydrator` lines 134-153 hydrate a non-empty `@annotation` as a
//! `value_annotations` resource; `Entity/Value.php` line 67 maps it
//! `orphanRemoval`, so a value that leaves the set takes its annotation with it.
//! Every test drives the router, so what it asserts is what a caller sees
//! through HTTP.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{append_key, get, json_request, seeded_pool, send};
use sqlx::SqlitePool;

fn keyed(uri: &str) -> String {
    append_key(uri)
}

/// The seeded `dcterms` vocabulary, a `dcterms:description` property: the
/// fixture catalog carries only `dcterms:title`, and the annotation bodies below
/// name the description term.
async fn add_description_property(pool: &SqlitePool) -> u64 {
    let vocabulary: i64 = sqlx::query_scalar("SELECT id FROM vocabulary WHERE prefix = 'dcterms'")
        .fetch_one(pool)
        .await
        .unwrap();
    let property = omeka_db::repos::PropertyRepo::new(pool)
        .create(&omeka_db::rows::NewProperty {
            owner_id: None,
            vocabulary_id: omeka_db::ids::VocabularyId(vocabulary as u64),
            local_name: "description".to_string(),
            label: "Description".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    property.id.0
}

/// A create body whose `dcterms:title` value carries an `@annotation`.
fn annotated_title(annotation_text: &str) -> String {
    format!(
        r#"{{"o:is_public":true,
            "dcterms:title":[{{"property_id":"auto","type":"literal","@value":"Annotated title",
              "@annotation":{{"dcterms:description":[
                {{"property_id":"auto","type":"literal","@value":"{annotation_text}"}}]}}}}]}}"#
    )
}

/// `ValueHydrator` lines 134-153: a value that carries a non-empty
/// `@annotation` creates a `value_annotation` resource whose values the read
/// side returns as the value's `@annotation`.
#[tokio::test]
async fn a_posted_annotation_is_stored_and_returned() {
    let (pool, _seed) = seeded_pool().await;
    add_description_property(&pool).await;
    let app = common::router(pool);

    let created = send(
        app,
        json_request(
            "POST",
            &keyed("/api/items"),
            &annotated_title("a note on the title"),
        ),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let document = created.json();
    assert_eq!(
        document["dcterms:title"][0]["@annotation"]["dcterms:description"][0]["@value"],
        "a note on the title"
    );
}

/// A following read returns the same `@annotation` the create answered with.
#[tokio::test]
async fn a_read_returns_the_stored_annotation() {
    let (pool, _seed) = seeded_pool().await;
    add_description_property(&pool).await;
    let app = common::router(pool);

    let created = send(
        app.clone(),
        json_request(
            "POST",
            &keyed("/api/items"),
            &annotated_title("a note on the title"),
        ),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let id = created.json()["o:id"].as_u64().unwrap();

    let read = send(app, get(&format!("/api/items/{id}"), false)).await;
    assert_eq!(read.status, StatusCode::OK, "{}", read.body);
    assert_eq!(
        read.json()["dcterms:title"][0]["@annotation"]["dcterms:description"][0]["@value"],
        "a note on the title"
    );
}

/// A `PUT` re-hydrates the value, so the annotation is replaced rather than
/// accumulated: the response carries the new text and not the old.
#[tokio::test]
async fn an_update_replaces_the_annotation() {
    let (pool, _seed) = seeded_pool().await;
    add_description_property(&pool).await;
    let app = common::router(pool);

    let created = send(
        app.clone(),
        json_request(
            "POST",
            &keyed("/api/items"),
            &annotated_title("the first note"),
        ),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let id = created.json()["o:id"].as_u64().unwrap();

    let updated = send(
        app,
        json_request(
            "PUT",
            &keyed(&format!("/api/items/{id}")),
            &annotated_title("the second note"),
        ),
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    let document = updated.json();
    let annotations = &document["dcterms:title"][0]["@annotation"]["dcterms:description"];
    assert_eq!(annotations[0]["@value"], "the second note");
    assert!(
        !updated.body.contains("the first note"),
        "the replaced annotation is still stored: {}",
        updated.body
    );
}

/// `ValueHydrator` line 162: a partial update that names no value keeps the
/// stored values, and so keeps their annotations.
#[tokio::test]
async fn a_patch_that_names_no_value_keeps_the_annotation() {
    let (pool, _seed) = seeded_pool().await;
    add_description_property(&pool).await;
    let app = common::router(pool);

    let created = send(
        app.clone(),
        json_request(
            "POST",
            &keyed("/api/items"),
            &annotated_title("a note on the title"),
        ),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let id = created.json()["o:id"].as_u64().unwrap();

    let patched = send(
        app,
        json_request(
            "PATCH",
            &keyed(&format!("/api/items/{id}")),
            r#"{"o:is_public":false}"#,
        ),
    )
    .await;
    assert_eq!(patched.status, StatusCode::OK, "{}", patched.body);
    assert_eq!(
        patched.json()["dcterms:title"][0]["@annotation"]["dcterms:description"][0]["@value"],
        "a note on the title"
    );
}

/// `Entity/Value.php` line 67: the annotation is `orphanRemoval`, so deleting
/// the item removes the annotation's `resource` row and its `value` rows.
#[tokio::test]
async fn a_delete_takes_the_annotation_with_the_value() {
    let (pool, _seed) = seeded_pool().await;
    add_description_property(&pool).await;
    let app = common::router(pool.clone());

    let created = send(
        app.clone(),
        json_request(
            "POST",
            &keyed("/api/items"),
            &annotated_title("a note on the title"),
        ),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let id = created.json()["o:id"].as_u64().unwrap();

    let annotation_id: i64 = sqlx::query_scalar(
        "SELECT value_annotation_id FROM value \
         WHERE resource_id = ? AND value_annotation_id IS NOT NULL",
    )
    .bind(id as i64)
    .fetch_one(&pool)
    .await
    .unwrap();

    let deleted = send(
        app,
        json_request("DELETE", &keyed(&format!("/api/items/{id}")), ""),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.body);

    let resource_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM resource WHERE id = ?")
        .bind(annotation_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(resource_rows, 0, "the annotation resource row survives");
    let value_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM value WHERE resource_id = ?")
        .bind(annotation_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(value_rows, 0, "the annotation value rows survive");
}

/// `ValueRepresentation` lines 66-95: `@annotation` appears only when the value
/// carries a non-empty annotation, so a plain value has no such member.
#[tokio::test]
async fn a_value_without_an_annotation_has_no_annotation_member() {
    let (pool, _seed) = seeded_pool().await;
    add_description_property(&pool).await;
    let app = common::router(pool);

    let body = r#"{"o:is_public":true,
        "dcterms:title":[{"property_id":"auto","type":"literal","@value":"Plain title"}]}"#;
    let created = send(app, json_request("POST", &keyed("/api/items"), body)).await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let document = created.json();
    assert_eq!(document["dcterms:title"][0]["@value"], "Plain title");
    assert!(
        document["dcterms:title"][0].get("@annotation").is_none(),
        "a plain value grew an annotation: {}",
        created.body
    );
}
