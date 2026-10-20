//! The `job` command group: run, list, show and stop.
//!
//! `run` is the entry point the dispatcher starts in a new process; the other
//! three mirror the admin jobs page (browse, show with the log, attempt stop).

use std::path::{Path, PathBuf};

use omeka_db::ids::JobId;
use omeka_db::repos::{JobFilter, JobRepo, Paging};
use omeka_db::rows::JobRow;

use omeka_api::import2::archive::ArchiveRoots;
use omeka_api::jsonld::BaseUrl;
use omeka_server::Config;

/// The name `--archive-root` and the v1 `ROMEKA_ARCHIVE_ROOT` give their root.
const ARCHIVE: &str = "archive";

use crate::CliError;
use crate::jobs::{JobEnv, JobStatus, Runner, refuse_live_hand_run};

/// Run one stored job in this process (`romeka job run <id>`).
///
/// # Errors
/// An invalid base URL, a database error, or the job's own failure after its
/// row is marked `error`.
pub async fn run(options: &RunOptions) -> Result<JobStatus, CliError> {
    let env = options.env()?;
    let pool = omeka_db::open_file(&options.database).await?;
    let id = JobId::from(options.id);
    refuse_live_hand_run(&pool, id).await?;
    Ok(Runner::with_env(&pool, env).run(id).await?)
}

/// Every job row, oldest first.
///
/// # Errors
/// A database error.
pub async fn list(options: &ListOptions) -> Result<Vec<JobRow>, CliError> {
    let pool = omeka_db::open_file(&options.database).await?;
    Ok(JobRepo::new(&pool)
        .list(&JobFilter::default(), &Paging::default())
        .await?)
}

/// One job row, or `None` when no row has that id.
///
/// # Errors
/// A database error.
pub async fn show(options: &ShowOptions) -> Result<Option<JobRow>, CliError> {
    let pool = omeka_db::open_file(&options.database).await?;
    Ok(JobRepo::new(&pool).get(JobId::from(options.id)).await?)
}

/// Ask a running job to stop. Returns whether the row changed, which is false
/// when the job is not `in_progress`.
///
/// # Errors
/// A database error.
pub async fn stop(options: &StopOptions) -> Result<bool, CliError> {
    let pool = omeka_db::open_file(&options.database).await?;
    Ok(JobRepo::new(&pool)
        .request_stop(JobId::from(options.id))
        .await?)
}

/// Input for `romeka job run`.
pub struct RunOptions {
    pub database: PathBuf,
    pub id: u64,
    /// `--store`; `files` beside the database when `None`.
    pub store: Option<PathBuf>,
    /// `--base-url`; `http://localhost` when `None`.
    pub base_url: Option<String>,
    /// `--archive-root`, the root named `archive`. The named roots come from
    /// `ROMEKA_ARCHIVE_ROOTS` and the config file, as `romeka serve` reads
    /// them, so a job the server starts sees the server's roots.
    pub archive_root: Option<PathBuf>,
}

impl RunOptions {
    fn env(&self) -> Result<JobEnv, CliError> {
        job_env(
            &self.database,
            self.store.as_deref(),
            self.base_url.as_deref(),
            self.archive_root.as_deref(),
        )
    }
}

/// The surroundings a job reads, as `romeka job run` and the import commands
/// build them from their flags.
///
/// # Errors
/// An invalid base URL, an unreadable config, or archive roots that do not
/// make a valid list.
pub(crate) fn job_env(
    database: &Path,
    store: Option<&Path>,
    base_url: Option<&str>,
    archive_root: Option<&Path>,
) -> Result<JobEnv, CliError> {
    let defaults = JobEnv::default();
    let base_url = match base_url {
        Some(raw) => BaseUrl::parse(raw)
            .map_err(|error| CliError::message(format!("invalid base url {raw}: {error}")))?,
        None => defaults.base_url,
    };
    Ok(JobEnv {
        store: crate::commands::file_store::open_store(database, store)?,
        base_url,
        archive_roots: archive_roots(archive_root)?,
    })
}

fn archive_roots(archive_root: Option<&Path>) -> Result<ArchiveRoots, CliError> {
    let config = Config::load().map_err(|error| CliError::message(error.to_string()))?;
    let Some(root) = archive_root else {
        return Ok(config.archive_roots);
    };
    let root = std::path::absolute(root).map_err(|error| {
        CliError::message(format!("cannot resolve {}: {error}", root.display()))
    })?;
    let named = config
        .archive_roots
        .iter()
        .filter(|(name, _)| *name != ARCHIVE)
        .map(|(name, path)| (name.to_string(), path.to_path_buf()))
        .chain(std::iter::once((ARCHIVE.to_string(), root)))
        .collect::<Vec<_>>();
    ArchiveRoots::new(named).map_err(|error| CliError::message(error.to_string()))
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
