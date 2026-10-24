//! One item's turn in a bulk run or its undo, each one transaction.
//!
//! A run's turn reads the item's before-image, asks the caller what to write, writes it, and
//! moves the target out of `pending` with the image and the `modified` the write left, so the
//! write and its record commit together or not at all. A rerun finds the target settled and
//! skips it. An undo's turn restores the image unless the item's `modified` is no longer the one
//! the run left, which means someone edited it since; then the target is `kept` unless forced.
//! The caller then reads the item as it is and the image, and says what to restore, that the
//! item stays (`kept`, for a change `modified` does not show), or refuses; a refused target keeps
//! its state, with the reason, so a later undo may restore it.
//!
//! A run's value writes touch only the rows they name: a value updated in place keeps its id and
//! its annotation, as upstream's batch visibility edits the value entity it has.
//!
//! The before-image and the restore are the import's own (`ImportWrites`), so a bulk edit and an
//! import read and put back an item the same way. The restore writes every value again, so value
//! ids change on undo.

use chrono::NaiveDateTime;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Transaction, Type};

use super::values::insert_value;
use super::{BulkRepo, ImportWrites, bind_id, bind_opt_id, json_text};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{BulkId, ItemId};
use crate::rows::{
    Applied, BeforeImage, Described, FailReason, ImportAction, ImportFields, ImportResource,
    ItemSlots, Restore, Reverted, Slot, TargetState, ValueWrites, Verdict,
};
use crate::time;

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
    /// Run one pending target: `decide` reads the item's before-image and says what to write.
    pub async fn apply_target(
        &self,
        id: BulkId,
        item: ItemId,
        decide: impl FnOnce(&BeforeImage) -> Verdict,
    ) -> Result<Applied, DbError> {
        let mut tx = self.pool.begin_with(DB::begin_write()).await?;
        if lock_target(&mut tx, id, item).await? != Some(TargetState::Pending) {
            tx.rollback().await?;
            return Ok(Applied::Settled);
        }
        let target = ImportResource::Item(item);
        let mut writes = ImportWrites::<DB>::new(&mut tx);
        let Some(image) = writes.before_image(target).await? else {
            fail(&mut tx, id, item, FailReason::Gone).await?;
            tx.commit().await?;
            return Ok(Applied::Failed(FailReason::Gone));
        };
        let applied = match decide(&image) {
            Verdict::Refuse(reason) => {
                fail(&mut tx, id, item, reason).await?;
                Applied::Failed(reason)
            }
            Verdict::Unchanged => {
                settle(&mut tx, id, item, TargetState::Unchanged, None).await?;
                Applied::Unchanged
            }
            Verdict::Write(written) => {
                write_values(&mut writes, item, &written.values).await?;
                let (fields, slots) = import_fields(&written.fields);
                writes
                    .apply_item(item, ImportAction::Update, &fields, &slots)
                    .await?;
                let written = writes.modified_of(target).await?.flatten();
                settle(
                    &mut tx,
                    id,
                    item,
                    TargetState::Done,
                    Some((&image, written)),
                )
                .await?;
                Applied::Written
            }
        };
        tx.commit().await?;
        Ok(applied)
    }

    /// Undo one done or kept target. `force` restores an item edited since the run wrote it.
    /// `decide` reads the item's image now and the image the run kept, and answers the image to
    /// restore, that the item stays as it is, or why it may not be restored.
    pub async fn revert_target(
        &self,
        id: BulkId,
        item: ItemId,
        force: bool,
        decide: impl FnOnce(&BeforeImage, BeforeImage) -> Restore,
    ) -> Result<Reverted, DbError> {
        let mut tx = self.pool.begin_with(DB::begin_write()).await?;
        if !matches!(
            lock_target(&mut tx, id, item).await?,
            Some(TargetState::Done | TargetState::Kept)
        ) {
            tx.rollback().await?;
            return Ok(Reverted::Settled);
        }
        let row = sqlx::query(
            "SELECT image, written FROM romeka_bulk_target WHERE bulk_id = ? AND item_id = ?",
        )
        .bind(bind_id(id))
        .bind(bind_id(item))
        .fetch_one(&mut *tx)
        .await?;
        let image = DB::opt_json(&row, "image")?.ok_or_else(|| DbError::Column {
            column: "image",
            reason: format!("the done target {item} of bulk run {id} has no before-image"),
        })?;
        let image = BeforeImage::from_json(&image)?;
        let written = DB::opt_datetime(&row, "written")?;
        let target = ImportResource::Item(item);
        let mut writes = ImportWrites::<DB>::new(&mut tx);
        let reverted = match writes.modified_of(target).await? {
            None => Reverted::Gone,
            Some(now) if now != written && !force => Reverted::Kept,
            Some(_) => match writes.before_image(target).await? {
                None => Reverted::Gone,
                Some(now) => match decide(&now, image) {
                    Restore::Image(restore) => {
                        writes.restore(target, &restore).await?;
                        Reverted::Restored
                    }
                    Restore::Keep => Reverted::Kept,
                    Restore::Refuse(reason) => Reverted::Refused(reason),
                },
            },
        };
        match reverted {
            Reverted::Gone => fail(&mut tx, id, item, FailReason::Gone).await?,
            Reverted::Kept => set_state(&mut tx, id, item, TargetState::Kept).await?,
            Reverted::Restored => set_state(&mut tx, id, item, TargetState::Restored).await?,
            Reverted::Refused(reason) => refuse(&mut tx, id, item, reason).await?,
            Reverted::Settled => {}
        }
        tx.commit().await?;
        Ok(reverted)
    }
}

