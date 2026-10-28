//! `romeka fixity check`: run one `Romeka\Job\FixityCheck` in this process, so a cron can call it.
//!
//! The run is an ordinary job row, so the Jobs screen and the dashboard show it like a check an
//! administrator started. The row has no owner, as a job the hourly sweep composes has none.

use std::path::PathBuf;

use omeka_api::fixity::{CHECK_JOB, CheckRecord};
use omeka_api::jsonld::BaseUrl;
use omeka_db::Location;
use omeka_db::ids::JobId;
use omeka_db::repos::JobRepo;
use omeka_db::with_db;

use crate::CliError;
use crate::jobs::{JobEnv, JobStatus, Runner, compose_job};

/// Input for `romeka fixity check`.
#[derive(Debug, Clone)]
pub struct CheckOptions {
    pub database: Location,
    /// Store root. Defaults to `files` beside the database.
    pub store: Option<PathBuf>,
}

/// What the run found.
#[derive(Debug, Clone)]
pub struct CheckReport {
    pub job: JobId,
    pub status: JobStatus,
    /// Empty when the job ended before it wrote a record.
    pub record: CheckRecord,
}

impl CheckReport {
    /// Whether any file differs, is missing or is unreadable, or the check did not finish.
    #[must_use]
    pub fn has_problems(&self) -> bool {
        self.status != JobStatus::Completed
            || !self.record.differs.is_empty()
            || !self.record.missing.is_empty()
            || !self.record.unreadable.is_empty()
    }
}

/// Compose a check and run it here.
///
/// # Errors
/// A [`CliError`] when the database or the store cannot be opened, or the job row cannot be
/// written or read. A file that cannot be read is in the report, not here.
pub async fn check(options: &CheckOptions) -> Result<CheckReport, CliError> {
    let pool = options.database.open().await?;
    let store =
        crate::commands::file_store::open_store(&options.database, options.store.as_deref())?;
    let job = compose_job(&pool, CHECK_JOB, &serde_json::json!({}), None).await?;
    let env = JobEnv {
        store,
        base_url: BaseUrl::parse(crate::jobs::DEFAULT_BASE_URL)
            .map_err(|error| CliError::message(error.to_string()))?,
        archive_roots: Default::default(),
        jobs: None,
    };
    let status = Runner::with_env(&pool, env).run(job).await?;
    let row = with_db!(&pool, |db| JobRepo::new(db).get(job).await)?;
    let record = CheckRecord::from_args(row.and_then(|row| row.args).as_ref()).unwrap_or_default();
    Ok(CheckReport {
        job,
        status,
        record,
    })
}
