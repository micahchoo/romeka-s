//! The fixity routes. Upstream has none: it records `media.sha256` and never checks it.
//!
//! | Route | Answer |
//! | --- | --- |
//! | `POST /api/romeka/fixity` | 201 `{job}` for a new check, 200 `{job}` for the one already running |
//! | `GET /api/romeka/fixity` | `{check, running}`: the latest check with a result, and the running one |
//! | `GET /api/romeka/fixity/{job}` | `{job, status, started, ended, checked, ok, differs, missing, unreadable}` |
//!
//! The rules live in `omeka_api::fixity::view`; the check runs as `Romeka\Job\FixityCheck` in the
//! job runner. These handlers authenticate, call one entry point, hand a new job to the process's
//! [`crate::JobDispatch`], and copy the answer onto the wire.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri, header};
use axum::response::Response;
use axum::routing::get;
use omeka_api::errors::ApiError;
use omeka_api::fixity::view;
use omeka_api::messages;
use omeka_api::read::ApiResponse;
use omeka_db::ids::JobId;
use omeka_db::repos::JobRepo;
use omeka_db::with_db;
use serde::Serialize;

use crate::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/romeka/fixity", get(latest).post(start))
        .route("/api/romeka/fixity/{job}", get(one))
}

async fn latest(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    let result = async {
        let api = crate::open(&state, &uri, &headers).await?;
        json(StatusCode::OK, &view::latest(&api).await?)
    }
    .await;
    no_store(crate::finish(result))
}

async fn one(
    State(state): State<AppState>,
    Path(job): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let result = async {
        let api = crate::open(&state, &uri, &headers).await?;
        let Ok(job) = job.parse::<u64>() else {
            return Err(ApiError::not_found(messages::NOT_FOUND));
        };
        json(StatusCode::OK, &view::one(&api, JobId(job)).await?)
    }
    .await;
    no_store(crate::finish(result))
}

#[derive(Serialize)]
struct Started {
    job: u64,
}

async fn start(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    let result = async {
        let api = crate::open(&state, &uri, &headers).await?;
        let (job, composed) = view::start(&api).await?;
        if !composed {
            return json(StatusCode::OK, &Started { job: job.0 });
        }
        if let Err(error) = state.jobs.start(job) {
            // A row left `starting` would stand for a running check and block the next one.
            let _ = with_db!(&state.pool, |db| JobRepo::new(db)
                .append_log(job, &error.to_string())
                .await);
            let _ = with_db!(&state.pool, |db| JobRepo::new(db)
                .finish(job, "error")
                .await);
            return Err(ApiError::internal(error));
        }
        json(StatusCode::CREATED, &Started { job: job.0 })
    }
    .await;
    crate::finish(result)
}

fn json<T: Serialize>(status: StatusCode, body: &T) -> Result<ApiResponse, ApiError> {
    let mut answer =
        ApiResponse::document(serde_json::to_string(body).map_err(ApiError::internal)?);
    answer.status = status;
    Ok(answer)
}

/// A count follows every check and every media write, and it answers one caller's visibility.
fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
