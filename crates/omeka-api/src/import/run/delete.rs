//! The `delete` action (`docs/import-v2.md`, "Actions"): each matched
//! resource goes through the API's delete path, so an item takes its media
//! with it and a media's files leave the store.
//!
//! A delete is not undoable. Its ledger row says `deleted`, for the report
//! and for a retry, which skips the record; the import then never starts an
//! undo. Media go first, then items, then item sets, so each row names a
//! resource this run removed rather than one its item took with it.

use std::collections::HashSet;

use omeka_db::Backend;
use omeka_db::repos::ImportResourceRepo;
use omeka_db::rows::{ImportChange, ImportTarget, NewImportResource};
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Type};

use super::fields::matched;
use super::units::catalogue_key;
use super::{Next, Run, RunEnd, RunError};
use crate::import::model::Record;
use crate::import::plan::RecordKind;
use crate::messages;

impl<'r, DB> Run<'r, DB>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> f64: Decode<'x, DB> + Type<DB>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
    for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    pub(super) async fn delete_all(&mut self) -> Result<RunEnd, RunError> {
        let derived: &'r crate::import::preview::Derived = self.derived;
        let rank = |kind| match kind {
            RecordKind::Media => 0,
            RecordKind::Item => 1,
            RecordKind::ItemSet => 2,
        };
        let mut order: Vec<&'r Record> = derived.model.kept().collect();
        order.sort_by_key(|record| rank(record.kind));
        let mut gone = HashSet::new();
        for record in order {
            self.delete_one(record, &mut gone).await?;
            match self.pulse().await? {
                Next::Go => {}
                Next::Stop => {
                    self.summary.stopped = true;
                    break;
                }
                Next::Lost => return Ok(RunEnd::Lost),
            }
        }
        self.end().await
    }

    /// Delete one matched resource and record it. Two records that matched
    /// one resource delete it once.
    async fn delete_one(
        &mut self,
        record: &Record,
        gone: &mut HashSet<ImportTarget>,
    ) -> Result<(), RunError> {
        self.done();
        let key = record.origin.key();
        let Some(target) = matched(record) else {
            return Ok(());
        };
        if self.written.holds(&key) || !gone.insert(target) {
            return Ok(());
        }
        if let Err(reason) = self.env.removal.remove(target).await {
            gone.remove(&target);
            self.fail(
                key,
                &catalogue_key("import_record_not_deleted"),
                messages::import_record_not_deleted(reason),
            );
            return Ok(());
        }
        ImportResourceRepo::<DB>::record(
            self.pool,
            self.import,
            &NewImportResource {
                target,
                record: key.clone(),
                change: ImportChange::Deleted,
            },
        )
        .await?;
        self.ids.insert(record.id, target);
        self.written.add(key, target);
        Ok(())
    }
}
