//! `Romeka\Job\ImportSweep`: the staging sweep (`omeka_api::import::sweep`),
//! with the install's retention settings and the clock of now. It takes no
//! args.

use async_trait::async_trait;
use omeka_api::import::sweep::{Retention, SWEEP_JOB, sweep};
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
        Ok(())
    }
}
