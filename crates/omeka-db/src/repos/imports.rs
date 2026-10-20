//! `romeka_import`: the draft, its plan saves, and the endings of a run or an undo.
//!
//! A draft's plan is saved with the revision the caller edited, so two editors cannot overwrite
//! each other silently ([`PlanSave::Stale`]). The starts, the heartbeat and when a worker counts
//! as dead live in `import_lifecycle.rs`.

use serde_json::Value;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::import_lifecycle::{HEARTBEAT, with_member};
use super::{
    Paging, Statement, bind_id, bind_opt_id, fetch_rows, insert_id, json_text, push_paging,
};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{ImportId, JobId};
use crate::rows::{ImportEnding, ImportRow, ImportStatus, NewImport};
use crate::time;

/// What a plan save did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanSave {
    /// Stored; the plan's revision is now this.
    Saved(u64),
    /// The caller edited an older revision than this one; nothing changed.
    Stale(u64),
    /// The import has left the draft state; its plan is frozen.
    NotDraft(ImportStatus),
    Missing,
}

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
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    /// Store a new draft at revision 0.
    pub async fn create_draft(&self, new: &NewImport) -> Result<ImportRow, DbError> {
        let created = time::now();
        let query = sqlx::query(
            "INSERT INTO romeka_import (owner_id, sources, plan, revision, status, copied_from, created)
             VALUES (?, ?, ?, 0, 'draft', ?, ?)",
        )
        .bind(bind_opt_id(new.owner_id))
        .bind(json_text(&new.sources)?)
        .bind(json_text(&new.plan)?)
        .bind(bind_opt_id(new.copied_from))
        .bind(time::format(created));
        let id = ImportId(insert_id(query, self.pool).await?);
        Ok(ImportRow {
            id,
            owner_id: new.owner_id,
            sources: new.sources.clone(),
            plan: new.plan.clone(),
            ran_plan: None,
            revision: 0,
            status: ImportStatus::Draft,
            job_id: None,
            summary: None,
            stop_requested: false,
            copied_from: new.copied_from,
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

    /// One page of the history, newest first.
    pub async fn page(&self, paging: &Paging) -> Result<Vec<ImportRow>, DbError> {
        let mut statement = Statement::<DB>::new("SELECT * FROM romeka_import ORDER BY id DESC");
        push_paging(&mut statement, paging)?;
        let (sql, arguments) = statement.into_parts();
        let rows = fetch_rows(&sql, arguments, self.pool).await?;
        rows.iter().map(ImportRow::from_db_row::<DB>).collect()
    }

    /// How many imports the history holds.
    pub async fn count(&self) -> Result<u64, DbError> {
        let row = sqlx::query("SELECT COUNT(*) AS n FROM romeka_import")
            .fetch_one(self.pool)
            .await?;
        DB::id(&row, "n")
    }

    /// Replace a draft's plan, if `revision` is the one stored. One statement, so two saves of
    /// the same revision cannot both land.
    pub async fn save_plan(
        &self,
        id: ImportId,
        plan: &Value,
        revision: u64,
    ) -> Result<PlanSave, DbError> {
        let done = sqlx::query(
            "UPDATE romeka_import SET plan = ?, revision = revision + 1
             WHERE id = ? AND status = 'draft' AND revision = ?",
        )
        .bind(json_text(plan)?)
        .bind(bind_id(id))
        .bind(bind_id(revision))
        .execute(self.pool)
        .await?;
        if DB::rows_affected(&done) > 0 {
            return Ok(PlanSave::Saved(revision + 1));
        }
        Ok(match self.get(id).await? {
            None => PlanSave::Missing,
            Some(row) if row.status != ImportStatus::Draft => PlanSave::NotDraft(row.status),
            Some(row) => PlanSave::Stale(row.revision),
        })
    }

    /// Delete an import that is still a draft, with everything it holds. False when it is
    /// missing or has started.
    pub async fn delete_draft(&self, id: ImportId) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM romeka_import WHERE id = ? AND status = 'draft'")
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Link the job that works on the import.
    pub async fn set_job(&self, id: ImportId, job: JobId) -> Result<bool, DbError> {
        let done = sqlx::query("UPDATE romeka_import SET job_id = ? WHERE id = ?")
            .bind(bind_id(job))
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Ask the running job to stop after the record in hand. One member, no heartbeat: only the
    /// worker proves it is alive, so a request cannot extend the life of a dead import. False
    /// unless the import is `running`.
    pub async fn request_stop(&self, id: ImportId) -> Result<bool, DbError> {
        let done = sqlx::query(
            "UPDATE romeka_import SET stop_requested = 1 WHERE id = ? AND status = 'running'",
        )
        .bind(bind_id(id))
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Whether a stop was asked for since the run started: what the worker reads after each
    /// record. False for a missing import.
    pub async fn stop_requested(&self, id: ImportId) -> Result<bool, DbError> {
        let row = sqlx::query("SELECT stop_requested FROM romeka_import WHERE id = ?")
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        row.map_or(Ok(false), |row| DB::boolean(&row, "stop_requested"))
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

    /// End a run or an undo with its finish time. False unless the import is in the state the
    /// ending ends, so a worker a reader already found dead cannot overwrite that record.
    pub async fn finish(&self, id: ImportId, ending: ImportEnding) -> Result<bool, DbError> {
        let done = sqlx::query(
            "UPDATE romeka_import SET status = ?, finished = ? WHERE id = ? AND status = ?",
        )
        .bind(ending.status().as_str())
        .bind(time::format(time::now()))
        .bind(bind_id(id))
        .bind(ending.ends().as_str())
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }
}
