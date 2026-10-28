//! What an import writes, and the before-image it keeps of a resource it changes
//! (`docs/import-v2.md`, "Actions" and "Database").
//!
//! A record arrives already resolved: property ids, resource ids, user ids. The plan's terms,
//! identifiers and defaults are the api layer's to settle before a record reaches this crate.

use chrono::NaiveDateTime;
use serde_json::{Map, Value, json};

use crate::DbError;
use crate::ids::{
    ClassId, ItemId, ItemSetId, MediaId, PropertyId, ResourceId, SiteId, TemplateId, UserId,
};
use crate::rows::{ImportTarget, ValueContent};
use crate::time;

/// A resource an import writes values to: an item, a media or an item set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportResource {
    Item(ItemId),
    Media(MediaId),
    ItemSet(ItemSetId),
}

impl ImportResource {
    pub fn id(self) -> ResourceId {
        match self {
            Self::Item(id) => id.as_resource(),
            Self::Media(id) => id.as_resource(),
            Self::ItemSet(id) => id.as_resource(),
        }
    }

    /// The ledger's name for this resource.
    pub fn target(self) -> ImportTarget {
        match self {
            Self::Item(id) => ImportTarget::Item(id),
            Self::Media(id) => ImportTarget::Media(id),
            Self::ItemSet(id) => ImportTarget::ItemSet(id),
        }
    }

    /// The `fulltext_search.resource` name.
    pub(crate) fn index_name(self) -> &'static str {
        match self {
            Self::Item(_) => "items",
            Self::Media(_) => "media",
            Self::ItemSet(_) => "item_sets",
        }
    }
}

/// What every created resource carries, whatever its kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewImportFields {
    /// In the order they are written, which is the order upstream reads them back.
    pub values: Vec<ValueContent>,
    pub template: Option<TemplateId>,
    pub class: Option<ClassId>,
    pub owner: Option<UserId>,
    pub is_public: bool,
}

/// An item record under `create`. Its media follow with [`NewImportMedia`], whose positions are
/// the item's media order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewImportItem {
    pub fields: NewImportFields,
    pub item_sets: Vec<ItemSetId>,
    pub sites: Vec<SiteId>,
}

/// A file already in the store, as the media row names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFile {
    pub storage_id: String,
    pub extension: Option<String>,
    pub media_type: Option<String>,
    pub sha256: Option<String>,
    pub size: Option<i64>,
    pub has_thumbnails: bool,
}

/// A media record under an item: a stored file, or what a URL ingester made (`youtube` and
/// `oembed` store no original, `html` stores none at all).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewImportMedia {
    pub item: ItemId,
    pub fields: NewImportFields,
    /// `None` when the ingester stored no original.
    pub file: Option<StoredFile>,
    /// Upstream's ingester name (`upload`, `sideload`, `url`, `youtube`).
    pub ingester: String,
    /// The ingester's renderer: `file` for every ingester that stores bytes.
    pub renderer: String,
    /// `media.data`, the ingester's payload.
    pub data: Option<Value>,
    /// The original file name or URL, as upstream keeps it in `media.source`.
    pub source: Option<String>,
    /// 1-based, as the api write path numbers inline media.
    pub position: i64,
}

/// An item set under `create`. An item-set record is closed, the `ItemSet` entity's default; a set
/// a record names by a title no set has is open (`docs/import-v2.md`, "Sets").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewImportItemSet {
    pub fields: NewImportFields,
    pub is_open: bool,
    pub sites: Vec<SiteId>,
}

/// The import action applied to a matched resource. `create` and `delete` are not here: a create
/// is [`NewImportItem`] and friends, and a delete goes through the api delete path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportAction {
    /// Values added, distinct by type, text and language; sets and sites added; the rest kept.
    Append,
    /// A slot with values replaces; an empty or unmapped slot leaves the stored state.
    Revise,
    /// A mapped slot replaces, and a mapped empty slot clears; an unmapped slot is left.
    Update,
    /// Every value removed, then the slots written; an unmapped slot is cleared, as a full `PUT`.
    Replace,
}

/// Whether a record's plan maps a slot, and what it gave when it does.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Slot<T> {
    #[default]
    Unmapped,
    Mapped(T),
}

/// A slot's value as one action sees it.
pub trait SlotValue: Sized + Clone {
    /// Whether the slot "has values" in the contract's sense.
    fn has_value(&self) -> bool;
    /// What `replace` writes for an unmapped slot. `None` keeps the stored state, which is the
    /// case only for visibility: a resource is always public or private.
    fn cleared() -> Option<Self>;
}

impl<T: Clone> SlotValue for Option<T> {
    fn has_value(&self) -> bool {
        self.is_some()
    }
    fn cleared() -> Option<Self> {
        Some(None)
    }
}

impl<T: Clone> SlotValue for Vec<T> {
    fn has_value(&self) -> bool {
        !self.is_empty()
    }
    fn cleared() -> Option<Self> {
        Some(Vec::new())
    }
}

