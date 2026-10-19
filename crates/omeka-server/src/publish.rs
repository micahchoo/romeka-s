//! Publishing term lists and vocabularies: the admin's `/api/{custom_vocabs|vocabularies}/{id}/
//! publication` and the public `/vocab/<slug>/[<local>]`.
//!
//! Every decision lives in `omeka_api::publish`; these handlers authenticate where the admin route
//! needs it, pass the path, query and `Accept` header through, and copy the answer onto the wire.
//! Upstream has neither route.

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri, header};
use axum::response::Response;
use axum::routing::get;
use omeka_api::publish::{admin, public};

use crate::AppState;

pub(crate) fn router() -> Router<AppState> {
    // Two literal paths rather than `/api/{resource}/{id}/publication`, so they never compete
    // with the vocabulary import's `/api/vocabularies/{id}/diff` for the same segments.
    Router::new()
        .route(
            "/api/custom_vocabs/{id}/publication",
            get(list_view).post(list_command),
        )
        .route(
            "/api/vocabularies/{id}/publication",
            get(vocabulary_view).post(vocabulary_command),
        )
        .route("/vocab/{slug}", get(scheme))
        .route("/vocab/{slug}/", get(scheme))
        .route("/vocab/{slug}/{local}", get(term))
}

async fn list_view(
    state: State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    view(state, "custom_vocabs", id, uri, headers).await
}

async fn vocabulary_view(
    state: State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    view(state, "vocabularies", id, uri, headers).await
}

async fn list_command(
    state: State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
    body: crate::limits::Payload,
) -> Response {
    command(state, "custom_vocabs", id, uri, headers, body).await
}

async fn vocabulary_command(
    state: State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
    body: crate::limits::Payload,
) -> Response {
    command(state, "vocabularies", id, uri, headers, body).await
}

async fn view(
    State(state): State<AppState>,
    resource: &'static str,
    id: String,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let api = match crate::open(&state, &uri, &headers).await {
        Ok(api) => api,
        Err(error) => return crate::error_response(error),
    };
    crate::finish(admin::view(&api, resource, &id).await)
}

async fn command(
    State(state): State<AppState>,
    resource: &'static str,
    id: String,
    uri: Uri,
    headers: HeaderMap,
    body: crate::limits::Payload,
) -> Response {
    if !crate::content_type_is_json(&headers) {
        return crate::error_response(omeka_api::write::body::content_type_error(
            crate::media_type(&headers),
        ));
    }
    let api = match crate::open(&state, &uri, &headers).await {
        Ok(api) => api,
        Err(error) => return crate::error_response(error),
    };
    crate::finish(admin::command(&api, resource, &id, &body.0).await)
}

async fn scheme(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    served(&state, &slug, None, &uri, &headers).await
}

async fn term(
    State(state): State<AppState>,
    Path((slug, local)): Path<(String, String)>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    served(&state, &slug, Some(&local), &uri, &headers).await
}

async fn served(
    state: &AppState,
    slug: &str,
    local: Option<&str>,
    uri: &Uri,
    headers: &HeaderMap,
) -> Response {
    let answer = public::serve(
        &state.pool,
        public::Request {
            slug,
            local,
            query: uri.query().unwrap_or(""),
            accept: headers
                .get(header::ACCEPT)
                .and_then(|value| value.to_str().ok()),
        },
    )
    .await;
    Response::builder()
        .status(answer.status)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static(answer.content_type),
        )
        .header(header::VARY, HeaderValue::from_static("Accept"))
        .header(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        )
        .body(Body::from(answer.body))
        .unwrap_or_else(|_| {
            let mut response = Response::new(Body::empty());
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            response
        })
}
