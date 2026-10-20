//! The `job` command group: run, list, show and stop.
//!
//! `run` is the entry point the dispatcher starts in a new process; the other
//! three mirror the admin jobs page (browse, show with the log, attempt stop).

use std::path::PathBuf;

use omeka_db::ids::JobId;
use omeka_db::rows::JobRow;

use omeka_api::jsonld::BaseUrl;

use crate::CliError;
use crate::jobs::{JobEnv, JobStatus, Runner};
use crate::store;

/// Run one stored job in this process (`romeka job run <id>`).
///
/// # Errors
/// An invalid base URL, a database error, or the job's own failure after its
/// row is marked `error`.
pub async fn run(options: &RunOptions) -> Result<JobStatus, CliError> {
    let env = options.env()?;
    let pool = omeka_db::open_file(&options.database).await?;
    Ok(Runner::with_env(&pool, env)
        .run(JobId::from(options.id))
        .await?)
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
    /// `--store`; `files` beside the database when `None`.
    pub store: Option<PathBuf>,
    /// `--base-url`; `http://localhost` when `None`.
    pub base_url: Option<String>,
    /// `--archive-root`; the archive import is off when `None`.
    pub archive_root: Option<PathBuf>,
}

impl RunOptions {
    fn env(&self) -> Result<JobEnv, CliError> {
        let defaults = JobEnv::default();
        let base_url = match &self.base_url {
            Some(raw) => BaseUrl::parse(raw)
                .map_err(|error| CliError::message(format!("invalid base url {raw}: {error}")))?,
            None => defaults.base_url,
        };
        Ok(JobEnv {
            store: crate::commands::media::open_store(&self.database, self.store.as_deref()),
            base_url,
            archive_root: self.archive_root.clone(),
        })
    }
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
