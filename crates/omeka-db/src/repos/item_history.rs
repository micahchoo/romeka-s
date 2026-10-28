//! `romeka_item_history`: the entry a write records, in the write's own transaction, and the
//! read of one item's entries.
//!
//! Every write path reads the item's [`BeforeImage`] before it changes anything, writes, and
//! then calls [`ImportWrites::record_history`] on the same connection, which reads the image
//! after and stores what differs. The entry and the write commit together or not at all.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::import_writes::ImportWrites;
use super::{bind_id, bind_opt_id, json_text};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{ItemId, ResourceId};
use crate::rows::item_history::{changes_json, diff, fold};
use crate::rows::{BeforeImage, HistoryEntryRow, HistoryStamp, ImportResource};
use crate::time;

impl<DB: Backend> ImportWrites<'_, DB>
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
    /// Record what a write changed on `item`: `before` is the image read before the write
    /// (`None` where the item did not exist), and the image after is read here. Answers whether
    /// an entry was written or folded. A write that changed none of the entry's fields, or whose
    /// item is gone or is not an item, records nothing.
    ///
    /// A run's second pass over an item whose newest entry is that same run's, by the same
    /// person, folds into that entry, so one import or bulk run makes one entry per item.
    pub async fn record_history(
        &mut self,
        item: ItemId,
        before: Option<&BeforeImage>,
        stamp: &HistoryStamp,
    ) -> Result<bool, DbError> {
        let Some(after) = self.before_image(ImportResource::Item(item)).await? else {
            return Ok(false);
        };
        let changes = diff(before, &after);
        if changes.is_empty() {
            return Ok(false);
        }
        if stamp.source.folds()
            && let Some(newest) = newest_entry::<DB>(self.conn, item).await?
            && newest.source == stamp.source
            && newest.user_id == stamp.user
        {
            let folded = fold(newest.changes, changes);
            if folded.is_empty() {
                sqlx::query("DELETE FROM romeka_item_history WHERE id = ?")
                    .bind(bind_id(newest.id))
                    .execute(&mut *self.conn)
                    .await?;
            } else {
                sqlx::query("UPDATE romeka_item_history SET changes = ? WHERE id = ?")
                    .bind(json_text(&changes_json(&folded))?)
                    .bind(bind_id(newest.id))
                    .execute(&mut *self.conn)
                    .await?;
            }
            return Ok(true);
        }
        let (source, bulk, import) = stamp.source.columns();
        sqlx::query(
            "INSERT INTO romeka_item_history
                 (item_id, created, user_id, source, bulk_id, import_id, changes)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(bind_id(item))
        .bind(time::format(time::now()))
        .bind(bind_opt_id(stamp.user))
        .bind(source)
        .bind(bind_opt_id(bulk))
        .bind(bind_opt_id(import))
        .bind(json_text(&changes_json(&changes))?)
        .execute(&mut *self.conn)
        .await?;
        Ok(true)
    }
}

/// The images of those `resources` that are items, read before a write that changes many at
/// once, for [`record_each`] after it.
pub(crate) async fn images_before<DB: Backend>(
    conn: &mut DB::Connection,
    resources: &[ResourceId],
) -> Result<Vec<(ItemId, BeforeImage)>, DbError>
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
    let mut writes = ImportWrites::<DB> { conn };
    let mut images = Vec::new();
    for resource in resources {
        let item = ItemId::from(u64::from(*resource));
        if let Some(image) = writes.before_image(ImportResource::Item(item)).await? {
            images.push((item, image));
        }
    }
    Ok(images)
}

/// One entry per item of `before` that the write changed.
pub(crate) async fn record_each<DB: Backend>(
    conn: &mut DB::Connection,
    before: &[(ItemId, BeforeImage)],
    stamp: &HistoryStamp,
) -> Result<(), DbError>
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
    let mut writes = ImportWrites::<DB> { conn };
    for (item, image) in before {
        writes.record_history(*item, Some(image), stamp).await?;
    }
    Ok(())
}

async fn newest_entry<DB: Backend>(
    conn: &mut DB::Connection,
    item: ItemId,
) -> Result<Option<HistoryEntryRow>, DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Type<DB>,
    for<'x> &'x str: ColumnIndex<DB::Row>,
{
    let row =
        sqlx::query("SELECT * FROM romeka_item_history WHERE item_id = ? ORDER BY id DESC LIMIT 1")
            .bind(bind_id(item))
            .fetch_optional(&mut *conn)
            .await?;
    row.as_ref()
        .map(HistoryEntryRow::from_db_row::<DB>)
        .transpose()
}

/// Delete the entries of an item that is gone for good, on the caller's connection.
pub(crate) async fn forget_item<DB: Backend>(
    conn: &mut DB::Connection,
    item: ItemId,
) -> Result<(), DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'x> i64: Encode<'x, DB> + Type<DB>,
{
    sqlx::query("DELETE FROM romeka_item_history WHERE item_id = ?")
        .bind(bind_id(item))
        .execute(conn)
        .await?;
    Ok(())
}

/// The read side of `romeka_item_history`.
pub struct ItemHistoryRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> ItemHistoryRepo<'a, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Type<DB>,
    for<'x> &'x str: ColumnIndex<DB::Row>,
{
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    /// The item's entries, newest first.
    pub async fn list(&self, item: ItemId) -> Result<Vec<HistoryEntryRow>, DbError> {
        let rows =
            sqlx::query("SELECT * FROM romeka_item_history WHERE item_id = ? ORDER BY id DESC")
                .bind(bind_id(item))
                .fetch_all(self.pool)
                .await?;
        rows.iter()
            .map(HistoryEntryRow::from_db_row::<DB>)
            .collect()
    }
}
