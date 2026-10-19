//! Tags through the router: the census and promoting tags into a term list (`omeka_api::tagging`).
//!
//! A tag is a plain-text `dcterms:subject` value. These routes are Romeka's own; what they write is
//! ordinary values, lists and template rows, so each test reads the result back through the
//! upstream-shaped routes.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod tag_fixtures;

use axum::http::StatusCode;
use common::{get, router, send};
use omeka_db::ids::TemplateId;
use serde_json::json;
use tag_fixtures::{
    Caller, archive, item, item_as, literal, pairs, post, post_as, read, rows_of, template,
    values_of,
};

/// Every tag with the resources carrying it, most used first; a URI or list value on the subject
/// and plain text on another property are not tags, and a tag repeated on one item counts once.
#[tokio::test]
async fn the_census_counts_each_tag_once_per_resource() {
    let archive = archive().await;
    let pool = &archive.pool;
    item(
        pool,
        "Letter",
        vec![
            literal("Cork"),
            literal("Cork"),
            json!({"type": "uri", "property_id": "auto", "@id": "http://example.org/cork", "o:label": "Cork"}),
        ],
    )
    .await;
    item(
        pool,
        "Postcard",
        vec![literal("Cork"), literal("emigration")],
    )
    .await;
    item_as(
        pool,
        Caller::Admin,
        "Map",
        vec![],
        json!({"dcterms:spatial": [literal("Skibbereen")]}),
    )
    .await;

    let census = read(pool, "/api/tags").await;
    assert_eq!(
        census,
        json!({
            "values": [
                {"value": "Cork", "resources": 2, "editable": 2},
                {"value": "emigration", "resources": 1, "editable": 1},
            ],
            "distinct": [],
        })
    );
    let _ = archive.spatial;
}

