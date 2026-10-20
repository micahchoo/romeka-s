//! The routes that read the evidence or start a job: scan, preview, run,
//! stop and undo (`docs/import-v2.md`, "Wire", "Lifecycle and the ledger").
//!
//! Run is the one that guards: it rescans, derives again, and starts only
//! when the model's digest is the one the admin checked, no blocking flag
//! stands, and a delete is confirmed with its count. The start itself is
//! `ImportRepo::start_run`, one locked transaction that freezes the plan.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::Response;
use omeka_api::auth::Actor;
use omeka_api::import2::check::check;
use omeka_api::import2::plan::{Action, ImportPlanV2, PlanError};
use omeka_api::import2::preview::{self, Derived, PreviewRequest, derive_import};
use omeka_api::import2::scan::{self, SCAN_IN_REQUEST, SCAN_JOB};
use omeka_api::messages;
use omeka_db::ids::{ImportId, JobId};
use omeka_db::repos::{EntryRepo, ImportRepo, JobRepo, SettingRepo, StartOutcome};
use omeka_db::rows::{ImportRow, ImportStatus, NewJob};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::wire::{ImportError, STOP, admin, blocking, body, json, load, owners};
use super::{Answer, answer};
use crate::AppState;

/// The job class of a run, `job.class`; its args are `{"importId": n}`.
pub const RUN_JOB: &str = "Romeka\\Job\\ImportRun";
/// The job class of an undo; its args are `{"importId": n, "force": ...}`.
pub const UNDO_JOB: &str = "Romeka\\Job\\ImportUndo";

/// The plan a non-draft runs from: the one its first run froze.
fn stored_plan(row: &ImportRow) -> Result<ImportPlanV2, ImportError> {
    let stored = row.ran_plan.as_ref().unwrap_or(&row.plan);
    ImportPlanV2::from_json(&stored.to_string()).map_err(|error| match error {
        PlanError::V1Plan => ImportError::conflict("v1Plan", messages::IMPORT_V1_PLAN),
        other => ImportError::Internal(format!("the stored plan does not parse: {other}")),
    })
}

fn is_busy(status: ImportStatus) -> bool {
    matches!(status, ImportStatus::Running | ImportStatus::Undoing)
}

