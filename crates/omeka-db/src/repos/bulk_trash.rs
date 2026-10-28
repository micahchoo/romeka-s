//! A bulk delete's turn on one record, its undo's, and the purge of its trash.
//!
//! A run's targets are items, or for a delete of media or of item sets ([`TrashKind`]) media or
//! sets, whose ids `romeka_bulk_target.item_id` holds. A delete reads the record's trash snapshot
//! ([`TrashImage`]), deletes the rows, and moves the target to `done` with the snapshot in one
//! transaction, so the record is either in place or in the trash. An undo inserts the rows again
//! with their ids. It puts back what still has a place: a column that names a row since deleted
//! (an owner, a class, an item type, a thumbnail) is emptied, a value whose property or linked
//! resource is gone stays out, a set membership comes back only if the item and the set both
//! exist, a site membership only if the site does and is one the caller may assign items to,
//! and a site's row that showed a set only if the site exists. A value of another resource that
//! linked to the record comes back only when that resource is still there and was not edited
//! since the delete; the others are reported. When a row holds one of the ids the record needs,
//! or a media's item is gone, the record stays in the trash (`kept`, with
//! [`FailReason::IdTaken`] or [`FailReason::ItemGone`]).
//!
//! The files are the caller's: this crate names no store. The purge claims the runs whose trash
//! is due by setting `purged` (which stops them from running or undoing again), and then clears
//! each target's snapshot after the caller has deleted its files.

use std::collections::BTreeSet;

use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Transaction, Type};

use super::bulk_writes::{fail, lock_target, set_state};
use super::item_history::{images_before, record_each};
use super::trash_rows::{TrashRows, by_table, named, null_missing};
use super::{BulkRepo, ImportWrites, bind_id, json_text};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{BulkId, ItemId, SiteId};
use crate::rows::TrashKind;
use crate::rows::trash::{TABLES, VALUE, int};
use crate::rows::{FailReason, HistoryStamp, TargetState, TrashImage, TrashRow};
use crate::time;

/// What one target's turn in a delete did.
#[derive(Debug, Clone, PartialEq)]
pub enum Trashed {
    /// The target was not pending: an earlier run dealt with it.
    Settled,
    /// In the trash; the caller moves the files the snapshot names.
    Trashed(Box<TrashImage>),
    Failed(FailReason),
}

/// What one target's turn in an undo of a delete did.
#[derive(Debug, Clone, PartialEq)]
pub enum Untrashed {
    /// The target was neither done nor kept.
    Settled,
    /// Back in place. `links_left` names the resources whose links to it stayed out.
    Restored {
        image: Box<TrashImage>,
        links_left: Vec<u64>,
    },
    /// A row holds an id the record needs, or a media's item is gone; it stays in the trash.
    Kept,
    /// The target has no snapshot: its run's trash was emptied.
    Purged,
}

/// The run states a purge may claim: those a job is not working in, with trash left.
const RESTING: &str = "('done', 'stopped', 'failed', 'partly_undone')";

