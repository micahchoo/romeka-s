//! Run, stop and undo: the requests that move a run between states. A start answers the job it
//! composed; the server hands that job to its runner.

use chrono::{TimeDelta, Utc};
use omeka_db::ids::JobId;
use omeka_db::repos::{BulkRepo, JobRepo};
use omeka_db::rows::{BulkPhase, BulkRow, BulkStatus, NewJob};
use omeka_db::with_db;
use serde_json::json;

use super::access::{denied, holds, load};
use super::change::BulkChange;
use super::preview::check;
use super::scope::Scope;
use super::{BulkError, RUN_JOB, UNDO_JOB};
use crate::errors::ApiError;
use crate::messages;
use crate::read::Api;

/// How long a preview stays good: past it, the selection and the counts it showed are stale.
const PREVIEW_LASTS: TimeDelta = TimeDelta::minutes(10);

/// `POST /api/romeka/bulk/{id}/run`: start the run its maker previewed, or go on with one that
/// stopped or failed. The rights and the change are checked again, as they may have changed.
///
/// # Errors
/// 403 for another person's run or a right lost since; 409 `cannotRun` for a run in another
/// state, `expired` for a preview older than ten minutes.
pub async fn run(api: &Api<'_>, raw: &str) -> Result<JobId, BulkError> {
    let row = load(api, raw, true).await?;
    if !row.status.may_run() {
        return Err(cannot_run(row.status));
    }
    if row.status == BulkStatus::Previewed && Utc::now().naive_utc() - row.created > PREVIEW_LASTS {
        return Err(BulkError::conflict(
            "expired",
            messages::BULK_EXPIRED,
            row.status,
        ));
    }
    let scope = Scope::of(&row.query)?;
    if !holds(api.actor(), scope.right()) {
        return Err(denied(scope.right()));
    }
    let change = BulkChange::parse(&row.change).map_err(ApiError::internal)?;
    check(api, &change).await?;
    if !with_db!(api.pool(), |db| BulkRepo::new(db)
        .start(row.id, BulkPhase::Run)
        .await)?
    {
        return Err(cannot_run(row.status));
    }
    compose(api, &row, RUN_JOB, json!({ "bulkId": row.id.0 })).await
}

/// `POST /api/romeka/bulk/{id}/undo {force?}`: restore every item the run changed. `force`
/// restores an item edited since the run wrote it too.
///
/// # Errors
/// 403 for another person's run; 409 `cannotUndo` while it runs or once it is undone.
pub async fn undo(api: &Api<'_>, raw: &str, force: bool) -> Result<JobId, BulkError> {
    let row = load(api, raw, false).await?;
    let cannot = || BulkError::conflict("cannotUndo", messages::BULK_CANNOT_UNDO, row.status);
    if !row.status.may_undo() {
        return Err(cannot());
    }
    if !with_db!(api.pool(), |db| BulkRepo::new(db)
        .start(row.id, BulkPhase::Undo)
        .await)?
    {
        return Err(cannot());
    }
    let args = json!({ "bulkId": row.id.0, "force": force });
    compose(api, &row, UNDO_JOB, args).await
}

/// `POST /api/romeka/bulk/{id}/stop`: ask the job to stop after the chunk in hand.
///
/// # Errors
/// 403 for another person's run; 409 `notRunning` when no job works on it.
pub async fn stop(api: &Api<'_>, raw: &str) -> Result<(), BulkError> {
    let row = load(api, raw, false).await?;
    if with_db!(api.pool(), |db| BulkRepo::new(db)
        .request_stop(row.id)
        .await)?
    {
        return Ok(());
    }
    Err(BulkError::conflict(
        "notRunning",
        messages::BULK_NOT_RUNNING,
        row.status,
    ))
}

/// A `starting` job for the run, linked from it. The run is already working: if the job never
/// starts, its heartbeat goes stale and the server settles it.
async fn compose(
    api: &Api<'_>,
    row: &BulkRow,
    class: &str,
    args: serde_json::Value,
) -> Result<JobId, BulkError> {
    let job = with_db!(api.pool(), |db| JobRepo::new(db)
        .create(&NewJob {
            owner_id: api.actor().user(),
            pid: None,
            status: Some("starting".to_string()),
            class: class.to_string(),
            args: Some(args),
            log: None,
            started: None,
        })
        .await)?;
    with_db!(api.pool(), |db| BulkRepo::new(db)
        .set_job(row.id, job.id)
        .await)?;
    Ok(job.id)
}

fn cannot_run(status: BulkStatus) -> BulkError {
    BulkError::conflict("cannotRun", messages::BULK_CANNOT_RUN, status)
}
