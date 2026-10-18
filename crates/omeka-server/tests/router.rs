//! Structural router tests: routing, method handling, the error shapes, and
//! the endpoints that answer without rows.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::{StatusCode, header};
use common::{empty_router, get, header as header_value, json_request, request, send};
use omeka_server::JSONLD_CONTENT_TYPE;

#[tokio::test]
async fn health_is_200() {
    let answer = send(empty_router().await, request("GET", "/health")).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.body, r#"{"status":"ok"}"#);
    assert_eq!(
        header_value(&answer, "content-type"),
        Some("application/json")
    );
}

#[tokio::test]
async fn get_items_on_an_empty_database_is_an_empty_list() {
    let answer = send(empty_router().await, get("/api/items", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.json(), serde_json::json!([]));
    assert_eq!(
        header_value(&answer, "content-type"),
        Some(JSONLD_CONTENT_TYPE)
    );
    assert_eq!(header_value(&answer, "omeka-s-total-results"), Some("0"));
    assert_eq!(
        header_value(&answer, "link"),
        Some(
            "<http://localhost/api/items?sort_by_default=&sort_order_default=&sort_by=id&sort_order=asc&page=1>; rel=\"first\""
        )
    );
    assert_eq!(
        header_value(&answer, "omeka-s-version"),
        Some("4.3.0-alpha")
    );
}

#[tokio::test]
async fn post_nonsense_is_404_shaped() {
    let answer = send(empty_router().await, request("POST", "/api/nonsense")).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"The API does not support the \"nonsense\" resource."}}"#
    );
    assert_eq!(
        header_value(&answer, "content-type"),
        Some(JSONLD_CONTENT_TYPE)
    );
}

#[tokio::test]
async fn put_on_a_collection_is_405_with_allow() {
    let answer = send(empty_router().await, request("PUT", "/api/items")).await;
    assert_eq!(answer.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(answer.headers.get(header::ALLOW).unwrap(), "GET, POST");
    assert_eq!(
        header_value(&answer, "content-type"),
        Some(JSONLD_CONTENT_TYPE)
    );
}

#[tokio::test]
async fn head_on_a_member_is_405_with_the_member_allow_list() {
    let answer = send(empty_router().await, request("HEAD", "/api/items/1")).await;
    assert_eq!(answer.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        answer.headers.get(header::ALLOW).unwrap(),
        "GET, POST, PUT, PATCH, DELETE"
    );
}

#[tokio::test]
async fn post_on_a_member_routes_to_create() {
    // Laminas maps POST with an identifier to create. An anonymous caller has
    // no create grant, so the call reaches the create path and is denied.
    let answer = send(
        empty_router().await,
        json_request("POST", "/api/media/5", "{}"),
    )
    .await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Permission denied for the current user to create the Omeka\\Api\\Adapter\\MediaAdapter resource."}}"#
    );
}

#[tokio::test]
async fn post_without_a_json_content_type_is_415() {
    let answer = send(empty_router().await, request("POST", "/api/items")).await;
    assert_eq!(answer.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Invalid Content-Type header. Expecting \"application/json\", got \"none\"."}}"#
    );
}

/// `DELETE` on a member reaches the delete handler. An anonymous caller has no
/// `delete` grant on `UserAdapter`, so the answer is the adapter refusal rather
/// than the 501 a resource with no handler still gives.
#[tokio::test]
async fn delete_on_a_member_reaches_the_delete_handler() {
    let answer = send(empty_router().await, request("DELETE", "/api/users/2")).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Permission denied for the current user to delete the Omeka\\Api\\Adapter\\UserAdapter resource."}}"#
    );
}

#[tokio::test]
async fn api_root_lists_every_registered_resource() {
    let answer = send(empty_router().await, get("/api", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    let documents = answer.json();
    let documents = documents.as_array().unwrap();
    assert_eq!(documents.len(), omeka_api::adapters::ADAPTERS.len());
    assert_eq!(documents[0]["o:id"], "api_resources");
    assert_eq!(
        documents[0]["@id"],
        "http://localhost/api/api_resources/api_resources"
    );
    assert_eq!(documents[0]["@type"], "o:ApiResource");
}

#[tokio::test]
async fn api_context_is_the_context_document() {
    let answer = send(empty_router().await, get("/api-context", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    let document = answer.json();
    assert_eq!(document["@context"]["o"], "http://omeka.org/s/vocabs/o#");
    assert_eq!(
        document["@context"]["o-time"],
        "http://www.w3.org/2006/time#"
    );
}

#[tokio::test]
async fn outside_paths_are_404_shaped() {
    let answer = send(empty_router().await, request("GET", "/nonsense")).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    assert_eq!(answer.body, r#"{"errors":{"error":"Not Found"}}"#);
    assert_eq!(
        header_value(&answer, "content-type"),
        Some(JSONLD_CONTENT_TYPE)
    );
}

#[tokio::test]
async fn anonymous_users_search_is_permission_denied() {
    let answer = send(empty_router().await, get("/api/users", false)).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Permission denied for the current user to search the Omeka\\Api\\Adapter\\UserAdapter resource."}}"#
    );
}

#[tokio::test]
async fn a_key_that_does_not_verify_stays_anonymous() {
    let answer = send(
        empty_router().await,
        request(
            "GET",
            "/api/users?key_identity=0123456789abcdef0123456789abcdef&key_credential=wrong",
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
}
