//! `romeka_item_history`: who changed what on an item, and when (migration 0014). Upstream has
//! none.
//!
//! An entry is the difference between two [`BeforeImage`]s of one item, the state before a write
//! and the state after it, cut to the fields that changed. A write that changes nothing makes no
//! entry. The item's owner, thumbnail, media and primary media are not fields of an entry.

use std::collections::BTreeMap;

use chrono::NaiveDateTime;
use serde_json::{Map, Value, json};

use super::import_writes::{value_from, value_json};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{
    BulkId, ClassId, ImportId, ItemId, ItemSetId, PropertyId, SiteId, TemplateId, UserId,
};
use crate::rows::{BeforeImage, ImageMemberships, ImageValue};

/// What made a write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistorySource {
    /// The item form, or any other single write through the API.
    Edit,
    Bulk(BulkId),
    Import(ImportId),
    /// The undo of a bulk run: an edit put back, or a delete taken out of the trash.
    BulkUndo(BulkId),
    ImportUndo(ImportId),
}

impl HistorySource {
    /// The stored `source`, `bulk_id` and `import_id`.
    pub(crate) fn columns(self) -> (&'static str, Option<BulkId>, Option<ImportId>) {
        match self {
            Self::Edit => ("edit", None, None),
            Self::Bulk(run) => ("bulk", Some(run), None),
            Self::Import(import) => ("import", None, Some(import)),
            Self::BulkUndo(run) => ("restore", Some(run), None),
            Self::ImportUndo(import) => ("restore", None, Some(import)),
        }
    }

    fn from_columns(
        source: &str,
        bulk: Option<BulkId>,
        import: Option<ImportId>,
    ) -> Result<Self, DbError> {
        Ok(match (source, bulk, import) {
            ("edit", None, None) => Self::Edit,
            ("bulk", Some(run), None) => Self::Bulk(run),
            ("import", None, Some(import)) => Self::Import(import),
            ("restore", Some(run), None) => Self::BulkUndo(run),
            ("restore", None, Some(import)) => Self::ImportUndo(import),
            _ => {
                return Err(DbError::Column {
                    column: "source",
                    reason: format!("`{source}` with bulk {bulk:?} and import {import:?}"),
                });
            }
        })
    }

    /// A run's later pass over an item it already changed folds into that entry: an import
    /// writes an item's links after the item. A single edit is always its own entry.
    pub(crate) fn folds(self) -> bool {
        !matches!(self, Self::Edit)
    }
}

/// Who made a write, and what made it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryStamp {
    pub user: Option<UserId>,
    pub source: HistorySource,
}

/// One changed field of an item, before and after. `before` is `None` where the item did not
/// exist before the write (a create, or an undo that took it out of the trash).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldChange {
    /// The values of one property, in their stored order.
    Values {
        property: PropertyId,
        before: Vec<ImageValue>,
        after: Vec<ImageValue>,
    },
    Visibility {
        before: Option<bool>,
        after: bool,
    },
    Class {
        before: Option<ClassId>,
        after: Option<ClassId>,
    },
    /// The item type (`resource_template`).
    Template {
        before: Option<TemplateId>,
        after: Option<TemplateId>,
    },
    ItemSets {
        before: Vec<ItemSetId>,
        after: Vec<ItemSetId>,
    },
    Sites {
        before: Vec<SiteId>,
        after: Vec<SiteId>,
    },
}

/// The key two changes of the same field share, which a fold matches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FieldKey {
    Values(PropertyId),
    Visibility,
    Class,
    Template,
    ItemSets,
    Sites,
}

impl FieldChange {
    fn key(&self) -> FieldKey {
        match self {
            Self::Values { property, .. } => FieldKey::Values(*property),
            Self::Visibility { .. } => FieldKey::Visibility,
            Self::Class { .. } => FieldKey::Class,
            Self::Template { .. } => FieldKey::Template,
            Self::ItemSets { .. } => FieldKey::ItemSets,
            Self::Sites { .. } => FieldKey::Sites,
        }
    }

    /// Whether the field ends where it started.
    fn is_noop(&self) -> bool {
        match self {
            Self::Values { before, after, .. } => before == after,
            Self::Visibility { before, after } => *before == Some(*after),
            Self::Class { before, after } => before == after,
            Self::Template { before, after } => before == after,
            Self::ItemSets { before, after } => before == after,
            Self::Sites { before, after } => before == after,
        }
    }

