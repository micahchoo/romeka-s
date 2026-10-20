//! The seam the server starts a job through.
//!
//! The server composes the `job` row; something outside it does the work.
//! `romeka serve` supplies a [`JobDispatch`] that starts `romeka job run` in a
//! new process, and a test supplies one that runs the job in-process. The
//! server never depends on the crate that holds the jobs.

use omeka_db::ids::JobId;

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
