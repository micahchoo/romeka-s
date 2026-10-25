//! `Romeka\Job\BulkRun` and `Romeka\Job\BulkUndo`: a bulk change to items and its undo.
//!
//! Each reads `bulkId` from its args, the undo also `force`. The body is
//! `omeka_api::bulk::work::work`; this module only reads the args and logs how it ended.

use async_trait::async_trait;
use omeka_api::bulk::work::{WorkEnd, work};
use omeka_api::bulk::{RUN_JOB, UNDO_JOB};
use omeka_db::ids::BulkId;
use omeka_db::rows::{BulkEnding, BulkPhase};

use super::{Job, JobContext, JobError};

/// `Romeka\Job\BulkRun`.
pub struct BulkRun;

/// `Romeka\Job\BulkUndo`; its args may carry `force`.
pub struct BulkUndo;

#[async_trait]
impl Job for BulkRun {
    fn class(&self) -> &'static str {
        RUN_JOB
    }

    async fn perform(&self, context: &JobContext<'_>) -> Result<(), JobError> {
        perform(context, BulkPhase::Run, false).await
    }
}

#[async_trait]
impl Job for BulkUndo {
    fn class(&self) -> &'static str {
        UNDO_JOB
    }

    async fn perform(&self, context: &JobContext<'_>) -> Result<(), JobError> {
        let force = match context.arg("force") {
            None => false,
            Some(force) => force.as_bool().ok_or_else(|| {
                JobError::InvalidArgument("argument \"force\" must be a boolean".into())
            })?,
        };
        perform(context, BulkPhase::Undo, force).await
    }
}

async fn perform(context: &JobContext<'_>, phase: BulkPhase, force: bool) -> Result<(), JobError> {
    let bulk = context
        .arg("bulkId")
        .and_then(serde_json::Value::as_u64)
        .map(BulkId)
        .ok_or_else(|| JobError::InvalidArgument("missing number argument \"bulkId\"".into()))?;
    let end = work(
        context.pool,
        &context.env.base_url,
        &context.env.store,
        bulk,
        context.job_id,
        phase,
        force,
    )
    .await
    .map_err(|error| JobError::Runtime(error.to_string()))?;
    let line = match end {
        WorkEnd::Ended(BulkEnding::Done) => format!("Bulk change {bulk} done."),
        WorkEnd::Ended(BulkEnding::Stopped) => format!("Bulk change {bulk} stopped on request."),
        WorkEnd::Ended(BulkEnding::Failed) => format!("Bulk change {bulk} failed; see its error."),
        WorkEnd::Ended(BulkEnding::Undone) => format!("Bulk change {bulk} undone."),
        WorkEnd::Ended(BulkEnding::PartlyUndone) => {
            format!("Bulk change {bulk} undone but for items edited since, gone, or not reached.")
        }
        WorkEnd::Lost => format!("Bulk change {bulk} is no longer worked on by this job."),
    };
    context.log(&line).await
}
