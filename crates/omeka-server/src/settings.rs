//! `GET` and `PATCH /api/settings`: the global settings the admin's settings
//! page edits. `GET /api/romeka/installation`: the four of them every admin
//! screen applies, for any caller. `GET /api/romeka/time-zones`: the names a
//! `time_zone` write accepts.
//!
//! The answer and the `PATCH` body are one map of setting keys to values. Every
//! decision lives in `omeka_api::settings`; these handlers authenticate, start
//! the index job a write composed, and copy the answer onto the wire. Upstream
//! has no such routes: its server-rendered `Admin\SettingController::browseAction`
//! writes `SettingForm` into the `setting` table directly, and its views read
//! the settings in-process.

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, Uri, header};
use axum::response::Response;
use axum::routing::get;
use omeka_api::errors::ApiError;
use omeka_db::repos::JobRepo;
use omeka_db::with_db;

use crate::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/settings", get(read).patch(write))
        .route("/api/romeka/installation", get(installation))
        .route("/api/romeka/time-zones", get(time_zones))
}

async fn installation(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    let api = match crate::open(&state, &uri, &headers).await {
        Ok(api) => api,
        Err(error) => return crate::error_response(error),
    };
    let mut response = crate::finish(omeka_api::settings::read_installation(&api).await);
    // A save on the Settings page changes the answer at once.
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn time_zones() -> Response {
    crate::finish(omeka_api::settings::read_time_zones())
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
    let result = async {
        let (answer, job) = omeka_api::settings::write_global(&api, &body.0).await?;
        if let Some(job) = job
            && let Err(error) = state.jobs.start(job)
        {
            // The settings are stored; only the index did not start. A row left
            // `starting` would stand for an index on its way.
            let _ = with_db!(&state.pool, |db| JobRepo::new(db)
                .append_log(job, &error.to_string())
                .await);
            let _ = with_db!(&state.pool, |db| JobRepo::new(db)
                .finish(job, "error")
                .await);
            return Err(ApiError::internal(error));
        }
        Ok(answer)
    }
    .await;
    crate::finish(result)
}