/// Write a run's value changes on `item`, whose values the transaction read for the before-image
/// in the same id order, so each place names the row it named there.
async fn write_values<DB: Backend>(
    writes: &mut ImportWrites<'_, DB>,
    item: ItemId,
    changes: &ValueWrites,
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
    if changes.is_empty() {
        return Ok(());
    }
    let resource = item.as_resource();
    let rows = writes.values_of(resource).await?;
    let row = |at: usize| {
        rows.get(at)
            .ok_or(DbError::Conflict("a value the run planned from is gone"))
    };
    for at in &changes.delete {
        let row = row(*at)?;
        sqlx::query("DELETE FROM value WHERE id = ?")
            .bind(bind_id(row.id))
            .execute(&mut *writes.conn)
            .await?;
        if let Some(annotation) = row.value_annotation_id {
            let annotation = annotation.as_resource();
            writes.delete_values(annotation, None).await?;
            sqlx::query("DELETE FROM resource WHERE id = ?")
                .bind(bind_id(annotation))
                .execute(&mut *writes.conn)
                .await?;
        }
    }
    for (at, content) in &changes.update {
        sqlx::query(
            "UPDATE value SET value_resource_id = ?, type = ?, lang = ?, value = ?, uri = ?,
             is_public = ? WHERE id = ?",
        )
        .bind(bind_opt_id(content.value_resource_id))
        .bind(&content.r#type)
        .bind(&content.lang)
        .bind(&content.value)
        .bind(&content.uri)
        .bind(content.is_public)
        .bind(bind_id(row(*at)?.id))
        .execute(&mut *writes.conn)
        .await?;
    }
    for content in &changes.insert {
        insert_value(writes.conn, &content.on(resource)).await?;
    }
    Ok(())
}

/// The import's `update` over every describing field: each slot is mapped, so each is written,
/// and no value term is mapped, so no value changes.
fn import_fields(fields: &Described) -> (ImportFields, ItemSlots) {
    (
        ImportFields {
            values: Vec::new(),
            mapped_terms: Vec::new(),
            template: Slot::Mapped(fields.template),
            class: Slot::Mapped(fields.class),
            owner: Slot::Mapped(fields.owner),
            is_public: Slot::Mapped(fields.is_public),
        },
        ItemSlots {
            item_sets: Slot::Mapped(fields.item_sets.iter().copied().collect()),
            sites: Slot::Mapped(fields.sites.iter().copied().collect()),
        },
    )
}

/// The target's state, locked until the transaction ends; `None` when the run has no such target.
async fn lock_target<DB: Backend>(
    tx: &mut Transaction<'_, DB>,
    id: BulkId,
    item: ItemId,
) -> Result<Option<TargetState>, DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Type<DB>,
    for<'x> &'x str: ColumnIndex<DB::Row>,
{
    let sql = format!(
        "SELECT state FROM romeka_bulk_target WHERE bulk_id = ? AND item_id = ?{}",
        DB::for_update()
    );
    let row = sqlx::query(&sql)
        .bind(bind_id(id))
        .bind(bind_id(item))
        .fetch_optional(&mut **tx)
        .await?;
    row.map(|row| TargetState::parse(&DB::text(&row, "state")?))
        .transpose()
}

async fn settle<DB: Backend>(
    tx: &mut Transaction<'_, DB>,
    id: BulkId,
    item: ItemId,
    state: TargetState,
    wrote: Option<(&BeforeImage, Option<NaiveDateTime>)>,
) -> Result<(), DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'x> i64: Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB>,
{
    let (image, written) = match wrote {
        Some((image, written)) => (Some(json_text(&image.to_json())?), written),
        None => (None, None),
    };
    sqlx::query(
        "UPDATE romeka_bulk_target SET state = ?, image = ?, written = ?, error = NULL
         WHERE bulk_id = ? AND item_id = ?",
    )
    .bind(state.as_str())
    .bind(image)
    .bind(written.map(time::format))
    .bind(bind_id(id))
    .bind(bind_id(item))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn set_state<DB: Backend>(
    tx: &mut Transaction<'_, DB>,
    id: BulkId,
    item: ItemId,
    state: TargetState,
) -> Result<(), DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'x> i64: Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB>,
{
    sqlx::query(
        "UPDATE romeka_bulk_target SET state = ?, error = NULL WHERE bulk_id = ? AND item_id = ?",
    )
    .bind(state.as_str())
    .bind(bind_id(id))
    .bind(bind_id(item))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Record why an undo left the target, in the state it was.
async fn refuse<DB: Backend>(
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
    sqlx::query("UPDATE romeka_bulk_target SET error = ? WHERE bulk_id = ? AND item_id = ?")
        .bind(reason.as_str())
        .bind(bind_id(id))
        .bind(bind_id(item))
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn fail<DB: Backend>(
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
        "UPDATE romeka_bulk_target SET state = 'failed', error = ? WHERE bulk_id = ? AND item_id = ?",
    )
    .bind(reason.as_str())
    .bind(bind_id(id))
    .bind(bind_id(item))
    .execute(&mut **tx)
    .await?;
    Ok(())
}