/// An anonymous census counts only what a read shows it: public resources, public values.
#[tokio::test]
async fn the_census_shows_only_what_the_caller_may_see() {
    let archive = archive().await;
    let pool = &archive.pool;
    item(
        pool,
        "Public",
        vec![
            literal("Cork"),
            json!({"type": "literal", "property_id": "auto", "@value": "secret", "is_public": false}),
        ],
    )
    .await;
    item_as(
        pool,
        Caller::Admin,
        "Private",
        vec![literal("Cork"), literal("hidden")],
        json!({"o:is_public": false}),
    )
    .await;
    let anonymous = send(router(pool.clone()), get("/api/tags", false)).await;
    assert_eq!(anonymous.status, StatusCode::OK, "{}", anonymous.body);
    assert_eq!(
        anonymous.json()["values"],
        json!([{"value": "Cork", "resources": 1, "editable": 0}])
    );
    assert_eq!(
        read(pool, "/api/tags").await["values"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

/// A promotion previews its counts without writing, then makes the list, retypes the tags on the
/// subject, and makes each changed item's type offer the list. Running it again changes nothing.
#[tokio::test]
async fn promoting_tags_retypes_them_and_every_item_type_offers_the_list() {
    let archive = archive().await;
    let pool = &archive.pool;
    let letter = template(pool, "Letter", None).await;
    let photograph = template(pool, "Photograph", Some(archive.subject)).await;
    let typed = |id: TemplateId| json!({"o:resource_template": {"o:id": id.0}});
    let first = item_as(
        pool,
        Caller::Admin,
        "One",
        vec![literal("emigration")],
        typed(letter),
    )
    .await;
    let second = item_as(
        pool,
        Caller::Admin,
        "Two",
        vec![literal("wartime"), literal("christmas")],
        typed(photograph),
    )
    .await;
    let third = item(pool, "Three", vec![literal("emigration")]).await;
    let body = json!({"tags": ["emigration", "wartime"], "list": "Topics", "dry_run": true});

    let preview = post(pool, "/api/tags/promote", &body).await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.body);
    assert_eq!(
        preview.json(),
        json!({
            "list": null, "created": true, "changed": 3, "skipped": 0,
            "templates": [
                {"o:id": letter.0, "o:label": "Letter"},
                {"o:id": photograph.0, "o:label": "Photograph"},
            ],
            "templates_skipped": [],
        })
    );
    assert_eq!(
        values_of(pool, first, "dcterms:subject").await,
        pairs(&[("literal", "emigration")])
    );
    assert_eq!(read(pool, "/api/custom_vocabs").await, json!([]));

    let mut body = body;
    body["dry_run"] = json!(false);
    let done = post(pool, "/api/tags/promote", &body).await;
    assert_eq!(done.status, StatusCode::OK, "{}", done.body);
    let list = done.json()["list"]["o:id"].as_u64().unwrap();
    let name = format!("customvocab:{list}");
    assert_eq!(done.json()["changed"], 3);
    assert_eq!(
        read(pool, &format!("/api/custom_vocabs/{list}")).await["o:terms"],
        json!(["emigration", "wartime"])
    );
    assert_eq!(
        values_of(pool, first, "dcterms:subject").await,
        pairs(&[(&name, "emigration")])
    );
    assert_eq!(
        values_of(pool, second, "dcterms:subject").await,
        pairs(&[(&name, "wartime"), ("literal", "christmas")])
    );
    assert_eq!(
        values_of(pool, third, "dcterms:subject").await,
        pairs(&[(&name, "emigration")])
    );
    let title = read(pool, &format!("/api/items/{first}")).await["o:title"].clone();
    assert_eq!(title, "One");
    let subject = archive.subject.0;
    assert_eq!(
        rows_of(pool, letter).await,
        vec![(subject, Some(vec![name.clone()]))]
    );
    assert_eq!(
        rows_of(pool, photograph).await,
        vec![(
            subject,
            Some(vec![
                "literal".to_string(),
                "uri".to_string(),
                "resource".to_string(),
                name.clone()
            ])
        )]
    );

    let again = post(pool, "/api/tags/promote", &body).await;
    assert_eq!(again.status, StatusCode::OK, "{}", again.body);
    assert_eq!(again.json()["list"]["o:id"], list);
    assert_eq!(again.json()["created"], false);
    assert_eq!(again.json()["changed"], 0);
    assert_eq!(
        read(pool, "/api/custom_vocabs")
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        rows_of(pool, letter).await,
        vec![(subject, Some(vec![name]))]
    );
    assert_eq!(
        read(pool, "/api/tags").await["values"],
        json!([{"value": "christmas", "resources": 1, "editable": 1}])
    );
}

/// A promotion into a stored list of words adds the tags it lacks; a list of links cannot take
/// words, so its label is refused.
#[tokio::test]
async fn a_promotion_joins_a_stored_list_of_words_and_refuses_a_list_of_links() {
    let archive = archive().await;
    let pool = &archive.pool;
    item(pool, "One", vec![literal("Cork"), literal("Boston")]).await;
    let places = post(
        pool,
        "/api/custom_vocabs",
        &json!({"o:label": "Places", "o:terms": ["Boston", "Liverpool"]}),
    )
    .await
    .json()["o:id"]
        .as_u64()
        .unwrap();
    post(
        pool,
        "/api/custom_vocabs",
        &json!({"o:label": "Headings", "o:uris": {"http://example.org/a": "A"}}),
    )
    .await;

    let joined = post(
        pool,
        "/api/tags/promote",
        &json!({"tags": ["Cork", "Boston"], "list": "Places"}),
    )
    .await;
    assert_eq!(joined.status, StatusCode::OK, "{}", joined.body);
    assert_eq!(joined.json()["list"]["o:id"], places);
    assert_eq!(
        read(pool, &format!("/api/custom_vocabs/{places}")).await["o:terms"],
        json!(["Boston", "Liverpool", "Cork"])
    );

    let refused = post(
        pool,
        "/api/tags/promote",
        &json!({"tags": ["Cork"], "list": "Headings"}),
    )
    .await;
    assert_eq!(
        refused.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        refused.body
    );
}

/// Only an editor or an admin may make a list (`Module::onBootstrap`), so an author's promotion is
/// refused and nothing changes.
#[tokio::test]
async fn an_author_may_not_promote_tags() {
    let archive = archive().await;
    let pool = &archive.pool;
    let own = item_as(
        pool,
        Caller::Author,
        "Mine",
        vec![literal("farm")],
        json!({}),
    )
    .await;
    let refused = post_as(
        pool,
        Caller::Author,
        "/api/tags/promote",
        &json!({"tags": ["farm"], "list": "Topics"}),
    )
    .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{}", refused.body);
    assert_eq!(
        values_of(pool, own, "dcterms:subject").await,
        pairs(&[("literal", "farm")])
    );
}