impl<DB: Backend> BulkRepo<'_, DB>
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
    /// Delete one pending target, a record of `kind`, into the trash. `refuse` reads the
    /// snapshot and says why the record may not be deleted, or nothing. Each other item the
    /// delete changes (a member of a set, an item whose value linked to the record) gains a
    /// history entry under `stamp`.
    pub async fn trash_target(
        &self,
        id: BulkId,
        kind: TrashKind,
        item: ItemId,
        stamp: &HistoryStamp,
        refuse: impl FnOnce(&TrashImage) -> Option<FailReason>,
    ) -> Result<Trashed, DbError> {
        let mut tx = self.pool.begin_with(DB::begin_write()).await?;
        if lock_target(&mut tx, id, item).await? != Some(TargetState::Pending) {
            tx.rollback().await?;
            return Ok(Trashed::Settled);
        }
        let mut rows = TrashRows::<DB> { conn: &mut *tx };
        let Some(image) = rows.snapshot(kind, item.0).await? else {
            fail(&mut tx, id, item, FailReason::Gone).await?;
            tx.commit().await?;
            return Ok(Trashed::Failed(FailReason::Gone));
        };
        if let Some(reason) = refuse(&image) {
            fail(&mut tx, id, item, reason).await?;
            tx.commit().await?;
            return Ok(Trashed::Failed(reason));
        }
        let before = images_before::<DB>(&mut *rows.conn, &image.others_touched()).await?;
        rows.remove(&image).await?;
        record_each::<DB>(&mut *rows.conn, &before, stamp).await?;
        sqlx::query(
            "UPDATE romeka_bulk_target SET state = 'done', image = ?, written = ?, error = NULL
             WHERE bulk_id = ? AND item_id = ?",
        )
        .bind(json_text(&image.to_json())?)
        .bind(time::format(time::now()))
        .bind(bind_id(id))
        .bind(bind_id(item))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Trashed::Trashed(Box::new(image)))
    }

    /// The trash snapshot of a target; `None` when it has none.
    pub async fn trash_image(
        &self,
        id: BulkId,
        item: ItemId,
    ) -> Result<Option<TrashImage>, DbError> {
        let row =
            sqlx::query("SELECT image FROM romeka_bulk_target WHERE bulk_id = ? AND item_id = ?")
                .bind(bind_id(id))
                .bind(bind_id(item))
                .fetch_optional(self.pool)
                .await?;
        let image = match row {
            Some(row) => DB::opt_json(&row, "image")?,
            None => None,
        };
        image.as_ref().map(TrashImage::from_json).transpose()
    }

    /// The trash snapshots of those `records` (item, media or set ids) that are in the trash
    /// now: a target of a delete run whose trash is not purged, deleted and not put back.
    pub async fn in_trash(&self, records: &[u64]) -> Result<Vec<TrashImage>, DbError> {
        if records.is_empty() {
            return Ok(Vec::new());
        }
        let marks = vec!["?"; records.len()].join(", ");
        let sql = format!(
            "SELECT t.image FROM romeka_bulk_target t
             JOIN romeka_bulk b ON b.id = t.bulk_id
             WHERE b.kind = 'delete' AND b.purged IS NULL AND t.image IS NOT NULL
               AND t.state IN ('done', 'kept') AND t.item_id IN ({marks})
             ORDER BY t.bulk_id"
        );
        let mut query = sqlx::query::<DB>(&sql);
        for record in records {
            query = query.bind(i64::try_from(*record).unwrap_or(i64::MAX));
        }
        let rows = query.fetch_all(self.pool).await?;
        rows.iter()
            .map(|row| TrashImage::from_json(&DB::json(row, "image")?))
            .collect()
    }

    /// Put one done or kept target of a delete back in place. `sites` are the sites an item may
    /// be put back in: those the caller may assign items to. Under `stamp`, a restored item's
    /// history gains one entry from nothing to the item as it came back, and each other item the
    /// undo changes (a member of a set, an item whose link came back) one entry too.
    pub async fn untrash_target(
        &self,
        id: BulkId,
        item: ItemId,
        sites: &BTreeSet<SiteId>,
        stamp: &HistoryStamp,
    ) -> Result<Untrashed, DbError> {
        let mut tx = self.pool.begin_with(DB::begin_write()).await?;
        if !matches!(
            lock_target(&mut tx, id, item).await?,
            Some(TargetState::Done | TargetState::Kept)
        ) {
            tx.rollback().await?;
            return Ok(Untrashed::Settled);
        }
        let stored =
            sqlx::query("SELECT image FROM romeka_bulk_target WHERE bulk_id = ? AND item_id = ?")
                .bind(bind_id(id))
                .bind(bind_id(item))
                .fetch_one(&mut *tx)
                .await?;
        let Some(image) = DB::opt_json(&stored, "image")? else {
            tx.rollback().await?;
            return Ok(Untrashed::Purged);
        };
        let image = TrashImage::from_json(&image)?;
        let mut rows = TrashRows::<DB> { conn: &mut *tx };
        if let Some(reason) = blocked(&mut rows, &image).await? {
            keep(&mut tx, id, item, reason).await?;
            tx.commit().await?;
            return Ok(Untrashed::Kept);
        }
        let before = images_before::<DB>(&mut *rows.conn, &image.others_touched()).await?;
        put_back(&mut rows, &image, sites).await?;
        let links_left = relink(&mut rows, &image).await?;
        if image.kind == TrashKind::Item {
            ImportWrites::<DB> {
                conn: &mut *rows.conn,
            }
            .record_history(item, None, stamp)
            .await?;
        }
        record_each::<DB>(&mut *rows.conn, &before, stamp).await?;
        set_state(&mut tx, id, item, TargetState::Restored).await?;
        if !links_left.is_empty() {
            sqlx::query(
                "UPDATE romeka_bulk_target SET error = ? WHERE bulk_id = ? AND item_id = ?",
            )
            .bind(FailReason::LinksLeft.as_str())
            .bind(bind_id(id))
            .bind(bind_id(item))
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(Untrashed::Restored {
            image: Box::new(image),
            links_left,
        })
    }

    /// Claim the delete runs whose trash is due, by setting their `purged`: each resting run
    /// that ended before `ended_before`, or every resting one for `None`. A claimed run neither
    /// runs nor undoes again. Answers the runs claimed.
    pub async fn claim_purge(
        &self,
        ended_before: Option<NaiveDateTime>,
    ) -> Result<Vec<BulkId>, DbError> {
        let rows = sqlx::query(&format!(
            "SELECT id, ended FROM romeka_bulk
             WHERE kind = 'delete' AND purged IS NULL AND status IN {RESTING}"
        ))
        .fetch_all(self.pool)
        .await?;
        let now = time::format(time::now());
        let mut claimed = Vec::new();
        for row in &rows {
            let ended = DB::opt_datetime(row, "ended")?;
            let due = match ended_before {
                None => true,
                Some(before) => ended.is_some_and(|ended| ended < before),
            };
            if !due {
                continue;
            }
            let run: BulkId = DB::id(row, "id")?;
            let done = sqlx::query(&format!(
                "UPDATE romeka_bulk SET purged = ?
                 WHERE id = ? AND purged IS NULL AND status IN {RESTING}"
            ))
            .bind(now.clone())
            .bind(bind_id(run))
            .execute(self.pool)
            .await?;
            if DB::rows_affected(&done) > 0 {
                claimed.push(run);
            }
        }
        Ok(claimed)
    }

    /// Up to `limit` targets of purged runs that still hold a snapshot.
    pub async fn purged_trash(
        &self,
        limit: i64,
    ) -> Result<Vec<(BulkId, ItemId, TrashImage)>, DbError> {
        let rows = sqlx::query(
            "SELECT t.bulk_id, t.item_id, t.image FROM romeka_bulk_target t
             JOIN romeka_bulk b ON b.id = t.bulk_id
             WHERE b.kind = 'delete' AND b.purged IS NOT NULL AND t.image IS NOT NULL
               AND t.state IN ('done', 'kept')
             ORDER BY t.bulk_id, t.item_id LIMIT ?",
        )
        .bind(limit)
        .fetch_all(self.pool)
        .await?;
        rows.iter()
            .map(|row| {
                let image = DB::json(row, "image")?;
                Ok((
                    DB::id(row, "bulk_id")?,
                    DB::id(row, "item_id")?,
                    TrashImage::from_json(&image)?,
                ))
            })
            .collect()
    }

    /// Drop a purged target's snapshot, once its files are gone, and the history of its item,
    /// which can no longer come back.
    pub async fn clear_trash(&self, id: BulkId, item: ItemId) -> Result<(), DbError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE romeka_bulk_target SET image = NULL WHERE bulk_id = ? AND item_id = ?")
            .bind(bind_id(id))
            .bind(bind_id(item))
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "DELETE FROM romeka_item_history
             WHERE item_id = ? AND NOT EXISTS (SELECT 1 FROM item WHERE id = ?)",
        )
        .bind(bind_id(item))
        .bind(bind_id(item))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Record why an undo left a target where it is, in the state it was.
    pub async fn note_refusal(
        &self,
        id: BulkId,
        item: ItemId,
        reason: FailReason,
    ) -> Result<(), DbError> {
        sqlx::query("UPDATE romeka_bulk_target SET error = ? WHERE bulk_id = ? AND item_id = ?")
            .bind(reason.as_str())
            .bind(bind_id(id))
            .bind(bind_id(item))
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// Whether a media row holds `storage_id`, so its files at their own paths are in use.
    pub async fn storage_in_use(&self, storage_id: &str) -> Result<bool, DbError> {
        let row = sqlx::query("SELECT id FROM media WHERE storage_id = ?")
            .bind(storage_id.to_string())
            .fetch_optional(self.pool)
            .await?;
        Ok(row.is_some())
    }
}

