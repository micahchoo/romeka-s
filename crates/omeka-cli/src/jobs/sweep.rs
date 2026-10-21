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

use async_trait::async_trait;
use omeka_api::derivatives::queue_worker;
use omeka_api::import::sweep::{Retention, SWEEP_JOB, sweep};
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
        context
            .log(&messages::import_sweep_done(swept.imports, swept.uploads))
            .await?;
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
        start_derivatives(context).await
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
async fn start_derivatives(context: &JobContext<'_>) -> Result<(), JobError> {
    if with_db!(context.pool, |db| ImportRepo::new(db).busy_id().await)?.is_some() {
        return Ok(());
    }
    let Some(worker) = queue_worker(context.pool, None).await? else {
        return Ok(());
    };
    let Some(jobs) = &context.env.jobs else {
        context
            .log(&format!(
                "derivatives: worker {} is queued, but this process was not told how to start one; \
                 the next create or `romeka media derivatives` starts it.",
                worker.0
            ))
            .await?;
        return Ok(());
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
    Ok(())
}
