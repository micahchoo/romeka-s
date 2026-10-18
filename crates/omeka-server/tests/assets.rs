//! Theme asset route tests: `/assets/<path>` served from the bundled theme
//! registry, so the document the layout renders is styled.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::{StatusCode, header};
use common::{empty_router, get, header as header_value, request, router, send, site_pool};
use omeka_server::JSONLD_CONTENT_TYPE;
use omeka_site::theme::theme_asset;

/// The bundled theme's stylesheet answers 200 with `text/css`, a cache header,
/// and the bytes the registry holds for its manifest path.
#[tokio::test]
async fn stylesheet_asset_answers_200_with_its_bytes() {
    let answer = send(empty_router().await, get("/assets/css/style.css", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(header_value(&answer, "content-type"), Some("text/css"));
    assert_eq!(
        header_value(&answer, "cache-control"),
        Some("public, max-age=86400")
    );
    let expected = theme_asset("default", "css/style.css")
        .expect("the bundled stylesheet")
        .content;
    assert!(!expected.is_empty());
    assert_eq!(answer.body, expected);
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

/// A rendered page links the theme asset URL this route serves: the path the
/// frame builds from the render context's asset base is the path the route
/// reads.
#[tokio::test]
async fn rendered_page_links_an_asset_the_route_serves() {
    let (pool, _seed) = site_pool().await;
    let app = router(pool);
    let page = send(app.clone(), get("/s/museum/page/welcome", false)).await;
    assert_eq!(page.status, StatusCode::OK);
    let href = asset_reference(&page.body, "css/style.css");
    assert_eq!(href, "/assets/css/style.css", "page body: {}", page.body);
    let answer = send(app, get(&href, false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(header_value(&answer, "content-type"), Some("text/css"));
    assert!(answer.body.contains(".blocks-inner:after"), "not the sheet");
}

/// The first `href` or `src` in a document whose value ends with `suffix`.
fn asset_reference(document: &str, suffix: &str) -> String {
    for attr in ["href=\"", "src=\""] {
        let mut rest = document;
        while let Some(index) = rest.find(attr) {
            rest = &rest[index + attr.len()..];
            let Some(end) = rest.find('"') else {
                break;
            };
            let value = &rest[..end];
            if value.ends_with(suffix) {
                return value.to_string();
            }
            rest = &rest[end..];
        }
    }
    panic!("no reference ending in {suffix} in {document}");
}
