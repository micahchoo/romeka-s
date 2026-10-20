//! `romeka_import` and `romeka_import_resource` repositories.
//!
//! An import is a draft until a run starts. At most one import holds an open undo window
//! (`undoable`). SQLite refuses a second one with a partial unique index; MySQL has no partial
//! index, so [`ImportRepo::start_run`] clears the others and sets this one in its serialized
//! transaction on both engines (`import_lifecycle.rs`, which also owns when a run is dead).
//!
//! [`ImportResourceRepo::record`] takes the caller's executor, so a run writes the row inside the
//! transaction that created the target: a rollback leaves neither.

use serde_json::Value;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::import_lifecycle::{HEARTBEAT, with_member};
use super::{bind_id, insert_id, json_text};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{ImportId, JobId};
use crate::rows::{
    ImportKind, ImportResourceRow, ImportRow, ImportStatus, ImportTarget, NewImport,
};
use crate::time;

/// `romeka_import` rows.
pub struct ImportRepo<'a, DB: Backend = Sqlite> {
    pub(super) pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> ImportRepo<'a, DB>
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
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    /// Store a new draft.
    pub async fn create_draft(&self, new: &NewImport) -> Result<ImportRow, DbError> {
        let created = time::now();
        let query = sqlx::query(
            "INSERT INTO romeka_import (owner_id, folder, plan, status, undoable, created)
             VALUES (?, ?, ?, 'draft', 0, ?)",
        )
        .bind(super::bind_opt_id(new.owner_id))
        .bind(&new.folder)
        .bind(json_text(&new.plan)?)
        .bind(time::format(created));
        let id = ImportId(insert_id(query, self.pool).await?);
        Ok(ImportRow {
            id,
            owner_id: new.owner_id,
            folder: new.folder.clone(),
            plan: new.plan.clone(),
            status: ImportStatus::Draft,
            undoable: false,
            job_id: None,
            summary: None,
            created,
            finished: None,
        })
    }

    pub async fn get(&self, id: ImportId) -> Result<Option<ImportRow>, DbError> {
        let row = sqlx::query("SELECT * FROM romeka_import WHERE id = ?")
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(ImportRow::from_db_row::<DB>).transpose()
    }

    /// Every import, newest first.
    pub async fn list(&self) -> Result<Vec<ImportRow>, DbError> {
        let rows = sqlx::query("SELECT * FROM romeka_import ORDER BY id DESC")
            .fetch_all(self.pool)
            .await?;
        rows.iter().map(ImportRow::from_db_row::<DB>).collect()
    }

    /// The import that holds the open undo window, if any.
    pub async fn undoable(&self) -> Result<Option<ImportRow>, DbError> {
        let row = sqlx::query("SELECT * FROM romeka_import WHERE undoable = 1")
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(ImportRow::from_db_row::<DB>).transpose()
    }

    /// Replace a draft's plan. False when the import is missing or no longer a draft.
    pub async fn update_plan(&self, id: ImportId, plan: &Value) -> Result<bool, DbError> {
        let done =
            sqlx::query("UPDATE romeka_import SET plan = ? WHERE id = ? AND status = 'draft'")
                .bind(json_text(plan)?)
                .bind(bind_id(id))
                .execute(self.pool)
                .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn set_status(&self, id: ImportId, status: ImportStatus) -> Result<bool, DbError> {
        let done = sqlx::query("UPDATE romeka_import SET status = ? WHERE id = ?")
            .bind(status.as_str())
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// End a run: store the final status and the finish time together.
    pub async fn finish(&self, id: ImportId, status: ImportStatus) -> Result<bool, DbError> {
        let done = sqlx::query("UPDATE romeka_import SET status = ?, finished = ? WHERE id = ?")
            .bind(status.as_str())
            .bind(time::format(time::now()))
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Replace the summary. Only a working job writes it, so the write is also its heartbeat
    /// ([`HEARTBEAT`]): replacing the summary never leaves a running import without one.
    pub async fn set_summary(&self, id: ImportId, summary: &Value) -> Result<bool, DbError> {
        let stamped = with_member(
            Some(summary.clone()),
            HEARTBEAT,
            Value::String(time::format(time::now())),
        );
        let done = sqlx::query("UPDATE romeka_import SET summary = ? WHERE id = ?")
            .bind(json_text(&stamped)?)
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn set_job(&self, id: ImportId, job: JobId) -> Result<bool, DbError> {
        let done = sqlx::query("UPDATE romeka_import SET job_id = ? WHERE id = ?")
            .bind(bind_id(job))
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Close this import's undo window, for example once its undo has finished.
    pub async fn close_undo_window(&self, id: ImportId) -> Result<bool, DbError> {
        let done = sqlx::query("UPDATE romeka_import SET undoable = 0 WHERE id = ?")
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Delete an import that is still a draft. False when it is missing or has started.
    pub async fn delete_draft(&self, id: ImportId) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM romeka_import WHERE id = ? AND status = 'draft'")
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }
}

/// `romeka_import_resource` rows.
pub struct ImportResourceRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> ImportResourceRepo<'a, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    /// Record that an import created `target`, on the caller's executor. Pass `&mut *tx` from the
    /// transaction that creates the target, so a rollback removes both.
    pub async fn record<'e, E>(
        executor: E,
        import: ImportId,
        target: ImportTarget,
    ) -> Result<(), DbError>
    where
        E: Executor<'e, Database = DB>,
    {
        sqlx::query(
            "INSERT INTO romeka_import_resource (import_id, kind, target_id) VALUES (?, ?, ?)",
        )
        .bind(bind_id(import))
        .bind(target.kind().as_str())
        .bind(target.raw_id())
        .execute(executor)
        .await?;
        Ok(())
    }

    /// What an import created, in the order it was recorded, optionally of one kind.
    pub async fn list(
        &self,
        import: ImportId,
        kind: Option<ImportKind>,
    ) -> Result<Vec<ImportResourceRow>, DbError> {
        let rows = match kind {
            Some(kind) => {
                sqlx::query(
                    "SELECT * FROM romeka_import_resource WHERE import_id = ? AND kind = ?
                     ORDER BY target_id",
                )
                .bind(bind_id(import))
                .bind(kind.as_str())
                .fetch_all(self.pool)
                .await?
            }
            None => {
                sqlx::query(
                    "SELECT * FROM romeka_import_resource WHERE import_id = ?
                     ORDER BY kind, target_id",
                )
                .bind(bind_id(import))
                .fetch_all(self.pool)
                .await?
            }
        };
        rows.iter()
            .map(ImportResourceRow::from_db_row::<DB>)
            .collect()
    }

    /// Forget one recorded target, as undo does when it has deleted the thing. False when no
    /// such row exists.
    pub async fn delete(&self, import: ImportId, target: ImportTarget) -> Result<bool, DbError> {
        let done = sqlx::query(
            "DELETE FROM romeka_import_resource
             WHERE import_id = ? AND kind = ? AND target_id = ?",
        )
        .bind(bind_id(import))
        .bind(target.kind().as_str())
        .bind(target.raw_id())
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }
}
