//! The routes that read the evidence or start a job: scan, preview, run and
//! stop (`docs/import-v2.md`, "Wire", "Lifecycle and the ledger"). Undo is
//! in [`super::undo`].
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
use omeka_api::import::plan::{ImportPlan, PlanError};
use omeka_api::import::preview::{self, Derived, PreviewRequest, derive_import};
use omeka_api::import::run::RUN_JOB;
use omeka_api::import::scan::{self, SCAN_IN_REQUEST, SCAN_JOB};
use omeka_api::import::start::{self, Go, Refusal, StartError};
use omeka_api::messages;
use omeka_db::ids::{ImportId, JobId};
use omeka_db::repos::{EntryRepo, ImportRepo, JobRepo};
use omeka_db::rows::{ImportRow, ImportStatus, NewJob};
use omeka_db::with_db;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::wire::{ImportError, admin, blocking, body, json, load, owners};
use super::{Answer, answer};
use crate::AppState;

/// The plan a non-draft runs from: the one its first run froze.
pub(super) fn stored_plan(row: &ImportRow) -> Result<ImportPlan, ImportError> {
    let stored = row.ran_plan.as_ref().unwrap_or(&row.plan);
    ImportPlan::from_json(&stored.to_string()).map_err(|error| match error {
        PlanError::V1Plan => ImportError::conflict("v1Plan", messages::IMPORT_V1_PLAN),
        other => ImportError::Internal(format!("the stored plan does not parse: {other}")),
    })
}

fn is_busy(status: ImportStatus) -> bool {
    matches!(status, ImportStatus::Running | ImportStatus::Undoing)
}

pub(super) async fn derive(
    state: &AppState,
    actor: &Actor,
    import: ImportId,
    plan: ImportPlan,
) -> Result<Derived, ImportError> {
    let files = omeka_api::settings::file_validation(&state.pool).await?;
    Ok(with_db!(&state.pool, |db| derive_import(
        db,
        &state.archive_roots,
        &state.store,
        &state.import_caches,
        import,
        Arc::new(plan),
        owners(actor),
        files,
    )
    .await)?)
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
            if with_db!(&state.pool, |db| EntryRepo::new(db).count(row.id).await)? > SCAN_IN_REQUEST
            {
                let job = compose_job(&state, &actor, SCAN_JOB, row.id, None).await?;
                return start_job(&state, job, StatusCode::ACCEPTED).await;
            }
            let scanned = with_db!(&state.pool, |db| scan::scan(
                db,
                &state.archive_roots,
                &state.store,
                row.id,
                &plan.sources,
            )
            .await)
            .map_err(|error| match error {
                // The run started after the check above: the index it reads stays.
                scan::ScanError::Db(db) if db.is_conflict() => {
                    ImportError::conflict("busy", messages::IMPORT_SCAN_BUSY)
                }
                other => other.into(),
            })?;
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
                    ImportPlan::from_json(&sent.to_string()).map_err(|error| match error {
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
/// retry a failed run or a done one with failures, once the model is the one
/// the admin checked. The start names the revision this request derived from,
/// so a plan saved since answers `changed`.
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
            if !row.may_run() {
                return Err(cannot_run(row.status));
            }
            let plan = stored_plan(&row)?;
            let files = omeka_api::settings::file_validation(&state.pool).await?;
            let assessed = with_db!(&state.pool, |db| start::assess(
                db,
                &state.archive_roots,
                &state.store,
                &state.import_caches,
                files,
                owners(&actor),
                row.id,
                plan,
            )
            .await)?;
            let go = Go {
                digest: Some(&request.digest),
                confirm: request.confirm,
                resume: false,
            };
            with_db!(&state.pool, |db| start::begin(
                db,
                &row,
                &assessed.check,
                go
            )
            .await)
            .map_err(|error| refused(error, row.id))?;
            let job = compose_job(&state, &actor, RUN_JOB, row.id, None).await?;
            start_job(&state, job, StatusCode::OK).await
        }
        .await,
    )
}

