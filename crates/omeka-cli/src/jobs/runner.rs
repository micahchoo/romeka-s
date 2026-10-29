//! The worker: run one composed `job` row to a terminal status.
//!
//! This is the port of `application/data/scripts/perform-job.php` plus
//! `Omeka\Job\DispatchStrategy\Synchronous`. It loads the row, records the
//! process id and `in_progress`, enters the job, then writes the ending.
//! `romeka job run <id>` calls it; the dispatcher starts that command in a new
//! process so a long job does not hold the caller.

use serde_json::Value;

use omeka_db::DbPool;
use omeka_db::ids::JobId;
use omeka_db::repos::JobRepo;
use omeka_db::with_db;

use super::status::JobStatus;
use super::{JobContext, JobEnv, JobError, job_for};

/// Runs one stored job.
pub struct Runner<'a> {
    pool: &'a DbPool,
    env: JobEnv,
}

impl<'a> Runner<'a> {
    /// A runner with the default [`JobEnv`].
    #[must_use]
    pub fn new(pool: &'a DbPool) -> Self {
        Self::with_env(pool, JobEnv::default())
    }

    #[must_use]
    pub fn with_env(pool: &'a DbPool, env: JobEnv) -> Self {
        Self { pool, env }
    }

    /// Run the job row `id` to a terminal status.
    ///
    /// A row already at a terminal status is returned unchanged, so running a
    /// finished job again does no work. A `starting`, `in_progress` or
    /// `stopping` row is run: the runner records `in_progress` with this
    /// process id, hands the job a context whose `should_stop` polls the row,
    /// then writes `completed`, `stopped` or `error`.
    ///
    /// # Errors
    /// [`JobError::InvalidArgument`] for a missing row or an unknown
    /// `job.class`; the job's own error after the row is marked `error`.
    pub async fn run(&self, id: JobId) -> Result<JobStatus, JobError> {
        let row = with_db!(self.pool, |db| JobRepo::new(db).get(id).await)?
            .ok_or_else(|| JobError::InvalidArgument(format!("no job with id {}", id.0)))?;
        let stored = row.status.as_deref().and_then(JobStatus::parse);
        if let Some(status) = stored
            && status.is_terminal()
        {
            return Ok(status);
        }
        let job = match job_for(&row.class) {
            Some(job) => job,
            None => {
                let message = format!("unknown job class \"{}\"", row.class);
                let _ = with_db!(self.pool, |db| JobRepo::new(db)
                    .append_log(id, &message)
                    .await);
                let _ = with_db!(self.pool, |db| JobRepo::new(db)
                    .finish(id, JobStatus::Error.as_str())
                    .await);
                return Err(JobError::InvalidArgument(message));
            }
        };
        let args = row.args.clone().unwrap_or(Value::Null);

        with_db!(self.pool, |db| JobRepo::new(db)
            .start(id, &std::process::id().to_string())
            .await)?;
        let context = JobContext {
            pool: self.pool,
            job_id: id,
            args: &args,
            env: &self.env,
        };
        match job.perform(&context).await {
            Ok(()) => {
                let Some(stored) = with_db!(self.pool, |db| JobRepo::new(db).get(id).await)? else {
                    // The job forgot its own row (`JobContext::forget`): nothing to end.
                    return Ok(JobStatus::Completed);
                };
                let stored = stored.status.as_deref().and_then(JobStatus::parse);
                let status = stored.unwrap_or(JobStatus::Starting).ending();
                with_db!(self.pool, |db| JobRepo::new(db)
                    .finish(id, status.as_str())
                    .await)?;
                with_db!(self.pool, |db| JobRepo::new(db).clear_pid(id).await)?;
                Ok(status)
            }
            Err(error) => {
                let _ = with_db!(self.pool, |db| JobRepo::new(db)
                    .append_log(id, &error.to_string())
                    .await);
                let _ = with_db!(self.pool, |db| JobRepo::new(db)
                    .finish(id, JobStatus::Error.as_str())
                    .await);
                let _ = with_db!(self.pool, |db| JobRepo::new(db).clear_pid(id).await);
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::BatchDelete;
    use crate::testing::{compose, seeded_db};
    use omeka_db::repos::ItemRepo;
    use omeka_db::rows::{NewItem, NewResource};

    async fn new_item(pool: &sqlx::SqlitePool, title: &str) {
        ItemRepo::new(pool)
            .create(&NewItem {
                resource: NewResource {
                    title: Some(title.to_string()),
                    ..NewResource::default()
                },
                primary_media_id: None,
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn a_run_records_completion_and_clears_the_pid() {
        let db = seeded_db().await;
        let job_id = compose(
            &db.db_pool,
            &BatchDelete,
            serde_json::json!({"resource": "items"}),
        )
        .await;

        let status = Runner::new(&db.db_pool).run(job_id).await.expect("run");

        assert_eq!(status, JobStatus::Completed);
        let row = JobRepo::new(&db.pool)
            .get(job_id)
            .await
            .unwrap()
            .expect("row");
        assert_eq!(row.status.as_deref(), Some("completed"));
        assert!(row.ended.is_some(), "the runner writes the ending");
        assert_eq!(row.pid, None, "the pid is cleared when the process ends");
    }

    #[tokio::test]
    async fn a_finished_job_is_not_run_again() {
        let db = seeded_db().await;
        let job_id = compose(
            &db.db_pool,
            &BatchDelete,
            serde_json::json!({"resource": "items"}),
        )
        .await;
        Runner::new(&db.db_pool).run(job_id).await.expect("run");
        let ended = JobRepo::new(&db.pool)
            .get(job_id)
            .await
            .unwrap()
            .expect("row")
            .ended;

        let status = Runner::new(&db.db_pool).run(job_id).await.expect("re-run");

        assert_eq!(status, JobStatus::Completed);
        assert_eq!(
            JobRepo::new(&db.pool)
                .get(job_id)
                .await
                .unwrap()
                .expect("row")
                .ended,
            ended,
            "a terminal row keeps its ending"
        );
    }

    #[tokio::test]
    async fn an_unknown_class_marks_the_row_error_without_starting_it() {
        let db = seeded_db().await;
        let job_id = crate::jobs::compose_job(
            &db.db_pool,
            "Omeka\\Job\\Missing",
            &serde_json::json!({}),
            None,
        )
        .await
        .unwrap();

        let error = Runner::new(&db.db_pool)
            .run(job_id)
            .await
            .expect_err("unknown class");

        assert!(
            matches!(error, JobError::InvalidArgument(_)),
            "got {error:?}"
        );
        let row = JobRepo::new(&db.pool)
            .get(job_id)
            .await
            .unwrap()
            .expect("row");
        assert_eq!(row.status.as_deref(), Some("error"));
        assert_eq!(row.pid, None, "no process was started");
        assert!(
            row.log.unwrap_or_default().contains("unknown job class"),
            "the reason is logged"
        );
    }

    #[tokio::test]
    async fn a_failing_job_marks_the_row_error_and_logs_the_reason() {
        let db = seeded_db().await;
        let job_id = compose(
            &db.db_pool,
            &BatchDelete,
            serde_json::json!({"resource": "widgets"}),
        )
        .await;

        let error = Runner::new(&db.db_pool)
            .run(job_id)
            .await
            .expect_err("failure");

        let row = JobRepo::new(&db.pool)
            .get(job_id)
            .await
            .unwrap()
            .expect("row");
        assert_eq!(row.status.as_deref(), Some("error"));
        assert!(row.ended.is_some(), "a failure still ends the job");
        let log = row.log.unwrap_or_default();
        assert!(log.contains(&error.to_string()), "log was {log:?}");
        assert!(log.ends_with('\n'), "each log line ends with a newline");
    }

    #[tokio::test]
    async fn job_log_lines_are_appended_with_a_trailing_newline() {
        let db = seeded_db().await;
        let job_id = compose(
            &db.db_pool,
            &BatchDelete,
            serde_json::json!({"resource": "items"}),
        )
        .await;
        let args = serde_json::Value::Null;
        let env = JobEnv::default();
        let context = JobContext {
            pool: &db.db_pool,
            job_id,
            args: &args,
            env: &env,
        };

        context.log("first").await.unwrap();
        context.log("second").await.unwrap();

        let row = JobRepo::new(&db.pool)
            .get(job_id)
            .await
            .unwrap()
            .expect("row");
        assert_eq!(row.log.as_deref(), Some("first\nsecond\n"));
    }

    #[tokio::test]
    async fn a_stop_request_ends_the_job_stopped() {
        let db = seeded_db().await;
        for index in 0..200 {
            new_item(&db.pool, &format!("item {index}")).await;
        }
        omeka_db::repos::SettingRepo::new(&db.pool)
            .set("batch_chunk_size", &serde_json::json!(1))
            .await
            .unwrap();
        let job_id = compose(
            &db.db_pool,
            &BatchDelete,
            serde_json::json!({"resource": "items"}),
        )
        .await;

        let pool = db.db_pool.clone();
        let running = tokio::spawn(async move { Runner::new(&pool).run(job_id).await });

        while JobRepo::new(&db.pool)
            .status(job_id)
            .await
            .unwrap()
            .as_deref()
            != Some("in_progress")
        {
            tokio::task::yield_now().await;
        }
        assert!(JobRepo::new(&db.pool).request_stop(job_id).await.unwrap());

        let status = running.await.unwrap().expect("run");
        assert_eq!(status, JobStatus::Stopped);
        let row = JobRepo::new(&db.pool)
            .get(job_id)
            .await
            .unwrap()
            .expect("row");
        assert_eq!(row.status.as_deref(), Some("stopped"));
        assert!(row.ended.is_some(), "a stopped job still ends");
    }
}
