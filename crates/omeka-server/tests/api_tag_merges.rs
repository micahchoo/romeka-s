//! Tags through the router: merging spellings and remembering spellings that differ
//! (`omeka_api::tagging`), for every tag and for one term list's words.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod tag_fixtures;

use axum::http::StatusCode;
use common::{get, router, send};
use serde_json::json;
use tag_fixtures::{
    Caller, archive, item, item_as, literal, pairs, post, post_as, read, values_of,
};

/// A merge rewrites the spellings only on resources the caller may edit, counts the rest, and
/// leaves every other value as it was. A value the merge would repeat goes.
#[tokio::test]
async fn a_merge_rewrites_what_the_caller_may_edit_and_keeps_every_other_value() {
    let archive = archive().await;
    let pool = &archive.pool;
    let admins = item_as(
        pool,
        Caller::Admin,
        "Admin's",
        vec![literal("Co. Cork")],
        json!({"o:is_public": true}),
    )
    .await;
    let own = item_as(
        pool,
        Caller::Author,
        "Author's",
        vec![
            literal("Cork"),
            literal("Co. Cork"),
            literal("Cork City"),
            json!({"type": "uri", "property_id": "auto", "@id": "http://example.org/co-cork", "o:label": "Co. Cork"}),
        ],
        json!({"dcterms:spatial": [literal("Co. Cork")]}),
    )
    .await;
    let body = json!({"from": ["Co. Cork", "Cork City"], "to": "Cork", "dry_run": true});

    let preview = post_as(pool, Caller::Author, "/api/tags/merge", &body).await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.body);
    assert_eq!(preview.json(), json!({"changed": 1, "skipped": 1}));
    assert_eq!(values_of(pool, own, "dcterms:subject").await.len(), 4);

    let mut body = body;
    body["dry_run"] = json!(false);
    let done = post_as(pool, Caller::Author, "/api/tags/merge", &body).await;
    assert_eq!(done.json(), json!({"changed": 1, "skipped": 1}));
    assert_eq!(
        values_of(pool, own, "dcterms:subject").await,
        pairs(&[("literal", "Cork"), ("uri", "http://example.org/co-cork")])
    );
    assert_eq!(
        values_of(pool, own, "dcterms:spatial").await,
        pairs(&[("literal", "Co. Cork")])
    );
    assert_eq!(
        values_of(pool, own, "dcterms:title").await,
        pairs(&[("literal", "Author's")])
    );
    assert_eq!(
        values_of(pool, admins, "dcterms:subject").await,
        pairs(&[("literal", "Co. Cork")])
    );
    let census = read(pool, "/api/tags").await;
    assert_eq!(
        census["values"],
        json!([
            {"value": "Co. Cork", "resources": 1, "editable": 1},
            {"value": "Cork", "resources": 1, "editable": 1},
        ])
    );
    let _ = archive.author;
}

/// The search index follows a merge (`Module::saveFulltext`): the old spelling no longer finds the
/// item and the kept one does.
#[tokio::test]
async fn a_merged_item_is_found_by_its_new_spelling() {
    let archive = archive().await;
    let pool = &archive.pool;
    let letter = item(pool, "Letter", vec![literal("Corcaigh")]).await;
    let found = |needle: &'static str| async move {
        read(pool, &format!("/api/items?fulltext_search={needle}"))
            .await
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["o:id"].as_u64().unwrap())
            .collect::<Vec<u64>>()
    };
    assert_eq!(found("Corcaigh").await, vec![letter]);
    post(
        pool,
        "/api/tags/merge",
        &json!({"from": ["Corcaigh"], "to": "Cork"}),
    )
    .await;
    assert!(found("Corcaigh").await.is_empty());
    assert_eq!(found("Cork").await, vec![letter]);
}

