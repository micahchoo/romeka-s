//! The two response layers every read document passes through: a validator for
//! conditional GET, and gzip for the client that asked for it.
//!
//! Both act on the same document media types ([`document_media_type`]), so a
//! stylesheet, a site page and an API document are cacheable and compressible
//! alike, while `/health` and `/ready` are neither.
//!
//! The validator is a weak `ETag`: a digest of the response body, with a `W/`
//! prefix. Weak is the correct strength because the compressed and uncompressed
//! forms of one document share the tag, and the `Vary: accept-encoding` header
//! tells a cache the two forms differ. Upstream Omeka S sends no `ETag` or
//! `Last-Modified` on any response, so this is a port addition; the read layer
//! owns every other header (`crates/omeka-api/src/read/response.rs`).

use std::fmt::Write as _;
use std::io::Write as _;

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;
use flate2::Compression;
use flate2::write::GzEncoder;
use http_body_util::BodyExt;
use sha2::{Digest, Sha256};

/// The header value that records the two forms of one document.
const VARY_VALUE: &str = "accept-encoding";
/// The prefix of a weak entity tag.
const WEAK: &str = "W/";

/// Whether a content type names a document this layer may cache and compress.
///
/// The parameters after `;` are presentation (`charset=utf-8`), not the type.
fn document_media_type(content_type: &str) -> bool {
    let media_type = content_type.split(';').next().unwrap_or("").trim();
    matches!(
        media_type,
        "application/ld+json" | "text/html" | "text/css" | "application/javascript"
    )
}

/// Add a validator, or answer 304 when the client already holds it.
pub(crate) async fn conditional_get(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let if_none_match = request
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let response = next.run(request).await;
    if method != Method::GET || response.status() != StatusCode::OK {
        return response;
    }
    // A route that wrote its own validator owns it: `/annotations` sends a
    // strong per-row ETag that `If-Match` compares strongly, so a weak digest
    // here would turn every conditional write into a 412.
    if response.headers().contains_key(header::ETAG) {
        return response;
    }
    if !is_document(&response) {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    let bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => return crate::empty_internal_error(),
    };
    let tag = etag(&bytes);
    match tag_value(&tag) {
        Ok(value) => {
            let matches = if_none_match
                .as_deref()
                .is_some_and(|list| if_none_match_matches(list, &tag));
            if matches {
                parts.status = StatusCode::NOT_MODIFIED;
                parts.headers.insert(header::ETAG, value);
                return Response::from_parts(parts, Body::empty());
            }
            parts.headers.insert(header::ETAG, value);
        }
        Err(_) => return crate::empty_internal_error(),
    }
    Response::from_parts(parts, Body::from(bytes))
}

/// gzip the body when the client accepts it and the result is smaller.
///
/// The `Vary` header is set on every document, compressed or not, so a shared
/// cache keeps the two forms apart.
pub(crate) async fn compress_response(request: Request, next: Next) -> Response {
    let accepts = accepts_gzip(request.headers());
    let response = next.run(request).await;
    if response.status() != StatusCode::OK
        || !is_document(&response)
        || response.headers().contains_key(header::CONTENT_ENCODING)
    {
        return response;
    }
    if !accepts {
        let (mut parts, body) = response.into_parts();
        add_vary(&mut parts.headers);
        return Response::from_parts(parts, body);
    }
    let (mut parts, body) = response.into_parts();
    let bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => return crate::empty_internal_error(),
    };
    let encoded = (!bytes.is_empty())
        .then(|| gzip(&bytes))
        .flatten()
        .filter(|encoded| encoded.len() < bytes.len());
    add_vary(&mut parts.headers);
    match encoded {
        Some(encoded) => {
            parts.headers.remove(header::CONTENT_LENGTH);
            parts
                .headers
                .insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
            Response::from_parts(parts, Body::from(encoded))
        }
        None => Response::from_parts(parts, Body::from(bytes)),
    }
}

fn is_document(response: &Response) -> bool {
    response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(document_media_type)
}

/// Add the compression dimension to whatever `Vary` the route wrote, so a
/// route's own `Accept, Cookie, Prefer` survives beside it.
fn add_vary(headers: &mut HeaderMap) {
    let joined = match headers
        .get(header::VARY)
        .and_then(|value| value.to_str().ok())
    {
        Some(existing) if existing.to_ascii_lowercase().contains(VARY_VALUE) => return,
        Some(existing) if !existing.trim().is_empty() => format!("{existing}, {VARY_VALUE}"),
        _ => VARY_VALUE.to_string(),
    };
    if let Ok(value) = HeaderValue::from_str(&joined) {
        headers.insert(header::VARY, value);
    }
}

