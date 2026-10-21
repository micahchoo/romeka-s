//! `Romeka\Job\ImportSweep`: the staging sweep (`omeka_api::import::sweep`),
//! with the install's retention settings and the clock of now. It takes no
//! args.
//!
//! The hourly tick also starts one derivative worker when rows are queued, so a
//! pending preview does not wait for the next media create. The worker runs in
//! this process (`Runner`) because the sweep job has the store open and a
//! `romeka job run` child would have to name it again. Nothing starts while an
//! archive import is running or undoing: the derivative worker's own yield rule
//! applies here first, and the next tick tries again.

use async_trait::async_trait;
use omeka_api::derivatives::queue_worker;
use omeka_api::import::sweep::{Retention, SWEEP_JOB, sweep};
use omeka_api::messages;
use omeka_db::repos::ImportRepo;
use omeka_db::with_db;

use super::{Job, JobContext, JobError, Runner};

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
        context
            .log(&messages::import_sweep_done(swept.imports, swept.uploads))
            .await?;
        start_derivatives(context).await
    }
}

/// Start one `MediaDerivatives` worker if rows are queued, none is active and no
/// import holds the machine.
///
/// `queue_worker` composes the row only when the queue has work and no worker of
/// that class is starting, running or stopping, so this is the same one-worker
/// cap the media route uses. The row is run here rather than spawned: the sweep
/// process has the store open and a child would need its root, its base URL and
/// its archive root named again.
async fn start_derivatives(context: &JobContext<'_>) -> Result<(), JobError> {
    if with_db!(context.pool, |db| ImportRepo::new(db).busy_id().await)?.is_some() {
        return Ok(());
    }
    let Some(worker) = queue_worker(context.pool, None).await? else {
        return Ok(());
    };
    context
        .log(&format!("derivatives: started worker {}", worker.0))
        .await?;
    Runner::with_env(context.pool, context.env.clone())
        .run(worker)
        .await?;
    Ok(())
}
