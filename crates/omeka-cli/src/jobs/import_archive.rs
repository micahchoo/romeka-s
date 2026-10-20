//! The archive import's jobs: `Romeka\Job\ImportArchive` runs an import and
//! `Romeka\Job\UndoImport` undoes one (`docs/archive-import.md`, "Jobs").
//!
//! The server moves the import to `running` or `undoing`, links this job, and
//! starts it. The job writes the ending: `done` or `failed` after a run (the
//! undo window stays open either way), `undone` after an undo, which also
//! closes the window. An undo that fails leaves `undoing`; once this job row
//! is `error`, the server records the import failed and allows the undo
//! again, which converges because undo works from what is still recorded.
//!
//! While it works, a job writes its heartbeat from a separate task (see
//! [`beating`]), so a process that dies stops beating and the server records
//! the import failed after `omeka_db::repos::DEAD_AFTER`.

use async_trait::async_trait;
use omeka_api::import::archive::ArchiveRoot;
use omeka_api::import::catalog;
use omeka_api::import::plan::ImportPlan;
use omeka_api::import::run::{self, ImportSummary, Progress, RunInput};
use omeka_api::import::undo;
use omeka_db::DbError;
use omeka_db::ids::{ImportId, JobId};
use omeka_db::repos::{HEARTBEAT, HEARTBEAT_EVERY, ImportRepo, ImportResourceRepo};
use omeka_db::rows::{ImportRow, ImportStatus};
use serde_json::Value;
use sqlx::SqlitePool;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

use super::{Job, JobContext, JobError};
use crate::store;

/// `Romeka\Job\ImportArchive`, args `{"importId": n}`.
pub struct ImportArchive;

/// `Romeka\Job\UndoImport`, args `{"importId": n}`.
pub struct UndoImport;

#[async_trait]
impl Job for ImportArchive {
    fn class(&self) -> &'static str {
        omeka_server::IMPORT_JOB
    }

    async fn perform(&self, context: &JobContext<'_>) -> Result<(), JobError> {
        let row = claimed(context, ImportStatus::Running).await?;
        let id = row.id;
        let repo = ImportRepo::new(context.pool);
        match beating(context, id, run_import(context, row)).await {
            Ok(summary) => {
                context
                    .log(&format!(
                        "imported {} items and {} files; {} rows left out, {} failed",
                        summary.items, summary.files, summary.left, summary.failed
                    ))
                    .await?;
                repo.finish(id, ImportStatus::Done).await?;
                Ok(())
            }
            Err(error) => {
                repo.finish(id, ImportStatus::Failed).await?;
                Err(error)
            }
        }
    }
}

#[async_trait]
impl Job for UndoImport {
    fn class(&self) -> &'static str {
        omeka_server::UNDO_JOB
    }

    async fn perform(&self, context: &JobContext<'_>) -> Result<(), JobError> {
        let row = claimed(context, ImportStatus::Undoing).await?;
        let work = async {
            let (progress, writer) = log_sink(context);
            let outcome = undo::undo(context.pool, &context.env.store, row.id, progress).await;
            finish_log(writer).await?;
            outcome.map_err(|error| JobError::Runtime(error.to_string()))
        };
        let report = beating(context, row.id, work).await?;
        context
            .log(&format!(
                "undone: {} items, {} media, {} item sets, {} item types, {} fields; {} kept in use",
                report.items,
                report.media,
                report.item_sets,
                report.templates,
                report.fields,
                report.kept.len()
            ))
            .await?;
        let repo = ImportRepo::new(context.pool);
        repo.finish(row.id, ImportStatus::Undone).await?;
        repo.close_undo_window(row.id).await?;
        Ok(())
    }
}

