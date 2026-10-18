//! `item_site`: the item-to-site membership.
//!
//! Upstream's `Entity/Item.php` owns the collection and the database's unique key covers
//! `(item_id, site_id)`, so attaching an item twice keeps one row. The migrator is the caller that
//! needs the table rather than one item's sites: it writes the rows it read and reads them back.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::bind_id;
use crate::DbError;
use crate::backend::Backend;
use crate::rows::ItemSiteRow;

/// `item_site` rows.
pub struct ItemSiteRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> ItemSiteRepo<'a, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    /// Attach an item to a site. Attaching twice is a no-op.
    pub async fn create(&self, row: &ItemSiteRow) -> Result<(), DbError> {
        sqlx::query(DB::insert_item_site())
            .bind(bind_id(row.item_id))
            .bind(bind_id(row.site_id))
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// Every membership, ordered by item then site.
    pub async fn list(&self) -> Result<Vec<ItemSiteRow>, DbError> {
        let rows = sqlx::query("SELECT item_id, site_id FROM item_site ORDER BY item_id, site_id")
            .fetch_all(self.pool)
            .await?;
        rows.iter().map(ItemSiteRow::from_db_row::<DB>).collect()
    }
}