/// Why the snapshot cannot go back: a row holds a resource id, a value id or a storage id it
/// needs, or the item of a media is gone.
async fn blocked<DB: Backend>(
    rows: &mut TrashRows<'_, DB>,
    image: &TrashImage,
) -> Result<Option<FailReason>, DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let resources = image.resources();
    let values = named(image.rows("value"), "id");
    let storage: Vec<String> = image
        .media_files()
        .into_iter()
        .map(|media| media.storage_id)
        .collect();
    if !rows
        .existing("resource", "id", &resources)
        .await?
        .is_empty()
        || !rows.existing("value", "id", &values).await?.is_empty()
        || rows.storage_taken(&storage).await?
    {
        return Ok(Some(FailReason::IdTaken));
    }
    if let Some(item) = image.item_of_media() {
        let item = BTreeSet::from([item.0]);
        if rows.existing("item", "id", &item).await?.is_empty() {
            return Ok(Some(FailReason::ItemGone));
        }
    }
    Ok(None)
}

/// Insert the snapshot's rows, each table after the tables it names, with every reference to a
/// row since deleted emptied or left out, and fill again each pointer the delete emptied.
async fn put_back<DB: Backend>(
    rows: &mut TrashRows<'_, DB>,
    image: &TrashImage,
    sites: &BTreeSet<SiteId>,
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
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let primary = image
        .rows("item")
        .first()
        .and_then(|row| int(row, "primary_media_id"));
    let ready = fitted(rows, image, sites).await?;
    for table in TABLES {
        let Some((_, rows_of)) = ready.iter().find(|(name, _)| *name == table.name) else {
            continue;
        };
        for row in rows_of {
            rows.insert(table, row).await?;
        }
    }
    let item_id = image
        .rows("item")
        .first()
        .and_then(|row| int(row, "id"))
        .unwrap_or_default();
    if let Some(primary) = primary {
        sqlx::query("UPDATE item SET primary_media_id = ? WHERE id = ?")
            .bind(primary as i64)
            .bind(item_id as i64)
            .execute(&mut *rows.conn)
            .await?;
    }
    for emptied in &image.emptied {
        let (table, column) = (emptied.pointer.table, emptied.pointer.column);
        sqlx::query(&format!(
            "UPDATE {table} SET {column} = ? WHERE id = ? AND {column} IS NULL"
        ))
        .bind(emptied.target as i64)
        .bind(emptied.row as i64)
        .execute(&mut *rows.conn)
        .await?;
    }
    Ok(())
}

