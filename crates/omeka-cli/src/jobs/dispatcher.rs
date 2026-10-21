//! The job context, and the dispatcher that starts a job in a new process.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use omeka_db::DbPool;
use omeka_db::Location;
use omeka_db::ids::{JobId, UserId};
use omeka_db::repos::JobRepo;
use omeka_db::rows::NewJob;
use omeka_db::with_db;
use omeka_server::{DispatchError, JobDispatch};
use serde_json::Value;

use super::status::JobStatus;
use super::{Job, JobEnv, JobError};

/// What a job sees: the pool, its own row id, its `args` JSON, and the
/// process's [`JobEnv`].
pub struct JobContext<'a> {
    pub pool: &'a DbPool,
    pub job_id: JobId,
    pub args: &'a Value,
    pub env: &'a JobEnv,
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
        Ok(
            with_db!(self.pool, |db| JobRepo::new(db).status(self.job_id).await)?.as_deref()
                == Some("stopping"),
        )
    }

    /// Append a line to the job log.
    ///
    /// # Errors
    /// A database error while updating the job row.
    pub async fn log(&self, message: &str) -> Result<(), JobError> {
        with_db!(self.pool, |db| JobRepo::new(db)
            .append_log(self.job_id, message)
            .await)?;
        Ok(())
    }
}

/// Compose a `job` row in `starting` status, as `Dispatcher::dispatch` does.
///
/// # Errors
/// A database error writing the row.
pub async fn compose_job(
    pool: &DbPool,
    class: &str,
    args: &Value,
    owner_id: Option<UserId>,
) -> Result<JobId, omeka_db::DbError> {
    let row = with_db!(pool, |db| JobRepo::new(db)
        .create(&NewJob {
            owner_id,
            pid: None,
            status: Some("starting".to_string()),
            class: class.to_string(),
            args: Some(args.clone()),
            log: None,
            started: None,
        })
        .await)?;
    Ok(row.id)
}

/// The variable a `--database-url` reaches a child process through, and the one
/// `--database-url` falls back to.
pub const DATABASE_URL_ENV: &str = "ROMEKA_DATABASE_URL";

/// The `romeka job run <id>` command line a new job process runs.
#[derive(Debug, Clone)]
pub struct JobCommand {
    /// The program to run `job run <id>` with. `None` means this executable,
    /// which is the port's own `romeka` binary.
    pub program: Option<PathBuf>,
    pub database: Location,
    /// `--store`; the child's default (`files` beside the database) when `None`.
    pub store: Option<PathBuf>,
    /// `--base-url`.
    pub base_url: Option<String>,
    /// `--archive-root`.
    pub archive_root: Option<PathBuf>,
}

impl JobCommand {
    /// The command for `database` with every other flag left to its default.
    #[must_use]
    pub fn new(database: impl Into<Location>) -> Self {
        Self {
            program: None,
            database: database.into(),
            store: None,
            base_url: None,
            archive_root: None,
        }
    }

    /// Start `job run <id>` without waiting for it. The caller owns the child.
    ///
    /// # Errors
    /// [`JobError::Runtime`] when the executable cannot be found or started.
    pub fn spawn(&self, id: JobId) -> Result<Child, JobError> {
        self.command(id)?
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| JobError::Runtime(format!("cannot start job {}: {error}", id.0)))
    }

    /// The `job run <id>` command [`Self::spawn`] starts.
    ///
    /// # Errors
    /// [`JobError::Runtime`] when this executable cannot be found.
    pub fn command(&self, id: JobId) -> Result<Command, JobError> {
        let program = match &self.program {
            Some(program) => program.clone(),
            None => std::env::current_exe().map_err(|error| {
                JobError::Runtime(format!("cannot find the romeka executable: {error}"))
            })?,
        };
        let mut command = Command::new(program);
        command.arg("job").arg("run").arg(id.0.to_string());
        match &self.database {
            Location::File(path) => {
                command.arg("--database").arg(path);
            }
            // The environment, not the command line, so the password stays out of `ps`.
            Location::Url(url) => {
                command.env(DATABASE_URL_ENV, url);
            }
        }
        if let Some(store) = &self.store {
            command.arg("--store").arg(store);
        }
        if let Some(base_url) = &self.base_url {
            command.arg("--base-url").arg(base_url);
        }
        if let Some(root) = &self.archive_root {
            command.arg("--archive-root").arg(root);
        }
        Ok(command)
    }
}

/// Starts a composed job in a new process, mirroring
/// `Omeka\Job\DispatchStrategy\PhpCli`, which runs `perform-job.php` in the
/// background. `dispatch` returns as soon as the child is started, so the caller
/// is free even while the job runs.
pub struct Dispatcher<'a> {
    pool: &'a DbPool,
    command: JobCommand,
}

impl<'a> Dispatcher<'a> {
    /// A dispatcher that starts `romeka job run <id>` from this executable.
    #[must_use]
    pub fn new(pool: &'a DbPool, database: &Location) -> Self {
        Self {
            pool,
            command: JobCommand::new(database.clone()),
        }
    }

    /// A dispatcher that starts `job run <id>` with a named program. The
    /// process tests point this at the built `romeka` binary.
    #[must_use]
    pub fn with_program(
        pool: &'a DbPool,
        database: &Location,
        program: impl Into<PathBuf>,
    ) -> Self {
        Self {
            pool,
            command: JobCommand {
                program: Some(program.into()),
                ..JobCommand::new(database.clone())
            },
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
        let id = compose_job(self.pool, job.class(), &args, None).await?;
        if let Err(error) = self.command.spawn(id) {
            let _ = with_db!(self.pool, |db| JobRepo::new(db)
                .append_log(id, &error.to_string())
                .await);
            let _ = with_db!(self.pool, |db| JobRepo::new(db)
                .finish(id, JobStatus::Error.as_str())
                .await);
            return Err(error);
        }
        Ok(id)
    }
}

/// Starts each job in a new `romeka job run` process and reaps it when it
/// exits, so a long-lived server leaves no zombie processes.
pub struct ProcessDispatch(pub JobCommand);

impl JobDispatch for ProcessDispatch {
    fn start(&self, job: JobId) -> Result<(), DispatchError> {
        let mut child = self
            .0
            .spawn(job)
            .map_err(|error| DispatchError(error.to_string()))?;
        std::thread::spawn(move || child.wait());
        Ok(())
    }
}