/// A refused start as the route answers it.
fn refused(error: StartError, id: ImportId) -> ImportError {
    let refusal = match error {
        StartError::Db(error) => return error.into(),
        StartError::Refused(refusal) => refusal,
    };
    match refusal {
        Refusal::DigestChanged { digest } => {
            ImportError::conflict("changed", messages::IMPORT_CHANGED).with("digest", json!(digest))
        }
        Refusal::Blocked(flags) => ImportError::conflict("blocked", messages::IMPORT_BLOCKED)
            .with("blocking", json!(flags)),
        Refusal::Nothing => ImportError::conflict("nothing", messages::IMPORT_NOTHING_TO_RUN),
        Refusal::Confirm { deletes } => {
            ImportError::conflict("confirm", messages::import_confirm_delete(deletes))
                .with("deletes", json!(deletes))
        }
        Refusal::Missing => super::wire::not_found(id.0),
        Refusal::CannotRun(status) => cannot_run(status),
        Refusal::Busy(other) => busy(other),
        Refusal::RevisionChanged { revision } => {
            ImportError::conflict("changed", messages::IMPORT_CHANGED)
                .with("revision", json!(revision))
        }
        Refusal::Deleted => {
            ImportError::Internal("a run start answered that the import deleted".to_string())
        }
    }
}

fn cannot_run(status: ImportStatus) -> ImportError {
    ImportError::conflict("cannotRun", messages::IMPORT_CANNOT_RUN)
        .with("status", json!(status.as_str()))
}

pub(super) fn busy(other: ImportId) -> ImportError {
    ImportError::conflict("busy", messages::IMPORT_BUSY).with("importId", json!(other.0))
}

/// `POST /api/romeka/imports/{id}/stop`: ask the running or undoing job to
/// stop after the resource in hand. The request is
/// `ImportRepo::request_stop`, which the job reads between resources; it
/// does not count as a heartbeat.
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
            if !is_busy(row.status) {
                return Err(not_running(row.status));
            }
            if !with_db!(&state.pool, |db| ImportRepo::new(db)
                .request_stop(row.id)
                .await)?
            {
                return Err(not_running(row.status));
            }
            Ok(json(StatusCode::ACCEPTED, &json!({})))
        }
        .await,
    )
}

fn not_running(status: ImportStatus) -> ImportError {
    ImportError::conflict("notRunning", messages::IMPORT_NOT_RUNNING)
        .with("status", json!(status.as_str()))
}

/// A `starting` job row for `import`. A run or undo job is linked from the
/// import, which is already running or undoing: a failure from here on
/// leaves it to die by its heartbeat, which records it failed.
pub(super) async fn compose_job(
    state: &AppState,
    actor: &Actor,
    class: &str,
    import: ImportId,
    force: Option<Value>,
) -> Result<JobId, ImportError> {
    Ok(import_job(&state.pool, actor.user(), class, import, force).await?)
}

/// [`compose_job`] for an owner the caller names: the route's actor, or the
/// owner of the run a resume goes on with.
pub(super) async fn import_job(
    pool: &omeka_db::DbPool,
    owner: Option<omeka_db::ids::UserId>,
    class: &str,
    import: ImportId,
    force: Option<Value>,
) -> Result<JobId, omeka_db::DbError> {
    let mut args = json!({ "importId": import.0 });
    if let Some(force) = force {
        args["force"] = force;
    }
    let job = with_db!(pool, |db| JobRepo::new(db)
        .create(&NewJob {
            owner_id: owner,
            pid: None,
            status: Some("starting".to_string()),
            class: class.to_string(),
            args: Some(args),
            log: None,
            started: None,
        })
        .await)?;
    if class != SCAN_JOB {
        with_db!(pool, |db| ImportRepo::new(db).set_job(import, job.id).await)?;
    }
    Ok(job.id)
}

/// Hand the job to the runner and answer `{jobId}`. A start that fails ends
/// the job `error`, so the next read records the import failed.
pub(super) async fn start_job(state: &AppState, job: JobId, status: StatusCode) -> Answer {
    if let Err(error) = state.jobs.start(job) {
        with_db!(&state.pool, |db| JobRepo::new(db)
            .append_log(job, &error.to_string())
            .await)?;
        with_db!(&state.pool, |db| JobRepo::new(db)
            .finish(job, "error")
            .await)?;
        return Err(ImportError::Internal(messages::import_job_not_started(
            error,
        )));
    }
    Ok(json(status, &JobStarted { job_id: job.0 }))
}
