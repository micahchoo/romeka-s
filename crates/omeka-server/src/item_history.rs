//! `GET /api/romeka/items/{id}/history`: an item's history entries, newest first. Upstream has
//! none. The rules live in `omeka_api::read::item_history`; this handler authenticates and copies
//! the answer onto the wire.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, Uri, header};
use axum::response::Response;
use axum::routing::get;
use omeka_api::read::item_history;

use crate::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/api/romeka/items/{id}/history", get(history))
}

async fn history(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let mut response = match crate::open(&state, &uri, &headers).await {
        Ok(api) => crate::finish(item_history::item_history(&api, &id).await),
        Err(error) => crate::error_response(error),
    };
    // Every item write adds an entry, and the answer is one caller's view.
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
