//! Theme asset route tests: `/assets/<path>` served from the bundled theme
//! registry, so the document the layout renders is styled.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::{StatusCode, header};
use common::{empty_router, get, header as header_value, request, router, send, site_pool};
use omeka_server::JSONLD_CONTENT_TYPE;
use omeka_site::theme::theme_asset;

/// The cache header a versioned asset URL answers with: the URL names the
/// bytes, so a client may keep them for a year without asking again.
const IMMUTABLE: &str = "public, max-age=31536000, immutable";

/// The cache header an unversioned asset URL answers with: the bytes behind it
/// change on a deploy, so a client revalidates with the `ETag` every time.
const REVALIDATE: &str = "no-cache";

fn stylesheet() -> &'static omeka_site::theme::ThemeAsset {
    theme_asset("default", "css/style.css").expect("the bundled stylesheet")
}

/// The bundled theme's stylesheet answers 200 with `text/css` and the bytes the
/// registry holds for its manifest path. Without a version the client must
/// revalidate, so a deploy is never hidden behind a stale copy.
#[tokio::test]
async fn unversioned_stylesheet_answers_200_and_must_revalidate() {
    let answer = send(empty_router().await, get("/assets/css/style.css", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(header_value(&answer, "content-type"), Some("text/css"));
    assert_eq!(header_value(&answer, "cache-control"), Some(REVALIDATE));
    assert!(header_value(&answer, "etag").is_some(), "no validator");
    assert!(!stylesheet().content.is_empty());
    assert_eq!(answer.body.as_bytes(), stylesheet().content);
}

/// The URL that names the current bytes is immutable for a year.
#[tokio::test]
async fn versioned_stylesheet_is_immutable() {
    let uri = format!("/assets/css/style.css?v={}", stylesheet().version());
    let answer = send(empty_router().await, get(&uri, false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(header_value(&answer, "cache-control"), Some(IMMUTABLE));
    assert_eq!(answer.body.as_bytes(), stylesheet().content);
}

/// A page cached before a deploy names the old version. It still gets the
/// sheet, but not as immutable, because those bytes are not the ones it named.
#[tokio::test]
async fn a_stale_version_gets_the_current_sheet_without_immutable() {
    let answer = send(
        empty_router().await,
        get("/assets/css/style.css?v=0000000000000000", false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(header_value(&answer, "cache-control"), Some(REVALIDATE));
    assert_eq!(answer.body.as_bytes(), stylesheet().content);
}

/// A client that holds the current bytes revalidates to a 304.
#[tokio::test]
async fn a_matching_validator_answers_304() {
    let first = send(empty_router().await, get("/assets/css/style.css", false)).await;
    let tag = header_value(&first, "etag")
        .expect("a validator")
        .to_string();
    let request = axum::http::Request::builder()
        .uri("/assets/css/style.css")
        .header(header::IF_NONE_MATCH, tag)
        .body(axum::body::Body::empty())
        .unwrap();
    let answer = send(empty_router().await, request).await;
    assert_eq!(answer.status, StatusCode::NOT_MODIFIED);
    assert_eq!(answer.body, "");
}

/// The version is a digest of the bytes, so a change to the sheet changes the
/// URL every page links.
#[test]
fn the_version_follows_the_bytes() {
    let version = stylesheet().version();
    assert_eq!(version.len(), 16);
    assert!(version.chars().all(|ch| ch.is_ascii_hexdigit()));
    let placeholder = theme_asset("default", "img/placeholder.svg").expect("the placeholder");
    assert_ne!(placeholder.version(), version);
}

/// A path no bundled theme ships is a 404 in the error document shape.
#[tokio::test]
async fn missing_asset_is_a_404_error_document() {
    let answer = send(empty_router().await, get("/assets/css/missing.css", false)).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    assert_eq!(
        header_value(&answer, "content-type"),
        Some(JSONLD_CONTENT_TYPE)
    );
    assert_eq!(
        answer.json(),
        serde_json::json!({"errors": {"error": "Not Found"}})
    );
}

/// Only `GET` serves an asset; another method is a 405 with `Allow`.
#[tokio::test]
async fn post_to_an_asset_is_405() {
    let answer = send(
        empty_router().await,
        request("POST", "/assets/css/style.css"),
    )
    .await;
    assert_eq!(answer.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(answer.headers.get(header::ALLOW).unwrap(), "GET");
}

/// A rendered page links the versioned theme asset URL this route serves: the
/// path the frame builds from the render context's asset base is the path the
/// route reads, and the link is the immutable form.
#[tokio::test]
async fn rendered_page_links_an_asset_the_route_serves() {
    let (pool, _seed) = site_pool().await;
    let app = router(pool);
    let page = send(app.clone(), get("/s/museum/page/welcome", false)).await;
    assert_eq!(page.status, StatusCode::OK);
    let href = asset_reference(&page.body, "css/style.css?v=");
    assert_eq!(
        href,
        format!("/assets/css/style.css?v={}", stylesheet().version()),
        "page body: {}",
        page.body
    );
    let answer = send(app, get(&href, false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(header_value(&answer, "content-type"), Some("text/css"));
    assert_eq!(header_value(&answer, "cache-control"), Some(IMMUTABLE));
    assert!(answer.body.contains(".blocks-inner:after"), "not the sheet");
}

/// The first `href` or `src` in a document whose value contains `marker`.
fn asset_reference(document: &str, marker: &str) -> String {
    for attr in ["href=\"", "src=\""] {
        let mut rest = document;
        while let Some(index) = rest.find(attr) {
            rest = &rest[index + attr.len()..];
            let Some(end) = rest.find('"') else {
                break;
            };
            let value = &rest[..end];
            if value.contains(marker) {
                return value.to_string();
            }
            rest = &rest[end..];
        }
    }
    panic!("no reference containing {marker} in {document}");
}