/// The import this job was started for, refused unless the server moved it
/// to `status` and linked this job.
async fn claimed(context: &JobContext<'_>, status: ImportStatus) -> Result<ImportRow, JobError> {
    let id = context
        .arg("importId")
        .and_then(Value::as_u64)
        .map(ImportId)
        .ok_or_else(|| {
            JobError::InvalidArgument("missing integer argument \"importId\"".to_string())
        })?;
    let row = ImportRepo::new(context.pool)
        .get(id)
        .await?
        .ok_or_else(|| JobError::InvalidArgument(format!("no import with id {}", id.0)))?;
    if row.status != status || row.job_id != Some(context.job_id) {
        return Err(JobError::InvalidArgument(format!(
            "import {} is {} and belongs to another job",
            id.0,
            row.status.as_str()
        )));
    }
    Ok(row)
}

async fn run_import(context: &JobContext<'_>, row: ImportRow) -> Result<ImportSummary, JobError> {
    // A run that already wrote is never repeated: it is undone instead.
    if has_written(context.pool, &row).await? {
        return Err(JobError::InvalidArgument(format!(
            "import {} has already written; undo it instead of running it again",
            row.id.0
        )));
    }
    let root = context.env.archive_root.as_ref().ok_or_else(|| {
        JobError::Runtime("the archive import is off: no archive root".to_string())
    })?;
    let folder = ArchiveRoot::new(root)
        .and_then(|root| root.folder(&row.folder))
        .map_err(|error| JobError::Runtime(error.to_string()))?;
    let plan = ImportPlan::from_json(&row.plan)
        .map_err(|error| JobError::InvalidArgument(error.to_string()))?;
    let input = RunInput {
        import_id: row.id,
        owner: row.owner_id,
        folder,
        plan,
        catalog: catalog::load(context.pool).await?,
        base_url: context.env.base_url.clone(),
    };
    let (progress, writer) = log_sink(context);
    let outcome = run::run(context.pool, &context.env.store, input, progress).await;
    finish_log(writer).await?;
    outcome.map_err(|error| JobError::Runtime(error.to_string()))
}

/// Whether a run of `row` has written anything: a summary beyond the start's
/// heartbeat, or a recorded target.
async fn has_written(pool: &SqlitePool, row: &ImportRow) -> Result<bool, JobError> {
    let reported = row
        .summary
        .as_ref()
        .and_then(Value::as_object)
        .is_some_and(|members| members.keys().any(|key| key != HEARTBEAT));
    Ok(reported
        || !ImportResourceRepo::new(pool)
            .list(row.id, None)
            .await?
            .is_empty())
}

/// Run `work` while a spawned task writes the import's heartbeat every
/// [`HEARTBEAT_EVERY`]. The task runs beside `work`, so one long step (a big
/// file stored, a TIFF thumbnailed) still beats. A heartbeat that finds the
/// import no longer running or undoing under this job means the server found
/// the job dead: the work stops there and is dropped, which rolls back the
/// row it was writing.
async fn beating<T>(
    context: &JobContext<'_>,
    id: ImportId,
    work: impl Future<Output = Result<T, JobError>>,
) -> Result<T, JobError> {
    let (lost, claim_lost) = oneshot::channel::<()>();
    let pool = context.pool.clone();
    let job = context.job_id;
    let beat = tokio::spawn(async move {
        let mut tick = tokio::time::interval(HEARTBEAT_EVERY);
        tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            // A failed write (a busy database) is tried again on the next tick.
            if let Ok(false) = ImportRepo::new(&pool).heartbeat(id, job).await {
                let _ = lost.send(());
                return;
            }
        }
    });
    let outcome = tokio::select! {
        outcome = work => outcome,
        Ok(()) = claim_lost => Err(JobError::Runtime(format!(
            "import {} is no longer this job's: the server found the job dead",
            id.0
        ))),
    };
    beat.abort();
    outcome
}

