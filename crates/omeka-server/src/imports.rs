//! The import routes that outlive the v1 importer (`docs/import-v2.md`,
//! "Wire"). Upstream has no such routes. P3-wire adds the v2 import routes
//! here.
//!
//! `GET /api/romeka/starters` serves the starter table the Item types page
//! reads, and `GET /api/romeka/jobs/{id}/log` the log the Jobs page polls.

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::Response;
use axum::routing::get;
use omeka_api::errors::{ApiError, ErrorDocument};
use omeka_api::import2::starters::STARTERS;
use omeka_db::DbError;
use omeka_db::ids::JobId;
use omeka_db::repos::JobRepo;

use crate::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/romeka/starters", get(starters))
        .route("/api/romeka/jobs/{id}/log", get(job_log))
}

/// A failure of an import route, with the status it answers.
#[derive(Debug)]
enum ImportError {
    Unauthenticated,
    Forbidden,
    NotFound(String),
    Internal(String),
    Api(ApiError),
}

impl ImportError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::Api(error) => return crate::error_response(error),
            Self::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                omeka_api::messages::AUTHENTICATION_REQUIRED.to_string(),
            ),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                "Only a global or site administrator may import an archive.".to_string(),
            ),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, message),
            Self::Internal(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
        };
        crate::document_response(status, &ErrorDocument::general(message))
    }
}

impl From<DbError> for ImportError {
    fn from(error: DbError) -> Self {
        Self::Internal(error.to_string())
    }
}

fn answer(result: Result<Response, ImportError>) -> Response {
    result.unwrap_or_else(ImportError::into_response)
}

/// Refused unless signed in with a role that may write items, item sets,
/// templates and vocabularies: the two administrator roles.
async fn admin(state: &AppState, uri: &Uri, headers: &HeaderMap) -> Result<(), ImportError> {
    let actor = crate::actor_for(state, uri, headers)
        .await
        .map_err(ImportError::Api)?;
    if actor.user().is_none() {
        return Err(ImportError::Unauthenticated);
    }
    if !actor.is_admin() {
        return Err(ImportError::Forbidden);
    }
    Ok(())
}

/// `GET /api/romeka/starters`: the item type starters, the one table the run
/// creates templates from.
async fn starters(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    answer(
        async {
            admin(&state, &uri, &headers).await?;
            let body = serde_json::to_vec(&STARTERS)
                .map_err(|error| ImportError::Internal(error.to_string()))?;
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap_or_else(|_| crate::empty_internal_error()))
        }
        .await,
    )
}

/// `GET /api/romeka/jobs/{id}/log`: the job's log so far, as text.
async fn job_log(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    answer(
        async {
            admin(&state, &uri, &headers).await?;
            let missing = || ImportError::NotFound(format!("No job has the id {id:?}."));
            let raw: u64 = id.parse().map_err(|_| missing())?;
            let row = JobRepo::new(&state.pool)
                .get(JobId::from(raw))
                .await?
                .ok_or_else(missing)?;
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                .body(Body::from(row.log.unwrap_or_default()))
                .unwrap_or_else(|_| crate::empty_internal_error()))
        }
        .await,
    )
}
