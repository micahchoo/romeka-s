//! `value` (upstream `schema.sql` `value`).
//!
//! A value is one triple object: its `type` is `literal`, `uri`, `resource`, `resource:item`,
//! `resource:itemset` or `resource:media` (`application/src/DataType/*`), and the matching column
//! (`value`, `uri`, `value_resource_id`) carries the object. Upstream orders a resource's values by
//! id ascending (`Entity/Resource.php`, `@OrderBy({"id" = "ASC"})`).

use crate::backend::Backend;

use crate::error::DbError;
use crate::ids::{AnnotationId, PropertyId, ResourceId, ValueId};
use crate::rows::NewResource;

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

/// A value before its resource is known: [`NewValue`] without `resource_id`. A write that creates
/// a resource and its values in one transaction takes these, and ties each to the new id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueContent {
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

impl ValueContent {
    /// A public literal, the value a CSV cell or a batch edit writes.
    pub fn literal(property_id: PropertyId, text: impl Into<String>) -> Self {
        Self {
            property_id,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some(text.into()),
            uri: None,
            is_public: true,
        }
    }

    /// This content as a value of `resource_id`.
    pub fn on(&self, resource_id: ResourceId) -> NewValue {
        NewValue {
            resource_id,
            property_id: self.property_id,
            value_resource_id: self.value_resource_id,
            value_annotation_id: self.value_annotation_id,
            r#type: self.r#type.clone(),
            lang: self.lang.clone(),
            value: self.value.clone(),
            uri: self.uri.clone(),
            is_public: self.is_public,
        }
    }
}

/// A value to write together with the annotation resource it carries.
/// `ValueRepo::replace_all` writes the annotation first and sets the value's
/// `value_annotation_id` to it, so `content.value_annotation_id` is ignored.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueTree {
    pub content: ValueContent,
    pub annotation: Option<AnnotationTree>,
}

/// A value annotation to create: its resource row and the values it carries.
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationTree {
    pub resource: NewResource,
    pub values: Vec<ValueTree>,
}