/// The snapshot's rows as an undo inserts them: each reference to a row since deleted emptied,
/// each row that cannot stand without one left out, and an item's primary media emptied until
/// its media are in.
async fn fitted<DB: Backend>(
    rows: &mut TrashRows<'_, DB>,
    image: &TrashImage,
    sites: &BTreeSet<SiteId>,
) -> Result<Vec<(&'static str, Vec<TrashRow>)>, DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let mut tables = by_table(image);
    let own = image.resources();
    let mut table = |name: &'static str| tables.remove(name).unwrap_or_default();
    let (mut resources, mut item, mut values) = (table("resource"), table("item"), table("value"));
    let (mut sets, mut site_rows, mut notes) = (
        table("item_item_set"),
        table("item_site"),
        table("romeka_media_annotation"),
    );
    let mut shown_in = table("site_item_set");
    for (column, target) in [
        ("owner_id", ("user", "id")),
        ("resource_class_id", ("resource_class", "id")),
        ("resource_template_id", ("resource_template", "id")),
        ("thumbnail_id", ("asset", "id")),
    ] {
        let found = rows
            .existing(target.0, target.1, &named(&resources, column))
            .await?;
        null_missing(&mut resources, column, &found);
    }
    let users = rows
        .existing("user", "id", &named(&notes, "owner_id"))
        .await?;
    null_missing(&mut notes, "owner_id", &users);
    let properties = rows
        .existing("property", "id", &named(&values, "property_id"))
        .await?;
    let outside: BTreeSet<u64> = named(&values, "value_resource_id")
        .difference(&own)
        .copied()
        .collect();
    let linked = rows.existing("resource", "id", &outside).await?;
    values.retain(|row| {
        int(row, "property_id").is_some_and(|id| properties.contains(&id))
            && int(row, "value_resource_id")
                .is_none_or(|id| own.contains(&id) || linked.contains(&id))
    });
    let mut members = rows
        .existing("item", "id", &named(&sets, "item_id"))
        .await?;
    members.extend(
        rows.existing("item_set", "id", &named(&sets, "item_set_id"))
            .await?,
    );
    members.extend(&own);
    sets.retain(|row| {
        ["item_id", "item_set_id"]
            .iter()
            .all(|column| int(row, column).is_some_and(|id| members.contains(&id)))
    });
    let showing = rows
        .existing("site", "id", &named(&shown_in, "site_id"))
        .await?;
    shown_in.retain(|row| int(row, "site_id").is_some_and(|id| showing.contains(&id)));
    let site_ids = rows
        .existing("site", "id", &named(&site_rows, "site_id"))
        .await?;
    site_rows.retain(|row| {
        int(row, "site_id").is_some_and(|id| site_ids.contains(&id) && sites.contains(&SiteId(id)))
    });
    for row in &mut item {
        row.insert("primary_media_id".to_string(), Value::Null);
    }
    let mut ready: Vec<(&'static str, Vec<TrashRow>)> = vec![
        ("resource", resources),
        ("item", item),
        ("value", values),
        ("item_item_set", sets),
        ("item_site", site_rows),
        ("site_item_set", shown_in),
        ("romeka_media_annotation", notes),
    ];
    ready.extend(tables);
    Ok(ready)
}

