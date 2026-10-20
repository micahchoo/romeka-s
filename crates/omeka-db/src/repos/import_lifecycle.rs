//! When an import's job is alive, and the two starts that depend on it.
//!
//! A job process that dies (killed, container restart) cannot write its ending, so `running` or
//! `undoing` alone does not prove that a worker is there. The worker proves it with a heartbeat:
//! the time it last wrote, kept in the summary JSON as [`HEARTBEAT`]. An import that claims to run
//! is dead when its job row has ended, or when its newest evidence of life (the heartbeat, else its
//! job's start) is older than [`DEAD_AFTER`]. The first reader that finds it dead records that
//! ([`ImportRepo::settle_dead`]): job `error`, import `failed` with `finished` and the summary's
//! [`DIED`]. After that, every reader sees stored states only.
//!
//! [`ImportRepo::start_run`] and [`ImportRepo::start_undo`] are one serialized transaction each
//! ([`Backend::begin_write`], [`Backend::for_update`]): settle the dead, check this import's state,
//! refuse while another import runs or undoes, then move it. Two concurrent starts cannot both win.

use std::time::Duration;

use chrono::{NaiveDateTime, TimeDelta};
use serde_json::{Map, Value};
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Transaction, Type};

use super::{ImportRepo, bind_id, json_text};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{ImportId, JobId};
use crate::rows::ImportStatus;
use crate::time;

/// How long a running or undoing import may go without a heartbeat before it counts as dead.
pub const DEAD_AFTER: TimeDelta = TimeDelta::minutes(2);

/// How often a working job writes its heartbeat: well inside [`DEAD_AFTER`].
pub const HEARTBEAT_EVERY: Duration = Duration::from_secs(10);

/// The summary member that holds the worker's last heartbeat, in the stored datetime format.
pub const HEARTBEAT: &str = "heartbeat";

/// The summary member that records when a reader found the import's worker dead.
pub const DIED: &str = "died";

/// The job statuses of a worker that may still write: upstream's `STATUS_STARTING`,
/// `STATUS_IN_PROGRESS` and `STATUS_STOPPING`.
const LIVE_JOB: [&str; 3] = ["starting", "in_progress", "stopping"];

