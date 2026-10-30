//! The rows a bulk value rewrite reads and the writes it applies (`ValueRewriteRepo`).
//!
//! A tag is a plain-text `dcterms:subject` value; a term list's word is a text value whose type is
//! `customvocab:<id>`. Both are `value` rows with text in `value` and no `uri` or
//! `value_resource_id`, so one row shape serves both, joined to the `resource` row the caller needs
//! for its permission and visibility checks.

use serde_json::Value;

use crate::backend::Backend;
use crate::error::DbError;
use crate::ids::{CustomVocabId, PropertyId, ResourceId, TemplateId, UserId, ValueId};
use crate::rows::NewCustomVocab;

/// One text value with the resource that carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextValueRow {
    pub id: ValueId,
    pub resource_id: ResourceId,
    pub property_id: PropertyId,
    pub r#type: String,
    pub lang: Option<String>,
    pub text: String,
    pub is_public: bool,
    pub has_annotation: bool,
    /// `resource.resource_type`, one of [`crate::rows::resource_type`].
    pub resource_type: String,
    pub owner_id: Option<UserId>,
    pub resource_is_public: bool,
    pub template_id: Option<TemplateId>,
}

impl TextValueRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            resource_id: DB::id(row, "resource_id")?,
            property_id: DB::id(row, "property_id")?,
            r#type: DB::text(row, "type")?,
            lang: DB::opt_text(row, "lang")?,
            text: DB::text(row, "value")?,
            is_public: DB::boolean(row, "is_public")?,
            has_annotation: DB::opt_int(row, "value_annotation_id")?.is_some(),
            resource_type: DB::text(row, "resource_type")?,
            owner_id: DB::opt_id(row, "owner_id")?,
            resource_is_public: DB::boolean(row, "resource_is_public")?,
            template_id: DB::opt_id(row, "resource_template_id")?,
        })
    }
}

/// The list a promotion writes its words into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromoteList {
    /// A new list.
    Create(NewCustomVocab),
    /// A stored list, whose `terms` column becomes this JSON list.
    Terms(CustomVocabId, Value),
}

/// Everything one promotion writes, in one transaction: the list, the values retyped to the
/// list's data type name, and the template rows that come to offer the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromoteWrite {
    pub list: PromoteList,
    /// Values retyped from `from_type` to the list's name. A value whose type is no longer
    /// `from_type` when the transaction runs is left alone.
    pub values: Vec<ValueId>,
    pub from_type: String,
    /// The property whose template rows come to offer the list.
    pub property: PropertyId,
    /// Each template's row for `property` gains the list's name; a template with no such row gets
    /// one, last in order, offering only the list.
    pub templates: Vec<TemplateId>,
    /// What a row with no data types offers: such a row keeps offering these, plus the list.
    pub unrestricted: Vec<String>,
    /// The resources whose `modified` moves.
    pub resources: Vec<ResourceId>,
    /// For [`PromoteList::Terms`]: the list's `terms` as the write was planned from. The write
    /// fails with [`crate::DbError::Conflict`] when the stored column holds something else, so
    /// a term another promotion added in between is never dropped.
    pub terms_read: Option<Option<Value>>,
}

/// Everything one merge writes, in one transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeWrite {
    /// Values whose text becomes `to`. A value whose type is no longer `of_type` is left alone.
    pub rewrite: Vec<ValueId>,
    pub to: String,
    pub of_type: String,
    /// Values deleted because the merge made them repeat another value of the same resource.
    pub delete: Vec<ValueId>,
    /// A list whose `terms` column becomes this JSON list.
    pub list_terms: Option<(CustomVocabId, Value)>,
    /// For `list_terms`: the list's `terms` as the write was planned from, checked as
    /// [`PromoteWrite::terms_read`] is.
    pub terms_read: Option<Option<Value>>,
    /// A published list whose `romeka_publication.terms` becomes this JSON, so the words the merge
    /// keeps as deprecated are marked in the same transaction.
    pub list_states: Option<(CustomVocabId, Value)>,
    /// The resources whose `modified` moves and whose title is read again.
    pub resources: Vec<ResourceId>,
    /// The title property of a resource whose template names none (`dcterms:title`).
    pub default_title_property: Option<PropertyId>,
}

/// One move of a text from one property to another, planned by the caller. Each statement
/// re-checks that the value still sits on `from` with the text `text`, so a value another writer
/// changed after the read is left as that writer left it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveWrite {
    pub from: PropertyId,
    pub to: PropertyId,
    pub text: String,
    /// Values whose property becomes `to`.
    pub retarget: Vec<ValueId>,
    /// Values deleted because their resource already holds the text on `to`.
    pub delete: Vec<ValueId>,
    /// Values that stay on `from` and gain a copy on `to`, with their language and visibility.
    pub copy: Vec<ValueId>,
    /// The resources whose `modified` moves and whose title is read again.
    pub resources: Vec<ResourceId>,
    /// The title property of a resource whose template names none (`dcterms:title`).
    pub default_title_property: Option<PropertyId>,
}
