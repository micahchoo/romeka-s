//! Raw SQL for the tables `omeka-db` has row structs for but no repository:
//! `setting`, `job`, `fulltext_search` and `item_site`.
//!
//! `DESIGN.md` says SQL strings live in `omeka-db` only. This unit may not edit
//! another crate, and the db unit shipped these rows without repositories
//! (`crates/omeka-db/src/repos/`), so the cli carries the smallest SQL needed to
//! run jobs. The proposed `omeka-db` API that would delete this module is in the
//! unit report. Everything here takes and returns plain values; the repositories
//! still own resource, value and vocabulary SQL.

use chrono::{NaiveDateTime, Timelike, Utc};
use serde_json::Value;
use sqlx::SqlitePool;

use omeka_db::DbError;
use omeka_db::ids::{ItemId, JobId, SiteId, UserId};
use omeka_db::rows::JobRow;

/// Upstream's datetime storage format, matching `omeka-db/src/time.rs`.
const DATETIME: &str = "%Y-%m-%d %H:%M:%S";

fn now() -> NaiveDateTime {
    let current = Utc::now().naive_utc();
    current.with_nanosecond(0).unwrap_or(current)
}

fn stamp(value: NaiveDateTime) -> String {
    value.format(DATETIME).to_string()
}

fn encode(value: &Value) -> Result<String, DbError> {
    serde_json::to_string(value).map_err(|error| DbError::Column {
        column: "json",
        reason: format!("cannot store as JSON: {error}"),
    })
}

/// One `setting` value, or `None` when the setting was never written.
///
/// # Errors
/// A `DbError` for a SQL failure, or `DbError::Column` when a stored value is not JSON.
pub async fn read_setting(pool: &SqlitePool, id: &str) -> Result<Option<Value>, DbError> {
    let stored: Option<String> = sqlx::query_scalar("SELECT value FROM setting WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    stored
        .map(|text| {
            serde_json::from_str(&text).map_err(|error| DbError::Column {
                column: "setting.value",
                reason: format!("`{text}` is not JSON: {error}"),
            })
        })
        .transpose()
}

/// Read an integer setting, as upstream `Settings::get` returns its stored value.
pub async fn read_setting_i64(pool: &SqlitePool, id: &str) -> Result<Option<i64>, DbError> {
    match read_setting(pool, id).await? {
        Some(Value::Number(number)) => Ok(number.as_i64()),
        Some(Value::String(text)) => Ok(text.parse().ok()),
        _ => Ok(None),
    }
}

/// Insert or replace a setting, as upstream `Settings::set` does.
pub async fn write_setting(pool: &SqlitePool, id: &str, value: &Value) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO setting (id, value) VALUES (?, ?)
         ON CONFLICT (id) DO UPDATE SET value = excluded.value",
    )
    .bind(id)
    .bind(encode(value)?)
    .execute(pool)
    .await?;
    Ok(())
}

/// Compose a `job` row in `starting` status, as `Dispatcher::dispatch` does.
pub async fn job_create(
    pool: &SqlitePool,
    class: &str,
    args: &Value,
    owner_id: Option<UserId>,
) -> Result<JobId, DbError> {
    let raw: i64 = sqlx::query_scalar(
        "INSERT INTO job (owner_id, status, class, args, started)
         VALUES (?, 'starting', ?, ?, ?) RETURNING id",
    )
    .bind(owner_id.map(|id| id.0 as i64))
    .bind(class)
    .bind(encode(args)?)
    .bind(stamp(now()))
    .fetch_one(pool)
    .await?;
    u64::try_from(raw)
        .map(JobId::from)
        .map_err(|_| DbError::Column {
            column: "job.id",
            reason: format!("insert returned id {raw}"),
        })
}

/// The job's stored status, which `shouldStop` polls.
pub async fn job_status(pool: &SqlitePool, id: JobId) -> Result<Option<String>, DbError> {
    let status = sqlx::query_scalar("SELECT status FROM job WHERE id = ?")
        .bind(id.0 as i64)
        .fetch_optional(pool)
        .await?;
    Ok(status)
}

/// Set a job status without ending it (`in_progress`, or `stopping`).
pub async fn job_set_status(pool: &SqlitePool, id: JobId, status: &str) -> Result<(), DbError> {
    sqlx::query("UPDATE job SET status = ? WHERE id = ?")
        .bind(status)
        .bind(id.0 as i64)
        .execute(pool)
        .await?;
    Ok(())
}

/// Record the running process and `in_progress`, the two writes
/// `perform-job.php` makes before it enters the job.
pub async fn job_start(pool: &SqlitePool, id: JobId, pid: u32) -> Result<(), DbError> {
    sqlx::query("UPDATE job SET pid = ?, status = 'in_progress' WHERE id = ?")
        .bind(pid.to_string())
        .bind(id.0 as i64)
        .execute(pool)
        .await?;
    Ok(())
}

