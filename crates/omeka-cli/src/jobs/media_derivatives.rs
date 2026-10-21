//! `Romeka\Job\MediaDerivatives`: the worker that makes a media's preview and
//! its playable copy.
//!
//! A create outside the archive import stores the original and queues a row
//! (`omeka_api::derivatives`); this worker drains the queue. It claims one row
//! at a time, makes the still from the stored original, and settles the row
//! ready, failed or none. An `access_copy` row whose tool is `ffmpeg` becomes a
//! transcode instead: an H.264/AAC MP4 for video or an AAC M4A for audio, which
//! runs on its own thread and reports its progress to the row. One whose tool is
//! `soffice` becomes a PDF made from an office document, on the same thread and
//! with no progress. Upstream makes
//! the same derivatives inside the request (`TempFile::storeThumbnails`); the
//! archive import keeps doing that, because it already runs off the request and
//! batches its work.
//!
//! The queue holds one worker per install: a still or a transcode runs on a file
//! that may be gigabytes, and the deployment is one box whose disk the import
//! shares. A worker that finds an import running or undoing holds the queue and
//! starts nothing until the import ends ([`YIELD_LIMIT`]), a long run checks for
//! a stop request between rows, and a running transcode checks between progress
//! updates: it stops cleanly and goes back to the queue when the job is stopped
//! or an import starts.
//!
//! The work itself is [`omeka_api::derivatives::settle_next`], so a test and the
//! `romeka media derivatives` backfill drain the same queue the same way.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use omeka_api::derivatives::{DrainStats, Settled, settle_next};
use omeka_db::DbPool;
use omeka_db::ids::ImportId;
use omeka_db::repos::ImportRepo;
use omeka_db::rows::DerivativeState;
use omeka_db::with_db;

use super::{Job, JobContext, JobError};

/// How long the worker waits for an import that is running or undoing before
/// it ends with its rows still queued. The next create, or
/// `romeka media derivatives`, starts it again.
pub const YIELD_LIMIT: Duration = Duration::from_secs(60 * 60);

/// How often it asks whether the import has finished.
pub const YIELD_POLL: Duration = Duration::from_millis(250);

/// The worker.
pub struct MediaDerivatives;

#[async_trait]
impl Job for MediaDerivatives {
    fn class(&self) -> &'static str {
        omeka_api::derivatives::DERIVATIVES_JOB
    }

    async fn perform(&self, context: &JobContext<'_>) -> Result<(), JobError> {
        let mut stats = DrainStats::default();
        loop {
            if context.should_stop().await? {
                context.log("stopped on request.").await?;
                return Ok(());
            }
            if let Some(import) = busy_import(context.pool).await? {
                if !wait_for_import(context, import).await? {
                    stats.left = pending(context.pool).await?;
                    break;
                }
                continue;
            }
            let Some(settled) =
                settle_next(context.pool, &context.env.store, Some(context.job_id)).await?
            else {
                break;
            };
            match settled {
                Settled::State {
                    state: DerivativeState::Ready,
                    ..
                } => stats.ready += 1,
                Settled::State {
                    state: DerivativeState::None,
                    reason,
                } => {
                    stats.skipped += 1;
                    if let Some(reason) = reason {
                        context.log(&format!("no derivative: {reason}")).await?;
                    }
                }
                Settled::State { reason, .. } => {
                    stats.failed += 1;
                    if let Some(reason) = reason {
                        context.log(&format!("failed: {reason}")).await?;
                    }
                }
                Settled::Queued { reason } => {
                    stats.left = pending(context.pool).await?;
                    if let Some(reason) = reason {
                        context.log(&format!("left queued: {reason}")).await?;
                    }
                    // The copy stopped because the machine was not free or the
                    // job was asked to. The rows stay queued for the next worker.
                    break;
                }
                Settled::Gone => {}
            }
        }
        let line = format!(
            "Derivatives: {} ready, {} with no derivative, {} failed, {} left queued.",
            stats.ready, stats.skipped, stats.failed, stats.left
        );
        context.log(&line).await
    }
}

/// The import a run or undo is working, if any.
async fn busy_import(pool: &DbPool) -> Result<Option<ImportId>, JobError> {
    Ok(with_db!(pool, |db| ImportRepo::new(db).busy_id().await)?)
}

async fn pending(pool: &DbPool) -> Result<u32, JobError> {
    let commands = omeka_media::tool::Tools::on_path().derivative_commands();
    let count = with_db!(pool, |db| omeka_db::repos::MediaDerivativeRepo::new(db)
        .pending(&commands)
        .await)?;
    Ok(u32::try_from(count).unwrap_or(u32::MAX))
}

/// Hold the queue for an import. `true` when the machine is free again, `false`
/// when the worker should end with its rows queued: a stop was asked for, or
/// the import outlasted [`YIELD_LIMIT`].
async fn wait_for_import(context: &JobContext<'_>, import: ImportId) -> Result<bool, JobError> {
    context
        .log(&format!(
            "waiting: import {} is running or undoing; no derivative starts until it ends.",
            import.0
        ))
        .await?;
    let start = Instant::now();
    loop {
        if context.should_stop().await? {
            return Ok(false);
        }
        if busy_import(context.pool).await?.is_none() {
            return Ok(true);
        }
        if start.elapsed() >= YIELD_LIMIT {
            context
                .log(&format!(
                    "yielded after {} minutes: import {} still holds the machine.",
                    YIELD_LIMIT.as_secs() / 60,
                    import.0
                ))
                .await?;
            return Ok(false);
        }
        tokio::time::sleep(YIELD_POLL).await;
    }
}
