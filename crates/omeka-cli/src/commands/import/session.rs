//! What the import commands share: the database, the job surroundings, and
//! the administrator they act as.

use std::future::Future;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use omeka_api::auth::Actor;
use omeka_api::import2::catalog_load::Owners;
use omeka_api::messages;
use omeka_db::ids::{ImportId, UserId};
use omeka_db::repos::{ImportRepo, JobRepo, UserRepo};
use omeka_db::rows::{ImportRow, NewJob};
use serde_json::{Value, json};
use sqlx::SqlitePool;

use crate::CliError;
use crate::commands::job::job_env;
use crate::jobs::{JobEnv, JobStatus, Runner};

/// The flags every import command takes.
#[derive(Debug, Clone)]
pub struct Where {
    pub database: PathBuf,
    /// `--store`; `files` beside the database when `None`.
    pub store: Option<PathBuf>,
    /// `--archive-root`, the root named `archive`; the configured roots when
    /// `None`.
    pub archive_root: Option<PathBuf>,
}

/// How a command writes what it reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    /// Lines for an operator, progress included.
    #[default]
    Text,
    /// One JSON document when the command ends, and nothing before it.
    Json,
}

/// One job a command runs for an import.
pub(super) struct Work<'a> {
    pub owner: Option<UserId>,
    pub class: &'a str,
    pub import: ImportId,
    /// The undo's `force` argument.
    pub force: Option<Value>,
    /// The summary member that holds the count: the run keeps it at the top,
    /// the undo under `undo`.
    pub member: Option<&'a str>,
}

pub(super) struct Session {
    pub pool: SqlitePool,
    pub env: JobEnv,
}

impl Session {
    pub async fn open(at: &Where) -> Result<Self, CliError> {
        let env = job_env(
            &at.database,
            at.store.as_deref(),
            None,
            at.archive_root.as_deref(),
        )?;
        let pool = omeka_db::open_file(&at.database).await?;
        // A reader records the dead first, as every route does, so the status
        // shown is the one the admin would see.
        ImportRepo::new(&pool).settle_dead().await?;
        Ok(Self { pool, env })
    }

    /// The administrator `email` names, who owns the import and the job.
    pub async fn admin(&self, email: &str) -> Result<Admin, CliError> {
        let user = UserRepo::new(&self.pool)
            .get_by_email(email)
            .await?
            .ok_or_else(|| CliError::message(format!("no user has the email {email}")))?;
        if !Actor::new(user.id, user.role.clone()).is_admin() {
            return Err(CliError::message(messages::IMPORT_ADMINS_ONLY));
        }
        Ok(Admin {
            id: user.id,
            owners: Owners::of(Some(&user.role), Some(user.id)),
        })
    }

    pub async fn import(&self, id: u64) -> Result<ImportRow, CliError> {
        ImportRepo::new(&self.pool)
            .get(ImportId(id))
            .await?
            .ok_or_else(|| CliError::message(messages::import_not_found(id)))
    }

    /// Run `work`'s job in this process, as `romeka job run` would in a
    /// child, and write a progress line to `out` whenever the import's count
    /// moves. The import is already `running` or `undoing`.
    ///
    /// When `stop` resolves, the command asks the job to stop, as the Stop
    /// button does, and goes on waiting: the job ends after the record in hand.
    pub async fn perform(
        &self,
        work: Work<'_>,
        stop: impl Future<Output = ()>,
        out: &mut dyn Write,
    ) -> Result<JobStatus, CliError> {
        let Work {
            owner,
            class,
            import,
            force,
            member,
        } = work;
        let mut args = json!({ "importId": import.0 });
        if let Some(force) = force {
            args["force"] = force;
        }
        let job = JobRepo::new(&self.pool)
            .create(&NewJob {
                owner_id: owner,
                pid: None,
                status: Some("starting".to_string()),
                class: class.to_string(),
                args: Some(args),
                log: None,
                started: None,
            })
            .await?
            .id;
        ImportRepo::new(&self.pool).set_job(import, job).await?;
        let runner = Runner::with_env(&self.pool, self.env.clone());
        let running = runner.run(job);
        tokio::pin!(running);
        tokio::pin!(stop);
        let mut stopping = false;
        let mut ticker = tokio::time::interval(TICK);
        let mut shown = None;
        let ended = loop {
            tokio::select! {
                ended = &mut running => break ended,
                () = &mut stop, if !stopping => {
                    stopping = true;
                    ImportRepo::new(&self.pool).request_stop(import).await?;
                    writeln!(out, "stop requested: the job ends after the record in hand")?;
                }
                _ = ticker.tick() => {
                    let row = self.import(import.0).await?;
                    let count = progress(row.summary.as_ref(), member);
                    if count != shown {
                        if let Some((done, total)) = count {
                            writeln!(out, "progress: {done} of {total}")?;
                        }
                        shown = count;
                    }
                }
            }
        };
        Ok(ended?)
    }
}

/// How often the command looks at the import's count.
const TICK: Duration = Duration::from_millis(500);

fn progress(summary: Option<&Value>, member: Option<&str>) -> Option<(u64, u64)> {
    let report = match member {
        Some(member) => summary?.get(member)?,
        None => summary?,
    };
    let count = report.get("progress")?;
    Some((count.get("done")?.as_u64()?, count.get("total")?.as_u64()?))
}

pub(super) struct Admin {
    pub id: UserId,
    pub owners: Owners,
}
