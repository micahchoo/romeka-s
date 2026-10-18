//! `setting`, `site_setting` and `user_setting`: the three key-value setting tables.
//!
//! The port's shape follows upstream `Omeka\Settings\AbstractSettings` and
//! `AbstractTargetSettings`: `get` returns the stored JSON value, `set` writes it, and `delete`
//! removes the row. Upstream lower-cases the key before it stores it and treats a null value as a
//! delete; the port stores the key as given and lets a caller call `delete`, because the
//! `COLLATE NOCASE` on `id` already folds an ASCII case pair on a lookup.
//!
//! `site_setting` and `user_setting` are keyed by `(id, target_id)`, so the same key holds a
//! different value for each site or user.

use serde_json::Value;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::{Statement, bind_id, fetch_rows, json_text};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{SiteId, UserId};
use crate::rows::{SettingRow, SiteSettingRow, UserSettingRow};

/// The global `setting` table, one row per key.
pub struct SettingRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> SettingRepo<'a, DB>
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

    /// The stored value of `id`, or `None` when the setting was never written.
    pub async fn get(&self, id: &str) -> Result<Option<Value>, DbError> {
        let row = sqlx::query("SELECT id, value FROM setting WHERE id = ?")
            .bind(id)
            .fetch_optional(self.pool)
            .await?;
        row.as_ref()
            .map(SettingRow::from_db_row::<DB>)
            .transpose()
            .map(|row| row.map(|row| row.value))
    }

    /// `AbstractSettings::set`: write the value, replacing what is stored for `id`.
    pub async fn set(&self, id: &str, value: &Value) -> Result<(), DbError> {
        sqlx::query(DB::upsert_setting())
            .bind(id)
            .bind(json_text(value)?)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// Remove the setting. `false` when no row had that key.
    pub async fn delete(&self, id: &str) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM setting WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Every setting, in key order.
    pub async fn list(&self) -> Result<Vec<SettingRow>, DbError> {
        let rows = sqlx::query("SELECT id, value FROM setting ORDER BY id")
            .fetch_all(self.pool)
            .await?;
        rows.iter().map(SettingRow::from_db_row::<DB>).collect()
    }
}

/// The `site_setting` table, one row per `(key, site)`.
pub struct SiteSettingRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> SiteSettingRepo<'a, DB>
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

    /// The value this site stores for `id`, or `None` when the setting was never written.
    pub async fn get(&self, site_id: SiteId, id: &str) -> Result<Option<Value>, DbError> {
        let row =
            sqlx::query("SELECT id, site_id, value FROM site_setting WHERE site_id = ? AND id = ?")
                .bind(bind_id(site_id))
                .bind(id)
                .fetch_optional(self.pool)
                .await?;
        row.as_ref()
            .map(SiteSettingRow::from_db_row::<DB>)
            .transpose()
            .map(|row| row.map(|row| row.value))
    }

    /// `AbstractTargetSettings::set`: write the value for this site.
    pub async fn set(&self, site_id: SiteId, id: &str, value: &Value) -> Result<(), DbError> {
        sqlx::query(DB::upsert_site_setting())
            .bind(id)
            .bind(bind_id(site_id))
            .bind(json_text(value)?)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// Remove one of this site's settings. `false` when no row had that key.
    pub async fn delete(&self, site_id: SiteId, id: &str) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM site_setting WHERE site_id = ? AND id = ?")
            .bind(bind_id(site_id))
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Every setting this site stores, in key order.
    pub async fn list(&self, site_id: SiteId) -> Result<Vec<SiteSettingRow>, DbError> {
        let mut statement =
            Statement::<DB>::new("SELECT id, site_id, value FROM site_setting WHERE site_id = ");
        statement.bind(bind_id(site_id))?;
        statement.push(" ORDER BY id");
        let (sql, arguments) = statement.into_parts();
        let rows: Vec<DB::Row> = fetch_rows(&sql, arguments, self.pool).await?;
        rows.iter().map(SiteSettingRow::from_db_row::<DB>).collect()
    }
}

/// The `user_setting` table, one row per `(key, user)`.
pub struct UserSettingRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> UserSettingRepo<'a, DB>
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

    /// The value this user stores for `id`, or `None` when the setting was never written.
    pub async fn get(&self, user_id: UserId, id: &str) -> Result<Option<Value>, DbError> {
        let row =
            sqlx::query("SELECT id, user_id, value FROM user_setting WHERE user_id = ? AND id = ?")
                .bind(bind_id(user_id))
                .bind(id)
                .fetch_optional(self.pool)
                .await?;
        row.as_ref()
            .map(UserSettingRow::from_db_row::<DB>)
            .transpose()
            .map(|row| row.map(|row| row.value))
    }

    /// `AbstractTargetSettings::set`: write the value for this user.
    pub async fn set(&self, user_id: UserId, id: &str, value: &Value) -> Result<(), DbError> {
        sqlx::query(DB::upsert_user_setting())
            .bind(id)
            .bind(bind_id(user_id))
            .bind(json_text(value)?)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// Remove one of this user's settings. `false` when no row had that key.
    pub async fn delete(&self, user_id: UserId, id: &str) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM user_setting WHERE user_id = ? AND id = ?")
            .bind(bind_id(user_id))
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Every setting this user stores, in key order.
    pub async fn list(&self, user_id: UserId) -> Result<Vec<UserSettingRow>, DbError> {
        let mut statement =
            Statement::<DB>::new("SELECT id, user_id, value FROM user_setting WHERE user_id = ");
        statement.bind(bind_id(user_id))?;
        statement.push(" ORDER BY id");
        let (sql, arguments) = statement.into_parts();
        let rows: Vec<DB::Row> = fetch_rows(&sql, arguments, self.pool).await?;
        rows.iter().map(UserSettingRow::from_db_row::<DB>).collect()
    }
}
