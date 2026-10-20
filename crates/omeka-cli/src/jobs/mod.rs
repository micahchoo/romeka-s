//! The job runner: a `Job` trait, a registry of the upstream jobs, a
//! background dispatcher and the worker that runs one row.
//!
//! Upstream `Dispatcher` composes a `job` row and sends it to a
//! `DispatchStrategy`. The port keeps the two upstream strategies: the
//! dispatcher starts `romeka job run <id>` in a new process (the
//! `DispatchStrategy\PhpCli` behaviour), and that process runs the job
//! synchronously (`DispatchStrategy\Synchronous`, `perform-job.php`). So a long
//! job does not hold the caller, and the row moves through `starting`,
//! `in_progress`, and `completed`/`stopped`/`error`.
//!
//! Each job reads its arguments from the `job.args` JSON, the same shape
//! `BatchDelete`, `BatchUpdate`, `IndexFulltextSearch` and `UpdateSiteItems`
//! read with `AbstractJob::getArg`.

mod batch_delete;
mod batch_update;
mod dispatcher;
mod env;
mod import_run;
mod index_fulltext_search;
mod query;
mod runner;
mod status;
mod sweep;
mod unzip;
mod update_site_items;

pub use batch_delete::BatchDelete;
pub use batch_update::BatchUpdate;
pub use dispatcher::{DATABASE_URL_ENV, Dispatcher, JobCommand, JobContext, compose_job};
pub use env::JobEnv;
pub use import_run::{ImportRun, ImportScan, ImportUndo, refuse_live_hand_run};
pub use index_fulltext_search::IndexFulltextSearch;
pub use query::BatchQuery;
pub use runner::Runner;
pub use status::JobStatus;
pub use sweep::ImportSweep;
pub use unzip::ImportUnzip;
pub use update_site_items::UpdateSiteItems;

use async_trait::async_trait;

/// Everything a job can fail with.
#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error(transparent)]
    Db(#[from] omeka_db::DbError),
    /// Upstream `Omeka\Job\Exception\InvalidArgumentException`.
    #[error("{0}")]
    InvalidArgument(String),
    /// Upstream `Omeka\Job\Exception\RuntimeException`.
    #[error("{0}")]
    Runtime(String),
}

/// A read-layer failure inside a job is a runtime failure of the job.
impl From<omeka_api::errors::ApiError> for JobError {
    fn from(error: omeka_api::errors::ApiError) -> Self {
        Self::Runtime(error.to_string())
    }
}

/// One enterable job, the port's `Omeka\Job\JobInterface` plus the class name
/// stored in `job.class`.
#[async_trait]
pub trait Job: Send + Sync {
    /// The upstream class name, for example `Omeka\Job\BatchDelete`.
    fn class(&self) -> &'static str;

    /// Do the work. The runner owns the status transitions around this call.
    async fn perform(&self, context: &JobContext<'_>) -> Result<(), JobError>;
}

static BATCH_DELETE: BatchDelete = BatchDelete;
static BATCH_UPDATE: BatchUpdate = BatchUpdate;
static INDEX_FULLTEXT_SEARCH: IndexFulltextSearch = IndexFulltextSearch;
static IMPORT_RUN: ImportRun = ImportRun;
static IMPORT_SCAN: ImportScan = ImportScan;
static IMPORT_UNDO: ImportUndo = ImportUndo;
static UPDATE_SITE_ITEMS: UpdateSiteItems = UpdateSiteItems;
static IMPORT_UNZIP: ImportUnzip = ImportUnzip;
static IMPORT_SWEEP: ImportSweep = ImportSweep;

/// The job for a `job.class` value, the port's registry of the four upstream
/// jobs and Romeka's import jobs. `Dispatcher::dispatch` resolves the class the same way through the
/// service manager. `None` for a class this build does not register.
#[must_use]
pub fn job_for(class: &str) -> Option<&'static dyn Job> {
    match class {
        "Omeka\\Job\\BatchDelete" => Some(&BATCH_DELETE),
        "Omeka\\Job\\BatchUpdate" => Some(&BATCH_UPDATE),
        "Omeka\\Job\\IndexFulltextSearch" => Some(&INDEX_FULLTEXT_SEARCH),
        "Omeka\\Job\\UpdateSiteItems" => Some(&UPDATE_SITE_ITEMS),
        omeka_api::import::run::RUN_JOB => Some(&IMPORT_RUN),
        omeka_api::import::scan::SCAN_JOB => Some(&IMPORT_SCAN),
        omeka_api::import::undo::UNDO_JOB => Some(&IMPORT_UNDO),
        omeka_api::import::unzip::UNZIP_JOB => Some(&IMPORT_UNZIP),
        omeka_api::import::sweep::SWEEP_JOB => Some(&IMPORT_SWEEP),
        _ => None,
    }
}
