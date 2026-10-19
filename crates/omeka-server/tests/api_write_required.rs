//! `AbstractResourceEntityAdapter::validateEntity`: a resource whose template
//! marks a property required must carry a value of that property, or the write
//! is the 422 `o:resource_template_property` error. The oracle answers the same
//! bodies the same way (`.evidence/oracle-required.sh` in the change that added
//! this file).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{append_key, json_request, seeded_router, send};
use serde_json::json;

const TITLE: u64 = 1;

/// A template named `label` that requires `dcterms:title` under the alternate
/// label "Heading".
async fn required_title_template(app: axum::Router, label: &str) -> u64 {
    let body = format!(
        r#"{{"o:label":"{label}","o:resource_template_property":[
            {{"o:property":{{"o:id":{TITLE}}},"o:alternate_label":"Heading","o:is_required":true}}]}}"#
    );
    let created = send(
        app,
        json_request("POST", &append_key("/api/resource_templates"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    created.json()["o:id"].as_u64().unwrap()
}

fn missing(label: &str) -> serde_json::Value {
    json!({"errors": {"o:resource_template_property": [
        format!("The \"{label}\" resource template requires a \"Heading\" value")
    ]}})
}

fn body(template: u64, values: &str) -> String {
    format!(r#"{{"o:resource_template":{{"o:id":{template}}}{values}}}"#)
}

const TITLED: &str = r#","dcterms:title":[{"type":"literal","property_id":1,"@value":"Named"}]"#;
const EMPTY_TITLE: &str = r#","dcterms:title":[{"type":"literal","property_id":1,"@value":""}]"#;

#[tokio::test]
async fn an_item_create_without_a_required_value_is_rejected() {
    let (app, _seed) = seeded_router().await;
    let template = required_title_template(app.clone(), "Strict").await;
    let answer = send(
        app,
        json_request("POST", &append_key("/api/items"), &body(template, "")),
    )
    .await;
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
    assert_eq!(answer.json(), missing("Strict"));
}

/// `ValueHydrator` drops a literal with an empty `@value`, so an empty title
/// is no value at all.
#[tokio::test]
async fn an_empty_value_does_not_satisfy_a_required_property() {
    let (app, _seed) = seeded_router().await;
    let template = required_title_template(app.clone(), "Strict").await;
    let answer = send(
        app,
        json_request(
            "POST",
            &append_key("/api/items"),
            &body(template, EMPTY_TITLE),
        ),
    )
    .await;
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
    assert_eq!(answer.json(), missing("Strict"));
}

#[tokio::test]
async fn an_item_update_follows_the_required_rule() {
    let (app, _seed) = seeded_router().await;
    let template = required_title_template(app.clone(), "Strict").await;
    let created = send(
        app.clone(),
        json_request("POST", &append_key("/api/items"), &body(template, TITLED)),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let id = created.json()["o:id"].as_u64().unwrap();
    let uri = append_key(&format!("/api/items/{id}"));

    // A partial update that names no value keeps the stored title.
    let patched = send(
        app.clone(),
        json_request("PATCH", &uri, r#"{"o:is_public":false}"#),
    )
    .await;
    assert_eq!(patched.status, StatusCode::OK, "{}", patched.body);

    let put = send(app, json_request("PUT", &uri, &body(template, ""))).await;
    assert_eq!(put.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", put.body);
    assert_eq!(put.json(), missing("Strict"));
}

#[tokio::test]
async fn an_item_set_create_follows_the_required_rule() {
    let (app, _seed) = seeded_router().await;
    let template = required_title_template(app.clone(), "Strict sets").await;
    let rejected = send(
        app.clone(),
        json_request("POST", &append_key("/api/item_sets"), &body(template, "")),
    )
    .await;
    assert_eq!(
        rejected.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        rejected.body
    );
    assert_eq!(rejected.json(), missing("Strict sets"));

    let accepted = send(
        app,
        json_request(
            "POST",
            &append_key("/api/item_sets"),
            &body(template, TITLED),
        ),
    )
    .await;
    assert_eq!(accepted.status, StatusCode::OK, "{}", accepted.body);
}

/// `Literal::isValid` and `Uri::isValid`: `ValueHydrator` skips a literal whose
/// `@value` is blank and a uri whose `@id` is blank, and keeps the rest.
#[tokio::test]
async fn a_blank_literal_or_uri_is_not_stored() {
    let (app, _seed) = seeded_router().await;
    let body = r#"{"dcterms:title":[
        {"type":"literal","property_id":1,"@value":"   "},
        {"type":"uri","property_id":1,"@id":"","o:label":"blank uri"},
        {"type":"literal","property_id":1,"@value":"kept"}]}"#;
    let created = send(app, json_request("POST", &append_key("/api/items"), body)).await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    assert_eq!(
        created.json()["dcterms:title"],
        json!([{"type":"literal","property_id":1,"property_label":"Title","is_public":true,"@value":"kept"}])
    );
}

/// `Uri::hydrate` reads the language from `o:lang`, the member `getJsonLd`
/// writes, and ignores `@language`.
#[tokio::test]
async fn a_uri_value_takes_its_language_from_o_lang() {
    let (app, _seed) = seeded_router().await;
    let body = r#"{"dcterms:title":[
        {"type":"uri","property_id":1,"@id":"http://example.org/a","o:label":"A","o:lang":"fr"},
        {"type":"uri","property_id":1,"@id":"http://example.org/b","o:label":"B","@language":"de"}]}"#;
    let created = send(app, json_request("POST", &append_key("/api/items"), body)).await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    assert_eq!(
        created.json()["dcterms:title"],
        json!([
            {"type":"uri","property_id":1,"property_label":"Title","is_public":true,
             "@id":"http://example.org/a","o:label":"A","o:lang":"fr"},
            {"type":"uri","property_id":1,"property_label":"Title","is_public":true,
             "@id":"http://example.org/b","o:label":"B"}
        ])
    );
}
