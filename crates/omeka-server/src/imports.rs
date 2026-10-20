//! The archive import routes (`docs/archive-import.md`, "Wire"). Upstream has no
//! such routes.
//!
//! The import's rules live in `omeka_api::import`: the plan boundary, the
//! preview and its check. These handlers authenticate, read and move the
//! `romeka_import` rows, and start the run and undo jobs through the
//! [`crate::JobDispatch`] the process supplied. The states, when a job counts
//! as dead, and the one-import-at-a-time rule live in omeka-db
//! (`ImportRepo::start_run`, `start_undo`, `settle_dead`): every read first
//! records the dead, and a start is one serialized transaction.

mod wire;

use std::collections::BTreeSet;
use std::path::PathBuf;

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::Response;
use axum::routing::{get, post};
use omeka_api::auth::Actor;
use omeka_api::import::archive::{ArchiveError, ArchiveRoot, Entry};
use omeka_api::import::catalog;
use omeka_api::import::plan::ImportPlan;
use omeka_api::import::preview::{PreviewRequest, Section};
use omeka_api::import::run::ImportSummary;
use omeka_db::ids::{ImportId, JobId};
use omeka_db::repos::{ImportRepo, JobRepo, StartOutcome};
use omeka_db::rows::{ImportRow, ImportStatus, NewImport, NewJob};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::AppState;
use wire::{ImportError, ImportView, admin, json, require_json};

/// The job class of a run, `job.class`.
pub const IMPORT_JOB: &str = "Romeka\\Job\\ImportArchive";
/// The job class of an undo.
pub const UNDO_JOB: &str = "Romeka\\Job\\UndoImport";

type Answer = Result<Response, ImportError>;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/romeka/archive", get(archive))
        .route("/api/romeka/imports", get(list).post(create))
        .route(
            "/api/romeka/imports/{id}",
            get(show).put(save_plan).delete(delete_draft),
        )
        .route("/api/romeka/imports/{id}/preview", post(preview))
        .route("/api/romeka/imports/{id}/run", post(run))
        .route("/api/romeka/imports/{id}/undo", post(undo))
        .route("/api/romeka/imports/{id}/left.csv", get(left_out))
        .route("/api/romeka/jobs/{id}/log", get(job_log))
}

fn answer(result: Answer) -> Response {
    result.unwrap_or_else(ImportError::into_response)
}

/// The configured archive root, or 404 when the import is off.
fn archive_root(state: &AppState) -> Result<PathBuf, ImportError> {
    state
        .archive_root
        .clone()
        .ok_or_else(|| ImportError::NotFound("The archive import is off.".to_string()))
}

/// Folder work on the blocking pool: canonicalizing and walking hit the disk.
async fn blocking<T, F>(task: F) -> Result<T, ImportError>
where
    F: FnOnce() -> Result<T, ArchiveError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|error| ImportError::Internal(error.to_string()))?
        .map_err(ImportError::from)
}

fn parse_id(raw: &str) -> Result<ImportId, ImportError> {
    raw.parse()
        .map(ImportId)
        .map_err(|_| ImportError::NotFound(format!("No import has the id {raw:?}.")))
}

/// The import `raw` names, read after the dead have been recorded, so its
/// status is the one the wire shows.
async fn load(state: &AppState, raw: &str) -> Result<ImportRow, ImportError> {
    let id = parse_id(raw)?;
    let repo = ImportRepo::new(&state.pool);
    repo.settle_dead().await?;
    repo.get(id)
        .await?
        .ok_or_else(|| ImportError::NotFound(format!("No import has the id {raw:?}.")))
}

fn from_body<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, ImportError> {
    serde_json::from_slice(bytes).map_err(|error| ImportError::BadRequest(error.to_string()))
}

#[derive(Serialize)]
struct Listing {
    enabled: bool,
    path: String,
    entries: Vec<Entry>,
}

/// `GET /api/romeka/archive?path=`: one folder of the archive.
async fn archive(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    answer(
        async {
            admin(&state, &uri, &headers).await?;
            let Some(root) = state.archive_root.clone() else {
                let off = Listing {
                    enabled: false,
                    path: String::new(),
                    entries: Vec::new(),
                };
                return Ok(json(StatusCode::OK, &off));
            };
            let params = omeka_api::read::Params::parse(uri.query().unwrap_or(""));
            let path = params.get("path").unwrap_or("").to_string();
            let listing = blocking(move || {
                let folder = ArchiveRoot::new(root)?.folder(&path)?;
                Ok(Listing {
                    enabled: true,
                    path: folder.relative().to_string(),
                    entries: folder.entries()?,
                })
            })
            .await?;
            Ok(json(StatusCode::OK, &listing))
        }
        .await,
    )
}

