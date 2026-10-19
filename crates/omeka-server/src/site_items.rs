//! `POST /api/sites/{id}/items`: change which items a site holds.
//!
//! The body is `{action, query | items | item_sets, dry_run?}` and the answer is
//! `{matched, added, removed, total}`. Every decision lives in
//! `omeka_api::site_items`; this handler checks the content type, authenticates,
//! and copies the answer onto the wire. Upstream has no such route: its admin
//! dispatches `Omeka\Job\UpdateSiteItems` from the server-rendered resources form.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Uri};
use axum::response::Response;
use axum::routing::post;

use crate::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/api/sites/{id}/items", post(change_items))
}

async fn change_items(
    State(state): State<AppState>,
    Path(id): Path<String>,
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
    crate::finish(omeka_api::site_items::handle(&api, &id, &body.0).await)
}