/// Insert the values of other resources that linked to the record, where the resource is there
/// and unedited since the delete. Answers the resources whose links stayed out.
async fn relink<DB: Backend>(
    rows: &mut TrashRows<'_, DB>,
    image: &TrashImage,
) -> Result<Vec<u64>, DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let mut left = BTreeSet::new();
    for link in &image.incoming {
        let Some(resource) = link.resource().map(|id| id.0) else {
            continue;
        };
        let unedited = rows.modified(resource).await? == Some(link.modified);
        let property = named(std::slice::from_ref(&link.value), "property_id");
        let id = named(std::slice::from_ref(&link.value), "id");
        let fits = unedited
            && !rows.existing("property", "id", &property).await?.is_empty()
            && rows.existing("value", "id", &id).await?.is_empty();
        if !fits {
            left.insert(resource);
            continue;
        }
        let mut value = link.value.clone();
        if let Some(annotation) = int(&value, "value_annotation_id") {
            let one = BTreeSet::from([annotation]);
            let free = !rows
                .existing("value_annotation", "id", &one)
                .await?
                .is_empty()
                && rows
                    .existing("value", "value_annotation_id", &one)
                    .await?
                    .is_empty();
            if !free {
                value.insert("value_annotation_id".to_string(), Value::Null);
            }
        }
        rows.insert(&VALUE, &value).await?;
    }
    Ok(left.into_iter().collect())
}

/// Leave the target in the trash, for `reason`.
async fn keep<DB: Backend>(
    tx: &mut Transaction<'_, DB>,
    id: BulkId,
    item: ItemId,
    reason: FailReason,
) -> Result<(), DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'x> i64: Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB>,
{
    sqlx::query(
        "UPDATE romeka_bulk_target SET state = 'kept', error = ? WHERE bulk_id = ? AND item_id = ?",
    )
    .bind(reason.as_str())
    .bind(bind_id(id))
    .bind(bind_id(item))
    .execute(&mut **tx)
    .await?;
    Ok(())
}