/// What a start did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartOutcome {
    /// The import moved to `running` or `undoing`.
    Started,
    /// No import has this id.
    Missing,
    /// This import's state does not allow the start; nothing changed.
    Refused {
        status: ImportStatus,
        undoable: bool,
    },
    /// Another import is running or undoing with a live worker; nothing changed.
    Busy(ImportId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Start {
    Run,
    Undo,
}

/// Whether the worker of a running or undoing import is gone. `job` is its job row's status
/// (`None` with no job row yet), `pulse` its newest evidence of life.
fn is_dead(job: Option<&str>, pulse: Option<NaiveDateTime>, now: NaiveDateTime) -> bool {
    job.is_some_and(|status| !LIVE_JOB.contains(&status))
        || pulse.is_none_or(|beat| now - beat > DEAD_AFTER)
}

fn heartbeat_of(summary: Option<&Value>) -> Option<NaiveDateTime> {
    summary?
        .get(HEARTBEAT)?
        .as_str()
        .and_then(|stored| time::parse(stored).ok())
}

/// `summary` as an object with `key` set to `value`. A missing or non-object summary starts empty.
pub(crate) fn with_member(summary: Option<Value>, key: &str, value: Value) -> Value {
    let mut members = match summary {
        Some(Value::Object(members)) => members,
        _ => Map::new(),
    };
    members.insert(key.to_string(), value);
    Value::Object(members)
}

/// An import that claims to run, with what its worker left behind.
struct Claim {
    id: ImportId,
    job: Option<JobId>,
    job_status: Option<String>,
    pulse: Option<NaiveDateTime>,
    summary: Option<Value>,
}

impl Claim {
    fn is_dead(&self, now: NaiveDateTime) -> bool {
        is_dead(self.job_status.as_deref(), self.pulse, now)
    }
}

impl<DB: Backend> ImportRepo<'_, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    /// Move a draft to `running` and give it the one open undo window, so a partial run can be
    /// undone. Refused unless the import is a draft and no other import is live.
    pub async fn start_run(&self, id: ImportId) -> Result<StartOutcome, DbError> {
        self.start(id, Start::Run).await
    }

    /// Move an import that holds the open window and has ended (`done` or `failed`) to
    /// `undoing`. Refused otherwise, or while another import is live.
    pub async fn start_undo(&self, id: ImportId) -> Result<StartOutcome, DbError> {
        self.start(id, Start::Undo).await
    }

    /// Record every dead running or undoing import as failed, and its job as ended `error`.
    /// The ids it settled; repeating it finds nothing more to do.
    pub async fn settle_dead(&self) -> Result<Vec<ImportId>, DbError> {
        let now = time::now();
        let claims = claims(self.pool, "").await?;
        if !claims.iter().any(|claim| claim.is_dead(now)) {
            return Ok(Vec::new());
        }
        let mut tx = self.pool.begin_with(DB::begin_write()).await?;
        lock_imports(&mut tx).await?;
        let settled = settle(&mut tx, now).await?;
        tx.commit().await?;
        Ok(settled)
    }

    /// Write the heartbeat of `job`, the worker of `id`. False when the import no longer runs or
    /// undoes under this job, for example because a reader found it dead and a new undo started:
    /// the worker should then stop.
    pub async fn heartbeat(&self, id: ImportId, job: JobId) -> Result<bool, DbError> {
        let done = sqlx::query(
            "UPDATE romeka_import SET summary = json_set(COALESCE(summary, '{}'), '$.heartbeat', ?)
             WHERE id = ? AND job_id = ? AND status IN ('running', 'undoing')",
        )
        .bind(time::format(time::now()))
        .bind(bind_id(id))
        .bind(bind_id(job))
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    async fn start(&self, id: ImportId, start: Start) -> Result<StartOutcome, DbError> {
        let now = time::now();
        let mut tx = self.pool.begin_with(DB::begin_write()).await?;
        lock_imports(&mut tx).await?;
        settle(&mut tx, now).await?;
        let outcome = decide(&mut tx, id, start, now).await?;
        tx.commit().await?;
        Ok(outcome)
    }
}

/// Lock every import row, so concurrent starts take turns. SQLite already holds its write lock.
async fn lock_imports<DB: Backend>(tx: &mut Transaction<'static, DB>) -> Result<(), DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
{
    sqlx::query(&format!("SELECT id FROM romeka_import{}", DB::for_update()))
        .fetch_all(&mut **tx)
        .await?;
    Ok(())
}

async fn claims<'e, DB, E>(executor: E, lock: &str) -> Result<Vec<Claim>, DbError>
where
    DB: Backend,
    E: Executor<'e, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
{
    let rows = sqlx::query(&format!(
        "SELECT i.id, i.job_id, i.summary, j.status AS job_status, j.started AS job_started
         FROM romeka_import i LEFT JOIN job j ON j.id = i.job_id
         WHERE i.status IN ('running', 'undoing'){lock}"
    ))
    .fetch_all(executor)
    .await?;
    rows.iter()
        .map(|row| {
            let summary = DB::opt_json(row, "summary")?;
            let pulse = heartbeat_of(summary.as_ref()).or(DB::opt_datetime(row, "job_started")?);
            Ok(Claim {
                id: DB::id(row, "id")?,
                job: DB::opt_id(row, "job_id")?,
                job_status: DB::opt_text(row, "job_status")?,
                pulse,
                summary,
            })
        })
        .collect()
}

async fn settle<DB: Backend>(
    tx: &mut Transaction<'static, DB>,
    now: NaiveDateTime,
) -> Result<Vec<ImportId>, DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'x> i64: Encode<'x, DB> + Type<DB>,
    for<'x> String: Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB>,
{
    let stamp = time::format(now);
    let mut settled = Vec::new();
    for claim in claims(&mut **tx, DB::for_update()).await? {
        if !claim.is_dead(now) {
            continue;
        }
        let summary = with_member(claim.summary, DIED, Value::String(stamp.clone()));
        sqlx::query(
            "UPDATE romeka_import SET status = 'failed', finished = ?, summary = ? WHERE id = ?",
        )
        .bind(stamp.clone())
        .bind(json_text(&summary)?)
        .bind(bind_id(claim.id))
        .execute(&mut **tx)
        .await?;
        if let Some(job) = claim.job
            && claim
                .job_status
                .as_deref()
                .is_some_and(|status| LIVE_JOB.contains(&status))
        {
            let since = claim
                .pulse
                .map_or_else(|| "its start".to_string(), time::format);
            sqlx::query("UPDATE job SET status = 'error', ended = ? WHERE id = ?")
                .bind(stamp.clone())
                .bind(bind_id(job))
                .execute(&mut **tx)
                .await?;
            sqlx::query(DB::append_job_log())
                .bind(format!(
                    "The job process stopped: no heartbeat since {since}. The import is failed."
                ))
                .bind(bind_id(job))
                .execute(&mut **tx)
                .await?;
        }
        settled.push(claim.id);
    }
    Ok(settled)
}

async fn decide<DB: Backend>(
    tx: &mut Transaction<'static, DB>,
    id: ImportId,
    start: Start,
    now: NaiveDateTime,
) -> Result<StartOutcome, DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'x> i64: Encode<'x, DB> + Type<DB>,
    for<'x> String: Encode<'x, DB> + Type<DB>,
{
    let lock = DB::for_update();
    let Some(row) = sqlx::query(&format!(
        "SELECT status, undoable, summary FROM romeka_import WHERE id = ?{lock}"
    ))
    .bind(bind_id(id))
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok(StartOutcome::Missing);
    };
    let status = ImportStatus::parse(&DB::text(&row, "status")?)?;
    let undoable = DB::boolean(&row, "undoable")?;
    let allowed = match start {
        Start::Run => status == ImportStatus::Draft,
        Start::Undo => undoable && matches!(status, ImportStatus::Done | ImportStatus::Failed),
    };
    if !allowed {
        return Ok(StartOutcome::Refused { status, undoable });
    }
    let busy = sqlx::query(&format!(
        "SELECT id FROM romeka_import WHERE status IN ('running', 'undoing') AND id <> ?{lock}"
    ))
    .bind(bind_id(id))
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(busy) = busy {
        return Ok(StartOutcome::Busy(DB::id(&busy, "id")?));
    }
    let mut summary = DB::opt_json(&row, "summary")?;
    if let Some(Value::Object(members)) = &mut summary {
        members.remove(DIED);
    }
    let summary = with_member(summary, HEARTBEAT, Value::String(time::format(now)));
    if start == Start::Run {
        sqlx::query("UPDATE romeka_import SET undoable = 0 WHERE undoable = 1 AND id <> ?")
            .bind(bind_id(id))
            .execute(&mut **tx)
            .await?;
    }
    let next = match start {
        Start::Run => ImportStatus::Running,
        Start::Undo => ImportStatus::Undoing,
    };
    sqlx::query("UPDATE romeka_import SET status = ?, undoable = 1, summary = ? WHERE id = ?")
        .bind(next.as_str().to_string())
        .bind(json_text(&summary)?)
        .bind(bind_id(id))
        .execute(&mut **tx)
        .await?;
    Ok(StartOutcome::Started)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(stored: &str) -> NaiveDateTime {
        time::parse(stored).unwrap_or_default()
    }

    #[test]
    fn a_worker_is_dead_once_its_job_ends_or_its_pulse_is_two_minutes_old() {
        let now = at("2026-10-19 12:00:00");
        let fresh = Some(at("2026-10-19 11:58:00"));
        let stale = Some(at("2026-10-19 11:57:59"));
        for live in LIVE_JOB {
            assert!(
                !is_dead(Some(live), fresh, now),
                "{live} with a fresh pulse"
            );
            assert!(is_dead(Some(live), stale, now), "{live} with a stale pulse");
        }
        assert!(!is_dead(None, fresh, now), "no job row yet, fresh pulse");
        for ended in ["error", "stopped", "completed"] {
            assert!(is_dead(Some(ended), fresh, now), "{ended}");
        }
        assert!(
            is_dead(Some("in_progress"), None, now),
            "no evidence of life"
        );
    }

    #[test]
    fn the_heartbeat_is_read_from_the_summary_in_the_stored_format() {
        let summary = serde_json::json!({ "heartbeat": "2026-10-19 12:00:00", "items": 1 });
        assert_eq!(
            heartbeat_of(Some(&summary)),
            Some(at("2026-10-19 12:00:00"))
        );
        assert_eq!(
            heartbeat_of(Some(&serde_json::json!({ "heartbeat": 5 }))),
            None
        );
        assert_eq!(heartbeat_of(None), None);
    }
}