/// On a list of words the values stored with the list follow the merge on any property, and the
/// list itself keeps the kept spelling in place of the merged ones.
#[tokio::test]
async fn a_list_merge_rewrites_its_values_and_its_words() {
    let archive = archive().await;
    let pool = &archive.pool;
    let places = post(
        pool,
        "/api/custom_vocabs",
        &json!({"o:label": "Places", "o:terms": ["Bantry", "Co. Cork", "Boston", "Cork City"]}),
    )
    .await
    .json()["o:id"]
        .as_u64()
        .unwrap();
    let name = format!("customvocab:{places}");
    let map = item_as(
        pool,
        Caller::Admin,
        "Map",
        vec![literal("Co. Cork")],
        json!({"dcterms:spatial": [
            {"type": name, "property_id": archive.spatial.0, "@value": "Co. Cork"},
            {"type": name, "property_id": archive.spatial.0, "@value": "Boston"},
        ]}),
    )
    .await;

    let census = read(pool, &format!("/api/custom_vocabs/{places}/values")).await;
    assert_eq!(
        census["values"],
        json!([
            {"value": "Boston", "resources": 1, "editable": 1},
            {"value": "Co. Cork", "resources": 1, "editable": 1},
        ])
    );
    let done = post(
        pool,
        &format!("/api/custom_vocabs/{places}/merge"),
        &json!({"from": ["Co. Cork", "Cork City"], "to": "Cork"}),
    )
    .await;
    assert_eq!(done.json(), json!({"changed": 1, "skipped": 0}));
    assert_eq!(
        values_of(pool, map, "dcterms:spatial").await,
        pairs(&[(&name, "Cork"), (&name, "Boston")])
    );
    assert_eq!(
        values_of(pool, map, "dcterms:subject").await,
        pairs(&[("literal", "Co. Cork")])
    );
    assert_eq!(
        read(pool, &format!("/api/custom_vocabs/{places}")).await["o:terms"],
        json!(["Bantry", "Cork", "Boston"])
    );
}

/// "They are different" is remembered per scope; a group inside a remembered one adds nothing and
/// a larger group replaces the ones it holds.
#[tokio::test]
async fn spellings_judged_different_are_remembered_per_scope() {
    let archive = archive().await;
    let pool = &archive.pool;
    let first = post(
        pool,
        "/api/tags/distinct",
        &json!({"values": ["Cork City", "Cork"]}),
    )
    .await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.body);
    assert_eq!(first.json()["distinct"], json!([["Cork", "Cork City"]]));
    post(
        pool,
        "/api/tags/distinct",
        &json!({"values": ["Cork", "Cork City"]}),
    )
    .await;
    let wider = post(
        pool,
        "/api/tags/distinct",
        &json!({"values": ["Cork", "Cork City", "Co. Cork"]}),
    )
    .await;
    assert_eq!(
        wider.json()["distinct"],
        json!([["Co. Cork", "Cork", "Cork City"]])
    );

    let places = post(
        pool,
        "/api/custom_vocabs",
        &json!({"o:label": "Places", "o:terms": ["Cork"]}),
    )
    .await
    .json()["o:id"]
        .as_u64()
        .unwrap();
    let list = read(pool, &format!("/api/custom_vocabs/{places}/values")).await;
    assert_eq!(list["distinct"], json!([]));
}

/// A body that names no spelling, an empty one, or an unknown member is a 400 and writes nothing.
#[tokio::test]
async fn a_malformed_body_is_refused() {
    let archive = archive().await;
    let pool = &archive.pool;
    for body in [
        json!({"from": [], "to": "Cork"}),
        json!({"from": ["Co. Cork"], "to": "  "}),
        json!({"from": ["Co. Cork"], "to": "Cork", "into": "x"}),
    ] {
        let answer = post(pool, "/api/tags/merge", &body).await;
        assert_eq!(
            answer.status,
            StatusCode::BAD_REQUEST,
            "{body}: {}",
            answer.body
        );
    }
    let missing = send(
        router(pool.clone()),
        get("/api/custom_vocabs/99/values", true),
    )
    .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}