    /// This change's `before` with `later`'s `after`: one change across two writes.
    fn then(self, later: Self) -> Self {
        match (self, later) {
            (
                Self::Values {
                    property, before, ..
                },
                Self::Values { after, .. },
            ) => Self::Values {
                property,
                before,
                after,
            },
            (Self::Visibility { before, .. }, Self::Visibility { after, .. }) => {
                Self::Visibility { before, after }
            }
            (Self::Class { before, .. }, Self::Class { after, .. }) => {
                Self::Class { before, after }
            }
            (Self::Template { before, .. }, Self::Template { after, .. }) => {
                Self::Template { before, after }
            }
            (Self::ItemSets { before, .. }, Self::ItemSets { after, .. }) => {
                Self::ItemSets { before, after }
            }
            (Self::Sites { before, .. }, Self::Sites { after, .. }) => {
                Self::Sites { before, after }
            }
            (_, later) => later,
        }
    }
}

/// The fields that differ between `before` (`None`: the item did not exist) and `after`, values
/// first in the order their properties first appear, then visibility, class, item type, sets and
/// sites.
pub(crate) fn diff(before: Option<&BeforeImage>, after: &BeforeImage) -> Vec<FieldChange> {
    let mut changes = Vec::new();
    let empty = Vec::new();
    let old = by_property(before.map_or(&empty, |image| &image.values));
    let new = by_property(&after.values);
    let mut order: Vec<PropertyId> = Vec::new();
    for value in after
        .values
        .iter()
        .chain(before.map_or(&empty, |image| &image.values))
    {
        if !order.contains(&value.content.property_id) {
            order.push(value.content.property_id);
        }
    }
    for property in order {
        changes.push(FieldChange::Values {
            property,
            before: old.get(&property).cloned().unwrap_or_default(),
            after: new.get(&property).cloned().unwrap_or_default(),
        });
    }
    changes.push(FieldChange::Visibility {
        before: before.map(|image| image.is_public),
        after: after.is_public,
    });
    changes.push(FieldChange::Class {
        before: before.and_then(|image| image.class),
        after: after.class,
    });
    changes.push(FieldChange::Template {
        before: before.and_then(|image| image.template),
        after: after.template,
    });
    let (sets_before, sites_before) = before.map(memberships).unwrap_or_default();
    let (sets_after, sites_after) = memberships(after);
    changes.push(FieldChange::ItemSets {
        before: sets_before,
        after: sets_after,
    });
    changes.push(FieldChange::Sites {
        before: sites_before,
        after: sites_after,
    });
    changes.retain(|change| !change.is_noop());
    changes
}

/// `earlier` followed by `later` as one list of changes, keeping each field's first `before` and
/// its last `after`, and dropping a field that ends where it started.
pub(crate) fn fold(earlier: Vec<FieldChange>, later: Vec<FieldChange>) -> Vec<FieldChange> {
    let mut out = earlier;
    for change in later {
        match out.iter().position(|kept| kept.key() == change.key()) {
            Some(at) => {
                let kept = out.remove(at);
                out.insert(at, kept.then(change));
            }
            None => out.push(change),
        }
    }
    out.retain(|change| !change.is_noop());
    out
}

fn by_property(values: &[ImageValue]) -> BTreeMap<PropertyId, Vec<ImageValue>> {
    let mut out: BTreeMap<PropertyId, Vec<ImageValue>> = BTreeMap::new();
    for value in values {
        out.entry(value.content.property_id)
            .or_default()
            .push(value.clone());
    }
    out
}

fn memberships(image: &BeforeImage) -> (Vec<ItemSetId>, Vec<SiteId>) {
    match &image.memberships {
        ImageMemberships::Item {
            item_sets, sites, ..
        } => (item_sets.clone(), sites.clone()),
        _ => (Vec::new(), Vec::new()),
    }
}

/// The stored `changes` column.
pub(crate) fn changes_json(changes: &[FieldChange]) -> Value {
    Value::Array(changes.iter().map(change_json).collect())
}