impl SlotValue for bool {
    fn has_value(&self) -> bool {
        true
    }
    fn cleared() -> Option<Self> {
        None
    }
}

impl<T: SlotValue> Slot<T> {
    /// What `action` writes for this slot, or `None` to keep the stored state. `append` never
    /// writes a single-valued slot; its set and site union is the caller's.
    pub(crate) fn written(&self, action: ImportAction) -> Option<T> {
        match (action, self) {
            (ImportAction::Append, _) => None,
            (ImportAction::Revise, Self::Mapped(value)) if value.has_value() => Some(value.clone()),
            (ImportAction::Revise, _) => None,
            (ImportAction::Update | ImportAction::Replace, Self::Mapped(value)) => {
                Some(value.clone())
            }
            (ImportAction::Update, Self::Unmapped) => None,
            (ImportAction::Replace, Self::Unmapped) => T::cleared(),
        }
    }
}

/// What a record says about a matched resource, whatever its kind.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportFields {
    /// The record's values, in order.
    pub values: Vec<ValueContent>,
    /// Every property the plan maps for this kind, with or without values in this record. `update`
    /// clears a mapped term that has none; `revise` ignores this list.
    pub mapped_terms: Vec<PropertyId>,
    pub template: Slot<Option<TemplateId>>,
    pub class: Slot<Option<ClassId>>,
    pub owner: Slot<Option<UserId>>,
    pub is_public: Slot<bool>,
}

/// The memberships an item record may set on a matched item.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemSlots {
    pub item_sets: Slot<Vec<ItemSetId>>,
    pub sites: Slot<Vec<SiteId>>,
}

/// One value as a before-image keeps it: the stored columns without the value id, plus the
/// values of its annotation, which a restore writes again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageValue {
    pub content: ValueContent,
    pub annotation: Vec<ImageValue>,
}

/// The memberships a before-image keeps, which depend on the kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageMemberships {
    Item {
        item_sets: Vec<ItemSetId>,
        sites: Vec<SiteId>,
        /// The item's media, first to last.
        media_order: Vec<MediaId>,
        primary_media: Option<MediaId>,
    },
    Media,
    ItemSet {
        /// Each site with the item set's position in that site's list.
        sites: Vec<(SiteId, Option<i64>)>,
    },
}

/// The stored state of a resource before an import changed it (`romeka_import_before.image`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeforeImage {
    pub values: Vec<ImageValue>,
    pub template: Option<TemplateId>,
    pub class: Option<ClassId>,
    pub owner: Option<UserId>,
    pub is_public: bool,
    /// The resource's `modified`, which a restore puts back, so undoing a later import does not
    /// make the resource look edited to the undo of an earlier one.
    pub modified: Option<NaiveDateTime>,
    pub memberships: ImageMemberships,
}

impl BeforeImage {
    pub fn to_json(&self) -> Value {
        let mut image = json!({
            "values": self.values.iter().map(value_json).collect::<Vec<_>>(),
            "template": self.template.map(|id| id.0),
            "class": self.class.map(|id| id.0),
            "owner": self.owner.map(|id| id.0),
            "isPublic": self.is_public,
            "modified": self.modified.map(time::format),
        });
        let members = match &self.memberships {
            ImageMemberships::Item {
                item_sets,
                sites,
                media_order,
                primary_media,
            } => json!({
                "kind": "item",
                "itemSets": item_sets.iter().map(|id| id.0).collect::<Vec<_>>(),
                "sites": sites.iter().map(|id| id.0).collect::<Vec<_>>(),
                "mediaOrder": media_order.iter().map(|id| id.0).collect::<Vec<_>>(),
                "primaryMedia": primary_media.map(|id| id.0),
            }),
            ImageMemberships::Media => json!({ "kind": "media" }),
            ImageMemberships::ItemSet { sites } => json!({
                "kind": "item_set",
                "sites": sites
                    .iter()
                    .map(|(site, position)| json!({ "site": site.0, "position": position }))
                    .collect::<Vec<_>>(),
            }),
        };
        if let (Value::Object(image), Value::Object(members)) = (&mut image, members) {
            image.extend(members);
        }
        image
    }