/// `W/"<sha256 of the body>"`.
fn etag(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut tag = String::with_capacity(2 + 64 + 1);
    tag.push_str(WEAK);
    tag.push('"');
    for byte in digest {
        let _ = write!(tag, "{byte:02x}");
    }
    tag.push('"');
    tag
}

fn tag_value(tag: &str) -> Result<HeaderValue, axum::http::header::InvalidHeaderValue> {
    HeaderValue::from_str(tag)
}

/// `If-None-Match` lists entity tags, or `*` for any representation. The
/// comparison ignores the weak prefix on either side (`If-None-Match` uses weak
/// comparison).
fn if_none_match_matches(list: &str, tag: &str) -> bool {
    list.split(',').any(|candidate| {
        let candidate = candidate.trim();
        candidate == "*" || without_weak(candidate) == without_weak(tag)
    })
}

fn without_weak(tag: &str) -> &str {
    tag.strip_prefix(WEAK).unwrap_or(tag)
}

/// Whether `Accept-Encoding` allows gzip. A coding list entry may carry `q=0`,
/// which refuses it.
fn accepts_gzip(headers: &HeaderMap) -> bool {
    let Some(value) = headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    value.split(',').any(|entry| {
        let mut parts = entry.split(';');
        let coding = parts.next().unwrap_or("").trim();
        if coding != "gzip" && coding != "x-gzip" {
            return false;
        }
        !parts.any(|parameter| {
            let parameter = parameter.trim();
            parameter
                .strip_prefix("q=")
                .is_some_and(|q| q.trim().parse::<f32>() == Ok(0.0))
        })
    })
}

fn gzip(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).ok()?;
    encoder.finish().ok()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use axum::Router;
    use axum::routing::get;
    use flate2::read::GzDecoder;
    use std::io::Read as _;
    use tower::ServiceExt;

    fn body(bytes: &[u8]) -> Response {
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/ld+json")
            .body(Body::from(bytes.to_vec()))
            .unwrap()
    }

    #[tokio::test]
    async fn a_matching_client_tag_answers_304_with_the_tag_and_no_body() {
        async fn document() -> Response {
            body(b"[]")
        }
        let app = Router::new()
            .route("/doc", get(document))
            .layer(axum::middleware::from_fn(conditional_get));
        let tag = etag(b"[]");
        let request = Request::builder()
            .uri("/doc")
            .header(header::IF_NONE_MATCH, &tag)
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(
            response
                .headers()
                .get(header::ETAG)
                .unwrap()
                .to_str()
                .unwrap(),
            tag
        );
    }

    #[test]
    fn the_document_types_are_cacheable_and_the_probes_are_not() {
        assert!(document_media_type("application/ld+json"));
        assert!(document_media_type("text/html; charset=utf-8"));
        assert!(document_media_type("text/css"));
        assert!(document_media_type("application/javascript"));
        assert!(!document_media_type("application/json"));
        assert!(!document_media_type("image/png"));
    }

    #[test]
    fn the_tag_is_a_weak_sha256_of_the_body() {
        assert_eq!(
            etag(b""),
            "W/\"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\""
        );
        assert_ne!(etag(b"a"), etag(b"b"));
    }

    #[test]
    fn a_list_matches_its_tag_and_any_representation() {
        let tag = etag(b"[]");
        assert!(if_none_match_matches(&tag, &tag));
        assert!(if_none_match_matches(&format!("W/\"x\", {tag}"), &tag));
        assert!(if_none_match_matches("*", &tag));
        assert!(
            if_none_match_matches(&tag[2..], &tag),
            "a strong client tag matches a weak one"
        );
        assert!(!if_none_match_matches("\"other\"", &tag));
    }

    #[test]
    fn gzip_is_refused_by_a_zero_quality_or_an_absent_header() {
        let mut headers = HeaderMap::new();
        assert!(!accepts_gzip(&headers));
        headers.insert(header::ACCEPT_ENCODING, HeaderValue::from_static("gzip"));
        assert!(accepts_gzip(&headers));
        headers.insert(
            header::ACCEPT_ENCODING,
            HeaderValue::from_static("br, gzip;q=0.5"),
        );
        assert!(accepts_gzip(&headers));
        headers.insert(
            header::ACCEPT_ENCODING,
            HeaderValue::from_static("gzip;q=0"),
        );
        assert!(!accepts_gzip(&headers));
    }

    #[test]
    fn gzip_round_trips_the_body() {
        let text = "hello hello hello hello hello".repeat(10);
        let encoded = gzip(text.as_bytes()).unwrap();
        assert!(encoded.len() < text.len());
        let mut decoded = String::new();
        GzDecoder::new(encoded.as_slice())
            .read_to_string(&mut decoded)
            .unwrap();
        assert_eq!(decoded, text);
    }
}
