//! `asset`: an uploaded file that is not attached to an item.
//!
//! `AssetAdapter` filters on `owner_id` (the number 0 selects assets with no owner), and its
//! update hydrates only `name` and `alt_text`; the file identity columns (`media_type`,
//! `storage_id`, `extension`) are fixed when the upload stores the file
//! (`application/src/Api/Adapter/AssetAdapter.php`). `storage_id` is unique, case-insensitively as
//! upstream's `utf8mb4_unicode_ci` key is.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::{Paging, Statement, bind_id, bind_opt_id, fetch_rows, insert_id, push_paging};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{AssetId, UserId};
use crate::rows::{AssetRow, NewAsset};

/// Which owner an asset list asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetOwner {
    /// Assets with no owner, upstream's `owner_id=0`.
    Unowned,
    /// Assets owned by this user.
    Owned(UserId),
}

/// A list filter for `asset`. `owner` absent means every asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AssetFilter {
    pub owner: Option<AssetOwner>,
}

/// `asset` rows.
pub struct AssetRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> AssetRepo<'a, DB>
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

    pub async fn create(&self, new: &NewAsset) -> Result<AssetRow, DbError> {
        let query = sqlx::query(
            "INSERT INTO asset (owner_id, name, media_type, storage_id, extension, alt_text)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(bind_opt_id(new.owner_id))
        .bind(&new.name)
        .bind(&new.media_type)
        .bind(&new.storage_id)
        .bind(&new.extension)
        .bind(&new.alt_text);
        let id = insert_id(query, self.pool).await?;
        Ok(AssetRow {
            id: AssetId::from(id),
            owner_id: new.owner_id,
            name: new.name.clone(),
            media_type: new.media_type.clone(),
            storage_id: new.storage_id.clone(),
            extension: new.extension.clone(),
            alt_text: new.alt_text.clone(),
        })
    }

    pub async fn get(&self, id: AssetId) -> Result<Option<AssetRow>, DbError> {
        let row = sqlx::query("SELECT * FROM asset WHERE id = ?")
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(AssetRow::from_db_row::<DB>).transpose()
    }

    /// The asset with this stored file id. The comparison follows the column's collation, as
    /// upstream's unique key does.
    pub async fn get_by_storage_id(&self, storage_id: &str) -> Result<Option<AssetRow>, DbError> {
        let row = sqlx::query("SELECT * FROM asset WHERE storage_id = ?")
            .bind(storage_id)
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(AssetRow::from_db_row::<DB>).transpose()
    }

    /// Write the two columns an asset update hydrates: `name` and `alt_text`.
    pub async fn update(&self, row: &AssetRow) -> Result<bool, DbError> {
        let done = sqlx::query("UPDATE asset SET name = ?, alt_text = ? WHERE id = ?")
            .bind(&row.name)
            .bind(&row.alt_text)
            .bind(bind_id(row.id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn list(
        &self,
        filter: &AssetFilter,
        paging: &Paging,
    ) -> Result<Vec<AssetRow>, DbError> {
        let mut statement = Statement::<DB>::new("SELECT * FROM asset WHERE 1 = 1");
        match filter.owner {
            Some(AssetOwner::Unowned) => {
                statement.push(" AND owner_id IS NULL");
            }
            Some(AssetOwner::Owned(user_id)) => {
                statement.push(" AND owner_id = ");
                statement.bind(bind_id(user_id))?;
            }
            None => {}
        }
        statement.push(" ORDER BY id");
        push_paging(&mut statement, paging)?;
        let (sql, arguments) = statement.into_parts();
        let rows: Vec<DB::Row> = fetch_rows(&sql, arguments, self.pool).await?;
        rows.iter().map(AssetRow::from_db_row::<DB>).collect()
    }

    /// Remove the asset. Resource and site thumbnails that named it become null.
    pub async fn delete(&self, id: AssetId) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM asset WHERE id = ?")
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }
}
