//! What the import commands share: the database, the job surroundings, and
//! the administrator they act as.

use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use omeka_api::auth::Actor;
use omeka_api::import2::catalog_load::Owners;
use omeka_api::messages;
use omeka_db::DbPool;
use omeka_db::Location;
use omeka_db::ids::{ImportId, UserId};
use omeka_db::repos::{ImportRepo, JobRepo, UserRepo};
use omeka_db::rows::{ImportRow, NewJob};
use omeka_db::with_db;
use serde_json::{Value, json};

use crate::CliError;
use crate::commands::job::job_env;
use crate::jobs::{JobEnv, JobStatus, Runner};

/// The flags every import command takes.
#[derive(Debug, Clone)]
pub struct Where {
    pub database: Location,
    /// `--store`; `files` beside the database when `None`.
    pub store: Option<PathBuf>,
    /// `--archive-root`, the root named `archive`; the configured roots when
    /// `None`.
    pub archive_root: Option<PathBuf>,
}

pub(super) struct Session {
    pub pool: DbPool,
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
        let pool = at.database.open().await?;
        // A reader records the dead first, as every route does, so the status
        // shown is the one the admin would see.
        with_db!(&pool, |db| ImportRepo::new(db).settle_dead().await)?;
        Ok(Self { pool, env })
    }

    /// The administrator `email` names, who owns the import and the job.
    pub async fn admin(&self, email: &str) -> Result<Admin, CliError> {
        let user = with_db!(&self.pool, |db| UserRepo::new(db).get_by_email(email).await)?
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
        with_db!(&self.pool, |db| ImportRepo::new(db).get(ImportId(id)).await)?
            .ok_or_else(|| CliError::message(messages::import_not_found(id)))
    }

    /// Run the job of `class` for `import` in this process, as `romeka job
    /// run` would in a child, and write a progress line to `out` whenever the
    /// import's count moves. The import is already `running` or `undoing`.
    /// `member` names the summary member that holds the count: the run keeps
    /// it at the top, the undo under `undo`.
    pub async fn perform(
        &self,
        owner: Option<UserId>,
        class: &str,
        import: ImportId,
        force: Option<Value>,
        member: Option<&str>,
        out: &mut dyn Write,
    ) -> Result<JobStatus, CliError> {
        let mut args = json!({ "importId": import.0 });
        if let Some(force) = force {
            args["force"] = force;
        }
        let job = with_db!(&self.pool, |db| JobRepo::new(db)
            .create(&NewJob {
                owner_id: owner,
                pid: None,
                status: Some("starting".to_string()),
                class: class.to_string(),
                args: Some(args),
                log: None,
                started: None,
            })
            .await)?
        .id;
        with_db!(&self.pool, |db| ImportRepo::new(db)
            .set_job(import, job)
            .await)?;
        let runner = Runner::with_env(&self.pool, self.env.clone());
        let work = runner.run(job);
        tokio::pin!(work);
        let mut ticker = tokio::time::interval(TICK);
        let mut shown = None;
        let ended = loop {
            tokio::select! {
                ended = &mut work => break ended,
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
