//! A run that a restart killed goes on by itself (`briefs/import-p6-design.md`,
//! section 3, "Resume at boot").
//!
//! At boot the server waits, for each import that claims to run, until its
//! old worker is proven gone: by the rule every reader trusts
//! (`ImportRepo::settle_dead`: no heartbeat for `DEAD_AFTER`, or its job
//! ended) or by the run's own `interrupted` end. Then it starts each failed
//! run whose worker went without the person asking through the retry path:
//! rescan, derive, check, `ImportRepo::start_resume`, a new job. A run the
//! person stopped never resumes. Each start is the locked transaction a
//! person's retry is, so a repeat, or a second server, starts nothing twice.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use omeka_api::import::archive::ArchiveRoots;
use omeka_api::import::catalog_load::Owners;
use omeka_api::import::preview::Caches;
use omeka_api::import::run::RUN_JOB;
use omeka_api::import::start::{self, Go, Refusal, StartError};
use omeka_db::DbPool;
use omeka_db::ids::{ImportId, JobId};
use omeka_db::repos::{ImportRepo, JobRepo, UserRepo};
use omeka_db::with_db;
use omeka_media::store::Store;

use super::run::import_job;
use crate::JobDispatch;

/// What a resume reads besides the database: what a run route reads.
pub struct ResumeEnv {
    pub jobs: Arc<dyn JobDispatch>,
    pub store: Arc<dyn Store>,
    pub roots: ArchiveRoots,
}

/// Resume every run a restart killed, then return once no import that ran at
/// the call is still unproven. `every` is how long it waits between looks;
/// `HEARTBEAT_EVERY` in the server, shorter in a test. A run that cannot
/// start now because another import holds the slot waits for the next boot.
pub async fn resume_imports(pool: &DbPool, env: &ResumeEnv, every: Duration) {
    let mut waiting: Vec<(ImportId, Option<JobId>)> =
        match with_db!(pool, |db| ImportRepo::new(db).running().await) {
            Ok(running) => running,
            Err(error) => {
                tracing::warn!("cannot read the running imports: {error}");
                return;
            }
        };
    let mut tried = HashSet::new();
    loop {
        if let Err(error) = with_db!(pool, |db| ImportRepo::new(db).settle_dead().await) {
            tracing::warn!("cannot settle dead imports: {error}");
        }
        let resumable =
            with_db!(pool, |db| ImportRepo::new(db).resumable().await).unwrap_or_else(|error| {
                tracing::warn!("cannot read the resumable imports: {error}");
                Vec::new()
            });
        for import in resumable {
            if tried.contains(&import) {
                continue;
            }
            match resume(pool, env, import).await {
                Ok(job) => {
                    tracing::info!(import = import.0, job = job.0, "resumed the import");
                    tried.insert(import);
                }
                Err(Resume::Later) => {}
                Err(Resume::Not(reason)) => {
                    tracing::warn!(import = import.0, "the import was not resumed: {reason}");
                    tried.insert(import);
                }
            }
        }
        let running = with_db!(pool, |db| ImportRepo::new(db).running().await).unwrap_or_default();
        waiting.retain(|claim| running.contains(claim));
        if waiting.is_empty() {
            return;
        }
        tokio::time::sleep(every).await;
    }
}

enum Resume {
    /// Another import holds the slot.
    Later,
    Not(String),
}

/// Start import `import` again under a new job owned by whoever ran it.
async fn resume(pool: &DbPool, env: &ResumeEnv, import: ImportId) -> Result<JobId, Resume> {
    let not = |error: &dyn std::fmt::Display| Resume::Not(error.to_string());
    let row = with_db!(pool, |db| ImportRepo::new(db).get(import).await)
        .map_err(|error| not(&error))?
        .ok_or_else(|| Resume::Not("it is gone".to_string()))?;
    let plan = super::run::stored_plan(&row).map_err(|error| Resume::Not(format!("{error:?}")))?;
    let owner = match row.job_id {
        Some(job) => with_db!(pool, |db| JobRepo::new(db).get(job).await)
            .map_err(|error| not(&error))?
            .and_then(|job| job.owner_id),
        None => None,
    };
    let role = match owner {
        Some(user) => with_db!(pool, |db| UserRepo::new(db).get(user).await)
            .map_err(|error| not(&error))?
            .map(|user| user.role),
        None => None,
    };
    let files = omeka_api::settings::file_validation(pool)
        .await
        .map_err(|error| not(&error))?;
    let caches = Arc::new(Caches::default());
    let assessed = with_db!(pool, |db| start::assess(
        db,
        &env.roots,
        &env.store,
        &caches,
        files,
        Owners::of(role.as_deref(), owner),
        row.id,
        plan,
    )
    .await)
    .map_err(|error| not(&error))?;
    let go = Go {
        resume: true,
        ..Go::default()
    };
    match with_db!(pool, |db| start::begin(db, &row, &assessed.check, go).await) {
        Ok(()) => {}
        Err(StartError::Refused(Refusal::Busy(_))) => return Err(Resume::Later),
        Err(error) => return Err(not(&error)),
    }
    let job = import_job(pool, owner, RUN_JOB, row.id, None)
        .await
        .map_err(|error| not(&error))?;
    if let Err(error) = env.jobs.start(job) {
        let _ = with_db!(pool, |db| JobRepo::new(db)
            .append_log(job, &error.to_string())
            .await);
        let _ = with_db!(pool, |db| JobRepo::new(db).finish(job, "error").await);
        return Err(not(&error));
    }
    Ok(job)
}
