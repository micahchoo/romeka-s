//! The `job` command group: run, list, show and stop.
//!
//! `run` is the entry point the dispatcher starts in a new process; the other
//! three mirror the admin jobs page (browse, show with the log, attempt stop).

use std::path::PathBuf;

use omeka_db::ids::JobId;
use omeka_db::rows::JobRow;

use crate::CliError;
use crate::jobs::{JobStatus, Runner};
use crate::store;

/// Run one stored job in this process (`romeka job run <id>`).
///
/// # Errors
/// A database error, or the job's own failure after its row is marked `error`.
pub async fn run(options: &RunOptions) -> Result<JobStatus, CliError> {
    let pool = omeka_db::open_file(&options.database).await?;
    Ok(Runner::new(&pool).run(JobId::from(options.id)).await?)
}

/// Every job row, oldest first.
///
/// # Errors
/// A database error.
pub async fn list(options: &ListOptions) -> Result<Vec<JobRow>, CliError> {
    let pool = omeka_db::open_file(&options.database).await?;
    Ok(store::job_list(&pool).await?)
}

/// One job row, or `None` when no row has that id.
///
/// # Errors
/// A database error.
pub async fn show(options: &ShowOptions) -> Result<Option<JobRow>, CliError> {
    let pool = omeka_db::open_file(&options.database).await?;
    Ok(store::job_get(&pool, JobId::from(options.id)).await?)
}

/// Ask a running job to stop. Returns whether the row changed, which is false
/// when the job is not `in_progress`.
///
/// # Errors
/// A database error.
pub async fn stop(options: &StopOptions) -> Result<bool, CliError> {
    let pool = omeka_db::open_file(&options.database).await?;
    Ok(store::job_request_stop(&pool, JobId::from(options.id)).await?)
}

/// Input for `romeka job run`.
pub struct RunOptions {
    pub database: PathBuf,
    pub id: u64,
}

/// Input for `romeka job list`.
pub struct ListOptions {
    pub database: PathBuf,
}

/// Input for `romeka job show`.
pub struct ShowOptions {
    pub database: PathBuf,
    pub id: u64,
}

/// Input for `romeka job stop`.
pub struct StopOptions {
    pub database: PathBuf,
    pub id: u64,
}
