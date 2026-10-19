//! The vocabulary import routes: import a vocabulary from RDF, compare a stored one with a file,
//! and apply the reviewed comparison.
//!
//! Every decision lives in `omeka_api::vocab_import::routes`; these handlers check the content
//! type, authenticate, and copy the answer onto the wire. Upstream has no such routes: its admin
//! runs `Omeka\Stdlib\RdfImporter` from server-rendered forms.
//!
//! `/api/vocabularies/import` is matched before `/api/{resource}/{id}`, so a `POST` there imports
//! instead of creating a vocabulary through the member path, and any other method answers 405.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Uri};
use axum::response::Response;
use axum::routing::post;

use crate::AppState;
use omeka_api::vocab_import::routes;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/vocabularies/import", post(import))
        .route("/api/vocabularies/{id}/diff", post(diff))
        .route("/api/vocabularies/{id}/diff/apply", post(apply))
}

async fn import(
    State(state): State<AppState>,
    uri: Uri,
    headers: HeaderMap,
    body: crate::limits::Payload,
) -> Response {
    let api = match open_json(&state, &uri, &headers).await {
        Ok(api) => api,
        Err(error) => return crate::error_response(error),
    };
    crate::finish(routes::import(&api, &body.0).await)
}

async fn diff(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
    body: crate::limits::Payload,
) -> Response {
    let api = match open_json(&state, &uri, &headers).await {
        Ok(api) => api,
        Err(error) => return crate::error_response(error),
    };
    crate::finish(routes::diff(&api, &id, &body.0).await)
}

async fn apply(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
    body: crate::limits::Payload,
) -> Response {
    let api = match open_json(&state, &uri, &headers).await {
        Ok(api) => api,
        Err(error) => return crate::error_response(error),
    };
    crate::finish(routes::apply(&api, &id, &body.0).await)
}

/// Refuse a body that is not JSON, then authenticate the caller.
async fn open_json<'a>(
    state: &'a AppState,
    uri: &Uri,
    headers: &HeaderMap,
) -> Result<omeka_api::read::Api<'a>, omeka_api::errors::ApiError> {
    if !crate::content_type_is_json(headers) {
        return Err(omeka_api::write::body::content_type_error(
            crate::media_type(headers),
        ));
    }
    crate::open(state, uri, headers).await
}
