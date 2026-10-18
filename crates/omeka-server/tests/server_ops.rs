//! Server operations: readiness, the response validator and compression, the
//! request limits, and the shutdown. The shutdown itself is covered by the unit
//! test in `src/serve.rs`, because it needs an ephemeral listener.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::io::Read as _;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use common::{Seed, append_key, get, header as header_value, seeded_pool, seeded_router, send};
use flate2::read::GzDecoder;
use http_body_util::BodyExt;
use omeka_server::{Config, JSONLD_CONTENT_TYPE};
use sqlx::SqlitePool;
use tower::ServiceExt;

fn with_config(pool: SqlitePool, config: &Config) -> Router {
    omeka_server::build_router_with_config(pool, config)
}

/// The seeded app with the row ids the tests address.
async fn seeded_app() -> (Router, Seed) {
    let (pool, seed) = seeded_pool().await;
    (with_config(pool, &Config::default()), seed)
}

/// Send one request and keep the body bytes, for a body that is not text.
async fn send_bytes(app: Router, request: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, headers, bytes)
}

fn a_request(method: &str, uri: &str, headers: &[(&str, &str)], body: &str) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn ready_reports_the_database() {
    let (app, _) = seeded_router().await;
    let answer = send(app, get("/ready", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.body, r#"{"status":"ready"}"#);
    assert_eq!(
        header_value(&answer, "content-type"),
        Some("application/json")
    );
}

/// A stored pool that `close()` has released no longer answers, which is the
/// failure a readiness probe exists to report.
#[tokio::test]
async fn ready_is_503_when_the_database_is_closed() {
    let (pool, _) = seeded_pool().await;
    let app = with_config(pool.clone(), &Config::default());
    pool.close().await;
    let answer = send(app, get("/ready", false)).await;
    assert_eq!(answer.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(answer.body, r#"{"status":"unavailable"}"#);

    // Liveness still answers: the process is up even when storage is not.
    let health = send(with_config(pool, &Config::default()), get("/health", false)).await;
    assert_eq!(health.status, StatusCode::OK);
}

#[tokio::test]
async fn the_probes_carry_no_validator() {
    let (app, _) = seeded_router().await;
    for path in ["/health", "/ready"] {
        let answer = send(app.clone(), get(path, false)).await;
        assert!(
            header_value(&answer, "etag").is_none(),
            "{path} is not a cached document: {:?}",
            answer.headers
        );
    }
}

#[tokio::test]
async fn a_read_document_carries_a_stable_etag() {
    let (app, seed) = seeded_app().await;
    let uri = format!("/api/items/{}", seed.item.0);
    let first = send(app.clone(), get(&uri, false)).await;
    let second = send(app, get(&uri, false)).await;
    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(
        header_value(&first, "content-type"),
        Some(JSONLD_CONTENT_TYPE)
    );
    let etag = header_value(&first, "etag").expect("a read document carries an ETag");
    assert!(etag.starts_with("W/\""), "the validator is weak: {etag}");
    assert_eq!(header_value(&second, "etag"), Some(etag));
}

#[tokio::test]
async fn a_repeat_get_with_if_none_match_is_304_without_a_body() {
    let (app, seed) = seeded_app().await;
    let uri = format!("/api/items/{}", seed.item.0);
    let first = send(app.clone(), get(&uri, false)).await;
    let etag = header_value(&first, "etag").expect("an ETag").to_string();

    // The client also offers gzip, which a 304 must not carry.
    let request = a_request(
        "GET",
        &uri,
        &[("if-none-match", &etag), ("accept-encoding", "gzip")],
        "",
    );
    let answer = send(app, request).await;
    assert_eq!(answer.status, StatusCode::NOT_MODIFIED);
    assert_eq!(header_value(&answer, "etag"), Some(etag.as_str()));
    assert_eq!(answer.body, "", "a 304 carries no body");
    assert!(
        header_value(&answer, "content-encoding").is_none(),
        "a 304 carries no encoding: {:?}",
        answer.headers
    );
}

/// A client tag for a different body is not the validator, so the document is
/// sent again.
#[tokio::test]
async fn a_stale_if_none_match_still_gets_the_document() {
    let (app, seed) = seeded_app().await;
    let uri = format!("/api/items/{}", seed.item.0);
    let request = a_request("GET", &uri, &[("if-none-match", "W/\"nope\"")], "");
    let answer = send(app, request).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(answer.body.contains("o:id"), "body: {}", answer.body);
}

#[tokio::test]
async fn a_gzip_client_gets_the_document_compressed() {
    let (app, seed) = seeded_app().await;
    let uri = format!("/api/items/{}", seed.item.0);
    let plain = send(app.clone(), get(&uri, false)).await;
    let request = a_request("GET", &uri, &[("accept-encoding", "gzip")], "");
    let (status, headers, bytes) = send_bytes(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers.get("content-encoding").map(|v| v.to_str().unwrap()),
        Some("gzip")
    );
    assert_eq!(
        headers.get("vary").map(|v| v.to_str().unwrap()),
        Some("accept-encoding")
    );
    assert_eq!(
        headers.get("etag").map(|v| v.to_str().unwrap()),
        header_value(&plain, "etag"),
        "the two forms share one validator"
    );
    let mut decoded = String::new();
    GzDecoder::new(bytes.as_slice())
        .read_to_string(&mut decoded)
        .expect("the body is gzip");
    assert_eq!(decoded, plain.body);
}

/// A client that does not offer gzip gets the document unchanged, and the
/// `Vary` header still tells a shared cache the two forms differ.
#[tokio::test]
async fn a_client_without_gzip_gets_the_document_whole() {
    let (app, seed) = seeded_app().await;
    let answer = send(app, get(&format!("/api/items/{}", seed.item.0), false)).await;
    assert!(header_value(&answer, "content-encoding").is_none());
    assert_eq!(header_value(&answer, "vary"), Some("accept-encoding"));
}

#[tokio::test]
async fn a_request_body_over_the_limit_is_413_with_the_error_document() {
    let config = Config {
        max_body_bytes: 16,
        ..Config::default()
    };
    let app = with_config(seeded_pool().await.0, &config);
    let uri = append_key("/api/items");
    let body = format!(r#"{{"dcterms:title":[{{"@value":"{}"}}]}}"#, "x".repeat(64));
    assert!(body.len() > config.max_body_bytes);
    let request = a_request("POST", &uri, &[("content-type", "application/json")], &body);
    let answer = send(app, request).await;
    assert_eq!(answer.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Request body too large"}}"#
    );
    assert_eq!(
        header_value(&answer, "content-type"),
        Some(JSONLD_CONTENT_TYPE)
    );
}

/// A body inside the limit still reaches the handler.
#[tokio::test]
async fn a_request_body_inside_the_limit_reaches_the_handler() {
    let app = with_config(seeded_pool().await.0, &Config::default());
    let uri = append_key("/api/items");
    let body =
        r#"{"dcterms:title":[{"property_id":"auto","type":"literal","@value":"In the limit"}]}"#;
    let request = a_request("POST", &uri, &[("content-type", "application/json")], body);
    let answer = send(app, request).await;
    assert_eq!(answer.status, StatusCode::OK, "body: {}", answer.body);
    assert!(
        answer.body.contains("In the limit"),
        "body: {}",
        answer.body
    );
}
