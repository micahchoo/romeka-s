//! `module`: the installed module list.
//!
//! Upstream `Module\Manager` never has a module repository of its own; it writes the table through
//! Doctrine on install (`is_active` true and the ini version), activate (`is_active` true),
//! deactivate (`is_active` false), upgrade (the new version) and uninstall (delete). This
//! repository offers those five writes and the list and read `ModuleAdapter` serves
//! (`application/src/Module/Manager.php`).

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use crate::DbError;
use crate::backend::Backend;
use crate::rows::ModuleRow;

/// `module` rows.
pub struct ModuleRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> ModuleRepo<'a, DB>
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

    pub async fn get(&self, id: &str) -> Result<Option<ModuleRow>, DbError> {
        let row = sqlx::query("SELECT id, is_active, version FROM module WHERE id = ?")
            .bind(id)
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(ModuleRow::from_db_row::<DB>).transpose()
    }

    /// Every installed module, in id order, the order `ModuleAdapter::search` lists.
    pub async fn list(&self) -> Result<Vec<ModuleRow>, DbError> {
        let rows = sqlx::query("SELECT id, is_active, version FROM module ORDER BY id")
            .fetch_all(self.pool)
            .await?;
        rows.iter().map(ModuleRow::from_db_row::<DB>).collect()
    }

    /// `Module\Manager::install`: a newly installed module is active at the ini version.
    pub async fn install(&self, id: &str, version: &str) -> Result<ModuleRow, DbError> {
        sqlx::query("INSERT INTO module (id, is_active, version) VALUES (?, ?, ?)")
            .bind(id)
            .bind(true)
            .bind(version)
            .execute(self.pool)
            .await?;
        Ok(ModuleRow {
            id: id.to_string(),
            is_active: true,
            version: version.to_string(),
        })
    }

    /// `Module\Manager::activate` and `::deactivate`.
    pub async fn set_active(&self, id: &str, is_active: bool) -> Result<bool, DbError> {
        let done = sqlx::query("UPDATE module SET is_active = ? WHERE id = ?")
            .bind(is_active)
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// `Module\Manager::upgrade`: record the new ini version.
    pub async fn set_version(&self, id: &str, version: &str) -> Result<bool, DbError> {
        let done = sqlx::query("UPDATE module SET version = ? WHERE id = ?")
            .bind(version)
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// `Module\Manager::uninstall`: remove the row.
    pub async fn delete(&self, id: &str) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM module WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }
}
