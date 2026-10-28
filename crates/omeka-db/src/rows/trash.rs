//! The trash snapshot of one record a bulk delete removed (`romeka_bulk_target.image`): an item
//! with its media, a media alone, or an item set alone ([`TrashKind`]).
//!
//! The snapshot holds every row the record owns, column by column, so an undo can insert them
//! again with the same ids: the `resource` rows (the record's, its media's and its value
//! annotations'), the `item`, `media` or `item_set` row, every value of those resources, the
//! memberships (an item's sets and sites, a set's items and the sites that show it), and the
//! media's notes, extents and derivative states. Each table's columns are listed once in
//! [`TABLES`], with how each is read and written, so the snapshot and the restore cannot drift
//! apart. Two kinds of row belong to someone else, so the snapshot records them apart: the rows
//! whose [`Pointer`] named the record (the delete leaves them empty), and the values on other
//! resources that linked to it (the delete removes them).
//!
//! The JSON form is `{"kind": "trash", "of": "item", "tables": {table: [row]}, "emptied": [...],
//! "incoming": [...]}`; a row is an object of column to value. A snapshot written before media
//! and sets could be trashed has no `of` (it is an item's) and lists its emptied page block
//! attachments as `"attachments": [{"id", "item", "media"}]`.

use std::collections::BTreeSet;

use chrono::NaiveDateTime;
use serde_json::{Map, Value, json};

use crate::error::DbError;
use crate::ids::{ItemId, ResourceId, UserId};
use crate::time;

/// How one column is read from a row and bound into an insert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cell {
    Int,
    Text,
    Flag,
    Time,
}

/// A table the snapshot copies, with its columns.
#[derive(Debug)]
pub(crate) struct Table {
    pub(crate) name: &'static str,
    pub(crate) columns: &'static [(&'static str, Cell)],
}

use Cell::{Flag, Int, Text, Time};

/// What one snapshot holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrashKind {
    /// An item with its media.
    #[default]
    Item,
    /// A media alone; its item stays.
    Media,
    /// An item set alone; its items stay.
    ItemSet,
}

impl TrashKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::Media => "media",
            Self::ItemSet => "item_set",
        }
    }

    fn parse(stored: &str) -> Option<Self> {
        [Self::Item, Self::Media, Self::ItemSet]
            .into_iter()
            .find(|kind| kind.as_str() == stored)
    }

    /// The table of the record's own row, beside its `resource` row.
    pub(crate) const fn table(self) -> &'static Table {
        match self {
            Self::Item => &ITEM,
            Self::Media => &MEDIA,
            Self::ItemSet => &ITEM_SET,
        }
    }

    /// The resource name the full-text index files the record under.
    #[must_use]
    pub const fn resource_name(self) -> &'static str {
        match self {
            Self::Item => "items",
            Self::Media => "media",
            Self::ItemSet => "item_sets",
        }
    }
}

pub(crate) const RESOURCE: Table = Table {
    name: "resource",
    columns: &[
        ("id", Int),
        ("owner_id", Int),
        ("resource_class_id", Int),
        ("resource_template_id", Int),
        ("thumbnail_id", Int),
        ("title", Text),
        ("is_public", Flag),
        ("created", Time),
        ("modified", Time),
        ("resource_type", Text),
    ],
};

pub(crate) const ITEM_SET: Table = Table {
    name: "item_set",
    columns: &[("id", Int), ("is_open", Flag)],
};

pub(crate) const ITEM: Table = Table {
    name: "item",
    columns: &[("id", Int), ("primary_media_id", Int)],
};

pub(crate) const MEDIA: Table = Table {
    name: "media",
    columns: &[
        ("id", Int),
        ("item_id", Int),
        ("ingester", Text),
        ("renderer", Text),
        ("data", Text),
        ("source", Text),
        ("media_type", Text),
        ("storage_id", Text),
        ("extension", Text),
        ("sha256", Text),
        ("size", Int),
        ("has_original", Flag),
        ("has_thumbnails", Flag),
        ("position", Int),
        ("lang", Text),
        ("alt_text", Text),
    ],
};

