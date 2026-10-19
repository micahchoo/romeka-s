//! `GET` and `PATCH /api/settings`: the global settings the admin's settings
//! page edits.
//!
//! The answer and the `PATCH` body are one map of setting keys to values. Every
//! decision lives in `omeka_api::settings`; this handler authenticates and
//! copies the answer onto the wire. Upstream has no such route: its
//! server-rendered `Admin\SettingController::browseAction` writes `SettingForm`
//! into the `setting` table directly.

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, Uri};
use axum::response::Response;
use axum::routing::get;

use crate::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/api/settings", get(read).patch(write))
}

async fn read(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    let api = match crate::open(&state, &uri, &headers).await {
        Ok(api) => api,
        Err(error) => return crate::error_response(error),
    };
    crate::finish(omeka_api::settings::read_global(&api).await)
}

async fn write(
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
    let api = match crate::open(&state, &uri, &headers).await {
        Ok(api) => api,
        Err(error) => return crate::error_response(error),
    };
    crate::finish(omeka_api::settings::write_global(&api, &body.0).await)
}
