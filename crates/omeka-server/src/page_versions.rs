//! `GET /api/site_pages/{id}/versions` and `GET /api/site_pages/{id}/versions/{version}`: a site
//! page's saved versions. Every decision lives in `omeka_api::page_history`; these handlers
//! authenticate and copy the answer onto the wire. Upstream has neither route.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Uri};
use axum::response::Response;
use axum::routing::get;

use crate::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/site_pages/{id}/versions", get(list))
        .route("/api/site_pages/{id}/versions/{version}", get(version))
}

async fn list(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    match crate::open(&state, &uri, &headers).await {
        Ok(api) => crate::finish(omeka_api::page_history::list(&api, &id).await),
        Err(error) => crate::error_response(error),
    }
}

async fn version(
    State(state): State<AppState>,
    Path((id, version)): Path<(String, String)>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    match crate::open(&state, &uri, &headers).await {
        Ok(api) => crate::finish(omeka_api::page_history::version(&api, &id, &version).await),
        Err(error) => crate::error_response(error),
    }
}
