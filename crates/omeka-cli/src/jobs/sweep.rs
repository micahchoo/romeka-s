//! `Romeka\Job\ImportSweep`: the staging sweep (`omeka_api::import2::sweep`),
//! with the install's retention settings and the clock of now. It takes no
//! args.

use async_trait::async_trait;
use omeka_api::import2::sweep::{Retention, SWEEP_JOB, sweep};
use omeka_api::messages;
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
        context
            .log(&messages::import_sweep_done(swept.imports, swept.uploads))
            .await
    }
}
