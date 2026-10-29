//! The job context, and the dispatcher that starts a job in a new process.

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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

    /// Delete the job's own row, for a job that found nothing to do: an hourly
    /// timer's idle tick would otherwise bury the jobs that did work. The runner
    /// then writes no ending. Call it last, after the job's final write.
    ///
    /// # Errors
    /// A database error while deleting the row.
    pub async fn forget(&self) -> Result<(), JobError> {
        with_db!(self.pool, |db| JobRepo::new(db).delete(self.job_id).await)?;
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

/// How many job processes one server runs at once. Each job opens its own database pool, so
/// without a bound a burst of jobs could open more connections than MySQL allows. A job past the
/// bound waits, its row in `starting`, until a running one exits.
pub const MAX_RUNNING_JOBS: usize = 4;

/// Starts each job in a new `romeka job run` process and reaps it when it
/// exits, so a long-lived server leaves no zombie processes. At most
/// [`MAX_RUNNING_JOBS`] run at once; later starts wait their turn in order.
///
/// Each job leads its own process group, so the server's shutdown reaches
/// the jobs it started and nothing else ([`Self::shutdown`]).
pub struct ProcessDispatch {
    shared: Arc<Shared>,
}

struct Shared {
    command: JobCommand,
    limit: usize,
    state: Mutex<Running>,
}

#[derive(Default)]
struct Running {
    /// The pid of each job this dispatcher started that has not exited.
    live: HashSet<u32>,
    /// Jobs waiting for a free slot, first come first started.
    waiting: VecDeque<JobId>,
    /// Set by [`ProcessDispatch::shutdown`]: nothing starts after it.
    stopping: bool,
}

impl ProcessDispatch {
    #[must_use]
    pub fn new(command: JobCommand) -> Self {
        Self::with_limit(command, MAX_RUNNING_JOBS)
    }

    /// A dispatcher that runs at most `limit` jobs at once (at least one).
    #[must_use]
    pub fn with_limit(command: JobCommand, limit: usize) -> Self {
        Self {
            shared: Arc::new(Shared {
                command,
                limit: limit.max(1),
                state: Mutex::default(),
            }),
        }
    }

    /// End the jobs this dispatcher started, for a server that is stopping, and
    /// return the jobs still waiting for a slot: they never started, and the
    /// caller settles their rows.
    ///
    /// Each job process gets a TERM, and only it: a job that runs tools of
    /// its own (the derivative worker's ffmpeg and soffice) decides what they
    /// do, since a TERM to a tool fails its copy and spends one of its
    /// attempts. An import run ends after the record in hand. After `grace`,
    /// or once every job has exited, each job's process group gets a KILL, so
    /// no tool outlives the server. Blocks until then.
    pub fn shutdown(&self, grace: Duration) -> Vec<JobId> {
        let (started, waiting) = {
            let mut running = self.shared.lock();
            running.stopping = true;
            let waiting: Vec<JobId> = running.waiting.drain(..).collect();
            (running.live.iter().copied().collect::<Vec<u32>>(), waiting)
        };
        for pid in &started {
            signal(*pid, Ending::Term);
        }
        let deadline = Instant::now() + grace;
        while !self.shared.lock().live.is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        for pid in &started {
            signal(*pid, Ending::KillGroup);
        }
        waiting
    }
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, Running> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Start `job` now, under the state lock the caller holds, so two starts
    /// never both take the last slot.
    fn launch(self: &Arc<Self>, running: &mut Running, job: JobId) -> Result<(), DispatchError> {
        let mut command = self
            .command
            .command(job)
            .map_err(|error| DispatchError(error.to_string()))?;
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| DispatchError(format!("cannot start job {}: {error}", job.0)))?;
        let pid = child.id();
        running.live.insert(pid);
        let shared = Arc::clone(self);
        std::thread::spawn(move || {
            let _ = child.wait();
            shared.exited(pid);
        });
        Ok(())
    }

    /// Reap `pid` and start the jobs that waited for its slot. A waiting job
    /// that cannot start keeps its `starting` row; nothing here can write it.
    fn exited(self: &Arc<Self>, pid: u32) {
        let mut running = self.lock();
        running.live.remove(&pid);
        while !running.stopping && running.live.len() < self.limit {
            let Some(next) = running.waiting.pop_front() else {
                break;
            };
            let _ = self.launch(&mut running, next);
        }
    }
}

impl JobDispatch for ProcessDispatch {
    fn start(&self, job: JobId) -> Result<(), DispatchError> {
        let mut running = self.shared.lock();
        if running.stopping {
            return Err(DispatchError("the server is stopping".to_string()));
        }
        if running.live.len() >= self.shared.limit {
            running.waiting.push_back(job);
            return Ok(());
        }
        self.shared.launch(&mut running, job)
    }
}

enum Ending {
    /// TERM to the job process alone.
    Term,
    /// KILL to the job's whole process group.
    KillGroup,
}

/// Send `ending` to job `pid`. A job that already exited answers no such
/// process, which is the outcome wanted.
fn signal(pid: u32, ending: Ending) {
    #[cfg(unix)]
    {
        use rustix::process::{Pid, Signal, kill_process, kill_process_group};
        let Some(pid) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
            return;
        };
        let _ = match ending {
            Ending::Term => kill_process(pid, Signal::TERM),
            Ending::KillGroup => kill_process_group(pid, Signal::KILL),
        };
    }
    #[cfg(not(unix))]
    {
        let _ = (pid, ending);
    }
}

#[cfg(all(test, unix))]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::os::unix::fs::PermissionsExt;

    use super::*;

    /// A job program that ignores its arguments and runs for a moment.
    fn sleeper(dir: &std::path::Path) -> JobCommand {
        let program = dir.join("job.sh");
        std::fs::write(&program, "#!/bin/sh\nsleep 0.3\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        JobCommand {
            program: Some(program),
            ..JobCommand::new(dir.join("unused.db"))
        }
    }

    fn counts(dispatch: &ProcessDispatch) -> (usize, usize) {
        let running = dispatch.shared.lock();
        (running.live.len(), running.waiting.len())
    }

    #[test]
    fn jobs_past_the_limit_wait_and_start_as_slots_free() {
        let dir = tempfile::tempdir().unwrap();
        let dispatch = ProcessDispatch::with_limit(sleeper(dir.path()), 1);
        for id in 1..=3 {
            dispatch.start(JobId(id)).unwrap();
        }
        assert_eq!(counts(&dispatch), (1, 2));
        let deadline = Instant::now() + Duration::from_secs(10);
        while counts(&dispatch) != (0, 0) && Instant::now() < deadline {
            assert!(counts(&dispatch).0 <= 1, "never more than the limit");
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(counts(&dispatch), (0, 0), "every waiting job ran");
    }

    #[test]
    fn a_shutdown_returns_the_jobs_that_never_started() {
        let dir = tempfile::tempdir().unwrap();
        let dispatch = ProcessDispatch::with_limit(sleeper(dir.path()), 1);
        for id in 1..=3 {
            dispatch.start(JobId(id)).unwrap();
        }
        let waiting = dispatch.shutdown(Duration::from_secs(5));
        assert_eq!(waiting, [JobId(2), JobId(3)]);
        assert!(dispatch.start(JobId(4)).is_err(), "nothing starts after");
    }
}
