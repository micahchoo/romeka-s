//! `job` rows and the writes the job runner makes to them.
//!
//! Upstream splits the runner across `perform-job.php` and the sync/async job strategies: a job is
//! composed in `starting`, `start` records the process and moves it to `in_progress`, the runner
//! appends to `log`, polls `status` for a `stopping` request, and `finish` writes the final status
//! and `ended` (`application/src/Job/Dispatcher.php`, `application/bin/perform-job.php`).
//!
//! The list filter mirrors `JobAdapter::buildQuery`: `class` and `status`, both exact.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::{
    Paging, Statement, bind_id, bind_opt_id, fetch_rows, insert_id, json_text, push_paging,
};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{JobId, UserId};
use crate::rows::{JobRow, NewJob};
use crate::time;

/// A list filter for `job`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JobFilter {
    pub owner_id: Option<UserId>,
    pub class: Option<String>,
    pub status: Option<String>,
}

/// `job` rows.
pub struct JobRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> JobRepo<'a, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
    for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
    for<'x> f64: Decode<'x, DB> + Type<DB>,
{
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    /// Compose a row, `starting` when the caller passes that status. `started` defaults to now.
    pub async fn create(&self, new: &NewJob) -> Result<JobRow, DbError> {
        let started = new.started.unwrap_or_else(time::now);
        let query = sqlx::query(
            "INSERT INTO job (owner_id, pid, status, class, args, log, started, ended)
             VALUES (?, ?, ?, ?, ?, ?, ?, NULL)",
        )
        .bind(bind_opt_id(new.owner_id))
        .bind(&new.pid)
        .bind(&new.status)
        .bind(&new.class)
        .bind(new.args.as_ref().map(json_text).transpose()?)
        .bind(&new.log)
        .bind(time::format(started));
        let id = insert_id(query, self.pool).await?;
        Ok(JobRow {
            id: JobId::from(id),
            owner_id: new.owner_id,
            pid: new.pid.clone(),
            status: new.status.clone(),
            class: new.class.clone(),
            args: new.args.clone(),
            log: new.log.clone(),
            started,
            ended: None,
        })
    }

    pub async fn get(&self, id: JobId) -> Result<Option<JobRow>, DbError> {
        let row = sqlx::query("SELECT * FROM job WHERE id = ?")
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(JobRow::from_db_row::<DB>).transpose()
    }

    /// The stored status, which the runner polls for a stop request.
    pub async fn status(&self, id: JobId) -> Result<Option<String>, DbError> {
        Ok(self.get(id).await?.and_then(|row| row.status))
    }

    pub async fn list(&self, filter: &JobFilter, paging: &Paging) -> Result<Vec<JobRow>, DbError> {
        let mut statement = Statement::<DB>::new("SELECT * FROM job WHERE 1 = 1");
        if let Some(owner_id) = filter.owner_id {
            statement.push(" AND owner_id = ");
            statement.bind(bind_id(owner_id))?;
        }
        if let Some(class) = &filter.class {
            statement.push(" AND class = ");
            statement.bind(class)?;
        }
        if let Some(status) = &filter.status {
            statement.push(" AND status = ");
            statement.bind(status)?;
        }
        statement.push(" ORDER BY id");
        push_paging(&mut statement, paging)?;
        let (sql, arguments) = statement.into_parts();
        let rows: Vec<DB::Row> = fetch_rows(&sql, arguments, self.pool).await?;
        rows.iter().map(JobRow::from_db_row::<DB>).collect()
    }

    /// `perform-job.php`: record the running process and enter the job.
    pub async fn start(&self, id: JobId, pid: &str) -> Result<bool, DbError> {
        let done = sqlx::query("UPDATE job SET pid = ?, status = 'in_progress' WHERE id = ?")
            .bind(pid)
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Clear the process id once the run ends, as `perform-job.php` does.
    pub async fn clear_pid(&self, id: JobId) -> Result<bool, DbError> {
        let done = sqlx::query("UPDATE job SET pid = NULL WHERE id = ?")
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Set a status without ending the job (`in_progress`, or `stopping`).
    pub async fn set_status(&self, id: JobId, status: &str) -> Result<bool, DbError> {
        let done = sqlx::query("UPDATE job SET status = ? WHERE id = ?")
            .bind(status)
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// `Dispatcher::stop`'s one transition: an `in_progress` job becomes `stopping`. `false` when
    /// the job is not running, so the caller can refuse the request.
    pub async fn request_stop(&self, id: JobId) -> Result<bool, DbError> {
        let done = sqlx::query(
            "UPDATE job SET status = 'stopping' WHERE id = ? AND status = 'in_progress'",
        )
        .bind(bind_id(id))
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Set the final status and the `ended` timestamp.
    pub async fn finish(&self, id: JobId, status: &str) -> Result<bool, DbError> {
        let done = sqlx::query("UPDATE job SET status = ?, ended = ? WHERE id = ?")
            .bind(status)
            .bind(time::format(time::now()))
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Replace the `args` JSON. Upstream's jobs only read their args; a Romeka job that keeps a
    /// result keeps it here, beside the arguments it ran with, so the result goes when the job does.
    pub async fn set_args(&self, id: JobId, args: &serde_json::Value) -> Result<bool, DbError> {
        let done = sqlx::query("UPDATE job SET args = ? WHERE id = ?")
            .bind(json_text(args)?)
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Append one line to the log, as `Entity\Job::addLog` does: the line then a newline, after
    /// whatever is stored. The concatenation spelling comes from the backend.
    pub async fn append_log(&self, id: JobId, line: &str) -> Result<bool, DbError> {
        let done = sqlx::query(DB::append_job_log())
            .bind(line)
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn delete(&self, id: JobId) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM job WHERE id = ?")
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Whether a job of `class` is composed or running: `starting`, `in_progress` or `stopping`.
    ///
    /// The derivative queue reads this to keep one worker per install, instead of starting a
    /// process for every media a page creates.
    pub async fn active_of_class(&self, class: &str) -> Result<bool, DbError> {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM job
             WHERE class = ? AND status IN ('starting', 'in_progress', 'stopping')",
        )
        .bind(class)
        .fetch_one(self.pool)
        .await?;
        Ok(count > 0)
    }

    /// Every job that is composed or running (`starting`, `in_progress` or `stopping`), of `class`
    /// when one is named, in id order.
    pub async fn active(&self, class: Option<&str>) -> Result<Vec<JobRow>, DbError> {
        let mut statement = Statement::<DB>::new(
            "SELECT * FROM job WHERE status IN ('starting', 'in_progress', 'stopping')",
        );
        if let Some(class) = class {
            statement.push(" AND class = ");
            statement.bind(class.to_string())?;
        }
        statement.push(" ORDER BY id");
        let (sql, arguments) = statement.into_parts();
        let rows: Vec<DB::Row> = fetch_rows(&sql, arguments, self.pool).await?;
        rows.iter().map(JobRow::from_db_row::<DB>).collect()
    }

    /// End a job that can no longer finish as `error`, with `why` as its last log line: only while
    /// it is still composed or running and still records `pid`, so a job that ended or was taken
    /// up by another process since the caller read it is left alone. False when nothing changed.
    pub async fn settle(&self, id: JobId, pid: Option<&str>, why: &str) -> Result<bool, DbError> {
        const SETTLE: &str = "UPDATE job SET status = 'error', ended = ?, pid = NULL
             WHERE id = ? AND status IN ('starting', 'in_progress', 'stopping') AND ";
        let ended = time::format(time::now());
        let done = match pid {
            Some(pid) => {
                sqlx::query(&format!("{SETTLE}pid = ?"))
                    .bind(ended)
                    .bind(bind_id(id))
                    .bind(pid)
                    .execute(self.pool)
                    .await?
            }
            None => {
                sqlx::query(&format!("{SETTLE}pid IS NULL"))
                    .bind(ended)
                    .bind(bind_id(id))
                    .execute(self.pool)
                    .await?
            }
        };
        if DB::rows_affected(&done) == 0 {
            return Ok(false);
        }
        self.append_log(id, why).await
    }

    /// Delete the job unless an archive import that is running or undoing holds it. One statement,
    /// so no import can start between the check and the delete. False when the job is missing or held.
    pub async fn delete_unless_held(&self, id: JobId) -> Result<bool, DbError> {
        let done = sqlx::query(
            "DELETE FROM job WHERE id = ? AND NOT EXISTS (
                SELECT 1 FROM romeka_import
                WHERE job_id = job.id AND status IN ('running', 'undoing'))",
        )
        .bind(bind_id(id))
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }
}
