//! Operations on the whole database rather than on one table's rows: the readiness ping, a
//! table's row count for an archive manifest, and the consistent snapshot a backup archives.

use std::path::Path;

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use crate::DbError;
use crate::backend::Backend;

/// The whole database behind a pool.
pub struct DatabaseRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> DatabaseRepo<'a, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB>,
{
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    /// Run one statement that touches no table. An error means the server is not answering.
    pub async fn ping(&self) -> Result<(), DbError> {
        sqlx::query_scalar::<DB, i64>("SELECT 1")
            .fetch_one(self.pool)
            .await?;
        Ok(())
    }

    /// The number of rows in `table`. The name is `'static` so it comes from the program, never
    /// from a request, and the backend quotes it (MariaDB reserves `value`).
    pub async fn count_rows(&self, table: &'static str) -> Result<i64, DbError> {
        let sql = format!("SELECT COUNT(*) FROM {}", DB::quote_identifier(table));
        let count = sqlx::query_scalar::<DB, i64>(&sql)
            .fetch_one(self.pool)
            .await?;
        Ok(count)
    }

    /// Write a consistent copy of the database to `target`, a file that must not exist yet.
    ///
    /// `DbError::Unsupported` on an engine with no such statement ([`Backend::snapshot_into`]);
    /// no file is written then.
    pub async fn snapshot_into(&self, target: &Path) -> Result<(), DbError> {
        let sql = Self::snapshot_statement()?;
        let path = target.to_string_lossy();
        sqlx::query::<DB>(sql)
            .bind(path.as_ref())
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// `Ok` when [`Self::snapshot_into`] can run on this engine, else the
    /// [`DbError::Unsupported`] it would return. A backup asks before it writes anything.
    pub fn snapshot_supported(&self) -> Result<(), DbError> {
        Self::snapshot_statement().map(|_| ())
    }

    fn snapshot_statement() -> Result<&'static str, DbError> {
        DB::snapshot_into().map_err(|reason| DbError::Unsupported {
            engine: <DB as Backend>::NAME,
            operation: "snapshot",
            reason,
        })
    }
}
