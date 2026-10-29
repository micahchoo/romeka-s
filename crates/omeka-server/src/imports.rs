//! The import routes (`docs/import-v2.md`, "Wire"). Upstream has no such
//! routes.
//!
//! The rules live in `omeka_api::import`: the plan boundary and the draft
//! life (`draft`), the evidence index (`scan`), the derive and its preview,
//! and Check. These handlers authenticate, read the request, call one of
//! those, and start the scan, run and undo jobs through the
//! [`crate::JobDispatch`] the process supplied. The states, when a job
//! counts as dead, and the one-import-at-a-time rule live in omeka-db
//! (`ImportRepo::start_run`, `start_undo`, `settle_dead`): every read first
//! records the dead, and a start is one serialized transaction.
//!
//! `template.csv` and `sheets` are the list round trip (`lists`); `pairs.csv`
//! and `pairs` are the pairs list round trip (`pairs`); `uploads` are the tus
//! routes into the staging area (`uploads`).
//!
//! `GET /api/romeka/starters` serves the starter table the Item types page
//! reads too, and `GET /api/romeka/jobs/{id}/log` the log the Jobs page polls.

mod archive;
mod lists;
mod multipart;
mod pairs;
mod progress;
mod resume;
mod run;
mod undo;
mod uploads;
mod wire;

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::Response;
use axum::routing::{get, head, post};
use omeka_api::import::draft::{self, Start};
use omeka_api::import::plan::SourcePlan;
use omeka_api::import::starters::STARTERS;
use omeka_api::messages;
use omeka_db::ids::{ImportId, JobId};
use omeka_db::repos::{ImportRepo, JobRepo, Paging};
use omeka_db::with_db;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::AppState;
pub use resume::{ResumeEnv, resume_imports};
use wire::{ImportError, ImportView, admin, blocking, body, draft_error, json, load, parse_id};

type Answer = Result<Response, ImportError>;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/romeka/archive", get(archive::archive))
        .route("/api/romeka/starters", get(starters))
        .route("/api/romeka/imports", get(list).post(create))
        .route(
            "/api/romeka/imports/{id}",
            get(show).put(save).delete(delete),
        )
        .route("/api/romeka/imports/{id}/scan", post(run::scan))
        .route("/api/romeka/imports/{id}/preview", post(run::preview))
        .route("/api/romeka/imports/{id}/run", post(run::run))
        .route("/api/romeka/imports/{id}/stop", post(run::stop))
        .route("/api/romeka/imports/{id}/undo", post(undo::undo))
        .route("/api/romeka/imports/{id}/undo/preview", get(undo::preview))
        .route("/api/romeka/imports/{id}/left.csv", get(left_csv))
        .route(
            "/api/romeka/imports/{id}/template.csv",
            get(lists::template),
        )
        .route("/api/romeka/imports/{id}/sheets", post(lists::upload))
        .route(
            "/api/romeka/imports/{id}/sheets/header",
            post(lists::header),
        )
        .route(
            "/api/romeka/imports/{id}/uploads",
            get(uploads::list)
                .post(uploads::create)
                .options(uploads::options),
        )
        .route(
            "/api/romeka/imports/{id}/uploads/{upload}",
            head(uploads::head)
                .patch(uploads::patch)
                .delete(uploads::delete)
                .options(uploads::options_one),
        )
        .route("/api/romeka/imports/{id}/pairs.csv", get(pairs::list))
        .route("/api/romeka/imports/{id}/pairs", post(pairs::upload))
        .route("/api/romeka/jobs/{id}/log", get(job_log))
}

fn answer(result: Answer) -> Response {
    result.unwrap_or_else(ImportError::into_response)
}

