//! `item_site`: the item-to-site membership.
//!
//! Upstream's `Entity/Item.php` owns the collection and the database's unique key covers
//! `(item_id, site_id)`, so attaching an item twice keeps one row. The migrator writes the rows it
//! read and reads them back. A site's membership changes in batches: `UpdateSiteItems` chunks item
//! ids at 1000 per statement, and [`ItemSiteRepo::attach`] and [`ItemSiteRepo::detach`] do the
//! same inside one transaction, so a change to 50 000 items is 50 statements and one commit.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::{Statement, bind_id, new_id};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{ItemId, SiteId};
use crate::rows::ItemSiteRow;

/// Item ids per statement (`UpdateSiteItems::updateSiteItems` chunks at 1000).
const CHUNK: usize = 1000;

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
        self.attach(row.site_id, &[row.item_id]).await
    }

    /// Every membership, ordered by item then site.
    pub async fn list(&self) -> Result<Vec<ItemSiteRow>, DbError> {
        let rows = sqlx::query("SELECT item_id, site_id FROM item_site ORDER BY item_id, site_id")
            .fetch_all(self.pool)
            .await?;
        rows.iter().map(ItemSiteRow::from_db_row::<DB>).collect()
    }

    /// The ids of the items attached to a site, ascending.
    pub async fn site_items(&self, site: SiteId) -> Result<Vec<ItemId>, DbError> {
        let ids: Vec<i64> = sqlx::query_scalar::<DB, i64>(
            "SELECT item_id FROM item_site WHERE site_id = ? ORDER BY item_id",
        )
        .bind(bind_id(site))
        .fetch_all(self.pool)
        .await?;
        ids.into_iter()
            .map(|raw| Ok(ItemId::from(new_id(raw)?)))
            .collect()
    }

    /// Attach items to a site in one transaction. An item already attached keeps its one row.
    pub async fn attach(&self, site: SiteId, items: &[ItemId]) -> Result<(), DbError> {
        let mut tx = self.pool.begin().await?;
        for chunk in items.chunks(CHUNK) {
            let mut statement = Statement::<DB>::new(DB::insert_item_site());
            for (at, item) in chunk.iter().enumerate() {
                statement.push(if at == 0 { "(" } else { ", (" });
                statement.bind(bind_id(*item))?;
                statement.push(", ");
                statement.bind(bind_id(site))?;
                statement.push(")");
            }
            let (sql, arguments) = statement.into_parts();
            sqlx::query_with::<DB, _>(&sql, arguments)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Detach items from a site in one transaction. An item that is not attached is skipped.
    pub async fn detach(&self, site: SiteId, items: &[ItemId]) -> Result<(), DbError> {
        let mut tx = self.pool.begin().await?;
        for chunk in items.chunks(CHUNK) {
            let mut statement = Statement::<DB>::new("DELETE FROM item_site WHERE site_id = ");
            statement.bind(bind_id(site))?;
            statement.push(" AND item_id IN (");
            for (at, item) in chunk.iter().enumerate() {
                if at > 0 {
                    statement.push(", ");
                }
                statement.bind(bind_id(*item))?;
            }
            statement.push(")");
            let (sql, arguments) = statement.into_parts();
            sqlx::query_with::<DB, _>(&sql, arguments)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
