//! The job context, and the dispatcher that starts a job in a new process.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use omeka_db::ids::JobId;
use serde_json::Value;
use sqlx::SqlitePool;

use super::Job;
use super::JobError;
use super::status::JobStatus;
use crate::store;

/// What a job sees: the pool, its own row id, and its `args` JSON.
pub struct JobContext<'a> {
    pub pool: &'a SqlitePool,
    pub job_id: JobId,
    pub args: &'a Value,
}

impl JobContext<'_> {
    /// One named job argument (`AbstractJob::getArg`).
    #[must_use]
    pub fn arg(&self, name: &str) -> Option<&Value> {
        self.args.get(name)
    }

    /// A required string argument.
    ///
    /// # Errors
    /// A [`JobError::InvalidArgument`] when it is missing or not a string.
    pub fn required_str(&self, name: &str) -> Result<&str, JobError> {
        self.arg(name)
            .and_then(Value::as_str)
            .ok_or_else(|| JobError::InvalidArgument(format!("missing string argument \"{name}\"")))
    }

    /// Whether another process set the job to `stopping` (`AbstractJob::shouldStop`).
    ///
    /// # Errors
    /// A database error while reading the job row.
    pub async fn should_stop(&self) -> Result<bool, JobError> {
        Ok(store::job_status(self.pool, self.job_id).await?.as_deref() == Some("stopping"))
    }

    /// Append a line to the job log.
    ///
    /// # Errors
    /// A database error while updating the job row.
    pub async fn log(&self, message: &str) -> Result<(), JobError> {
        store::job_append_log(self.pool, self.job_id, message).await?;
        Ok(())
    }
}

/// Starts a composed job in a new process, mirroring
/// `Omeka\Job\DispatchStrategy\PhpCli`, which runs `perform-job.php` in the
/// background. `dispatch` returns as soon as the child is started, so the caller
/// is free even while the job runs.
pub struct Dispatcher<'a> {
    pool: &'a SqlitePool,
    database: &'a Path,
    /// The program to run `job run <id>` with. `None` means this executable,
    /// which is the port's own `omeka` binary.
    program: Option<PathBuf>,
}

impl<'a> Dispatcher<'a> {
    /// A dispatcher that starts `omeka job run <id>` from this executable.
    #[must_use]
    pub fn new(pool: &'a SqlitePool, database: &'a Path) -> Self {
        Self {
            pool,
            database,
            program: None,
        }
    }

    /// A dispatcher that starts `job run <id>` with a named program. The
    /// process tests point this at the built `omeka` binary.
    #[must_use]
    pub fn with_program(
        pool: &'a SqlitePool,
        database: &'a Path,
        program: impl Into<PathBuf>,
    ) -> Self {
        Self {
            pool,
            database,
            program: Some(program.into()),
        }
    }

    /// Compose a `job` row in `starting` and start it in a new process.
    ///
    /// Returns the row id at once. The child writes `in_progress`, the pid and
    /// the ending, so the caller observes the final status through the row.
    ///
    /// # Errors
    /// A database error composing the row, or [`JobError::Runtime`] when the
    /// process cannot be started. A start failure leaves the row `error`.
    pub async fn dispatch(&self, job: &dyn Job, args: Value) -> Result<JobId, JobError> {
        let id = store::job_create(self.pool, job.class(), &args, None).await?;
        if let Err(error) = self.spawn(id) {
            let _ = store::job_append_log(self.pool, id, &error.to_string()).await;
            let _ = store::job_finish(self.pool, id, JobStatus::Error.as_str()).await;
            return Err(error);
        }
        Ok(id)
    }

    /// Start `job run <id> --database <path>` without waiting for it.
    fn spawn(&self, id: JobId) -> Result<(), JobError> {
        let program = match &self.program {
            Some(program) => program.clone(),
            None => std::env::current_exe().map_err(|error| {
                JobError::Runtime(format!("cannot find the omeka executable: {error}"))
            })?,
        };
        Command::new(program)
            .arg("job")
            .arg("run")
            .arg(id.0.to_string())
            .arg("--database")
            .arg(self.database)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_child| ())
            .map_err(|error| JobError::Runtime(format!("cannot start job {}: {error}", id.0)))
    }
}
