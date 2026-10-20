//! `romeka_import_upload` and `romeka_import_entry`: an import's uploads and its evidence index.
//!
//! An upload advances only from the offset stored, in one statement, so two `PATCH` requests from
//! the same offset cannot both land (`docs/import-v2.md`, "Arrival"). A rescan replaces the whole
//! index in one transaction, so a reader sees the old index or the new one, never a mix.

use chrono::NaiveDateTime;
use sha2::{Digest, Sha256};
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::{Paging, Statement, bind_id, fetch_rows, json_text, push_paging};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::ImportId;
use crate::rows::{EntryRow, NewEntry, NewUpload, UploadRow};
use crate::time;

/// What an advance did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadAdvance {
    /// Stored; this many bytes have arrived.
    Advanced(u64),
    /// The request started at another offset than the stored one; nothing changed.
    Conflict {
        received: u64,
    },
    /// The bytes would pass the declared length; nothing changed.
    PastLength {
        length: u64,
    },
    Missing,
}

/// Entries per `INSERT`: eight values each, well inside both engines' placeholder limits.
const ENTRIES_PER_INSERT: usize = 500;

/// The key of a path in the index: its SHA-256 in lowercase hex. A key on the path itself would
/// pass InnoDB's 3,072-byte key limit.
fn path_hash(path: &str) -> String {
    format!("{:x}", Sha256::digest(path.as_bytes()))
}

/// `romeka_import_upload` rows.
pub struct UploadRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> UploadRepo<'a, DB>
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

    /// Store a new upload with nothing received. Fails when the import already has the id.
    pub async fn create(&self, new: &NewUpload) -> Result<UploadRow, DbError> {
        let created = time::now();
        sqlx::query(
            "INSERT INTO romeka_import_upload (import_id, id, path, length, received, created)
             VALUES (?, ?, ?, ?, 0, ?)",
        )
        .bind(bind_id(new.import_id))
        .bind(new.id.as_str())
        .bind(new.path.as_str())
        .bind(bind_id(new.length))
        .bind(time::format(created))
        .execute(self.pool)
        .await?;
        Ok(UploadRow {
            import_id: new.import_id,
            id: new.id.clone(),
            path: new.path.clone(),
            length: new.length,
            received: 0,
            created,
        })
    }

    /// Count `bytes` more as received, if the request started at `from`, the stored offset.
    pub async fn advance(
        &self,
        import: ImportId,
        id: &str,
        from: u64,
        bytes: u64,
    ) -> Result<UploadAdvance, DbError> {
        let to = from.saturating_add(bytes);
        let done = sqlx::query(
            "UPDATE romeka_import_upload SET received = ?
             WHERE import_id = ? AND id = ? AND received = ? AND length >= ?",
        )
        .bind(bind_id(to))
        .bind(bind_id(import))
        .bind(id)
        .bind(bind_id(from))
        .bind(bind_id(to))
        .execute(self.pool)
        .await?;
        if DB::rows_affected(&done) > 0 {
            return Ok(UploadAdvance::Advanced(to));
        }
        Ok(match self.get(import, id).await? {
            None => UploadAdvance::Missing,
            Some(row) if row.received != from => UploadAdvance::Conflict {
                received: row.received,
            },
            Some(row) => UploadAdvance::PastLength { length: row.length },
        })
    }

    pub async fn get(&self, import: ImportId, id: &str) -> Result<Option<UploadRow>, DbError> {
        let row = sqlx::query("SELECT * FROM romeka_import_upload WHERE import_id = ? AND id = ?")
            .bind(bind_id(import))
            .bind(id)
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(UploadRow::from_db_row::<DB>).transpose()
    }

    /// An import's uploads, oldest first.
    pub async fn list(&self, import: ImportId) -> Result<Vec<UploadRow>, DbError> {
        let rows = sqlx::query(
            "SELECT * FROM romeka_import_upload WHERE import_id = ? ORDER BY created, id",
        )
        .bind(bind_id(import))
        .fetch_all(self.pool)
        .await?;
        rows.iter().map(UploadRow::from_db_row::<DB>).collect()
    }

    /// False when no such upload exists.
    pub async fn delete(&self, import: ImportId, id: &str) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM romeka_import_upload WHERE import_id = ? AND id = ?")
            .bind(bind_id(import))
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Every upload, of any import, still missing bytes and created before `before`: what the
    /// sweep removes.
    pub async fn incomplete_before(
        &self,
        before: NaiveDateTime,
    ) -> Result<Vec<UploadRow>, DbError> {
        let rows = sqlx::query(
            "SELECT * FROM romeka_import_upload WHERE received < length AND created < ?
             ORDER BY created, import_id, id",
        )
        .bind(time::format(before))
        .fetch_all(self.pool)
        .await?;
        rows.iter().map(UploadRow::from_db_row::<DB>).collect()
    }
}

