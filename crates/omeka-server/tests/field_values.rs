//! One field's texts across the archive, and moving one text to another field
//! (`omeka_api::field_values`), through the router.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod tag_fixtures;

use axum::http::StatusCode;
use common::{get, request, router, send};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tag_fixtures::{
    Caller, archive, item, item_as, keyed, literal, pairs, post, post_as, read, template, values_of,
};

async fn read_as(pool: &SqlitePool, caller: Caller, path: &str) -> Value {
    let answer = send(router(pool.clone()), request("GET", &keyed(path, caller))).await;
    assert_eq!(answer.status, StatusCode::OK, "{path}: {}", answer.body);
    answer.json()
}

async fn get_status(pool: &SqlitePool, path: &str) -> StatusCode {
    send(router(pool.clone()), get(path, true)).await.status
}

/// A media with a subject on `item`.
async fn media(pool: &SqlitePool, item: u64, subjects: Vec<Value>) -> u64 {
    let answer = post(
        pool,
        "/api/media",
        &json!({
            "o:ingester": "html",
            "html": "<p>scan</p>",
            "o:item": {"o:id": item},
            "dcterms:subject": subjects,
        }),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    answer.json()["o:id"].as_u64().unwrap()
}

async fn media_values(pool: &SqlitePool, media: u64, term: &str) -> Vec<String> {
    read(pool, &format!("/api/media/{media}")).await[term]
        .as_array()
        .map(|values| {
            values
                .iter()
                .map(|value| value["@value"].as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// The census counts each text once per resource, by kind, with what the caller may edit, most
/// used first; another field's values and link values stay out.
#[tokio::test]
async fn the_census_counts_each_text_by_kind_and_by_what_the_caller_may_edit() {
    let archive = archive().await;
    let pool = &archive.pool;
    let letter = item(
        pool,
        "Letter",
        vec![literal("Bidar"), literal("Bidar"), literal("Coin")],
    )
    .await;
    item_as(
        pool,
        Caller::Author,
        "Author's",
        vec![
            literal("Bidar"),
            json!({"type": "uri", "property_id": "auto", "@id": "http://example.org/bidar", "o:label": "Bidar"}),
        ],
        json!({"o:is_public": true, "dcterms:spatial": [literal("Coin")]}),
    )
    .await;
    media(pool, letter, vec![literal("Bidar")]).await;

    let path = format!("/api/romeka/field-values/{}", archive.subject.0);
    let census = read_as(pool, Caller::Author, &path).await;
    assert_eq!(
        census,
        json!({
            "property": archive.subject.0,
            "values": [
                {"value": "Bidar", "items": 2, "media": 1, "item_sets": 0, "editable": 1},
                {"value": "Coin", "items": 1, "media": 0, "item_sets": 0, "editable": 0},
            ],
        })
    );
}

/// A move takes the text off one field and puts it on the other, on what the caller may edit; a
/// resource that already holds it there just loses it from the source. A dry run answers the same
/// and writes nothing.
#[tokio::test]
async fn a_move_retargets_merges_and_skips_and_a_dry_run_writes_nothing() {
    let archive = archive().await;
    let pool = &archive.pool;
    let plain = item_as(
        pool,
        Caller::Author,
        "Plain",
        vec![literal("Bidar"), literal("Coin")],
        json!({}),
    )
    .await;
    let already = item_as(
        pool,
        Caller::Author,
        "Already",
        vec![literal("Bidar")],
        json!({"dcterms:spatial": [literal("Bidar")]}),
    )
    .await;
    let admins = item_as(
        pool,
        Caller::Admin,
        "Admin's",
        vec![literal("Bidar")],
        json!({"o:is_public": true}),
    )
    .await;
    let body = json!({
        "value": "Bidar",
        "from": archive.subject.0,
        "to": archive.spatial.0,
        "dry_run": true,
    });
    let expected = json!({
        "changed": 2,
        "skipped": 1,
        "moved": [plain],
        "merged": [already],
        "copied": [],
        "not_in_form": [],
    });

    let preview = post_as(pool, Caller::Author, "/api/romeka/field-values/move", &body).await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.body);
    assert_eq!(preview.json(), expected);
    assert_eq!(values_of(pool, plain, "dcterms:spatial").await, pairs(&[]));

    let entries = |history: Value| history["entries"].as_array().unwrap().len();
    let history_before = entries(read(pool, &format!("/api/romeka/items/{plain}/history")).await);
    let mut body = body;
    body["dry_run"] = json!(false);
    let done = post_as(pool, Caller::Author, "/api/romeka/field-values/move", &body).await;
    assert_eq!(done.json(), expected);
    assert_eq!(
        values_of(pool, plain, "dcterms:subject").await,
        pairs(&[("literal", "Coin")])
    );
    assert_eq!(
        values_of(pool, plain, "dcterms:spatial").await,
        pairs(&[("literal", "Bidar")])
    );
    assert_eq!(
        values_of(pool, already, "dcterms:subject").await,
        pairs(&[])
    );
    assert_eq!(
        values_of(pool, already, "dcterms:spatial").await,
        pairs(&[("literal", "Bidar")])
    );
    assert_eq!(
        values_of(pool, admins, "dcterms:subject").await,
        pairs(&[("literal", "Bidar")])
    );
    let history = read(pool, &format!("/api/romeka/items/{plain}/history")).await;
    assert_eq!(
        entries(history),
        history_before + 1,
        "a move records one history entry"
    );
}

/// The answer of a move is its own undo: the same move back, only on the moved resources, with
/// the merged ones copied, restores every field as it was. It reaches media too.
#[tokio::test]
async fn the_reverse_move_with_only_and_copy_restores_every_field() {
    let archive = archive().await;
    let pool = &archive.pool;
    let plain = item(pool, "Plain", vec![literal("Bidar")]).await;
    let already = item_as(
        pool,
        Caller::Admin,
        "Already",
        vec![literal("Bidar")],
        json!({"dcterms:spatial": [literal("Bidar")]}),
    )
    .await;
    let placed = item_as(
        pool,
        Caller::Admin,
        "Placed",
        vec![],
        json!({"dcterms:spatial": [literal("Bidar")]}),
    )
    .await;
    let scan = media(pool, plain, vec![literal("Bidar")]).await;
    let snapshot = || async {
        let mut fields = Vec::new();
        for id in [plain, already, placed] {
            fields.push(values_of(pool, id, "dcterms:subject").await);
            fields.push(values_of(pool, id, "dcterms:spatial").await);
        }
        fields.push(pairs(
            &media_values(pool, scan, "dcterms:subject")
                .await
                .iter()
                .map(|text| ("literal", text.as_str()))
                .collect::<Vec<_>>(),
        ));
        fields
    };
    let before = snapshot().await;

    let done = post(
        pool,
        "/api/romeka/field-values/move",
        &json!({"value": "Bidar", "from": archive.subject.0, "to": archive.spatial.0}),
    )
    .await
    .json();
    assert_eq!(done["moved"], json!([plain, scan]));
    assert_eq!(done["merged"], json!([already]));
    assert_eq!(
        media_values(pool, scan, "dcterms:spatial").await,
        vec!["Bidar"]
    );

    let undone = post(
        pool,
        "/api/romeka/field-values/move",
        &json!({
            "value": "Bidar",
            "from": archive.spatial.0,
            "to": archive.subject.0,
            "only": done["moved"],
            "copy": done["merged"],
        }),
    )
    .await
    .json();
    assert_eq!(undone["moved"], json!([plain, scan]));
    assert_eq!(undone["copied"], json!([already]));
    assert_eq!(snapshot().await, before);
}

/// The answer names each item type whose form lacks the target field, with how many of the
/// changed resources use it.
#[tokio::test]
async fn a_move_names_the_item_types_whose_form_lacks_the_target() {
    let archive = archive().await;
    let pool = &archive.pool;
    let narrative = template(pool, "Photo Narrative", Some(archive.subject)).await;
    let mapped = template(pool, "Mapped", Some(archive.spatial)).await;
    for (title, template) in [("One", narrative), ("Two", narrative), ("Three", mapped)] {
        item_as(
            pool,
            Caller::Admin,
            title,
            vec![literal("Bidar")],
            json!({"o:resource_template": {"o:id": template.0}}),
        )
        .await;
    }
    item(pool, "No type", vec![literal("Bidar")]).await;
    let answer = post(
        pool,
        "/api/romeka/field-values/move",
        &json!({"value": "Bidar", "from": archive.subject.0, "to": archive.spatial.0, "dry_run": true}),
    )
    .await
    .json();
    assert_eq!(answer["changed"], json!(4));
    assert_eq!(
        answer["not_in_form"],
        json!([{"template": narrative.0, "label": "Photo Narrative", "resources": 2}])
    );
}

/// A move names two different known fields and a text with a letter in it.
#[tokio::test]
async fn a_move_refuses_one_field_twice_an_unknown_field_and_an_empty_text() {
    let archive = archive().await;
    let pool = &archive.pool;
    let move_ =
        |body: Value| async move { post(pool, "/api/romeka/field-values/move", &body).await };
    let (subject, spatial) = (archive.subject.0, archive.spatial.0);
    let same = move_(json!({"value": "Bidar", "from": subject, "to": subject})).await;
    assert_eq!(same.status, StatusCode::BAD_REQUEST, "{}", same.body);
    let unknown = move_(json!({"value": "Bidar", "from": subject, "to": 999_999})).await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND, "{}", unknown.body);
    let empty = move_(json!({"value": "  ", "from": subject, "to": spatial})).await;
    assert_eq!(empty.status, StatusCode::BAD_REQUEST, "{}", empty.body);
    let missing = get_status(pool, "/api/romeka/field-values/999999").await;
    assert_eq!(missing, StatusCode::NOT_FOUND);
}
