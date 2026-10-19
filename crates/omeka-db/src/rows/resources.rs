//! `resource` and the tables that share its primary key: `item`, `item_set`, `media` and
//! `value_annotation` (upstream `schema.sql` `resource`, `item`, `item_set`, `media`,
//! `value_annotation`).
//!
//! Upstream uses JOINED inheritance: the `resource` row carries the shared fields and the per-kind
//! table adds its own, so `ItemRow` and friends nest a [`ResourceRow`].

use crate::backend::Backend;
use chrono::NaiveDateTime;
use serde_json::Value;

use super::ValueContent;
use crate::error::DbError;
use crate::ids::{
    AnnotationId, AssetId, ClassId, ItemId, ItemSetId, MediaId, ResourceId, TemplateId, UserId,
};

/// Doctrine discriminator values stored in `resource.resource_type`
/// (`application/config/module.config.php`, `resource_discriminator_map`).
pub mod resource_type {
    pub const ITEM: &str = "Omeka\\Entity\\Item";
    pub const ITEM_SET: &str = "Omeka\\Entity\\ItemSet";
    pub const MEDIA: &str = "Omeka\\Entity\\Media";
    pub const VALUE_ANNOTATION: &str = "Omeka\\Entity\\ValueAnnotation";
}

/// One row of `resource`.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceRow {
    pub id: ResourceId,
    pub owner_id: Option<UserId>,
    pub resource_class_id: Option<ClassId>,
    pub resource_template_id: Option<TemplateId>,
    pub thumbnail_id: Option<AssetId>,
    pub title: Option<String>,
    pub is_public: bool,
    pub created: NaiveDateTime,
    pub modified: Option<NaiveDateTime>,
    /// See [`resource_type`].
    pub resource_type: String,
}

impl ResourceRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            owner_id: DB::opt_id(row, "owner_id")?,
            resource_class_id: DB::opt_id(row, "resource_class_id")?,
            resource_template_id: DB::opt_id(row, "resource_template_id")?,
            thumbnail_id: DB::opt_id(row, "thumbnail_id")?,
            title: DB::opt_text(row, "title")?,
            is_public: DB::boolean(row, "is_public")?,
            created: DB::datetime(row, "created")?,
            modified: DB::opt_datetime(row, "modified")?,
            resource_type: DB::text(row, "resource_type")?,
        })
    }
}

/// The `resource` half of a create call. The per-kind `New*` structs embed it.
#[derive(Debug, Clone, PartialEq)]
pub struct NewResource {
    pub owner_id: Option<UserId>,
    pub resource_class_id: Option<ClassId>,
    pub resource_template_id: Option<TemplateId>,
    pub thumbnail_id: Option<AssetId>,
    pub title: Option<String>,
    pub is_public: bool,
    /// `None` means now. The repository writes the same instant into `modified`.
    pub created: Option<NaiveDateTime>,
}

impl Default for NewResource {
    /// Defaults from the upstream entities and API hydrate: public (`o:is_public` defaults true in
    /// `AbstractResourceEntityAdapter::hydrate`), no owner, class or template, created now.
    fn default() -> Self {
        Self {
            owner_id: None,
            resource_class_id: None,
            resource_template_id: None,
            thumbnail_id: None,
            title: None,
            is_public: true,
            created: None,
        }
    }
}

/// An `item`: a `resource` row plus `item.primary_media_id`.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemRow {
    pub resource: ResourceRow,
    pub primary_media_id: Option<MediaId>,
}

impl ItemRow {
    /// The item's id. It is the id of its `resource` row.
    pub fn id(&self) -> ItemId {
        ItemId(self.resource.id.0)
    }

    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            resource: ResourceRow::from_db_row::<DB>(row)?,
            primary_media_id: DB::opt_id(row, "primary_media_id")?,
        })
    }
}

/// Insert input for an `item` and its `resource` row.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NewItem {
    pub resource: NewResource,
    pub primary_media_id: Option<MediaId>,
}

/// Everything one item create writes, for `ItemRepo::create_whole`: the item,
/// its item-set links and its values. Upstream writes the same graph in one
/// Doctrine flush.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NewWholeItem {
    pub item: NewItem,
    pub item_sets: Vec<ItemSetId>,
    pub values: Vec<ValueContent>,
}

