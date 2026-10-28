//! `Romeka\Job\FixityCheck`: read every stored original and compare its SHA-256 with the record.
//!
//! It takes no args. The work and the record it writes are `omeka_api::fixity::check_all`; the
//! Jobs screen starts it through `POST /api/romeka/fixity`, and `romeka fixity check` runs it in
//! its own process.

use async_trait::async_trait;
use omeka_api::fixity::{CHECK_JOB, check_all};

use super::{Job, JobContext, JobError};

/// `Romeka\Job\FixityCheck`.
pub struct FixityCheck;

#[async_trait]
impl Job for FixityCheck {
    fn class(&self) -> &'static str {
        CHECK_JOB
    }

    async fn perform(&self, context: &JobContext<'_>) -> Result<(), JobError> {
        check_all(context.pool, &context.env.store, context.job_id).await?;
        Ok(())
    }
}
