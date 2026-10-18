//! `resource_template` and `resource_template_property` (upstream `schema.sql`).

use crate::backend::Backend;

use crate::error::DbError;
use crate::ids::{ClassId, PropertyId, TemplateId, TemplatePropertyId, UserId};

/// One row of `resource_template`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateRow {
    pub id: TemplateId,
    pub owner_id: Option<UserId>,
    pub resource_class_id: Option<ClassId>,
    pub title_property_id: Option<PropertyId>,
    pub description_property_id: Option<PropertyId>,
    pub label: String,
}

impl TemplateRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            owner_id: DB::opt_id(row, "owner_id")?,
            resource_class_id: DB::opt_id(row, "resource_class_id")?,
            title_property_id: DB::opt_id(row, "title_property_id")?,
            description_property_id: DB::opt_id(row, "description_property_id")?,
            label: DB::text(row, "label")?,
        })
    }
}

/// Insert input for a `resource_template`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTemplate {
    pub owner_id: Option<UserId>,
    pub resource_class_id: Option<ClassId>,
    pub title_property_id: Option<PropertyId>,
    pub description_property_id: Option<PropertyId>,
    pub label: String,
}

/// One row of `resource_template_property`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplatePropertyRow {
    pub id: TemplatePropertyId,
    pub resource_template_id: TemplateId,
    pub property_id: PropertyId,
    pub alternate_label: Option<String>,
    pub alternate_comment: Option<String>,
    pub position: Option<i64>,
    /// Upstream `json_array`, for example `["literal"]`. `None` means no data types were set.
    pub data_type: Option<Vec<String>>,
    pub is_required: bool,
    pub is_private: bool,
    pub default_lang: Option<String>,
}

impl TemplatePropertyRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            resource_template_id: DB::id(row, "resource_template_id")?,
            property_id: DB::id(row, "property_id")?,
            alternate_label: DB::opt_text(row, "alternate_label")?,
            alternate_comment: DB::opt_text(row, "alternate_comment")?,
            position: DB::opt_int(row, "position")?,
            data_type: DB::opt_string_list(row, "data_type")?,
            is_required: DB::boolean(row, "is_required")?,
            is_private: DB::boolean(row, "is_private")?,
            default_lang: DB::opt_text(row, "default_lang")?,
        })
    }
}

/// Insert input for a `resource_template_property`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTemplateProperty {
    pub resource_template_id: TemplateId,
    pub property_id: PropertyId,
    pub alternate_label: Option<String>,
    pub alternate_comment: Option<String>,
    pub position: Option<i64>,
    pub data_type: Option<Vec<String>>,
    pub is_required: bool,
    pub is_private: bool,
    pub default_lang: Option<String>,
}
