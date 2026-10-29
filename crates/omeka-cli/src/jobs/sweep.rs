//! `Romeka\Job\ImportSweep`: the staging sweep (`omeka_api::import::sweep`),
//! with the install's retention settings and the clock of now. It takes no
//! args.
//!
//! The same hourly tick also sweeps the per-user staged uploads
//! (`omeka_api::uploads`, migration 0009), after the import sweep and its log
//! line, because the import session requires that order.
//!
//! Last, it starts one derivative worker when rows are queued, so a pending
//! preview does not wait for the next media create. The worker runs as its own
//! `romeka job run` process ([`JobEnv::jobs`]), never inside this one: a
//! derivative is no longer only a still, and a transcode of a long video would
//! hold the sweep for the length of the film. Nothing starts while an archive
//! import is running or undoing: the derivative worker's own yield rule applies
//! here first, and the next tick tries again.
//!
//! A run that removed nothing and started no worker deletes its own row
//! ([`JobContext::forget`]), so the jobs list keeps only the sweeps that did
//! something. A run that fails keeps its row, with the reason.

use async_trait::async_trait;
use omeka_api::derivatives::queue_worker;
use omeka_api::import::sweep::{Retention, SWEEP_JOB, Swept, sweep};
use omeka_api::messages;
use omeka_db::repos::{ImportRepo, JobRepo};
use omeka_db::with_db;

use super::{Job, JobContext, JobError};

/// `Romeka\Job\ImportSweep`.
pub struct ImportSweep;

#[async_trait]
impl Job for ImportSweep {
    fn class(&self) -> &'static str {
        SWEEP_JOB
    }

    async fn perform(&self, context: &JobContext<'_>) -> Result<(), JobError> {
        let settings = omeka_api::settings::upload_settings(context.pool).await?;
        let swept = with_db!(context.pool, |db| sweep(
            db,
            &context.env.store,
            chrono::Utc::now().naive_utc(),
            Retention::from(settings),
        )
        .await)?;
        // The import sweep's log is written first, so a failed user-upload sweep
        // below cannot lose the record of work the import sweep already did.
        let removed_staging = swept != Swept::default();
        if removed_staging {
            context
                .log(&messages::import_sweep_done(swept.imports, swept.uploads))
                .await?;
        }
        let uploads = with_db!(context.pool, |db| omeka_api::uploads::sweep(
            db,
            &context.env.store,
            chrono::Utc::now().naive_utc(),
        )
        .await)?;
        if uploads > 0 {
            context
                .log(&messages::media_upload_sweep_done(uploads))
                .await?;
        }
        let started = start_derivatives(context).await?;
        // An hourly tick that changed nothing leaves no row, so the jobs list
        // shows the work that happened instead of a page of idle sweeps.
        if !removed_staging && uploads == 0 && !started {
            context.forget().await?;
        }
        Ok(())
    }
}

/// Start one `MediaDerivatives` worker in its own process if rows are queued,
/// none is active and no import holds the machine.
///
/// `queue_worker` composes the row only when the queue has work and no worker of
/// that class is starting, running or stopping, so this is the same one-worker
/// cap the media route uses. The row is handed to a child process rather than
/// run here, because a transcode outlasts this run: the sweep would be held
/// open for the length of a film.
///
/// True when it composed a worker row, which it then logs whether or not the
/// worker could start.
async fn start_derivatives(context: &JobContext<'_>) -> Result<bool, JobError> {
    if with_db!(context.pool, |db| ImportRepo::new(db).busy_id().await)?.is_some() {
        return Ok(false);
    }
    let Some(worker) = queue_worker(context.pool, None).await? else {
        return Ok(false);
    };
    let Some(jobs) = &context.env.jobs else {
        context
            .log(&format!(
                "derivatives: worker {} is queued, but this process was not told how to start one; \
                 the next create or `romeka media derivatives` starts it.",
                worker.0
            ))
            .await?;
        return Ok(true);
    };
    match jobs.spawn(worker) {
        Ok(mut child) => {
            // Reap the child on this thread, as `romeka serve` does, so the
            // sweep process leaves no zombie if it outlives the run.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            context
                .log(&format!(
                    "derivatives: started worker {} in its own process",
                    worker.0
                ))
                .await?;
        }
        Err(error) => {
            // A row left `starting` would hold the one-worker cap for good.
            let _ = with_db!(context.pool, |db| JobRepo::new(db)
                .append_log(worker, &error.to_string())
                .await);
            let _ = with_db!(context.pool, |db| JobRepo::new(db)
                .finish(worker, "error")
                .await);
            context
                .log(&format!(
                    "derivatives: cannot start worker {}: {error}",
                    worker.0
                ))
                .await?;
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use omeka_db::ids::JobId;

    use super::*;
    use crate::jobs::{JobEnv, JobStatus, Runner};
    use crate::testing::{TempDb, compose, seeded_db};

    async fn run_sweep(db: &TempDb, store: &tempfile::TempDir) -> (JobId, JobStatus) {
        let job = compose(&db.db_pool, &ImportSweep, serde_json::json!({})).await;
        let env = JobEnv {
            store: omeka_server::local_store(store.path()),
            ..JobEnv::default()
        };
        let status = Runner::with_env(&db.db_pool, env)
            .run(job)
            .await
            .expect("sweep ran");
        (job, status)
    }

    #[tokio::test]
    async fn a_sweep_that_removed_nothing_leaves_no_job() {
        let db = seeded_db().await;
        let store = tempfile::tempdir().expect("store");

        let (job, status) = run_sweep(&db, &store).await;

        assert_eq!(status, JobStatus::Completed);
        assert_eq!(JobRepo::new(&db.pool).get(job).await.expect("read"), None);
    }

    #[tokio::test]
    async fn a_sweep_that_removed_an_upload_keeps_its_job_and_says_what_went() {
        let db = seeded_db().await;
        let store = tempfile::tempdir().expect("store");
        sqlx::query(
            "INSERT INTO romeka_user_upload (user_id, id, path, length, created, expires)
             VALUES (1, 'old', 'old.jpg', 4, '2026-01-01 00:00:00', '2026-01-02 00:00:00')",
        )
        .execute(&db.pool)
        .await
        .expect("an expired upload");

        let (job, status) = run_sweep(&db, &store).await;

        assert_eq!(status, JobStatus::Completed);
        let row = JobRepo::new(&db.pool)
            .get(job)
            .await
            .expect("read")
            .expect("the sweep that removed an upload stays");
        assert_eq!(row.status.as_deref(), Some("completed"));
        assert_eq!(
            row.log.as_deref(),
            Some(format!("{}\n", messages::media_upload_sweep_done(1)).as_str())
        );
    }
}