/// `GET /api/romeka/starters`: the item type starters, the one table the run
/// creates templates from.
async fn starters(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    answer(
        async {
            admin(&state, &uri, &headers).await?;
            Ok(json(StatusCode::OK, &STARTERS))
        }
        .await,
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct History {
    total: u64,
    page: u64,
    per_page: u64,
    items: Vec<ImportView>,
}

/// `GET /api/romeka/imports?page=&perPage=`: the history, newest first.
async fn list(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    answer(
        async {
            admin(&state, &uri, &headers).await?;
            let params = omeka_api::read::Params::parse(uri.query().unwrap_or(""));
            let number = |name: &str, default: u64| {
                params
                    .get(name)
                    .and_then(|raw| raw.parse::<u64>().ok())
                    .filter(|n| *n > 0)
                    .unwrap_or(default)
            };
            let per_page = number("perPage", 25).min(500);
            let page = number("page", 1);
            with_db!(&state.pool, |db| ImportRepo::new(db).settle_dead().await)?;
            let paging = Paging::new(
                i64::try_from(per_page).unwrap_or(500),
                i64::try_from((page - 1).saturating_mul(per_page)).unwrap_or(i64::MAX),
            );
            let items = with_db!(&state.pool, |db| ImportRepo::new(db).page(&paging).await)?
                .into_iter()
                .map(|row| ImportView::of(row, false))
                .collect();
            Ok(json(
                StatusCode::OK,
                &History {
                    total: with_db!(&state.pool, |db| ImportRepo::new(db).count().await)?,
                    page,
                    per_page,
                    items,
                },
            ))
        }
        .await,
    )
}

#[derive(Deserialize)]
#[serde(untagged)]
enum CreateBody {
    Sources(SourcesBody),
    Copy(CopyBody),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourcesBody {
    sources: Vec<SourcePlan>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CopyBody {
    copy_from: u64,
}

/// `POST /api/romeka/imports {sources} | {copyFrom}`: a new draft.
async fn create(
    State(state): State<AppState>,
    uri: Uri,
    headers: HeaderMap,
    payload: crate::limits::Payload,
) -> Response {
    answer(
        async {
            let actor = admin(&state, &uri, &headers).await?;
            let start = match body::<CreateBody>(&headers, &payload.0)? {
                CreateBody::Sources(SourcesBody { sources }) => {
                    let roots = state.archive_roots.clone();
                    let checked = sources.clone();
                    blocking(move || {
                        for source in &checked {
                            if let SourcePlan::Folder { root, path, .. } = source {
                                roots.source(root, path)?;
                            }
                        }
                        Ok(())
                    })
                    .await?;
                    Start::Sources(sources)
                }
                CreateBody::Copy(CopyBody { copy_from }) => Start::CopyFrom(ImportId(copy_from)),
            };
            let copied = match &start {
                Start::CopyFrom(from) => *from,
                Start::Sources(_) => ImportId(0),
            };
            let row = with_db!(&state.pool, |db| draft::create(db, actor.user(), start)
                .await)
            .map_err(|error| draft_error(error, copied, false))?;
            Ok(json(StatusCode::CREATED, &ImportView::of(row, true)))
        }
        .await,
    )
}

/// `GET /api/romeka/imports/{id}`: the import with its plan and revision.
async fn show(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    answer(
        async {
            admin(&state, &uri, &headers).await?;
            let row = load(&state, &id).await?;
            let started = match (row.status, row.job_id) {
                (omeka_db::rows::ImportStatus::Running, Some(job)) => {
                    with_db!(&state.pool, |db| JobRepo::new(db).get(job).await)?
                        .map(|job| job.started.and_utc().timestamp())
                }
                _ => None,
            };
            Ok(json(
                StatusCode::OK,
                &ImportView::running_since(row, true, started),
            ))
        }
        .await,
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveBody {
    plan: Value,
    revision: u64,
}

/// `PUT /api/romeka/imports/{id} {plan, revision}`: save a draft's plan.
async fn save(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
    payload: crate::limits::Payload,
) -> Response {
    answer(
        async {
            admin(&state, &uri, &headers).await?;
            let id = parse_id(&id)?;
            let saved: SaveBody = body(&headers, &payload.0)?;
            with_db!(&state.pool, |db| draft::save(
                db,
                id,
                &saved.plan,
                saved.revision
            )
            .await)
            .map_err(|error| draft_error(error, id, false))?;
            let row = load(&state, &id.0.to_string()).await?;
            Ok(json(StatusCode::OK, &ImportView::of(row, true)))
        }
        .await,
    )
}

/// `DELETE /api/romeka/imports/{id}`: discard a draft, an import that added
/// nothing, with its staged files.
async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    answer(
        async {
            admin(&state, &uri, &headers).await?;
            let id = parse_id(&id)?;
            // The row goes first, in the one statement that checks it is still a
            // draft, so a run that starts meanwhile never loses its files. The
            // files go after by folder, which needs no upload row.
            with_db!(&state.pool, |db| draft::delete(db, id).await)
                .map_err(|error| draft_error(error, id, true))?;
            with_db!(&state.pool, |db| omeka_api::import::staging::remove_all(
                db,
                &state.store,
                id
            )
            .await)?;
            Ok(Response::builder()
                .status(StatusCode::NO_CONTENT)
                .body(Body::empty())
                .unwrap_or_else(|_| crate::empty_internal_error()))
        }
        .await,
    )
}

/// `GET /api/romeka/imports/{id}/left.csv`: the file the run wrote to the
/// store when it ended.
async fn left_csv(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    answer(
        async {
            admin(&state, &uri, &headers).await?;
            let row = load(&state, &id).await?;
            let store = state.store.clone();
            let path = format!("imports/{}/left.csv", row.id.0);
            let bytes = blocking(move || {
                let missing = || ImportError::NotFound(messages::IMPORT_NO_LEFT_FILE.to_string());
                match store.exists(&path) {
                    Ok(true) => store
                        .read(&path)
                        .map_err(|error| ImportError::Internal(error.to_string())),
                    Ok(false) => Err(missing()),
                    Err(error) => Err(ImportError::Internal(error.to_string())),
                }
            })
            .await?;
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"import-{}-left.csv\"", row.id.0),
                )
                .body(Body::from(bytes))
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
            let row = with_db!(&state.pool, |db| JobRepo::new(db)
                .get(JobId::from(raw))
                .await)?
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