async fn derive(
    state: &AppState,
    actor: &Actor,
    import: ImportId,
    plan: ImportPlanV2,
) -> Result<Derived, ImportError> {
    let files = omeka_api::settings::file_validation(&SettingRepo::new(&state.pool)).await?;
    Ok(derive_import(
        &state.pool,
        &state.archive_roots,
        &state.import_caches,
        import,
        Arc::new(plan),
        owners(actor),
        files,
    )
    .await?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JobStarted {
    job_id: u64,
}

/// `POST /api/romeka/imports/{id}/scan`: rescan the sources into the index.
/// An index of more than [`SCAN_IN_REQUEST`] entries is rescanned by a job,
/// and the answer is 202 with its id.
pub(super) async fn scan(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    answer(
        async {
            let actor = admin(&state, &uri, &headers).await?;
            let row = load(&state, &id).await?;
            if is_busy(row.status) {
                return Err(ImportError::conflict("busy", messages::IMPORT_SCAN_BUSY));
            }
            let plan = stored_plan(&row)?;
            if EntryRepo::new(&state.pool).count(row.id).await? > SCAN_IN_REQUEST {
                let job = compose_job(&state, &actor, SCAN_JOB, row.id, None).await?;
                return start_job(&state, job, StatusCode::ACCEPTED).await;
            }
            let scanned =
                scan::scan(&state.pool, &state.archive_roots, row.id, &plan.sources).await?;
            Ok(json(StatusCode::OK, &scanned))
        }
        .await,
    )
}

/// `POST /api/romeka/imports/{id}/preview`: the sections the request names,
/// for the stored plan or the one the request carries.
pub(super) async fn preview(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
    payload: crate::limits::Payload,
) -> Response {
    answer(
        async {
            let actor = admin(&state, &uri, &headers).await?;
            let request: PreviewRequest = body(&headers, &payload.0)?;
            let row = load(&state, &id).await?;
            let plan = match &request.plan {
                Some(sent) => {
                    ImportPlanV2::from_json(&sent.to_string()).map_err(|error| match error {
                        PlanError::V1Plan => {
                            ImportError::conflict("v1Plan", messages::IMPORT_V1_PLAN)
                        }
                        other => ImportError::BadRequest(messages::import_plan_refused(other)),
                    })?
                }
                None => stored_plan(&row)?,
            };
            let derived = derive(&state, &actor, row.id, plan).await?;
            let answer = blocking(move || {
                serde_json::to_vec(&preview::preview(&derived, &request))
                    .map_err(|error| ImportError::Internal(error.to_string()))
            })
            .await?;
            Ok(axum::http::Response::builder()
                .status(StatusCode::OK)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(answer))
                .unwrap_or_else(|_| crate::empty_internal_error()))
        }
        .await,
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RunBody {
    digest: String,
    #[serde(default)]
    confirm: Option<u64>,
}

/// `POST /api/romeka/imports/{id}/run {digest, confirm?}`: start a draft, or
/// retry a failed run, once the model is the one the admin checked.
pub(super) async fn run(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
    payload: crate::limits::Payload,
) -> Response {
    answer(
        async {
            let actor = admin(&state, &uri, &headers).await?;
            let request: RunBody = body(&headers, &payload.0)?;
            let row = load(&state, &id).await?;
            if !matches!(row.status, ImportStatus::Draft | ImportStatus::RunFailed) {
                return Err(cannot_run(row.status));
            }
            let plan = stored_plan(&row)?;
            scan::scan(&state.pool, &state.archive_roots, row.id, &plan.sources).await?;
            let derived = derive(&state, &actor, row.id, plan).await?;
            let found = check(&derived.plan, &derived.model);
            if found.digest != request.digest {
                return Err(ImportError::conflict("changed", messages::IMPORT_CHANGED)
                    .with("digest", json!(found.digest)));
            }
            if !found.blocking.is_empty() {
                return Err(ImportError::conflict("blocked", messages::IMPORT_BLOCKED)
                    .with("blocking", json!(found.blocking)));
            }
            if !found.can_run {
                return Err(ImportError::conflict(
                    "nothing",
                    messages::IMPORT_NOTHING_TO_RUN,
                ));
            }
            if found.action == Action::Delete && request.confirm != Some(found.deletes as u64) {
                return Err(ImportError::conflict(
                    "confirm",
                    messages::import_confirm_delete(found.deletes),
                )
                .with("deletes", json!(found.deletes)));
            }
            match ImportRepo::new(&state.pool).start_run(row.id).await? {
                StartOutcome::Started => {}
                StartOutcome::Missing => return Err(super::wire::not_found(row.id.0)),
                StartOutcome::Refused(status) => return Err(cannot_run(status)),
                StartOutcome::Busy(other) => return Err(busy(other)),
            }
            let job = compose_job(&state, &actor, RUN_JOB, row.id, None).await?;
            start_job(&state, job, StatusCode::OK).await
        }
        .await,
    )
}

fn cannot_run(status: ImportStatus) -> ImportError {
    ImportError::conflict("cannotRun", messages::IMPORT_CANNOT_RUN)
        .with("status", json!(status.as_str()))
}

fn busy(other: ImportId) -> ImportError {
    ImportError::conflict("busy", messages::IMPORT_BUSY).with("importId", json!(other.0))
}

/// `POST /api/romeka/imports/{id}/stop`: ask the running job to stop after
/// the record in hand. The job reads `summary.stop`.
pub(super) async fn stop(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    answer(
        async {
            admin(&state, &uri, &headers).await?;
            let row = load(&state, &id).await?;
            if row.status != ImportStatus::Running {
                return Err(
                    ImportError::conflict("notRunning", messages::IMPORT_NOT_RUNNING)
                        .with("status", json!(row.status.as_str())),
                );
            }
            let mut summary = match row.summary {
                Some(Value::Object(members)) => members,
                _ => serde_json::Map::new(),
            };
            summary.insert(STOP.to_string(), Value::Bool(true));
            ImportRepo::new(&state.pool)
                .set_summary(row.id, &Value::Object(summary))
                .await?;
            Ok(json(StatusCode::ACCEPTED, &json!({})))
        }
        .await,
    )
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct UndoBody {
    #[serde(default)]
    force: Option<Force>,
}

/// Which touched resources an undo restores or deletes anyway.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum Force {
    All(AllWord),
    Some(Vec<Forced>),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum AllWord {
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Forced {
    kind: ForcedKind,
    id: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ForcedKind {
    Item,
    Media,
    ItemSet,
    ResourceTemplate,
    Property,
}

/// `POST /api/romeka/imports/{id}/undo {force?}`: undo an import that is
/// done, or whose run or undo failed.
pub(super) async fn undo(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
    payload: crate::limits::Payload,
) -> Response {
    answer(
        async {
            let actor = admin(&state, &uri, &headers).await?;
            let request: UndoBody = if payload.0.is_empty() {
                UndoBody::default()
            } else {
                body(&headers, &payload.0)?
            };
            let row = load(&state, &id).await?;
            match ImportRepo::new(&state.pool).start_undo(row.id).await? {
                StartOutcome::Started => {}
                StartOutcome::Missing => return Err(super::wire::not_found(row.id.0)),
                StartOutcome::Refused(status) => {
                    return Err(
                        ImportError::conflict("cannotUndo", messages::IMPORT_CANNOT_UNDO)
                            .with("status", json!(status.as_str())),
                    );
                }
                StartOutcome::Busy(other) => return Err(busy(other)),
            }
            let force = request
                .force
                .map(|force| serde_json::to_value(force).unwrap_or(Value::Null));
            let job = compose_job(&state, &actor, UNDO_JOB, row.id, force).await?;
            start_job(&state, job, StatusCode::OK).await
        }
        .await,
    )
}

/// A `starting` job row for `import`. A run or undo job is linked from the
/// import, which is already running or undoing: a failure from here on
/// leaves it to die by its heartbeat, which records it failed.
async fn compose_job(
    state: &AppState,
    actor: &Actor,
    class: &str,
    import: ImportId,
    force: Option<Value>,
) -> Result<JobId, ImportError> {
    let mut args = json!({ "importId": import.0 });
    if let Some(force) = force {
        args["force"] = force;
    }
    let job = JobRepo::new(&state.pool)
        .create(&NewJob {
            owner_id: actor.user(),
            pid: None,
            status: Some("starting".to_string()),
            class: class.to_string(),
            args: Some(args),
            log: None,
            started: None,
        })
        .await?;
    if class != SCAN_JOB {
        ImportRepo::new(&state.pool).set_job(import, job.id).await?;
    }
    Ok(job.id)
}

/// Hand the job to the runner and answer `{jobId}`. A start that fails ends
/// the job `error`, so the next read records the import failed.
async fn start_job(state: &AppState, job: JobId, status: StatusCode) -> Answer {
    if let Err(error) = state.jobs.start(job) {
        let jobs = JobRepo::new(&state.pool);
        jobs.append_log(job, &error.to_string()).await?;
        jobs.finish(job, "error").await?;
        return Err(ImportError::Internal(messages::import_job_not_started(
            error,
        )));
    }
    Ok(json(status, &JobStarted { job_id: job.0 }))
}
