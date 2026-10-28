//! The seam the server starts a job through.
//!
//! The server composes the `job` row; something outside it does the work.
//! `romeka serve` supplies a [`JobDispatch`] that starts `romeka job run` in a
//! new process, and a test supplies one that runs the job in-process. The
//! server never depends on the crate that holds the jobs.

use omeka_api::dead_jobs::{self, Unstarted};
use omeka_db::DbPool;
use omeka_db::ids::JobId;
use omeka_db::ids::UserId;
use omeka_db::repos::JobRepo;
use omeka_db::with_db;

/// Why a job could not be started.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct DispatchError(pub String);

/// Starts the work of a composed `job` row.
pub trait JobDispatch: Send + Sync {
    /// Start job `job`, whose row is in `starting`. Returns once the work has
    /// started, not when it ends; the worker writes the row's ending.
    ///
    /// # Errors
    /// A [`DispatchError`] when the work cannot be started.
    fn start(&self, job: JobId) -> Result<(), DispatchError>;
}

/// The dispatcher of a router built without a job runner: every start fails.
pub struct NoDispatch;

impl JobDispatch for NoDispatch {
    fn start(&self, job: JobId) -> Result<(), DispatchError> {
        Err(DispatchError(format!(
            "job {} cannot start: this server has no job runner",
            job.0
        )))
    }
}

/// How long before this server started a job with no process must have been composed for the
/// server to end it: a `romeka` command that composes a job runs it within moments.
const UNSTARTED_MARGIN: chrono::TimeDelta = chrono::TimeDelta::minutes(1);

/// End the jobs a stopped server left behind (`omeka_api::dead_jobs`): each row whose process is
/// gone, and each row that waited in that server's queue. Call it before this server composes a
/// job, so none of its own rows is judged.
pub async fn settle_dead_jobs(pool: &DbPool) {
    let cutoff = chrono::Utc::now().naive_utc() - UNSTARTED_MARGIN;
    match dead_jobs::settle(pool, None, Unstarted::ComposedBefore(cutoff)).await {
        Ok(settled) if !settled.is_empty() => {
            tracing::info!(jobs = ?settled, "settled jobs whose process is gone");
        }
        Ok(_) => {}
        Err(error) => tracing::warn!("cannot settle dead jobs: {error}"),
    }
}

/// Start one derivative worker when a create queued work and no worker holds the queue.
///
/// The media is already written, so a worker that cannot start is not the request's failure: the
/// job row ends `error` with the reason and its rows wait, pending, for the next create or for
/// `romeka media derivatives`. `omeka_api::derivatives::queue_worker` composes the row only when
/// something is queued and no worker is starting, running or stopping, so this is also the cap of
/// one worker per install.
pub async fn start_derivatives(pool: &DbPool, jobs: &dyn JobDispatch, owner: Option<UserId>) {
    let queued = match omeka_api::derivatives::queue_worker(pool, owner).await {
        Ok(queued) => queued,
        Err(error) => {
            tracing::warn!("cannot read the derivative queue: {error}");
            return;
        }
    };
    let Some(job) = queued else {
        return;
    };
    if let Err(error) = jobs.start(job) {
        let _ = with_db!(pool, |db| JobRepo::new(db)
            .append_log(job, &error.to_string())
            .await);
        let _ = with_db!(pool, |db| JobRepo::new(db).finish(job, "error").await);
        tracing::warn!("derivative worker {} did not start: {error}", job.0);
    }
}