/// `GET /api/romeka/imports`: the history, newest first.
async fn list(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    answer(
        async {
            admin(&state, &uri, &headers).await?;
            let repo = ImportRepo::new(&state.pool);
            repo.settle_dead().await?;
            let views: Vec<_> = repo
                .list()
                .await?
                .into_iter()
                .map(|row| ImportView::of(row, false))
                .collect();
            Ok(json(StatusCode::OK, &views))
        }
        .await,
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateBody {
    folder: String,
}

/// `POST /api/romeka/imports {folder}`: a new draft with the default plan.
async fn create(
    State(state): State<AppState>,
    uri: Uri,
    headers: HeaderMap,
    body: crate::limits::Payload,
) -> Response {
    answer(
        async {
            let actor = admin(&state, &uri, &headers).await?;
            require_json(&headers)?;
            let body: CreateBody = from_body(&body.0)?;
            let root = archive_root(&state)?;
            let folder = blocking(move || {
                Ok(ArchiveRoot::new(root)?
                    .folder(&body.folder)?
                    .relative()
                    .to_string())
            })
            .await?;
            let row = ImportRepo::new(&state.pool)
                .create_draft(&NewImport {
                    owner_id: actor.user(),
                    folder,
                    plan: ImportPlan::new_draft().to_json(),
                })
                .await?;
            Ok(json(StatusCode::OK, &ImportView::of(row, true)))
        }
        .await,
    )
}

/// `GET /api/romeka/imports/{id}`: the import with its plan.
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
            Ok(json(StatusCode::OK, &ImportView::of(row, true)))
        }
        .await,
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlanBody {
    plan: Value,
}

/// `PUT /api/romeka/imports/{id} {plan}`: save a draft's plan.
async fn save_plan(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
    body: crate::limits::Payload,
) -> Response {
    answer(
        async {
            admin(&state, &uri, &headers).await?;
            require_json(&headers)?;
            let row = load(&state, &id).await?;
            let body: PlanBody = from_body(&body.0)?;
            let plan = ImportPlan::from_json(&body.plan)
                .map_err(|error| ImportError::BadRequest(error.to_string()))?;
            let repo = ImportRepo::new(&state.pool);
            if !repo.update_plan(row.id, &plan.to_json()).await? {
                return Err(ImportError::Conflict(
                    "Only a draft's plan can change.".to_string(),
                ));
            }
            let row = load(&state, &id).await?;
            Ok(json(StatusCode::OK, &ImportView::of(row, true)))
        }
        .await,
    )
}

/// `DELETE /api/romeka/imports/{id}`: delete a draft.
async fn delete_draft(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    answer(
        async {
            admin(&state, &uri, &headers).await?;
            let row = load(&state, &id).await?;
            if !ImportRepo::new(&state.pool).delete_draft(row.id).await? {
                return Err(ImportError::Conflict(
                    "Only a draft can be deleted.".to_string(),
                ));
            }
            Ok(Response::builder()
                .status(StatusCode::NO_CONTENT)
                .body(Body::empty())
                .unwrap_or_else(|_| crate::empty_internal_error()))
        }
        .await,
    )
}

/// `POST /api/romeka/imports/{id}/preview`: the sections the request names.
async fn preview(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
    body: crate::limits::Payload,
) -> Response {
    answer(
        async {
            admin(&state, &uri, &headers).await?;
            require_json(&headers)?;
            let root = archive_root(&state)?;
            let row = load(&state, &id).await?;
            let request: PreviewRequest = from_body(&body.0)?;
            let preview = derive_preview(&state, root, &row, request).await?;
            Ok(json(StatusCode::OK, &preview))
        }
        .await,
    )
}

async fn derive_preview(
    state: &AppState,
    root: PathBuf,
    row: &ImportRow,
    request: PreviewRequest,
) -> Result<omeka_api::import::preview::Preview, ImportError> {
    let stored = ImportPlan::from_json(&row.plan)
        .map_err(|error| ImportError::Internal(format!("stored plan: {error}")))?;
    let catalog = catalog::load(&state.pool).await?;
    let folder = row.folder.clone();
    let reader = state.tag_reader.clone();
    blocking(move || {
        let folder = ArchiveRoot::new(root)?.folder(&folder)?;
        omeka_api::import::preview_with(&folder, &stored, &catalog, &request, &reader)
    })
    .await
}

/// `GET /api/romeka/imports/{id}/left.csv`: the rows the import leaves out
/// (skipped, left with problems, failed in its run), each with its reason.
async fn left_out(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    answer(
        async {
            admin(&state, &uri, &headers).await?;
            let root = archive_root(&state)?;
            let row = load(&state, &id).await?;
            let plan = ImportPlan::from_json(&row.plan)
                .map_err(|error| ImportError::Internal(format!("stored plan: {error}")))?;
            let summary: ImportSummary = match row.summary {
                Some(value) => serde_json::from_value(value)
                    .map_err(|error| ImportError::Internal(format!("stored summary: {error}")))?,
                None => ImportSummary::default(),
            };
            let catalog = catalog::load_for_import(&state.pool, row.id).await?;
            let reader = state.tag_reader.clone();
            let folder = row.folder;
            let csv = blocking(move || {
                let folder = ArchiveRoot::new(root)?.folder(&folder)?;
                omeka_api::import::left::left_out_csv(
                    &folder,
                    &plan,
                    &catalog,
                    &summary.failures,
                    &reader,
                )
            })
            .await?;
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
                .header(
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"left-out.csv\"",
                )
                .body(Body::from(csv))
                .unwrap_or_else(|_| crate::empty_internal_error()))
        }
        .await,
    )
}