fn change_json(change: &FieldChange) -> Value {
    let values = |list: &[ImageValue]| list.iter().map(value_json).collect::<Vec<_>>();
    match change {
        FieldChange::Values {
            property,
            before,
            after,
        } => json!({
            "field": "values",
            "property": property.0,
            "before": values(before),
            "after": values(after),
        }),
        FieldChange::Visibility { before, after } => {
            json!({ "field": "visibility", "before": before, "after": after })
        }
        FieldChange::Class { before, after } => json!({
            "field": "class",
            "before": before.map(|id| id.0),
            "after": after.map(|id| id.0),
        }),
        FieldChange::Template { before, after } => json!({
            "field": "template",
            "before": before.map(|id| id.0),
            "after": after.map(|id| id.0),
        }),
        FieldChange::ItemSets { before, after } => json!({
            "field": "item_sets",
            "before": before.iter().map(|id| id.0).collect::<Vec<_>>(),
            "after": after.iter().map(|id| id.0).collect::<Vec<_>>(),
        }),
        FieldChange::Sites { before, after } => json!({
            "field": "sites",
            "before": before.iter().map(|id| id.0).collect::<Vec<_>>(),
            "after": after.iter().map(|id| id.0).collect::<Vec<_>>(),
        }),
    }
}

/// Parse a stored `changes` column. The column is external data, so a malformed one is an error.
pub(crate) fn changes_from(stored: &Value) -> Result<Vec<FieldChange>, DbError> {
    stored
        .as_array()
        .ok_or_else(|| bad("not an array"))?
        .iter()
        .map(change_from)
        .collect()
}

fn change_from(change: &Value) -> Result<FieldChange, DbError> {
    let object = change.as_object().ok_or_else(|| bad("change"))?;
    let field = object
        .get("field")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("field"))?;
    Ok(match field {
        "values" => FieldChange::Values {
            property: PropertyId(
                object
                    .get("property")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| bad("property"))?,
            ),
            before: value_list(object, "before")?,
            after: value_list(object, "after")?,
        },
        "visibility" => FieldChange::Visibility {
            before: match object.get("before") {
                None | Some(Value::Null) => None,
                Some(flag) => Some(flag.as_bool().ok_or_else(|| bad("visibility"))?),
            },
            after: object
                .get("after")
                .and_then(Value::as_bool)
                .ok_or_else(|| bad("visibility"))?,
        },
        "class" => FieldChange::Class {
            before: opt_id(object, "before")?,
            after: opt_id(object, "after")?,
        },
        "template" => FieldChange::Template {
            before: opt_id(object, "before")?,
            after: opt_id(object, "after")?,
        },
        "item_sets" => FieldChange::ItemSets {
            before: ids(object, "before")?,
            after: ids(object, "after")?,
        },
        "sites" => FieldChange::Sites {
            before: ids(object, "before")?,
            after: ids(object, "after")?,
        },
        other => return Err(bad(&format!("field `{other}`"))),
    })
}

fn value_list(object: &Map<String, Value>, key: &str) -> Result<Vec<ImageValue>, DbError> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| bad(key))?
        .iter()
        .map(value_from)
        .collect()
}

fn opt_id<T: From<u64>>(object: &Map<String, Value>, key: &str) -> Result<Option<T>, DbError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(id) => id
            .as_u64()
            .map(|id| Some(T::from(id)))
            .ok_or_else(|| bad(key)),
    }
}

fn ids<T: From<u64>>(object: &Map<String, Value>, key: &str) -> Result<Vec<T>, DbError> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| bad(key))?
        .iter()
        .map(|id| id.as_u64().map(T::from).ok_or_else(|| bad(key)))
        .collect()
}

fn bad(what: &str) -> DbError {
    DbError::Column {
        column: "changes",
        reason: format!("item history: {what}"),
    }
}

/// One entry of an item's history, never changed afterwards.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntryRow {
    pub id: u64,
    pub item_id: ItemId,
    pub created: NaiveDateTime,
    /// `None` once the user is deleted.
    pub user_id: Option<UserId>,
    pub source: HistorySource,
    pub changes: Vec<FieldChange>,
}

impl HistoryEntryRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id::<u64>(row, "id")?,
            item_id: DB::id(row, "item_id")?,
            created: DB::datetime(row, "created")?,
            user_id: DB::opt_id(row, "user_id")?,
            source: HistorySource::from_columns(
                &DB::text(row, "source")?,
                DB::opt_id(row, "bulk_id")?,
                DB::opt_id(row, "import_id")?,
            )?,
            changes: changes_from(&DB::json(row, "changes")?)?,
        })
    }
}