pub(crate) const VALUE_ANNOTATION: Table = Table {
    name: "value_annotation",
    columns: &[("id", Int)],
};

pub(crate) const VALUE: Table = Table {
    name: "value",
    columns: &[
        ("id", Int),
        ("resource_id", Int),
        ("property_id", Int),
        ("value_resource_id", Int),
        ("value_annotation_id", Int),
        ("type", Text),
        ("lang", Text),
        ("value", Text),
        ("uri", Text),
        ("is_public", Flag),
    ],
};

pub(crate) const ITEM_ITEM_SET: Table = Table {
    name: "item_item_set",
    columns: &[("item_id", Int), ("item_set_id", Int)],
};

pub(crate) const ITEM_SITE: Table = Table {
    name: "item_site",
    columns: &[("item_id", Int), ("site_id", Int)],
};

/// No `id`: an undo inserts the row under a new one, so no other row can hold it.
pub(crate) const SITE_ITEM_SET: Table = Table {
    name: "site_item_set",
    columns: &[("site_id", Int), ("item_set_id", Int), ("position", Int)],
};

pub(crate) const MEDIA_ANNOTATION: Table = Table {
    name: "romeka_media_annotation",
    columns: &[
        ("id", Int),
        ("media_id", Int),
        ("owner_id", Int),
        ("revision", Int),
        ("created", Time),
        ("modified", Time),
        ("motivation", Text),
        ("origin", Text),
        ("lang", Text),
        ("selector_kind", Text),
        ("selector", Text),
        ("t_start_ms", Int),
        ("t_end_ms", Int),
        ("body_json", Text),
        ("body_text", Text),
        ("extensions", Text),
        ("client_key", Text),
    ],
};

pub(crate) const MEDIA_EXTENT: Table = Table {
    name: "romeka_media_extent",
    columns: &[
        ("media_id", Int),
        ("storage_path", Text),
        ("probed", Time),
        ("duration_ms", Int),
        ("width", Int),
        ("height", Int),
    ],
};

pub(crate) const MEDIA_DERIVATIVE: Table = Table {
    name: "romeka_media_derivative",
    columns: &[
        ("media_id", Int),
        ("kind", Text),
        ("state", Text),
        ("reason", Text),
        ("progress", Int),
        ("tool", Text),
        ("queued", Time),
        ("started", Time),
        ("finished", Time),
        ("attempts", Int),
    ],
};

/// Every table the snapshot copies, in the order an undo inserts them: a row comes after the
/// rows its foreign keys name.
pub(crate) const TABLES: [&Table; 12] = [
    &RESOURCE,
    &ITEM_SET,
    &ITEM,
    &MEDIA,
    &VALUE_ANNOTATION,
    &VALUE,
    &ITEM_ITEM_SET,
    &ITEM_SITE,
    &SITE_ITEM_SET,
    &MEDIA_ANNOTATION,
    &MEDIA_EXTENT,
    &MEDIA_DERIVATIVE,
];

/// One copied row: column name to its value.
pub type TrashRow = Map<String, Value>;

/// A column of another table that names a record by id and empties when the record goes (`ON
/// DELETE SET NULL`), so a delete leaves the row and an undo fills the column again.
#[derive(Debug, PartialEq, Eq)]
pub struct Pointer {
    pub table: &'static str,
    pub column: &'static str,
    /// The table of the records it names: `item`, `media` or `item_set`.
    pub names: &'static str,
}

/// Every pointer a trashed record may be named by.
pub(crate) const POINTERS: [Pointer; 4] = [
    Pointer {
        table: "site_block_attachment",
        column: "item_id",
        names: "item",
    },
    Pointer {
        table: "site_block_attachment",
        column: "media_id",
        names: "media",
    },
    Pointer {
        table: "item",
        column: "primary_media_id",
        names: "media",
    },
    Pointer {
        table: "custom_vocab",
        column: "item_set_id",
        names: "item_set",
    },
];

