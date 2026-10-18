//! `vocabulary`, `resource_class` and `property` (upstream `schema.sql` `vocabulary`,
//! `resource_class`, `property`).

use crate::backend::Backend;

use crate::error::DbError;
use crate::ids::{ClassId, PropertyId, UserId, VocabularyId};

/// One row of `vocabulary`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VocabularyRow {
    pub id: VocabularyId,
    pub owner_id: Option<UserId>,
    pub namespace_uri: String,
    pub prefix: String,
    pub label: String,
    pub comment: Option<String>,
}

impl VocabularyRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            owner_id: DB::opt_id(row, "owner_id")?,
            namespace_uri: DB::text(row, "namespace_uri")?,
            prefix: DB::text(row, "prefix")?,
            label: DB::text(row, "label")?,
            comment: DB::opt_text(row, "comment")?,
        })
    }
}

/// Insert input for a `vocabulary`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewVocabulary {
    pub owner_id: Option<UserId>,
    pub namespace_uri: String,
    pub prefix: String,
    pub label: String,
    pub comment: Option<String>,
}

/// One row of `resource_class`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassRow {
    pub id: ClassId,
    pub owner_id: Option<UserId>,
    pub vocabulary_id: VocabularyId,
    pub local_name: String,
    pub label: String,
    pub comment: Option<String>,
}

impl ClassRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            owner_id: DB::opt_id(row, "owner_id")?,
            vocabulary_id: DB::id(row, "vocabulary_id")?,
            local_name: DB::text(row, "local_name")?,
            label: DB::text(row, "label")?,
            comment: DB::opt_text(row, "comment")?,
        })
    }
}

/// Insert input for a `resource_class`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewClass {
    pub owner_id: Option<UserId>,
    pub vocabulary_id: VocabularyId,
    pub local_name: String,
    pub label: String,
    pub comment: Option<String>,
}

/// One row of `property`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyRow {
    pub id: PropertyId,
    pub owner_id: Option<UserId>,
    pub vocabulary_id: VocabularyId,
    pub local_name: String,
    pub label: String,
    pub comment: Option<String>,
}

impl PropertyRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            owner_id: DB::opt_id(row, "owner_id")?,
            vocabulary_id: DB::id(row, "vocabulary_id")?,
            local_name: DB::text(row, "local_name")?,
            label: DB::text(row, "label")?,
            comment: DB::opt_text(row, "comment")?,
        })
    }
}

/// Insert input for a `property`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewProperty {
    pub owner_id: Option<UserId>,
    pub vocabulary_id: VocabularyId,
    pub local_name: String,
    pub label: String,
    pub comment: Option<String>,
}
