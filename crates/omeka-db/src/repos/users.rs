//! `user` and `api_key` repositories.
//!
//! Filters mirror `UserAdapter::buildQuery`: `email`, `name`, `role`, `is_active` and the site
//! permission join. API keys are looked up by the id a request presents; the caller verifies the
//! stored hash, this crate only stores it.

use chrono::NaiveDateTime;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::{Paging, Statement, bind_id, fetch_rows, insert_id, push_paging};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{SiteId, UserId};
use crate::rows::{ApiKeyRow, NewApiKey, NewUser, UserRow};
use crate::time;

/// A list filter for `user`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UserFilter {
    pub email: Option<String>,
    pub name: Option<String>,
    pub role: Option<String>,
    pub is_active: Option<bool>,
    /// Users with a permission row on this site.
    pub site_id: Option<SiteId>,
}

/// `user` rows.
pub struct UserRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> UserRepo<'a, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
    for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
    for<'x> f64: Decode<'x, DB> + Type<DB>,
{
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    pub async fn create(&self, new: &NewUser) -> Result<UserRow, DbError> {
        let created = new.created.unwrap_or_else(time::now);
        let query = sqlx::query(
            "INSERT INTO user (email, name, created, modified, password_hash, role, is_active)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&new.email)
        .bind(&new.name)
        .bind(time::format(created))
        .bind(time::format(created))
        .bind(&new.password_hash)
        .bind(&new.role)
        .bind(new.is_active);
        let id = insert_id(query, self.pool).await?;
        Ok(UserRow {
            id: UserId::from(id),
            email: new.email.clone(),
            name: new.name.clone(),
            created,
            modified: Some(created),
            password_hash: new.password_hash.clone(),
            role: new.role.clone(),
            is_active: new.is_active,
        })
    }

    pub async fn get(&self, id: UserId) -> Result<Option<UserRow>, DbError> {
        let row = sqlx::query("SELECT * FROM user WHERE id = ?")
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(UserRow::from_db_row::<DB>).transpose()
    }

    /// The account with this email. The comparison follows the column's collation, which is
    /// case-insensitive as upstream's `utf8mb4_unicode_ci` unique key is.
    pub async fn get_by_email(&self, email: &str) -> Result<Option<UserRow>, DbError> {
        let row = sqlx::query("SELECT * FROM user WHERE email = ?")
            .bind(email)
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(UserRow::from_db_row::<DB>).transpose()
    }

    /// Write every user column except `created`, and set `modified` to now.
    pub async fn update(&self, row: &UserRow) -> Result<bool, DbError> {
        let done = sqlx::query(
            "UPDATE user
             SET email = ?, name = ?, modified = ?, password_hash = ?, role = ?, is_active = ?
             WHERE id = ?",
        )
        .bind(&row.email)
        .bind(&row.name)
        .bind(time::format(time::now()))
        .bind(&row.password_hash)
        .bind(&row.role)
        .bind(row.is_active)
        .bind(bind_id(row.id))
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Delete a user and their API keys.
    ///
    /// Upstream cascades to keys in Doctrine (`Entity/User.php`), and `api_key.owner_id` has no
    /// database action. Everything else the user owned keeps its row and loses the reference.
    pub async fn delete(&self, id: UserId) -> Result<bool, DbError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM api_key WHERE owner_id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        let done = sqlx::query("DELETE FROM user WHERE id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn list(
        &self,
        filter: &UserFilter,
        paging: &Paging,
    ) -> Result<Vec<UserRow>, DbError> {
        let mut statement = Statement::<DB>::new("SELECT * FROM user WHERE 1 = 1");
        if let Some(email) = &filter.email {
            statement.push(" AND email = ");
            statement.bind(email)?;
        }
        if let Some(name) = &filter.name {
            statement.push(" AND name = ");
            statement.bind(name)?;
        }
        if let Some(role) = &filter.role {
            statement.push(" AND role = ");
            statement.bind(role)?;
        }
        if let Some(is_active) = filter.is_active {
            statement.push(" AND is_active = ");
            statement.bind(is_active)?;
        }
        if let Some(site_id) = filter.site_id {
            statement.push(
                " AND EXISTS (SELECT 1 FROM site_permission
                       WHERE site_permission.user_id = user.id
                         AND site_permission.site_id = ",
            );
            statement.bind(bind_id(site_id))?;
            statement.push(")");
        }
        statement.push(" ORDER BY id");
        push_paging(&mut statement, paging)?;
        let (sql, arguments) = statement.into_parts();
        let rows: Vec<DB::Row> = fetch_rows(&sql, arguments, self.pool).await?;
        rows.iter().map(UserRow::from_db_row::<DB>).collect()
    }
}

/// `api_key` rows.
pub struct ApiKeyRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> ApiKeyRepo<'a, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
    for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
    for<'x> f64: Decode<'x, DB> + Type<DB>,
{
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    pub async fn create(&self, new: &NewApiKey) -> Result<ApiKeyRow, DbError> {
        let created = new.created.unwrap_or_else(time::now);
        sqlx::query(
            "INSERT INTO api_key (id, owner_id, label, credential_hash, last_ip, last_accessed, created)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&new.id)
        .bind(bind_id(new.owner_id))
        .bind(&new.label)
        .bind(&new.credential_hash)
        .bind(&new.last_ip)
        .bind(new.last_accessed.map(time::format))
        .bind(time::format(created))
        .execute(self.pool)
        .await?;
        Ok(ApiKeyRow {
            id: new.id.clone(),
            owner_id: new.owner_id,
            label: new.label.clone(),
            credential_hash: new.credential_hash.clone(),
            last_ip: new.last_ip.clone(),
            last_accessed: new.last_accessed,
            created,
        })
    }

    pub async fn get(&self, id: &str) -> Result<Option<ApiKeyRow>, DbError> {
        let row = sqlx::query("SELECT * FROM api_key WHERE id = ?")
            .bind(id)
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(ApiKeyRow::from_db_row::<DB>).transpose()
    }

    pub async fn list_for_owner(&self, owner_id: UserId) -> Result<Vec<ApiKeyRow>, DbError> {
        let rows = sqlx::query("SELECT * FROM api_key WHERE owner_id = ? ORDER BY id")
            .bind(bind_id(owner_id))
            .fetch_all(self.pool)
            .await?;
        rows.iter().map(ApiKeyRow::from_db_row::<DB>).collect()
    }

    /// Record a use of the key: the caller's address and the time.
    ///
    /// The address is a packed `varbinary(16)`; it is copied into an owned buffer because a bound
    /// `Option<&[u8]>` is not a type every backend's `Encode` covers in generic code.
    pub async fn touch(
        &self,
        id: &str,
        last_ip: Option<&[u8]>,
        at: Option<NaiveDateTime>,
    ) -> Result<bool, DbError> {
        let accessed = time::format(at.unwrap_or_else(time::now));
        let done = sqlx::query("UPDATE api_key SET last_ip = ?, last_accessed = ? WHERE id = ?")
            .bind(last_ip.map(<[u8]>::to_vec))
            .bind(accessed)
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn delete(&self, id: &str) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM api_key WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }
}