fn pointer(table: &str, column: &str) -> Option<&'static Pointer> {
    POINTERS
        .iter()
        .find(|pointer| pointer.table == table && pointer.column == column)
}

/// A row of another table whose pointer named a record the snapshot holds, emptied by the delete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Emptied {
    pub pointer: &'static Pointer,
    /// The row's `id`.
    pub row: u64,
    /// The record it named.
    pub target: u64,
}

/// A value on another resource that linked to the item or one of its media, and that resource's
/// `modified` when the delete removed the value: a later `modified` means it was edited since.
#[derive(Debug, Clone, PartialEq)]
pub struct IncomingLink {
    pub value: TrashRow,
    pub modified: Option<NaiveDateTime>,
}

impl IncomingLink {
    /// The resource the value belongs to.
    #[must_use]
    pub fn resource(&self) -> Option<ResourceId> {
        int(&self.value, "resource_id").map(ResourceId)
    }
}

/// The stored files of one media, as its row names them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashedMedia {
    pub storage_id: String,
    pub extension: Option<String>,
    pub media_type: Option<String>,
    pub has_original: bool,
    pub has_thumbnails: bool,
    /// The original's size in bytes, as the row records it.
    pub size: u64,
}

/// The trash snapshot of one record.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TrashImage {
    pub kind: TrashKind,
    /// Rows per table name, each table in [`TABLES`] order.
    pub tables: Vec<(&'static str, Vec<TrashRow>)>,
    pub emptied: Vec<Emptied>,
    pub incoming: Vec<IncomingLink>,
}

/// An integer cell, `None` when it is null or absent.
pub(crate) fn int(row: &TrashRow, column: &str) -> Option<u64> {
    row.get(column).and_then(Value::as_u64)
}

fn text(row: &TrashRow, column: &str) -> Option<String> {
    row.get(column).and_then(Value::as_str).map(str::to_string)
}

fn flag(row: &TrashRow, column: &str) -> bool {
    row.get(column).and_then(Value::as_bool).unwrap_or(false)
}

impl TrashImage {
    /// The rows copied from `table`.
    #[must_use]
    pub fn rows(&self, table: &str) -> &[TrashRow] {
        self.tables
            .iter()
            .find(|(name, _)| *name == table)
            .map_or(&[], |(_, rows)| rows.as_slice())
    }

    /// The id of the record the snapshot was taken of.
    #[must_use]
    pub fn root(&self) -> Option<u64> {
        self.rows(self.kind.table().name)
            .first()
            .and_then(|row| int(row, "id"))
    }

    /// The record's own `resource` row.
    fn root_row(&self) -> Option<&TrashRow> {
        let root = self.root()?;
        self.rows(RESOURCE.name)
            .iter()
            .find(|row| int(row, "id") == Some(root))
    }

    /// The record's owner, whose delete right the run checks.
    #[must_use]
    pub fn owner(&self) -> Option<UserId> {
        self.root_row()
            .and_then(|row| int(row, "owner_id"))
            .map(UserId)
    }

    #[must_use]
    pub fn title(&self) -> Option<String> {
        self.root_row().and_then(|row| text(row, "title"))
    }

    /// The item a media snapshot's media belongs to; `None` for the other kinds.
    #[must_use]
    pub fn item_of_media(&self) -> Option<ItemId> {
        if self.kind != TrashKind::Media {
            return None;
        }
        self.rows(MEDIA.name)
            .first()
            .and_then(|row| int(row, "item_id"))
            .map(ItemId)
    }

    /// Every resource the snapshot holds: the record, its media and the value annotations.
    #[must_use]
    pub fn resources(&self) -> BTreeSet<u64> {
        self.rows(RESOURCE.name)
            .iter()
            .filter_map(|row| int(row, "id"))
            .collect()
    }

