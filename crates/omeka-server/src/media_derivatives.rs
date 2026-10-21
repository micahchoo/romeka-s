//! `POST /api/romeka/media/{id}/derivatives/retry`: queue one media's access
//! copy again after the command that makes it failed, or after the tool a
//! previous decision found missing was installed.
//!
//! The admin media page offers the button only to a reader who may edit the
//! media and only when the copy failed, so the route's grant is the media
//! update grant (`AbstractEntityAdapter::authorize($entity, 'update')`). It
//! queues one row, starts the install's one derivative worker, and answers the
//! media document the admin reads, whose `o-module-romeka:access` member is
//! `pending` again.
//!
//! A media with nothing to retry (its copy is ready, waiting, or its original
//! is what the reader gets) is a 409, because the answer would be a media whose
//! state did not change. An id that names no media the caller may see is the
//! same 404 as the media read. Upstream has no such route: its admin makes
//! derivatives inside the request and has nothing to retry.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Uri};
use axum::response::Response;
use axum::routing::post;
use omeka_api::errors::ApiError;
use omeka_api::messages;
use omeka_api::read::ApiResponse;
use omeka_db::ids::MediaId;
use omeka_db::repos::MediaRepo;
use omeka_db::with_db;

use crate::{AppState, jobs, open};

/// The class name the media update grant names.
const MEDIA_CLASS: &str = "Omeka\\Entity\\Media";

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/api/romeka/media/{id}/derivatives/retry", post(retry))
}

async fn retry(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let result = match id.parse::<u64>() {
        Ok(id) => retry_one(&state, MediaId(id), &uri, &headers).await,
        Err(_) => Err(ApiError::not_found(messages::NOT_FOUND)),
    };
    crate::finish(result)
}

/// Queue the copy and answer the media document as the API read builds it.
async fn retry_one(
    state: &AppState,
    id: MediaId,
    uri: &Uri,
    headers: &HeaderMap,
) -> Result<ApiResponse, ApiError> {
    let api = open(state, uri, headers).await?;
    // The visible read first, so a private media the caller cannot see is the
    // same 404 as a missing id, not a 403 that tells them it exists.
    api.read_member("media", &id.0.to_string()).await?;
    let Some(row) =
        with_db!(&state.pool, |db| MediaRepo::new(db).get(id).await).map_err(ApiError::internal)?
    else {
        return Err(ApiError::not_found(messages::NOT_FOUND));
    };
    omeka_api::write::access::resource_entity_update(
        api.actor(),
        MEDIA_CLASS,
        row.resource.owner_id,
    )?;
    if !omeka_api::derivatives::retry_access(&state.pool, id).await? {
        return Err(ApiError::Conflict {
            message: messages::NO_DERIVATIVE_TO_RETRY.to_string(),
        });
    }
    jobs::start_derivatives(&state.pool, state.jobs.as_ref(), api.actor().user()).await;
    api.read_member("media", &id.0.to_string()).await
}
