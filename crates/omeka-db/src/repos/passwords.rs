//! `password_creation`: the one-time password key.
//!
//! Upstream `Stdlib\Mailer` creates a row when it sends an activation or reset mail, and
//! `Controller\LoginController` reads it by key, deletes it on use or expiry, and promotes the
//! user when `activate` is set. `user_id` is unique, so a user holds at most one live key, and
//! `id` is case-sensitive (`utf8mb4_bin` upstream), unlike the setting keys.

use chrono::NaiveDateTime;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::bind_id;
use crate::DbError;
use crate::backend::Backend;
use crate::ids::UserId;
use crate::rows::{NewPasswordCreation, PasswordCreationRow};
use crate::time;

/// `password_creation` rows.
pub struct PasswordCreationRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> PasswordCreationRepo<'a, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    /// `Mailer::getPasswordCreation`: store a fresh key for the user. `created` defaults to now.
    pub async fn create(&self, new: &NewPasswordCreation) -> Result<PasswordCreationRow, DbError> {
        let created: NaiveDateTime = new.created.unwrap_or_else(time::now);
        sqlx::query(
            "INSERT INTO password_creation (id, user_id, created, activate)
             VALUES (?, ?, ?, ?)",
        )
        .bind(&new.id)
        .bind(bind_id(new.user_id))
        .bind(time::format(created))
        .bind(new.activate)
        .execute(self.pool)
        .await?;
        Ok(PasswordCreationRow {
            id: new.id.clone(),
            user_id: new.user_id,
            created,
            activate: new.activate,
        })
    }

    /// The key `createPassword` looks up. `id` is case-sensitive, as upstream's `utf8mb4_bin`
    /// column is.
    pub async fn get(&self, id: &str) -> Result<Option<PasswordCreationRow>, DbError> {
        let row = sqlx::query("SELECT * FROM password_creation WHERE id = ?")
            .bind(id)
            .fetch_optional(self.pool)
            .await?;
        row.as_ref()
            .map(PasswordCreationRow::from_db_row::<DB>)
            .transpose()
    }

    /// The user's live key. `user_id` is unique, so at most one row exists.
    pub async fn get_for_user(
        &self,
        user_id: UserId,
    ) -> Result<Option<PasswordCreationRow>, DbError> {
        let row = sqlx::query("SELECT * FROM password_creation WHERE user_id = ?")
            .bind(bind_id(user_id))
            .fetch_optional(self.pool)
            .await?;
        row.as_ref()
            .map(PasswordCreationRow::from_db_row::<DB>)
            .transpose()
    }

    /// Remove the key, as `createPassword` does on use or expiry.
    pub async fn delete(&self, id: &str) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM password_creation WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Use the key: delete it and, in the same transaction, set the user's `password_hash` and
    /// `is_active` where given, leaving every other user column as it is now. False, with nothing
    /// written, when the key is already gone, so two requests with one key set one password.
    pub async fn consume(
        &self,
        id: &str,
        user_id: UserId,
        password_hash: Option<&str>,
        is_active: Option<bool>,
    ) -> Result<bool, DbError> {
        let mut tx = self.pool.begin_with(DB::begin_write()).await?;
        let taken = sqlx::query("DELETE FROM password_creation WHERE id = ? AND user_id = ?")
            .bind(id)
            .bind(bind_id(user_id))
            .execute(&mut *tx)
            .await?;
        if DB::rows_affected(&taken) == 0 {
            return Ok(false);
        }
        let mut columns = String::from("modified = ?");
        if password_hash.is_some() {
            columns.push_str(", password_hash = ?");
        }
        if is_active.is_some() {
            columns.push_str(", is_active = ?");
        }
        let sql = format!("UPDATE user SET {columns} WHERE id = ?");
        let mut query = sqlx::query(&sql).bind(time::format(time::now()));
        if let Some(hash) = password_hash {
            query = query.bind(hash);
        }
        if let Some(active) = is_active {
            query = query.bind(active);
        }
        query.bind(bind_id(user_id)).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Remove whatever key this user holds, so a new one can be created in its place.
    pub async fn delete_for_user(&self, user_id: UserId) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM password_creation WHERE user_id = ?")
            .bind(bind_id(user_id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }
}
