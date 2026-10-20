//! `romeka_import_resource` and `romeka_import_before`: what an import wrote, and the state of a
//! changed resource before it did.
//!
//! The two writes take the caller's executor. A run passes `&mut *tx` from the transaction that
//! writes the record, so the record, its ledger row and its before-image commit together or not at
//! all (`docs/import-v2.md`, "Per record").

use serde_json::Value;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::{bind_id, json_text};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::ImportId;
use crate::rows::{BeforeImageRow, ImportKind, ImportResourceRow, ImportTarget, NewImportResource};
use crate::time;

/// `romeka_import_resource` rows: the ledger.
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

    /// Record that `import` wrote `new.target`, now, on the caller's executor. A second row for
    /// the same target fails: one import writes a resource once.
    pub async fn record<'e, E>(
        executor: E,
        import: ImportId,
        new: &NewImportResource,
    ) -> Result<ImportResourceRow, DbError>
    where
        E: Executor<'e, Database = DB>,
    {
        let written = time::now();
        sqlx::query(
            "INSERT INTO romeka_import_resource
                 (import_id, kind, target_id, record, change_kind, written)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(bind_id(import))
        .bind(new.target.kind().as_str())
        .bind(new.target.raw_id())
        .bind(new.record.as_str())
        .bind(new.change.as_str())
        .bind(time::format(written))
        .execute(executor)
        .await?;
        Ok(ImportResourceRow {
            import_id: import,
            target: new.target,
            record: new.record.clone(),
            change: new.change,
            written,
        })
    }

    /// What an import wrote, by kind and id, optionally of one kind.
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

    /// What one record of an import wrote. Record keys compare byte for byte.
    pub async fn find_by_record(
        &self,
        import: ImportId,
        record: &str,
    ) -> Result<Vec<ImportResourceRow>, DbError> {
        let rows = sqlx::query(
            "SELECT * FROM romeka_import_resource WHERE import_id = ? AND record = ?
             ORDER BY kind, target_id",
        )
        .bind(bind_id(import))
        .bind(record)
        .fetch_all(self.pool)
        .await?;
        rows.iter()
            .map(ImportResourceRow::from_db_row::<DB>)
            .collect()
    }

    /// Forget one recorded target, as undo does once it has dealt with it. False when no such row
    /// exists.
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

/// `romeka_import_before` rows.
pub struct BeforeImageRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> BeforeImageRepo<'a, DB>
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

    /// Store `target`'s state before `import` changes it, on the caller's executor. A second
    /// image for the same target fails, so the stored one stays the state before the import.
    pub async fn put<'e, E>(
        executor: E,
        import: ImportId,
        target: ImportTarget,
        image: &Value,
    ) -> Result<(), DbError>
    where
        E: Executor<'e, Database = DB>,
    {
        sqlx::query(
            "INSERT INTO romeka_import_before (import_id, kind, target_id, image)
             VALUES (?, ?, ?, ?)",
        )
        .bind(bind_id(import))
        .bind(target.kind().as_str())
        .bind(target.raw_id())
        .bind(json_text(image)?)
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn get(
        &self,
        import: ImportId,
        target: ImportTarget,
    ) -> Result<Option<Value>, DbError> {
        let row = sqlx::query(
            "SELECT * FROM romeka_import_before WHERE import_id = ? AND kind = ? AND target_id = ?",
        )
        .bind(bind_id(import))
        .bind(target.kind().as_str())
        .bind(target.raw_id())
        .fetch_optional(self.pool)
        .await?;
        row.as_ref()
            .map(|row| BeforeImageRow::from_db_row::<DB>(row).map(|row| row.image))
            .transpose()
    }

    /// Every before-image of an import, by kind and id.
    pub async fn list(&self, import: ImportId) -> Result<Vec<BeforeImageRow>, DbError> {
        let rows = sqlx::query(
            "SELECT * FROM romeka_import_before WHERE import_id = ? ORDER BY kind, target_id",
        )
        .bind(bind_id(import))
        .fetch_all(self.pool)
        .await?;
        rows.iter().map(BeforeImageRow::from_db_row::<DB>).collect()
    }

    /// False when no such image exists.
    pub async fn delete(&self, import: ImportId, target: ImportTarget) -> Result<bool, DbError> {
        let done = sqlx::query(
            "DELETE FROM romeka_import_before WHERE import_id = ? AND kind = ? AND target_id = ?",
        )
        .bind(bind_id(import))
        .bind(target.kind().as_str())
        .bind(target.raw_id())
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }
}
