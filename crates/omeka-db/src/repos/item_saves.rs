//! One item create or update from the API, in one transaction with its history entry.
//!
//! The api layer resolves and checks everything the body names first, then hands the whole write
//! here: the item row, its values, the item sets and sites it joins and leaves. The transaction
//! locks the item, reads its image, writes, and records what changed
//! ([`ImportWrites::record_history`]), so a failure keeps the stored item and makes no entry.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Type};

use super::import_writes::ImportWrites;
use super::resources::update_resource_columns;
use super::values::replace_values;
use super::{ItemRepo, bind_id, bind_opt_id};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{ItemSetId, SiteId};
use crate::rows::{HistoryStamp, ImportResource, ItemRow, NewItem, ValueTree};

/// The row half of a save.
#[derive(Debug, Clone, Copy)]
pub enum ItemWrite<'a> {
    Create(&'a NewItem),
    /// The stored row with the body's columns applied.
    Update(&'a ItemRow),
}

/// The links one save adds and drops. Adding a link the item has, or dropping one it lacks, is a
/// no-op.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Membership<T> {
    pub add: Vec<T>,
    pub drop: Vec<T>,
}

impl<T> Default for Membership<T> {
    fn default() -> Self {
        Self {
            add: Vec::new(),
            drop: Vec::new(),
        }
    }
}

/// One item create or update.
#[derive(Debug, Clone)]
pub struct ItemSave<'a> {
    pub write: ItemWrite<'a>,
    /// The item's values after the save; `None` keeps the stored ones.
    pub values: Option<&'a [ValueTree]>,
    pub item_sets: Membership<ItemSetId>,
    pub sites: Membership<SiteId>,
    pub stamp: HistoryStamp,
}

impl<DB: Backend> ItemRepo<'_, DB>
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
    /// Write one item and record its history entry. Answers the row as written, or `None` when
    /// the item to update is gone.
    pub async fn save(&self, save: &ItemSave<'_>) -> Result<Option<ItemRow>, DbError> {
        let mut tx = self.pool.begin_with(DB::begin_write()).await?;
        let (row, before) = match save.write {
            ItemWrite::Create(new) => (Self::insert_item(&mut tx, new).await?, None),
            ItemWrite::Update(row) => {
                let lock = format!("SELECT id FROM resource WHERE id = ?{}", DB::for_update());
                sqlx::query(&lock)
                    .bind(bind_id(row.id()))
                    .fetch_optional(&mut *tx)
                    .await?;
                let Some(before) = ImportWrites::<DB>::new(&mut tx)
                    .before_image(ImportResource::Item(row.id()))
                    .await?
                else {
                    tx.rollback().await?;
                    return Ok(None);
                };
                update_resource_columns(&mut tx, &row.resource).await?;
                sqlx::query("UPDATE item SET primary_media_id = ? WHERE id = ?")
                    .bind(bind_opt_id(row.primary_media_id))
                    .bind(bind_id(row.id()))
                    .execute(&mut *tx)
                    .await?;
                (row.clone(), Some(before))
            }
        };
        let item = row.id();
        if let Some(values) = save.values {
            replace_values::<DB>(&mut tx, row.resource.id, values).await?;
        }
        for set in &save.item_sets.drop {
            sqlx::query("DELETE FROM item_item_set WHERE item_id = ? AND item_set_id = ?")
                .bind(bind_id(item))
                .bind(bind_id(*set))
                .execute(&mut *tx)
                .await?;
        }
        for set in &save.item_sets.add {
            sqlx::query(DB::insert_item_item_set())
                .bind(bind_id(item))
                .bind(bind_id(*set))
                .execute(&mut *tx)
                .await?;
        }
        for site in &save.sites.drop {
            sqlx::query("DELETE FROM item_site WHERE item_id = ? AND site_id = ?")
                .bind(bind_id(item))
                .bind(bind_id(*site))
                .execute(&mut *tx)
                .await?;
        }
        let insert_site = format!("{}(?, ?)", DB::insert_item_site());
        for site in &save.sites.add {
            sqlx::query(&insert_site)
                .bind(bind_id(item))
                .bind(bind_id(*site))
                .execute(&mut *tx)
                .await?;
        }
        ImportWrites::<DB>::new(&mut tx)
            .record_history(item, before.as_ref(), &save.stamp)
            .await?;
        tx.commit().await?;
        Ok(Some(row))
    }
}
