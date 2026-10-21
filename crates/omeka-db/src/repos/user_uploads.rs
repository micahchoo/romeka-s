//! `romeka_user_upload`: a user's staged uploads. The rules live in
//! `omeka_api::uploads`; this is the storage half.
//!
//! An advance moves the stored offset in one statement, from the offset the client named, so two
//! `PATCH` requests from the same offset cannot both land. The same statement moves `expires`
//! forward: the file is kept 24 hours after its last byte.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::{UploadAdvance, bind_id};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{ItemId, UserId};
use crate::rows::{NewUserUpload, UserUploadRow};
use crate::time;

/// `romeka_user_upload` rows.
pub struct UserUploadRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> UserUploadRepo<'a, DB>
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

    /// Store a new upload with nothing received. Fails when the user already has the id.
    pub async fn create(&self, new: &NewUserUpload) -> Result<UserUploadRow, DbError> {
        let created = time::now();
        sqlx::query(
            "INSERT INTO romeka_user_upload
                 (user_id, id, item_id, path, media_type, length, received, created, expires)
             VALUES (?, ?, ?, ?, ?, ?, 0, ?, ?)",
        )
        .bind(bind_id(new.user_id))
        .bind(new.id.as_str())
        .bind(new.item_id.map(bind_id))
        .bind(new.path.as_str())
        .bind(new.media_type.clone())
        .bind(bind_id(new.length))
        .bind(time::format(created))
        .bind(time::format(new.expires))
        .execute(self.pool)
        .await?;
        Ok(UserUploadRow {
            user_id: new.user_id,
            id: new.id.clone(),
            item_id: new.item_id,
            path: new.path.clone(),
            media_type: new.media_type.clone(),
            claimed: None,
            length: new.length,
            received: 0,
            created,
            expires: new.expires,
        })
    }

    /// One upload of `user`, or `None` when that person has no upload with the id.
    pub async fn get(&self, user: UserId, id: &str) -> Result<Option<UserUploadRow>, DbError> {
        let row = sqlx::query("SELECT * FROM romeka_user_upload WHERE user_id = ? AND id = ?")
            .bind(bind_id(user))
            .bind(id)
            .fetch_optional(self.pool)
            .await?;
        row.as_ref()
            .map(UserUploadRow::from_db_row::<DB>)
            .transpose()
    }

    /// A user's uploads, oldest first, or those started for `item`.
    pub async fn list(
        &self,
        user: UserId,
        item: Option<ItemId>,
    ) -> Result<Vec<UserUploadRow>, DbError> {
        let rows = match item {
            Some(item) => {
                sqlx::query(
                    "SELECT * FROM romeka_user_upload WHERE user_id = ? AND item_id = ?
                     ORDER BY created, id",
                )
                .bind(bind_id(user))
                .bind(bind_id(item))
                .fetch_all(self.pool)
                .await?
            }
            None => {
                sqlx::query(
                    "SELECT * FROM romeka_user_upload WHERE user_id = ? ORDER BY created, id",
                )
                .bind(bind_id(user))
                .fetch_all(self.pool)
                .await?
            }
        };
        rows.iter().map(UserUploadRow::from_db_row::<DB>).collect()
    }

    /// Count `bytes` more as received if the request started at `from`, the stored offset, and
    /// keep the upload until `expires`.
    pub async fn advance(
        &self,
        user: UserId,
        id: &str,
        from: u64,
        bytes: u64,
        expires: chrono::NaiveDateTime,
    ) -> Result<UploadAdvance, DbError> {
        let to = from.saturating_add(bytes);
        let done = sqlx::query(
            "UPDATE romeka_user_upload SET received = ?, expires = ?
             WHERE user_id = ? AND id = ? AND received = ? AND length >= ?",
        )
        .bind(bind_id(to))
        .bind(time::format(expires))
        .bind(bind_id(user))
        .bind(id)
        .bind(bind_id(from))
        .bind(bind_id(to))
        .execute(self.pool)
        .await?;
        if DB::rows_affected(&done) > 0 {
            return Ok(UploadAdvance::Advanced(to));
        }
        Ok(match self.get(user, id).await? {
            None => UploadAdvance::Missing,
            Some(row) if row.received != from => UploadAdvance::Conflict {
                received: row.received,
            },
            Some(row) => UploadAdvance::PastLength { length: row.length },
        })
    }

    /// Take the upload for one media create. The update matches only a row that
    /// no create holds, or one whose claim is at or before `stale_before`, so two
    /// creates cannot both answer true. False means another create holds it, or
    /// the user has no such upload; [`UserUploadRepo::get`] tells those apart.
    pub async fn claim(
        &self,
        user: UserId,
        id: &str,
        at: chrono::NaiveDateTime,
        stale_before: chrono::NaiveDateTime,
    ) -> Result<bool, DbError> {
        let done = sqlx::query(
            "UPDATE romeka_user_upload SET claimed = ?
             WHERE user_id = ? AND id = ? AND (claimed IS NULL OR claimed <= ?)",
        )
        .bind(time::format(at))
        .bind(bind_id(user))
        .bind(id)
        .bind(time::format(stale_before))
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Give the upload back after a create that could not finish. The claim time
    /// is part of the match, so a release never clears a newer claim.
    pub async fn release(
        &self,
        user: UserId,
        id: &str,
        at: chrono::NaiveDateTime,
    ) -> Result<bool, DbError> {
        let done = sqlx::query(
            "UPDATE romeka_user_upload SET claimed = NULL
             WHERE user_id = ? AND id = ? AND claimed = ?",
        )
        .bind(bind_id(user))
        .bind(id)
        .bind(time::format(at))
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// False when the user has no such upload.
    pub async fn delete(&self, user: UserId, id: &str) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM romeka_user_upload WHERE user_id = ? AND id = ?")
            .bind(bind_id(user))
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Every upload whose kept time has passed, oldest first: what the sweep removes.
    pub async fn expired(&self, now: chrono::NaiveDateTime) -> Result<Vec<UserUploadRow>, DbError> {
        let rows = sqlx::query(
            "SELECT * FROM romeka_user_upload WHERE expires <= ? ORDER BY expires, user_id, id",
        )
        .bind(time::format(now))
        .fetch_all(self.pool)
        .await?;
        rows.iter().map(UserUploadRow::from_db_row::<DB>).collect()
    }
}