/// A progress callback that appends each line to the job log. The callback
/// is synchronous, so a task writes the lines in order; it ends when the
/// callback is dropped.
fn log_sink(
    context: &JobContext<'_>,
) -> (
    impl FnMut(Progress<'_>) + Send + use<>,
    JoinHandle<Result<(), DbError>>,
) {
    let (sender, receiver) = mpsc::unbounded_channel::<String>();
    let writer = tokio::spawn(write_lines(context.pool.clone(), context.job_id, receiver));
    let progress = move |step: Progress<'_>| {
        let _ = sender.send(step.line.to_string());
    };
    (progress, writer)
}

async fn write_lines(
    pool: SqlitePool,
    job: JobId,
    mut lines: mpsc::UnboundedReceiver<String>,
) -> Result<(), DbError> {
    while let Some(line) = lines.recv().await {
        store::job_append_log(&pool, job, &line).await?;
    }
    Ok(())
}

async fn finish_log(writer: JoinHandle<Result<(), DbError>>) -> Result<(), JobError> {
    writer
        .await
        .map_err(|error| JobError::Runtime(format!("the log writer stopped: {error}")))??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use omeka_db::repos::{JobRepo, StartOutcome};
    use omeka_db::rows::{NewImport, NewJob};
    use serde_json::json;

    use super::*;
    use crate::jobs::JobEnv;
    use crate::testing::seeded_db;

    /// A running import and an `in_progress` job, linked to it or not.
    async fn running(pool: &SqlitePool, linked: bool) -> (ImportId, JobId) {
        let imports = ImportRepo::new(pool);
        let id = imports
            .create_draft(&NewImport {
                owner_id: None,
                folder: "a".to_string(),
                plan: json!({}),
            })
            .await
            .unwrap()
            .id;
        assert_eq!(imports.start_run(id).await.unwrap(), StartOutcome::Started);
        let new_job = || NewJob {
            owner_id: None,
            pid: None,
            status: Some("in_progress".to_string()),
            class: omeka_server::IMPORT_JOB.to_string(),
            args: Some(json!({ "importId": id.0 })),
            log: None,
            started: None,
        };
        let jobs = JobRepo::new(pool);
        let worker = jobs.create(&new_job()).await.unwrap().id;
        let other = jobs.create(&new_job()).await.unwrap().id;
        imports
            .set_job(id, if linked { worker } else { other })
            .await
            .unwrap();
        (id, worker)
    }

    async fn heartbeat(pool: &SqlitePool, id: ImportId) -> String {
        let summary = ImportRepo::new(pool)
            .get(id)
            .await
            .unwrap()
            .unwrap()
            .summary;
        summary.unwrap()[HEARTBEAT].as_str().unwrap().to_string()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_heartbeat_task_beats_while_one_long_step_runs() {
        let db = seeded_db().await;
        let (id, worker) = running(&db.pool, true).await;
        sqlx::query("UPDATE romeka_import SET summary = '{\"heartbeat\":\"2000-01-01 00:00:00\"}'")
            .execute(&db.pool)
            .await
            .unwrap();
        let args = Value::Null;
        let env = JobEnv::default();
        let context = JobContext {
            pool: &db.pool,
            job_id: worker,
            args: &args,
            env: &env,
        };

        let step = async {
            tokio::time::sleep(Duration::from_millis(300)).await;
            Ok(7)
        };
        assert_eq!(beating(&context, id, step).await.unwrap(), 7);

        assert_ne!(heartbeat(&db.pool, id).await, "2000-01-01 00:00:00");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_work_stops_once_the_import_is_no_longer_this_jobs() {
        let db = seeded_db().await;
        let (id, worker) = running(&db.pool, false).await;
        let args = Value::Null;
        let env = JobEnv::default();
        let context = JobContext {
            pool: &db.pool,
            job_id: worker,
            args: &args,
            env: &env,
        };

        let forever = std::future::pending::<Result<(), JobError>>();
        let stopped = tokio::time::timeout(Duration::from_secs(5), beating(&context, id, forever))
            .await
            .expect("the work is dropped, not awaited");

        assert!(
            matches!(&stopped, Err(JobError::Runtime(reason)) if reason.contains("found the job dead")),
            "{stopped:?}"
        );
    }
}
