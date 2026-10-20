//! The second pass: the links between this import's own records
//! (`docs/import-v2.md`, "Values" and "Write details").
//!
//! Once every record exists, each link value is appended to the resource
//! its record wrote, and the record's ledger row takes the time of that
//! write, so undo never counts the import's own link as touching. Append
//! keeps values distinct by link, so a retry that runs the pass again adds
//! nothing twice.

use omeka_db::Backend;
use omeka_db::DbError;
use omeka_db::ids::ResourceId;
use omeka_db::repos::{ImportResourceRepo, ImportWrites};
use omeka_db::rows::{ImportAction, ImportFields, ImportTarget, ItemSlots, Slot, ValueContent};
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Type};

use super::fields::{content, record_link_type};
use super::units::catalogue_key;
use super::{Run, RunError};
use crate::import2::model::{Link, Record};
use crate::import2::plan::SlotName;
use crate::messages;

fn resource(target: ImportTarget) -> ResourceId {
    match target {
        ImportTarget::Item(id) => id.as_resource(),
        ImportTarget::Media(id) => id.as_resource(),
        ImportTarget::ItemSet(id) => id.as_resource(),
        ImportTarget::ResourceTemplate(id) => ResourceId(id.0),
        ImportTarget::Property(id) => ResourceId(id.0),
    }
}

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
    pub(super) async fn links(&mut self) -> Result<(), RunError> {
        let derived: &'r crate::import2::preview::Derived = self.derived;
        for record in derived.model.kept() {
            let values = self.link_values(record);
            if values.is_empty() {
                continue;
            }
            let Some(target) = self.target_of(record.id) else {
                continue;
            };
            if let Err(error) = self.write_links(target, values).await {
                self.fail(
                    record.origin.key(),
                    &catalogue_key("import_links_not_saved"),
                    messages::import_links_not_saved(error),
                );
            }
        }
        Ok(())
    }

    /// The record's links to records that were written.
    fn link_values(&self, record: &Record) -> Vec<ValueContent> {
        let mut out = Vec::new();
        for (slot, values) in &record.slots {
            let SlotName::Term(term) = slot else {
                continue;
            };
            let Some(property) = self.lookup.properties.get(term) else {
                continue;
            };
            for value in values {
                let Some(Link::Record(other)) = value.link else {
                    continue;
                };
                let Some(target) = self.target_of(other) else {
                    continue;
                };
                let kind = self.model().record(other).kind;
                out.push(content(
                    *property,
                    value,
                    Some((resource(target), record_link_type(kind))),
                ));
            }
        }
        out
    }

    async fn write_links(
        &self,
        target: ImportTarget,
        values: Vec<ValueContent>,
    ) -> Result<(), DbError> {
        let fields = ImportFields {
            values,
            ..ImportFields::default()
        };
        let mut tx = self.pool.begin().await?;
        let mut writes = ImportWrites::<DB>::new(&mut tx);
        let append = ImportAction::Append;
        match target {
            ImportTarget::Item(id) => {
                writes
                    .apply_item(id, append, &fields, &ItemSlots::default())
                    .await?;
            }
            ImportTarget::Media(id) => {
                writes.apply_media(id, append, &fields).await?;
            }
            ImportTarget::ItemSet(id) => {
                writes
                    .apply_item_set(id, append, &fields, &Slot::Unmapped)
                    .await?;
            }
            ImportTarget::ResourceTemplate(_) | ImportTarget::Property(_) => return Ok(()),
        }
        ImportResourceRepo::<DB>::touch(&mut *tx, self.import, target).await?;
        tx.commit().await?;
        Ok(())
    }
}
