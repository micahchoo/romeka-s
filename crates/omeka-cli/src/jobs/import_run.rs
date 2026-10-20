//! `Romeka\Job\ImportRun` and `Romeka\Job\ImportScan`: the import v2 jobs
//! (`docs/import-v2.md`, "Lifecycle and the ledger").
//!
//! Both read `importId` from their args. The run's body is
//! `omeka_api::import2::run::run_import`; this module supplies what it reads
//! outside the database: the store, the archive roots, the install's file
//! settings and URL guard, who pressed Run (the job's owner), and the site
//! sync of a created item. [`refuse_live_hand_run`] keeps a second worker
//! off an import whose worker is still alive.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use omeka_api::import2::catalog_load::Owners;
use omeka_api::import2::preview::Caches;
use omeka_api::import2::run::{RUN_JOB, RunEnd, RunEnv, Runner, SiteSync, run_import};
use omeka_api::import2::scan::{SCAN_JOB, scan_import};
use omeka_api::read::Api;
use omeka_db::ids::{ImportId, ItemId, JobId};
use omeka_db::repos::{ImportRepo, SettingRepo, UserRepo};
use omeka_db::rows::{ImportEnding, ImportStatus};
use omeka_media::HttpClient;
use sqlx::SqlitePool;

use super::{Job, JobContext, JobError};
use crate::{store, system_api};

/// The job class of an undo, which `romeka job run` guards like a run.
pub const UNDO_JOB: &str = "Romeka\\Job\\ImportUndo";

fn import_id(context: &JobContext<'_>) -> Result<ImportId, JobError> {
    context
        .arg("importId")
        .and_then(serde_json::Value::as_u64)
        .map(ImportId)
        .ok_or_else(|| JobError::InvalidArgument("missing number argument \"importId\"".into()))
}

/// `Romeka\Job\ImportRun`.
pub struct ImportRun;

#[async_trait]
impl Job for ImportRun {
    fn class(&self) -> &'static str {
        RUN_JOB
    }

    async fn perform(&self, context: &JobContext<'_>) -> Result<(), JobError> {
        let import = import_id(context)?;
        let outcome = run(context, import).await;
        if outcome.is_err() {
            // The worker is alive and knows it failed; waiting for a reader
            // to find it dead would hold the install's one run slot.
            let _ = ImportRepo::new(context.pool)
                .finish(import, ImportEnding::RunFailed)
                .await;
        }
        let end = outcome?;
        let line = match &end {
            RunEnd::Done(summary) => format!(
                "Import {} done: {} of {} records, {} failed.",
                import.0,
                summary.done,
                summary.total,
                summary.failures.len()
            ),
            RunEnd::Failed(summary) if summary.stopped => {
                format!("Import {} stopped on request.", import.0)
            }
            RunEnd::Failed(summary) => format!(
                "Import {} wrote nothing: {} records failed.",
                import.0,
                summary.failures.len()
            ),
            RunEnd::Lost => format!(
                "Import {} is no longer run by this job; it stopped writing.",
                import.0
            ),
        };
        context.log(&line).await
    }
}

async fn run(context: &JobContext<'_>, import: ImportId) -> Result<RunEnd, JobError> {
    let pool = context.pool;
    let runner = runner(pool, context.job_id).await?;
    let settings = SettingRepo::new(pool);
    let validation = omeka_api::settings::file_validation(&settings).await?;
    let policy = omeka_api::settings::address_policy(&settings).await?;
    let sites = Sites {
        api: system_api::load(pool).await?,
    };
    let caches = Arc::new(Caches::default());
    let env = RunEnv {
        store: Arc::clone(&context.env.store),
        roots: &context.env.archive_roots,
        caches: &caches,
        validation,
        http: HttpClient::new().with_address_policy(policy),
        sites: &sites,
    };
    run_import(pool, &env, import, context.job_id, runner)
        .await
        .map_err(|error| JobError::Runtime(error.to_string()))
}

/// The job's owner pressed Run: they are the plan's `runner`, and a site
/// admin may name only themself as an owner, as the route derived.
async fn runner(pool: &SqlitePool, job: JobId) -> Result<Runner, JobError> {
    let owner = store::job_get(pool, job)
        .await?
        .and_then(|row| row.owner_id);
    let Some(user) = owner else {
        return Ok(Runner {
            user: None,
            owners: Owners::Anyone,
        });
    };
    let role = UserRepo::new(pool).get(user).await?.map(|row| row.role);
    Ok(Runner {
        user: Some(user),
        owners: if role.as_deref() == Some("global_admin") {
            Owners::Anyone
        } else {
            Owners::Only(user)
        },
    })
}

/// A created item joins the sites whose item pool takes it.
struct Sites<'p> {
    api: Api<'p>,
}

impl SiteSync for Sites<'_> {
    fn item_created<'a>(
        &'a self,
        item: ItemId,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>> {
        Box::pin(async move {
            omeka_api::site_items::attach_on_create(&self.api, item)
                .await
                .map_err(|error| error.to_string())
        })
    }
}

/// `Romeka\Job\ImportScan`: a rescan too large for the request.
pub struct ImportScan;

#[async_trait]
impl Job for ImportScan {
    fn class(&self) -> &'static str {
        SCAN_JOB
    }

    async fn perform(&self, context: &JobContext<'_>) -> Result<(), JobError> {
        let import = import_id(context)?;
        let scanned = scan_import(context.pool, &context.env.archive_roots, import)
            .await
            .map_err(|error| JobError::Runtime(error.to_string()))?;
        let files: usize = scanned.sources.iter().map(|source| source.files).sum();
        context
            .log(&format!("Import {} scanned: {files} files.", import.0))
            .await
    }
}

/// Refuse to run a run or undo job by hand while its import's worker is
/// alive: the job row is past `starting`, so a worker took it, and the
/// import's heartbeat is younger than `DEAD_AFTER` (2 minutes). A dispatched job is
/// still `starting` when its process begins, so it passes.
///
/// # Errors
/// [`JobError::Runtime`] naming the live import; a database error.
pub async fn refuse_live_hand_run(pool: &SqlitePool, job: JobId) -> Result<(), JobError> {
    let Some(row) = store::job_get(pool, job).await? else {
        return Ok(());
    };
    if row.class != RUN_JOB && row.class != UNDO_JOB {
        return Ok(());
    }
    if !matches!(row.status.as_deref(), Some("in_progress" | "stopping")) {
        return Ok(());
    }
    let Some(import) = row
        .args
        .as_ref()
        .and_then(|args| args.get("importId"))
        .and_then(serde_json::Value::as_u64)
        .map(ImportId)
    else {
        return Ok(());
    };
    let Some(held) = ImportRepo::new(pool).get(import).await? else {
        return Ok(());
    };
    let working = matches!(held.status, ImportStatus::Running | ImportStatus::Undoing)
        && held.job_id == Some(job);
    if working && held.heartbeat_is_live() {
        return Err(JobError::Runtime(format!(
            "import {} is {} and its worker wrote a heartbeat less than 2 minutes ago; \
             wait for it to end or to stop beating",
            import.0,
            held.status.as_str()
        )));
    }
    Ok(())
}