    /// The record and its media: what another resource's value may link to.
    #[must_use]
    pub fn linkable(&self) -> BTreeSet<u64> {
        self.rows(ITEM.name)
            .iter()
            .chain(self.rows(MEDIA.name))
            .chain(self.rows(ITEM_SET.name))
            .filter_map(|row| int(row, "id"))
            .collect()
    }

    /// The media ids, in id order.
    #[must_use]
    pub fn media_ids(&self) -> Vec<u64> {
        self.rows(MEDIA.name)
            .iter()
            .filter_map(|row| int(row, "id"))
            .collect()
    }

    /// The stored files of each media that has a storage id.
    #[must_use]
    pub fn media_files(&self) -> Vec<TrashedMedia> {
        self.rows(MEDIA.name)
            .iter()
            .filter_map(|row| {
                Some(TrashedMedia {
                    storage_id: text(row, "storage_id")?,
                    extension: text(row, "extension"),
                    media_type: text(row, "media_type"),
                    has_original: flag(row, "has_original"),
                    has_thumbnails: flag(row, "has_thumbnails"),
                    size: int(row, "size").unwrap_or(0),
                })
            })
            .collect()
    }

    pub fn to_json(&self) -> Value {
        let tables: Map<String, Value> = self
            .tables
            .iter()
            .map(|(name, rows)| {
                let rows = rows.iter().cloned().map(Value::Object).collect();
                ((*name).to_string(), Value::Array(rows))
            })
            .collect();
        json!({
            "kind": "trash",
            "of": self.kind.as_str(),
            "tables": tables,
            "emptied": self.emptied.iter().map(|emptied| json!({
                "table": emptied.pointer.table, "column": emptied.pointer.column,
                "row": emptied.row, "target": emptied.target,
            })).collect::<Vec<_>>(),
            "incoming": self.incoming.iter().map(|link| json!({
                "value": link.value, "modified": link.modified.map(time::format),
            })).collect::<Vec<_>>(),
        })
    }

    /// Parse a stored snapshot. The column is external data, so a malformed one is an error.
    pub fn from_json(image: &Value) -> Result<Self, DbError> {
        let object = image.as_object().ok_or_else(|| bad("not an object"))?;
        if object.get("kind").and_then(Value::as_str) != Some("trash") {
            return Err(bad("not a trash snapshot"));
        }
        let kind = match object.get("of") {
            None => TrashKind::Item,
            Some(of) => of
                .as_str()
                .and_then(TrashKind::parse)
                .ok_or_else(|| bad("an unknown kind"))?,
        };
        let stored = object
            .get("tables")
            .and_then(Value::as_object)
            .ok_or_else(|| bad("no tables"))?;
        let mut tables = Vec::new();
        for table in TABLES {
            let rows = match stored.get(table.name) {
                None => Vec::new(),
                Some(rows) => rows
                    .as_array()
                    .ok_or_else(|| bad("a table is not a list"))?
                    .iter()
                    .map(|row| row.as_object().cloned().ok_or_else(|| bad("a row")))
                    .collect::<Result<_, _>>()?,
            };
            tables.push((table.name, rows));
        }
        let list = |key: &str| -> Vec<Value> {
            object
                .get(key)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };
        let id = |entry: &Value, key: &str| entry.get(key).and_then(Value::as_u64);
        let mut emptied = Vec::new();
        for entry in list("emptied") {
            let named = entry
                .get("table")
                .and_then(Value::as_str)
                .zip(entry.get("column").and_then(Value::as_str))
                .and_then(|(table, column)| pointer(table, column))
                .ok_or_else(|| bad("an emptied row names no known column"))?;
            emptied.push(Emptied {
                pointer: named,
                row: id(&entry, "row").ok_or_else(|| bad("an emptied row"))?,
                target: id(&entry, "target").ok_or_else(|| bad("an emptied target"))?,
            });
        }
        for link in list("attachments") {
            let row = id(&link, "id").ok_or_else(|| bad("an attachment id"))?;
            for (column, key) in [("item_id", "item"), ("media_id", "media")] {
                if let Some(target) = id(&link, key) {
                    emptied.push(Emptied {
                        pointer: pointer("site_block_attachment", column)
                            .ok_or_else(|| bad("an attachment column"))?,
                        row,
                        target,
                    });
                }
            }
        }
        let incoming = list("incoming")
            .iter()
            .map(|link| {
                let value = link
                    .get("value")
                    .and_then(Value::as_object)
                    .cloned()
                    .ok_or_else(|| bad("an incoming value"))?;
                let modified = match link.get("modified").and_then(Value::as_str) {
                    Some(stored) => Some(time::parse(stored).map_err(|reason| bad(&reason))?),
                    None => None,
                };
                Ok(IncomingLink { value, modified })
            })
            .collect::<Result<_, DbError>>()?;
        Ok(Self {
            kind,
            tables,
            emptied,
            incoming,
        })
    }
}

