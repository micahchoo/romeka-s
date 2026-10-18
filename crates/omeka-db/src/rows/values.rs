//! `value` (upstream `schema.sql` `value`).
//!
//! A value is one triple object: its `type` is `literal`, `uri`, `resource`, `resource:item`,
//! `resource:itemset` or `resource:media` (`application/src/DataType/*`), and the matching column
//! (`value`, `uri`, `value_resource_id`) carries the object. Upstream orders a resource's values by
//! id ascending (`Entity/Resource.php`, `@OrderBy({"id" = "ASC"})`).

use crate::backend::Backend;

use crate::error::DbError;
use crate::ids::{AnnotationId, PropertyId, ResourceId, ValueId};

/// One row of `value`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueRow {
    pub id: ValueId,
    pub resource_id: ResourceId,
    pub property_id: PropertyId,
    pub value_resource_id: Option<ResourceId>,
    pub value_annotation_id: Option<AnnotationId>,
    /// The column is named `type`; the field keeps the name with a raw identifier.
    pub r#type: String,
    pub lang: Option<String>,
    pub value: Option<String>,
    pub uri: Option<String>,
    pub is_public: bool,
}

impl ValueRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            resource_id: DB::id(row, "resource_id")?,
            property_id: DB::id(row, "property_id")?,
            value_resource_id: DB::opt_id(row, "value_resource_id")?,
            value_annotation_id: DB::opt_id(row, "value_annotation_id")?,
            r#type: DB::text(row, "type")?,
            lang: DB::opt_text(row, "lang")?,
            value: DB::opt_text(row, "value")?,
            uri: DB::opt_text(row, "uri")?,
            is_public: DB::boolean(row, "is_public")?,
        })
    }
}

/// Insert input for a `value`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewValue {
    pub resource_id: ResourceId,
    pub property_id: PropertyId,
    pub value_resource_id: Option<ResourceId>,
    pub value_annotation_id: Option<AnnotationId>,
    /// `literal`, `uri`, `resource`, `resource:item`, `resource:itemset` or `resource:media`.
    pub r#type: String,
    pub lang: Option<String>,
    pub value: Option<String>,
    pub uri: Option<String>,
    pub is_public: bool,
}
