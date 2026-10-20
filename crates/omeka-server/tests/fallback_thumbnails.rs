//! The fallback thumbnails: a media with no thumbnail of its own links
//! `/application/asset/thumbnails/<kind>.png`, and those URLs must answer the
//! upstream images rather than a 404.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::empty_router;
use http_body_util::BodyExt;
use omeka_api::jsonld::BaseUrl;
use tower::ServiceExt;

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Each media type the API can link a fallback for, with the size of the
/// upstream file at the pinned commit, so a swapped or redrawn image fails.
const CASES: [(Option<&str>, usize); 5] = [
    (Some("image/jpeg"), 894),
    (Some("video/mp4"), 337),
    (Some("audio/mpeg"), 1722),
    (Some("application/pdf"), 465),
    (None, 465),
];

async fn fetch(uri: &str) -> (StatusCode, Option<String>, Vec<u8>) {
    let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let response = empty_router().await.oneshot(request).await.unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|value| value.to_str().unwrap().to_string());
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, content_type, bytes.to_vec())
}

#[tokio::test]
async fn every_fallback_url_the_api_links_answers_the_upstream_png() {
    let base = BaseUrl::parse("http://localhost").unwrap();
    for (media_type, size) in CASES {
        let url = base.fallback_thumbnail_url(media_type);
        let path = url.strip_prefix("http://localhost").unwrap();
        let (status, content_type, body) = fetch(path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(content_type.as_deref(), Some("image/png"), "{path}");
        assert!(body.starts_with(PNG_SIGNATURE), "{path} is not a PNG");
        assert_eq!(body.len(), size, "{path} is not the upstream file");
    }
}

#[tokio::test]
async fn an_unknown_fallback_name_is_a_404() {
    let (status, _, _) = fetch("/application/asset/thumbnails/model.png").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