fn bad(reason: &str) -> DbError {
    DbError::Column {
        column: "image",
        reason: format!("trash snapshot: {reason}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_snapshot_reads_back_what_it_wrote() {
        let mut item = TrashRow::new();
        item.insert("id".into(), json!(7));
        item.insert("primary_media_id".into(), Value::Null);
        let mut resource = TrashRow::new();
        resource.insert("id".into(), json!(7));
        resource.insert("owner_id".into(), json!(3));
        resource.insert("title".into(), json!("Letter"));
        let mut value = TrashRow::new();
        value.insert("resource_id".into(), json!(9));
        let image = TrashImage {
            kind: TrashKind::Item,
            tables: TABLES
                .iter()
                .map(|table| {
                    let rows = match table.name {
                        "resource" => vec![resource.clone()],
                        "item" => vec![item.clone()],
                        _ => Vec::new(),
                    };
                    (table.name, rows)
                })
                .collect(),
            emptied: vec![Emptied {
                pointer: &POINTERS[0],
                row: 2,
                target: 7,
            }],
            incoming: vec![IncomingLink {
                value,
                modified: Some(time::parse("2026-10-24 10:00:00").unwrap()),
            }],
        };
        let back = TrashImage::from_json(&image.to_json()).unwrap();
        assert_eq!(back, image);
        assert_eq!(back.owner(), Some(UserId(3)));
        assert_eq!(back.title().as_deref(), Some("Letter"));
        assert_eq!(back.incoming[0].resource(), Some(ResourceId(9)));
        assert!(TrashImage::from_json(&json!({ "kind": "item" })).is_err());
    }

    #[test]
    fn a_snapshot_from_before_media_and_sets_reads_as_an_item_s() {
        let stored = json!({
            "kind": "trash",
            "tables": { "item": [{ "id": 7, "primary_media_id": null }] },
            "attachments": [{ "id": 2, "item": 7, "media": 9 }],
            "incoming": [],
        });
        let image = TrashImage::from_json(&stored).unwrap();
        assert_eq!(image.kind, TrashKind::Item);
        assert_eq!(image.root(), Some(7));
        let emptied: Vec<(&str, u64, u64)> = image
            .emptied
            .iter()
            .map(|emptied| (emptied.pointer.column, emptied.row, emptied.target))
            .collect();
        assert_eq!(emptied, [("item_id", 2, 7), ("media_id", 2, 9)]);
    }

    #[test]
    fn a_stored_pointer_must_be_one_the_trash_knows() {
        let stored = json!({
            "kind": "trash", "of": "media", "tables": {},
            "emptied": [{ "table": "user", "column": "id", "row": 1, "target": 1 }],
        });
        assert!(TrashImage::from_json(&stored).is_err());
        let unknown = json!({ "kind": "trash", "of": "site", "tables": {} });
        assert!(TrashImage::from_json(&unknown).is_err());
    }
}
