//! The count routes behind a set's place, a record's "Linked from" and an item type's fields.
//! Upstream has none of them.
//!
//! | Route | Answer |
//! | --- | --- |
//! | `GET /api/romeka/item-sets/counts?id[]=..` | `{item_sets: [{id, items}]}` |
//! | `GET /api/romeka/links/{id}` | `{links: [{kind, property_id, term, label, count}]}` |
//! | `GET /api/romeka/field-counts?resource_template_id=..` | `{total, fields: [{property_id, filled}]}` |
//!
//! The rules live in `omeka_api::read::place_counts`; these handlers authenticate and copy the
//! answer onto the wire.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, Uri, header};
use axum::response::Response;
use axum::routing::get;
use omeka_api::read::{Params, place_counts};

use crate::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/romeka/item-sets/counts", get(item_set_counts))
        .route("/api/romeka/links/{id}", get(links))
        .route("/api/romeka/field-counts", get(field_counts))
}

async fn field_counts(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    let params = Params::parse(uri.query().unwrap_or(""));
    match crate::open(&state, &uri, &headers).await {
        Ok(api) => no_store(crate::finish(
            place_counts::field_counts(&api, &params).await,
        )),
        Err(error) => crate::error_response(error),
    }
}

async fn item_set_counts(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    let params = Params::parse(uri.query().unwrap_or(""));
    match crate::open(&state, &uri, &headers).await {
        Ok(api) => no_store(crate::finish(
            place_counts::item_set_counts(&api, &params).await,
        )),
        Err(error) => crate::error_response(error),
    }
}

async fn links(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    match crate::open(&state, &uri, &headers).await {
        Ok(api) => no_store(crate::finish(place_counts::links(&api, &id).await)),
        Err(error) => crate::error_response(error),
    }
}

/// A count follows every write, and it answers one caller's visibility.
fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