/// An `item_set`: a `resource` row plus `item_set.is_open`.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemSetRow {
    pub resource: ResourceRow,
    pub is_open: bool,
}

impl ItemSetRow {
    /// The item set's id. It is the id of its `resource` row.
    pub fn id(&self) -> ItemSetId {
        ItemSetId(self.resource.id.0)
    }

    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            resource: ResourceRow::from_db_row::<DB>(row)?,
            is_open: DB::boolean(row, "is_open")?,
        })
    }
}

/// Insert input for an `item_set` and its `resource` row.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NewItemSet {
    pub resource: NewResource,
    pub is_open: bool,
}

/// A `media` file: a `resource` row plus the media columns.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaRow {
    pub resource: ResourceRow,
    pub item_id: ItemId,
    pub ingester: String,
    pub renderer: String,
    /// Ingester-specific JSON, written back in the upstream key order by the ingester.
    pub data: Option<Value>,
    pub source: Option<String>,
    pub media_type: Option<String>,
    pub storage_id: Option<String>,
    pub extension: Option<String>,
    pub sha256: Option<String>,
    pub size: Option<i64>,
    pub has_original: bool,
    pub has_thumbnails: bool,
    pub position: Option<i64>,
    pub lang: Option<String>,
    pub alt_text: Option<String>,
}

impl MediaRow {
    /// The media's id. It is the id of its `resource` row.
    pub fn id(&self) -> MediaId {
        MediaId(self.resource.id.0)
    }

    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            resource: ResourceRow::from_db_row::<DB>(row)?,
            item_id: DB::id(row, "item_id")?,
            ingester: DB::text(row, "ingester")?,
            renderer: DB::text(row, "renderer")?,
            data: DB::opt_json(row, "data")?,
            source: DB::opt_text(row, "source")?,
            media_type: DB::opt_text(row, "media_type")?,
            storage_id: DB::opt_text(row, "storage_id")?,
            extension: DB::opt_text(row, "extension")?,
            sha256: DB::opt_text(row, "sha256")?,
            size: DB::opt_int(row, "size")?,
            has_original: DB::boolean(row, "has_original")?,
            has_thumbnails: DB::boolean(row, "has_thumbnails")?,
            position: DB::opt_int(row, "position")?,
            lang: DB::opt_text(row, "lang")?,
            alt_text: DB::opt_text(row, "alt_text")?,
        })
    }
}

/// Insert input for a `media` file and its `resource` row.
#[derive(Debug, Clone, PartialEq)]
pub struct NewMedia {
    pub resource: NewResource,
    pub item_id: ItemId,
    pub ingester: String,
    pub renderer: String,
    pub data: Option<Value>,
    pub source: Option<String>,
    pub media_type: Option<String>,
    pub storage_id: Option<String>,
    pub extension: Option<String>,
    pub sha256: Option<String>,
    pub size: Option<i64>,
    pub has_original: bool,
    pub has_thumbnails: bool,
    pub position: Option<i64>,
    pub lang: Option<String>,
    pub alt_text: Option<String>,
}

impl Default for NewMedia {
    /// `ingester` and `renderer` are required by the schema; the rest mirrors an empty upstream
    /// `Media` entity.
    fn default() -> Self {
        Self {
            resource: NewResource::default(),
            item_id: ItemId(0),
            ingester: String::new(),
            renderer: String::new(),
            data: None,
            source: None,
            media_type: None,
            storage_id: None,
            extension: None,
            sha256: None,
            size: None,
            has_original: false,
            has_thumbnails: false,
            position: None,
            lang: None,
            alt_text: None,
        }
    }
}

/// A `value_annotation`: a `resource` row and nothing else.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueAnnotationRow {
    pub resource: ResourceRow,
}

impl ValueAnnotationRow {
    /// The annotation's id. It is the id of its `resource` row.
    pub fn id(&self) -> AnnotationId {
        AnnotationId(self.resource.id.0)
    }

    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            resource: ResourceRow::from_db_row::<DB>(row)?,
        })
    }
}
