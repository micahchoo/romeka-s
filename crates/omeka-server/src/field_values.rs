//! The field value routes: one field's texts across the archive, and moving one text to another
//! field.
//!
//! | Route | Answer |
//! | --- | --- |
//! | `GET /api/romeka/field-values/{property}` | `{property, values: [{value, items, media, item_sets, editable}]}` |
//! | `POST /api/romeka/field-values/move` | `{changed, skipped, moved, merged, copied, not_in_form}` |
//!
//! Every decision lives in `omeka_api::field_values`. Upstream has neither route.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Uri};
use axum::response::Response;
use axum::routing::{get, post};
use omeka_api::field_values;

use crate::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/romeka/field-values/move", post(move_value))
        .route("/api/romeka/field-values/{property}", get(census))
}

async fn census(
    State(state): State<AppState>,
    Path(property): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    match crate::open(&state, &uri, &headers).await {
        Ok(api) => crate::finish(field_values::census(&api, &property).await),
        Err(error) => crate::error_response(error),
    }
}

async fn move_value(
    State(state): State<AppState>,
    uri: Uri,
    headers: HeaderMap,
    body: crate::limits::Payload,
) -> Response {
    if !crate::content_type_is_json(&headers) {
        return crate::error_response(omeka_api::write::body::content_type_error(
            crate::media_type(&headers),
        ));
    }
    match crate::open(&state, &uri, &headers).await {
        Ok(api) => crate::finish(field_values::move_value(&api, &body.0).await),
        Err(error) => crate::error_response(error),
    }
}