/// `romeka_import_entry` rows: the evidence index.
pub struct EntryRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> EntryRepo<'a, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    /// Replace an import's whole index with `entries`, in one transaction. A failure, such as one
    /// path listed twice in a source, leaves the old index.
    pub async fn replace(&self, import: ImportId, entries: &[NewEntry]) -> Result<(), DbError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM romeka_import_entry WHERE import_id = ?")
            .bind(bind_id(import))
            .execute(&mut *tx)
            .await?;
        for chunk in entries.chunks(ENTRIES_PER_INSERT) {
            let mut statement = Statement::<DB>::new(
                "INSERT INTO romeka_import_entry
                     (import_id, source, path, path_hash, size, modified, media_type, flags)
                 VALUES ",
            );
            for (index, entry) in chunk.iter().enumerate() {
                statement.push(if index == 0 { "(" } else { ", (" });
                statement.bind(bind_id(import))?.push(", ");
                statement.bind(entry.source.clone())?.push(", ");
                statement.bind(entry.path.clone())?.push(", ");
                statement.bind(path_hash(&entry.path))?.push(", ");
                statement.bind(bind_id(entry.size))?.push(", ");
                statement.bind(time::format(entry.modified))?.push(", ");
                statement.bind(entry.media_type.clone())?.push(", ");
                statement.bind(json_text(&entry.flags)?)?.push(")");
            }
            let (sql, arguments) = statement.into_parts();
            sqlx::query_with::<DB, _>(&sql, arguments)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// One page of an import's index, by source and then path, byte for byte.
    pub async fn page(&self, import: ImportId, paging: &Paging) -> Result<Vec<EntryRow>, DbError> {
        let mut statement =
            Statement::<DB>::new("SELECT * FROM romeka_import_entry WHERE import_id = ");
        statement
            .bind(bind_id(import))?
            .push(" ORDER BY source, path");
        push_paging(&mut statement, paging)?;
        let (sql, arguments) = statement.into_parts();
        let rows = fetch_rows(&sql, arguments, self.pool).await?;
        rows.iter().map(EntryRow::from_db_row::<DB>).collect()
    }

    pub async fn count(&self, import: ImportId) -> Result<u64, DbError> {
        let row = sqlx::query("SELECT COUNT(*) AS n FROM romeka_import_entry WHERE import_id = ?")
            .bind(bind_id(import))
            .fetch_one(self.pool)
            .await?;
        DB::id(&row, "n")
    }

    /// The entry at `path` in `source`, matched byte for byte.
    pub async fn get(
        &self,
        import: ImportId,
        source: &str,
        path: &str,
    ) -> Result<Option<EntryRow>, DbError> {
        let row = sqlx::query(
            "SELECT * FROM romeka_import_entry
             WHERE import_id = ? AND source = ? AND path_hash = ?",
        )
        .bind(bind_id(import))
        .bind(source)
        .bind(path_hash(path))
        .fetch_optional(self.pool)
        .await?;
        row.as_ref().map(EntryRow::from_db_row::<DB>).transpose()
    }
}
