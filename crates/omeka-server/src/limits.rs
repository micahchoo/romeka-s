//! Request limits: the body size cap and the request timeout, each answered
//! with the API error document.
//!
//! The size cap is axum's [`DefaultBodyLimit`](axum::extract::DefaultBodyLimit),
//! applied as a layer in [`crate::build_app`]; it makes the body extractor fail,
//! and [`Payload`] turns that failure into the typed document. The timeout is a
//! layer over the whole router, so it bounds every route including the site
//! pages.
//!
//! Upstream has no 408 and no 413; both are boundary decisions of the port, so
//! they carry the port's own sentence in the same `{"errors":{"error":...}}`
//! shape the API uses for every other failure.

use std::time::Duration;

use axum::body::Bytes;
use axum::extract::{FromRequest, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;

/// The request body, capped by [`axum::extract::DefaultBodyLimit`] and rejected
/// with the typed document.
pub(crate) struct Payload(pub Bytes);

impl<S> FromRequest<S> for Payload
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Bytes::from_request(request, state).await {
            Ok(bytes) => Ok(Self(bytes)),
            Err(rejection) => Err(limit_error(rejection.status())),
        }
    }
}

/// Answer 408 when a request runs past the configured timeout.
pub(crate) async fn timeout_request(
    State(timeout): State<Duration>,
    request: Request,
    next: Next,
) -> Response {
    match tokio::time::timeout(timeout, next.run(request)).await {
        Ok(response) => response,
        Err(_) => error_document(StatusCode::REQUEST_TIMEOUT, "Request Timeout"),
    }
}

/// A limit failure carries the shape every API failure carries.
fn limit_error(status: StatusCode) -> Response {
    let message = if status == StatusCode::PAYLOAD_TOO_LARGE {
        "Request body too large"
    } else {
        "Bad Request"
    };
    error_document(status, message)
}

fn error_document(status: StatusCode, message: &str) -> Response {
    crate::document_response(status, &omeka_api::errors::ErrorDocument::general(message))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use axum::Router;
    use axum::routing::get;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn a_slow_request_is_408_with_the_error_document() {
        async fn slow() -> &'static str {
            tokio::time::sleep(Duration::from_secs(30)).await;
            "late"
        }
        let app =
            Router::new()
                .route("/slow", get(slow))
                .layer(axum::middleware::from_fn_with_state(
                    Duration::from_millis(20),
                    timeout_request,
                ));
        let request = Request::builder()
            .uri("/slow")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            String::from_utf8(body.to_vec()).unwrap(),
            r#"{"errors":{"error":"Request Timeout"}}"#
        );
    }

    #[tokio::test]
    async fn a_fast_request_passes_through() {
        async fn quick() -> &'static str {
            "ok"
        }
        let app =
            Router::new()
                .route("/quick", get(quick))
                .layer(axum::middleware::from_fn_with_state(
                    Duration::from_secs(5),
                    timeout_request,
                ));
        let request = Request::builder()
            .uri("/quick")
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(app.oneshot(request).await.unwrap().status(), StatusCode::OK);
    }
}
