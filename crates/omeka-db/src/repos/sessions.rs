//! `session`: PHP session storage.
//!
//! Upstream `Omeka\Session\SaveHandler\Db` reads a session by id, deletes one whose
//! `modified` age passed `session.gc_maxlifetime`, writes with an upsert, destroys by id, and
//! garbage-collects every row older than a cutoff. `session.modified` is a Unix timestamp in
//! seconds, not a datetime, so it does not go through `time::format`.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use crate::DbError;
use crate::backend::Backend;
use crate::rows::SessionRow;
use crate::time;

/// `session` rows.
pub struct SessionRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> SessionRepo<'a, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    /// The stored session, or `None` when no row has that id.
    ///
    /// Upstream's save handler treats a row older than `gc_maxlifetime` as absent and destroys it.
    /// Expiry depends on the configured lifetime, which the repository does not hold, so the caller
    /// compares `modified` and calls [`delete`](Self::delete) itself.
    pub async fn get(&self, id: &str) -> Result<Option<SessionRow>, DbError> {
        let row = sqlx::query("SELECT * FROM session WHERE id = ?")
            .bind(id)
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(SessionRow::from_db_row::<DB>).transpose()
    }

    /// `SaveHandler\Db::write`: insert the session or replace its data and time. `modified`
    /// defaults to now.
    pub async fn save(&self, id: &str, data: &[u8], modified: Option<i64>) -> Result<(), DbError> {
        sqlx::query(DB::upsert_session())
            .bind(id)
            .bind(modified.unwrap_or_else(time::now_unix))
            .bind(data.to_vec())
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// `SaveHandler\Db::destroy`. `false` when no row had that id.
    pub async fn delete(&self, id: &str) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM session WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// `SaveHandler\Db::gc`: remove every session whose `modified` is older than `before`, and
    /// report how many went.
    pub async fn gc(&self, before: i64) -> Result<u64, DbError> {
        let done = sqlx::query("DELETE FROM session WHERE modified < ?")
            .bind(before)
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done))
    }
}