/// `POST /api/romeka/imports/{id}/run`: start a draft whose check can run.
async fn run(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    answer(
        async {
            let actor = admin(&state, &uri, &headers).await?;
            let root = archive_root(&state)?;
            let row = load(&state, &id).await?;
            if row.status != ImportStatus::Draft {
                return Err(not_a_draft());
            }
            let request = PreviewRequest {
                include: BTreeSet::from([Section::Check]),
                ..PreviewRequest::default()
            };
            let preview = derive_preview(&state, root, &row, request).await?;
            if !preview.check.is_some_and(|check| check.can_run) {
                let reason = preview.error.map_or_else(
                    || "the check finds nothing to create".to_string(),
                    |error| format!("the check reports {}", error.key),
                );
                return Err(ImportError::Conflict(format!(
                    "This import cannot run: {reason}."
                )));
            }
            let outcome = ImportRepo::new(&state.pool).start_run(row.id).await?;
            started(outcome, |_| not_a_draft())?;
            let job = compose_job(&state, &actor, IMPORT_JOB, row.id).await?;
            start_job(&state, job).await
        }
        .await,
    )
}

/// `POST /api/romeka/imports/{id}/undo`: undo the import that holds the open
/// window once its run has ended.
async fn undo(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    answer(
        async {
            let actor = admin(&state, &uri, &headers).await?;
            let row = load(&state, &id).await?;
            let outcome = ImportRepo::new(&state.pool).start_undo(row.id).await?;
            started(outcome, |undoable| {
                ImportError::Conflict(if undoable {
                    "An import can be undone once its run has ended.".to_string()
                } else {
                    "Only the last import can be undone, until the next one starts.".to_string()
                })
            })?;
            let job = compose_job(&state, &actor, UNDO_JOB, row.id).await?;
            start_job(&state, job).await
        }
        .await,
    )
}

fn not_a_draft() -> ImportError {
    ImportError::Conflict("Only a draft can run; an import never runs twice.".to_string())
}

/// A start's outcome as the route answers it. `refused` gets whether the
/// import holds the undo window.
fn started(
    outcome: StartOutcome,
    refused: impl FnOnce(bool) -> ImportError,
) -> Result<(), ImportError> {
    match outcome {
        StartOutcome::Started => Ok(()),
        StartOutcome::Missing => Err(ImportError::NotFound("No such import.".to_string())),
        StartOutcome::Refused { undoable, .. } => Err(refused(undoable)),
        StartOutcome::Busy(_) => Err(ImportError::Conflict(
            "Another import is running or undoing.".to_string(),
        )),
    }
}

/// A `starting` job row for `import`, linked from the import. The import is
/// already running or undoing: a failure from here on leaves it to die by
/// its heartbeat, which records it failed and undoable.
async fn compose_job(
    state: &AppState,
    actor: &Actor,
    class: &str,
    import: ImportId,
) -> Result<JobId, ImportError> {
    let job = JobRepo::new(&state.pool)
        .create(&NewJob {
            owner_id: actor.user(),
            pid: None,
            status: Some("starting".to_string()),
            class: class.to_string(),
            args: Some(json!({ "importId": import.0 })),
            log: None,
            started: None,
        })
        .await?;
    ImportRepo::new(&state.pool).set_job(import, job.id).await?;
    Ok(job.id)
}

/// Hand the job to the runner and answer `{jobId}`. A start that fails ends
/// the job `error`, so the next read records the import failed, and it may
/// be undone.
async fn start_job(state: &AppState, job: JobId) -> Answer {
    if let Err(error) = state.jobs.start(job) {
        let jobs = JobRepo::new(&state.pool);
        jobs.append_log(job, &error.to_string()).await?;
        jobs.finish(job, "error").await?;
        return Err(ImportError::Internal(error.to_string()));
    }
    Ok(json(StatusCode::OK, &json!({ "jobId": job.0 })))
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
