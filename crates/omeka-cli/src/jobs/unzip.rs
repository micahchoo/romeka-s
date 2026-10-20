//! `Romeka\Job\ImportUnzip`: a finished `.zip` upload unpacked into its
//! import's staging area (`omeka_api::import2::unzip`). Its args are
//! `{"importId": n, "upload": id}`.
//!
//! The log gets one line per entry left out or renamed, then the outcome. A
//! refused ZIP ends the job `error` with the refusal as the last log line,
//! which the admin shows.

use async_trait::async_trait;
use omeka_api::import2::staging::Limits;
use omeka_api::import2::unzip::{UNZIP_JOB, UnzipError, unzip_upload};
use omeka_api::messages;
use omeka_db::ids::ImportId;
use omeka_db::with_db;

use super::{Job, JobContext, JobError};

/// `Romeka\Job\ImportUnzip`.
pub struct ImportUnzip;

#[async_trait]
impl Job for ImportUnzip {
    fn class(&self) -> &'static str {
        UNZIP_JOB
    }

    async fn perform(&self, context: &JobContext<'_>) -> Result<(), JobError> {
        let import = context
            .arg("importId")
            .and_then(serde_json::Value::as_u64)
            .map(ImportId)
            .ok_or_else(|| {
                JobError::InvalidArgument("missing number argument \"importId\"".into())
            })?;
        let upload = context
            .arg("upload")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| JobError::InvalidArgument("missing string argument \"upload\"".into()))?
            .to_string();
        let settings = omeka_api::settings::upload_settings(context.pool).await?;
        match with_db!(context.pool, |db| unzip_upload(
            db,
            &context.env.store,
            import,
            &upload,
            Limits::from(settings),
        )
        .await)
        {
            Ok(unpacked) => {
                for note in &unpacked.notes {
                    context.log(&note.message()).await?;
                }
                context
                    .log(&messages::import_zip_unpacked(unpacked.files))
                    .await
            }
            Err(UnzipError::Refused(refusal)) => Err(JobError::Runtime(refusal.message())),
            Err(error) => Err(JobError::Runtime(error.to_string())),
        }
    }
}
