//! The bulk change routes (`/api/romeka/bulk`). Upstream has none: its batch edit is a
//! server-rendered form that dispatches `BatchUpdate`.
//!
//! The rules live in `omeka_api::bulk`; these handlers authenticate, call one entry point, hand a
//! composed job to the process's [`crate::JobDispatch`], and copy the answer onto the wire. A 409
//! carries `code` (`cannotRun`, `expired`, `cannotUndo`, `notRunning`, `purged`) and the run's
//! `status`, so the admin branches on the code.
//!
//! The trash of a bulk delete empties itself: each read of the recent list starts a purge of the
//! trash older than `romeka_trash_days` in the background, as the server's start does, and an
//! administrator can empty all of it now (`POST /api/romeka/bulk/trash/purge`).

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::Response;
use axum::routing::{get, post};
use omeka_api::bulk::{self, BulkError, PurgeScope, Purged};
use omeka_api::errors::ApiError;
use omeka_db::DbPool;
use omeka_db::ids::JobId;
use omeka_db::repos::{BulkRepo, JobRepo};
use omeka_db::with_db;
use omeka_media::store::Store;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/romeka/bulk", get(list).post(preview))
        .route("/api/romeka/bulk/{id}", get(show))
        .route("/api/romeka/bulk/{id}/run", post(run))
        .route("/api/romeka/bulk/{id}/stop", post(stop))
        .route("/api/romeka/bulk/{id}/undo/preview", post(undo_preview))
        .route("/api/romeka/bulk/{id}/undo", post(undo))
        .route("/api/romeka/bulk/trash/purge", post(empty_trash))
}

/// Empty the trash of every bulk delete older than the trash days, logging what it did. The
/// server runs it at start and after each read of the recent list.
pub async fn purge_due_trash(pool: &DbPool, store: &dyn Store) {
    match bulk::purge(pool, store, PurgeScope::Due).await {
        Ok(purged) if purged != Purged::default() => {
            tracing::info!(
                runs = purged.runs,
                items = purged.items,
                media = purged.media,
                item_sets = purged.item_sets,
                "emptied due bulk trash"
            );
        }
        Ok(_) => {}
        Err(error) => tracing::warn!("cannot empty the bulk trash: {error}"),
    }
}

/// Settle every bulk run whose worker died while the server was down: a run stops, an undo is
/// left partly undone. Either one goes on with a new click.
pub async fn settle_bulk_runs(pool: &DbPool) {
    match with_db!(pool, |db| BulkRepo::new(db).settle_dead().await) {
        Ok(settled) if !settled.is_empty() => {
            tracing::info!(runs = ?settled, "settled bulk runs whose worker died");
        }
        Ok(_) => {}
        Err(error) => tracing::warn!("cannot settle dead bulk runs: {error}"),
    }
}

type Answer = Result<Response, BulkError>;

fn answer(result: Answer) -> Response {
    result.unwrap_or_else(|error| match error {
        BulkError::Api(error) => crate::error_response(error),
        BulkError::Conflict {
            code,
            message,
            status,
        } => json_response(
            StatusCode::CONFLICT,
            &json!({
                "errors": { "error": message },
                "code": code,
                "status": status.as_str(),
            }),
        ),
    })
}

fn json_response<T: Serialize>(status: StatusCode, body: &T) -> Response {
    match serde_json::to_vec(body) {
        Ok(bytes) => Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, crate::JSONLD_CONTENT_TYPE)
            .body(Body::from(bytes))
            .unwrap_or_else(|_| crate::empty_internal_error()),
        Err(_) => crate::empty_internal_error(),
    }
}

fn require_json(headers: &HeaderMap) -> Result<(), BulkError> {
    if crate::content_type_is_json(headers) {
        return Ok(());
    }
    Err(BulkError::Api(omeka_api::write::body::content_type_error(
        crate::media_type(headers),
    )))
}

/// `POST /api/romeka/bulk {query, change}`: 201 with the preview of the run it wrote.
async fn preview(
    State(state): State<AppState>,
    uri: Uri,
    headers: HeaderMap,
    body: crate::limits::Payload,
) -> Response {
    answer(
        async {
            require_json(&headers)?;
            let api = crate::open(&state, &uri, &headers).await?;
            let preview = bulk::preview::preview(&api, &body.0).await?;
            Ok(json_response(StatusCode::CREATED, &preview))
        }
        .await,
    )
}

async fn list(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    let answered = answer(
        async {
            let api = crate::open(&state, &uri, &headers).await?;
            Ok(json_response(StatusCode::OK, &bulk::list(&api).await?))
        }
        .await,
    );
    let (pool, store) = (state.pool.clone(), state.store.clone());
    tokio::spawn(async move {
        purge_due_trash(&pool, store.as_ref()).await;
    });
    answered
}

/// `POST /api/romeka/bulk/trash/purge`: an administrator empties every trash at rest now and
/// reads `{runs, items}`.
async fn empty_trash(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    answer(
        async {
            let api = crate::open(&state, &uri, &headers).await?;
            Ok(json_response(
                StatusCode::OK,
                &bulk::empty_trash(&api).await?,
            ))
        }
        .await,
    )
}

async fn show(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    answer(
        async {
            let api = crate::open(&state, &uri, &headers).await?;
            Ok(json_response(StatusCode::OK, &bulk::show(&api, &id).await?))
        }
        .await,
    )
}

async fn run(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    answer(
        async {
            let api = crate::open(&state, &uri, &headers).await?;
            let job = bulk::run(&api, &id).await?;
            start_job(&state, job).await
        }
        .await,
    )
}

async fn stop(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    answer(
        async {
            let api = crate::open(&state, &uri, &headers).await?;
            bulk::stop(&api, &id).await?;
            Ok(json_response(StatusCode::ACCEPTED, &json!({})))
        }
        .await,
    )
}

async fn undo_preview(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    answer(
        async {
            let api = crate::open(&state, &uri, &headers).await?;
            let preview = bulk::undo_preview(&api, &id).await?;
            Ok(json_response(StatusCode::OK, &preview))
        }
        .await,
    )
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct UndoBody {
    #[serde(default)]
    force: bool,
}

async fn undo(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
    body: crate::limits::Payload,
) -> Response {
    answer(
        async {
            let request: UndoBody = if body.0.is_empty() {
                UndoBody::default()
            } else {
                require_json(&headers)?;
                serde_json::from_slice(&body.0).map_err(|error| {
                    ApiError::bad_request(omeka_api::messages::import_request_unreadable(error))
                })?
            };
            let api = crate::open(&state, &uri, &headers).await?;
            let job = bulk::undo(&api, &id, request.force).await?;
            start_job(&state, job).await
        }
        .await,
    )
}

/// Hand the job to the runner and answer `{jobId}`. A start that fails ends the job `error`;
/// the run's heartbeat then goes stale and a reader settles it.
async fn start_job(state: &AppState, job: JobId) -> Answer {
    if let Err(error) = state.jobs.start(job) {
        let repo_error = |error: omeka_db::DbError| BulkError::from(error);
        with_db!(&state.pool, |db| JobRepo::new(db)
            .append_log(job, &error.to_string())
            .await)
        .map_err(repo_error)?;
        with_db!(&state.pool, |db| JobRepo::new(db)
            .finish(job, "error")
            .await)
        .map_err(repo_error)?;
        return Err(BulkError::Api(ApiError::internal(error)));
    }
    Ok(json_response(StatusCode::OK, &json!({ "jobId": job.0 })))
}