    /// Parse a stored image. The column is external data, so a malformed image is an error.
    pub fn from_json(image: &Value) -> Result<Self, DbError> {
        let object = image.as_object().ok_or_else(|| bad("not an object"))?;
        let values = array(object, "values")?
            .iter()
            .map(value_from)
            .collect::<Result<Vec<_>, _>>()?;
        let memberships = match text(object, "kind")? {
            "item" => ImageMemberships::Item {
                item_sets: ids(object, "itemSets")?,
                sites: ids(object, "sites")?,
                media_order: ids(object, "mediaOrder")?,
                primary_media: opt_id(object, "primaryMedia")?,
            },
            "media" => ImageMemberships::Media,
            "item_set" => ImageMemberships::ItemSet {
                sites: array(object, "sites")?
                    .iter()
                    .map(|entry| {
                        let entry = entry.as_object().ok_or_else(|| bad("site entry"))?;
                        let site = opt_id(entry, "site")?.ok_or_else(|| bad("site entry"))?;
                        let position = match entry.get("position") {
                            None | Some(Value::Null) => None,
                            Some(position) => {
                                Some(position.as_i64().ok_or_else(|| bad("position"))?)
                            }
                        };
                        Ok((site, position))
                    })
                    .collect::<Result<Vec<_>, DbError>>()?,
            },
            other => return Err(bad(&format!("kind `{other}`"))),
        };
        Ok(Self {
            values,
            template: opt_id(object, "template")?,
            class: opt_id(object, "class")?,
            owner: opt_id(object, "owner")?,
            is_public: object
                .get("isPublic")
                .and_then(Value::as_bool)
                .ok_or_else(|| bad("isPublic"))?,
            modified: match object.get("modified") {
                None | Some(Value::Null) => None,
                Some(Value::String(stored)) => Some(time::parse(stored).map_err(|e| bad(&e))?),
                Some(_) => return Err(bad("modified")),
            },
            memberships,
        })
    }
}

pub(crate) fn value_json(value: &ImageValue) -> Value {
    let content = &value.content;
    json!({
        "property": content.property_id.0,
        "type": content.r#type,
        "value": content.value,
        "uri": content.uri,
        "resource": content.value_resource_id.map(|id| id.0),
        "lang": content.lang,
        "isPublic": content.is_public,
        "annotation": value.annotation.iter().map(value_json).collect::<Vec<_>>(),
    })
}

pub(crate) fn value_from(value: &Value) -> Result<ImageValue, DbError> {
    let object = value.as_object().ok_or_else(|| bad("value"))?;
    let property: PropertyId = opt_id(object, "property")?.ok_or_else(|| bad("property"))?;
    Ok(ImageValue {
        content: ValueContent {
            property_id: property,
            value_resource_id: opt_id(object, "resource")?,
            value_annotation_id: None,
            r#type: text(object, "type")?.to_string(),
            lang: opt_text(object, "lang")?,
            value: opt_text(object, "value")?,
            uri: opt_text(object, "uri")?,
            is_public: object
                .get("isPublic")
                .and_then(Value::as_bool)
                .ok_or_else(|| bad("value isPublic"))?,
        },
        annotation: array(object, "annotation")?
            .iter()
            .map(value_from)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn bad(what: &str) -> DbError {
    DbError::Column {
        column: "image",
        reason: format!("before-image: {what}"),
    }
}

fn array<'v>(object: &'v Map<String, Value>, key: &str) -> Result<&'v Vec<Value>, DbError> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| bad(key))
}

fn text<'v>(object: &'v Map<String, Value>, key: &str) -> Result<&'v str, DbError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| bad(key))
}

fn opt_text(object: &Map<String, Value>, key: &str) -> Result<Option<String>, DbError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(bad(key)),
    }
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
    array(object, key)?
        .iter()
        .map(|id| id.as_u64().map(T::from).ok_or_else(|| bad(key)))
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn a_slot_writes_what_its_action_says() {
        let given: Slot<Option<u8>> = Slot::Mapped(Some(1));
        let empty: Slot<Option<u8>> = Slot::Mapped(None);
        let unmapped: Slot<Option<u8>> = Slot::Unmapped;
        let cases = [
            (ImportAction::Append, [None, None, None]),
            (ImportAction::Revise, [Some(Some(1)), None, None]),
            (ImportAction::Update, [Some(Some(1)), Some(None), None]),
            (
                ImportAction::Replace,
                [Some(Some(1)), Some(None), Some(None)],
            ),
        ];
        for (action, expected) in cases {
            let found = [&given, &empty, &unmapped].map(|slot| slot.written(action));
            assert_eq!(found, expected, "{action:?}");
        }
        assert_eq!(Slot::<bool>::Unmapped.written(ImportAction::Replace), None);
        assert_eq!(
            Slot::Mapped(false).written(ImportAction::Revise),
            Some(false)
        );
    }

    #[test]
    fn an_image_round_trips_through_json() {
        let image = BeforeImage {
            values: vec![ImageValue {
                content: ValueContent {
                    lang: Some("fr".to_string()),
                    ..ValueContent::literal(PropertyId(1), "Lettre")
                },
                annotation: vec![ImageValue {
                    content: ValueContent::literal(PropertyId(2), "note"),
                    annotation: Vec::new(),
                }],
            }],
            template: Some(TemplateId(3)),
            class: None,
            owner: Some(UserId(4)),
            is_public: false,
            modified: crate::time::parse("2001-02-03 04:05:06").ok(),
            memberships: ImageMemberships::ItemSet {
                sites: vec![(SiteId(5), Some(2)), (SiteId(6), None)],
            },
        };
        assert_eq!(BeforeImage::from_json(&image.to_json()).unwrap(), image);
        assert!(BeforeImage::from_json(&json!({ "values": [] })).is_err());
    }
}