/// Clear the pid once the process ends, as `perform-job.php` does.
pub async fn job_clear_pid(pool: &SqlitePool, id: JobId) -> Result<(), DbError> {
    sqlx::query("UPDATE job SET pid = NULL WHERE id = ?")
        .bind(id.0 as i64)
        .execute(pool)
        .await?;
    Ok(())
}

/// Request a stop, the one transition `Dispatcher::stop` allows: an
/// `in_progress` job becomes `stopping`. Returns whether a row changed.
pub async fn job_request_stop(pool: &SqlitePool, id: JobId) -> Result<bool, DbError> {
    let done =
        sqlx::query("UPDATE job SET status = 'stopping' WHERE id = ? AND status = 'in_progress'")
            .bind(id.0 as i64)
            .execute(pool)
            .await?;
    Ok(done.rows_affected() > 0)
}

/// One `job` row, or `None` when no row has that id.
pub async fn job_get(pool: &SqlitePool, id: JobId) -> Result<Option<JobRow>, DbError> {
    let row = sqlx::query(
        "SELECT id, owner_id, pid, status, class, args, log, started, ended FROM job WHERE id = ?",
    )
    .bind(id.0 as i64)
    .fetch_optional(pool)
    .await?;
    row.map(|row| JobRow::from_row(&row)).transpose()
}

/// Every `job` row, oldest first, the order the admin browse page lists.
pub async fn job_list(pool: &SqlitePool) -> Result<Vec<JobRow>, DbError> {
    let rows = sqlx::query(
        "SELECT id, owner_id, pid, status, class, args, log, started, ended FROM job ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    rows.iter().map(JobRow::from_row).collect()
}

/// Set the final status and the `ended` timestamp, as the synchronous strategy does.
pub async fn job_finish(pool: &SqlitePool, id: JobId, status: &str) -> Result<(), DbError> {
    sqlx::query("UPDATE job SET status = ?, ended = ? WHERE id = ?")
        .bind(status)
        .bind(stamp(now()))
        .bind(id.0 as i64)
        .execute(pool)
        .await?;
    Ok(())
}

/// Append one line to the job log, matching upstream `Entity\Job::addLog`,
/// which appends the line and a newline to whatever is already stored.
pub async fn job_append_log(pool: &SqlitePool, id: JobId, line: &str) -> Result<(), DbError> {
    sqlx::query("UPDATE job SET log = COALESCE(log, '') || ? || char(10) WHERE id = ?")
        .bind(line)
        .bind(id.0 as i64)
        .execute(pool)
        .await?;
    Ok(())
}

/// One row of `fulltext_search`, ready to insert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FulltextRow {
    pub id: u64,
    pub resource: String,
    pub owner_id: Option<u64>,
    pub is_public: bool,
    pub title: Option<String>,
    pub text: Option<String>,
}

/// `DELETE FROM fulltext_search`, the first step of `IndexFulltextSearch`.
pub async fn fulltext_clear(pool: &SqlitePool) -> Result<(), DbError> {
    sqlx::query("DELETE FROM fulltext_search")
        .execute(pool)
        .await?;
    Ok(())
}

/// Upsert one fulltext row, matching `FulltextSearch::save`.
pub async fn fulltext_save(pool: &SqlitePool, row: &FulltextRow) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO fulltext_search (id, resource, owner_id, is_public, title, text)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT (id, resource) DO UPDATE SET
             owner_id = excluded.owner_id, is_public = excluded.is_public,
             title = excluded.title, text = excluded.text",
    )
    .bind(row.id as i64)
    .bind(&row.resource)
    .bind(row.owner_id.map(|id| id as i64))
    .bind(row.is_public)
    .bind(&row.title)
    .bind(&row.text)
    .execute(pool)
    .await?;
    Ok(())
}

/// Remove every `item_site` row for a site, upstream's `remove_all` step.
pub async fn item_site_clear(pool: &SqlitePool, site: SiteId) -> Result<(), DbError> {
    sqlx::query("DELETE FROM item_site WHERE site_id = ?")
        .bind(site.0 as i64)
        .execute(pool)
        .await?;
    Ok(())
}

/// Attach an item to a site, ignoring a duplicate as upstream `INSERT IGNORE` does.
pub async fn item_site_add(pool: &SqlitePool, item: ItemId, site: SiteId) -> Result<(), DbError> {
    sqlx::query("INSERT OR IGNORE INTO item_site (item_id, site_id) VALUES (?, ?)")
        .bind(item.0 as i64)
        .bind(site.0 as i64)
        .execute(pool)
        .await?;
    Ok(())
}

/// Detach an item from a site.
pub async fn item_site_remove(
    pool: &SqlitePool,
    item: ItemId,
    site: SiteId,
) -> Result<bool, DbError> {
    let done = sqlx::query("DELETE FROM item_site WHERE item_id = ? AND site_id = ?")
        .bind(item.0 as i64)
        .bind(site.0 as i64)
        .execute(pool)
        .await?;
    Ok(done.rows_affected() > 0)
}
