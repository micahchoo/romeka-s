//! The second pass: the links between this import's own records
//! (`docs/import-v2.md`, "Values" and "Write details").
//!
//! Once every record exists, each link value is written to the resource its
//! record wrote, and the record's ledger row takes the time of that write,
//! so undo never counts the import's own link as touching. A link joins the
//! values pass one wrote for its term. Under `revise`, `update` and
//! `replace`, a term that holds only links replaces the matched resource's
//! values of that term, as pass one would have done had the links been
//! known then. Append keeps values distinct by link and a replace writes the
//! same values again, so a retry that runs the pass again adds nothing
//! twice.

use omeka_db::Backend;
use omeka_db::DbError;
use omeka_db::ids::ResourceId;
use omeka_db::repos::{ImportResourceRepo, ImportWrites};
use omeka_db::rows::{ImportAction, ImportFields, ImportTarget, ItemSlots, Slot, ValueContent};
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Type};

use super::fields::{content, record_link_type, values as values_of};
use super::units::catalogue_key;
use super::{Run, RunError};
use crate::import::model::{Link, Record};
use crate::import::plan::SlotName;
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
        let derived: &'r crate::import::preview::Derived = self.derived;
        for record in derived.model.kept() {
            let values = self.link_values(record);
            if values.is_empty() {
                continue;
            }
            let Some(target) = self.target_of(record.id) else {
                continue;
            };
            let (replace, append) = if self.action == ImportAction::Append {
                (Vec::new(), values)
            } else {
                let written: Vec<_> = values_of(&self.lookup, record)
                    .iter()
                    .map(|value| value.property_id)
                    .collect();
                values
                    .into_iter()
                    .partition(|value| !written.contains(&value.property_id))
            };
            if let Err(error) = self.write_links(target, replace, append).await {
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

    /// `replace` takes the place of its terms' values (`revise` names only
    /// the terms its values hold); `append` joins what is there.
    async fn write_links(
        &self,
        target: ImportTarget,
        replace: Vec<ValueContent>,
        append: Vec<ValueContent>,
    ) -> Result<(), DbError> {
        let mut tx = self.pool.begin().await?;
        let mut writes = ImportWrites::<DB>::new(&mut tx);
        let passes = [
            (ImportAction::Revise, replace),
            (ImportAction::Append, append),
        ];
        for (action, values) in passes {
            if values.is_empty() {
                continue;
            }
            let fields = ImportFields {
                values,
                ..ImportFields::default()
            };
            match target {
                ImportTarget::Item(id) => {
                    writes
                        .apply_item(id, action, &fields, &ItemSlots::default())
                        .await?;
                }
                ImportTarget::Media(id) => {
                    writes.apply_media(id, action, &fields).await?;
                }
                ImportTarget::ItemSet(id) => {
                    writes
                        .apply_item_set(id, action, &fields, &Slot::Unmapped)
                        .await?;
                }
                ImportTarget::ResourceTemplate(_) | ImportTarget::Property(_) => return Ok(()),
            }
        }
        ImportResourceRepo::<DB>::touch(&mut *tx, self.import, target).await?;
        tx.commit().await?;
        Ok(())
    }
}
