//! `Romeka\Job\ImportRun`, `Romeka\Job\ImportUndo` and
//! `Romeka\Job\ImportScan`: the import v2 jobs (`docs/import-v2.md`,
//! "Lifecycle and the ledger").
//!
//! Each reads `importId` from its args. The run's body is
//! `omeka_api::import2::run::run_import`, the undo's
//! `omeka_api::import2::undo::undo_import` with the API's delete paths, which
//! remove a media's files from the store. This module supplies what the run reads
//! outside the database: the store, the archive roots, the install's file
//! settings and URL guard, who pressed Run (the job's owner), the site
//! sync of a created item, and the API's delete paths for a `delete` run. [`refuse_live_hand_run`] keeps a second worker
//! off an import whose worker is still alive.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use omeka_api::import2::catalog_load::Owners;
use omeka_api::import2::preview::Caches;
use omeka_api::import2::run::{RUN_JOB, RunEnd, RunEnv, Runner, SiteSync, run_import};
use omeka_api::import2::scan::{SCAN_JOB, scan_import};
use omeka_api::import2::undo::{ApiRemoval, Force, UNDO_JOB, UndoEnd, undo_import};
use omeka_api::read::Api;
use omeka_db::ids::{ImportId, ItemId, JobId};
use omeka_db::repos::{ImportRepo, SettingRepo, UserRepo};
use omeka_db::rows::{ImportEnding, ImportStatus};
use omeka_media::HttpClient;
use sqlx::SqlitePool;

use super::{Job, JobContext, JobError};
use crate::system_api;

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
    let max_file = omeka_api::settings::upload_settings(&settings)
        .await?
        .max_file;
    let sites = Sites {
        api: system_api::load(pool).await?,
    };
    let api = system_api::load_with_store(pool, Arc::clone(&context.env.store)).await?;
    let removal = ApiRemoval { api: &api };
    let caches = Arc::new(Caches::default());
    let env = RunEnv {
        store: Arc::clone(&context.env.store),
        roots: &context.env.archive_roots,
        caches: &caches,
        validation,
        http: HttpClient::new()
            .with_address_policy(policy)
            .with_max_bytes(usize::try_from(max_file).unwrap_or(usize::MAX)),
        sites: &sites,
        removal: &removal,
    };
    run_import(pool, &env, import, context.job_id, runner)
        .await
        .map_err(|error| JobError::Runtime(error.to_string()))
}

/// `Romeka\Job\ImportUndo`; its args may carry `force`.
pub struct ImportUndo;

#[async_trait]
impl Job for ImportUndo {
    fn class(&self) -> &'static str {
        UNDO_JOB
    }

    async fn perform(&self, context: &JobContext<'_>) -> Result<(), JobError> {
        let import = import_id(context)?;
        let force = Force::from_arg(context.arg("force")).map_err(JobError::InvalidArgument)?;
        let outcome = undo(context, import, force).await;
        if outcome.is_err() {
            // As for a run: a worker that knows it failed frees the slot now.
            let _ = ImportRepo::new(context.pool)
                .finish(import, ImportEnding::UndoFailed)
                .await;
        }
        let line = match outcome? {
            UndoEnd::Undone(report) => format!(
                "Import {} undone: {} removed, {} restored, {} already gone.",
                import.0,
                total(&report.removed),
                total(&report.restored),
                report.skipped.len()
            ),
            UndoEnd::PartlyUndone(report) => format!(
                "Import {} undone but for {} kept resources: {} removed, {} restored.",
                import.0,
                report.kept.len(),
                total(&report.removed),
                total(&report.restored)
            ),
            UndoEnd::Failed(report) => match &report.error {
                Some(error) => {
                    format!("Import {} was not fully undone: {}", import.0, error.detail)
                }
                None => format!("Import {} stopped undoing on request.", import.0),
            },
            UndoEnd::Lost => format!(
                "Import {} is no longer undone by this job; it stopped writing.",
                import.0
            ),
        };
        context.log(&line).await
    }
}

fn total(counts: &omeka_api::import2::undo::Counts) -> usize {
    counts.items + counts.media + counts.item_sets + counts.templates + counts.properties
}

async fn undo(
    context: &JobContext<'_>,
    import: ImportId,
    force: Force,
) -> Result<UndoEnd, JobError> {
    let api = system_api::load_with_store(context.pool, Arc::clone(&context.env.store)).await?;
    let removal = ApiRemoval { api: &api };
    undo_import(context.pool, &removal, import, context.job_id, force)
        .await
        .map_err(|error| JobError::Runtime(error.to_string()))
}

/// The job's owner pressed Run: they are the plan's `runner`, and a site
/// admin may name only themself as an owner, as the route derived.
async fn runner(pool: &SqlitePool, job: JobId) -> Result<Runner, JobError> {
    let owner = omeka_db::repos::JobRepo::new(pool)
        .get(job)
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
        owners: Owners::of(role.as_deref(), Some(user)),
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
        let scanned = scan_import(
            context.pool,
            &context.env.archive_roots,
            &context.env.store,
            import,
        )
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
    let Some(row) = omeka_db::repos::JobRepo::new(pool).get(job).await? else {
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
