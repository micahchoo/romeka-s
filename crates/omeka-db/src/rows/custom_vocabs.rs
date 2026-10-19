//! `custom_vocab`, the term list table the Custom Vocab module installs (`Module::install`).
//!
//! `terms` and `uris` are `(DC2Type:json)` columns. The row keeps them as the JSON the column holds,
//! because an upstream install can carry shapes its own older releases wrote (a list where a map
//! belongs); the domain parse in `omeka-core::term_list` decides what each shape means.

use serde_json::Value;

use crate::backend::Backend;
use crate::error::DbError;
use crate::ids::{CustomVocabId, ItemSetId, UserId};

/// One row of `custom_vocab`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomVocabRow {
    pub id: CustomVocabId,
    pub item_set_id: Option<ItemSetId>,
    pub owner_id: Option<UserId>,
    pub label: String,
    pub lang: Option<String>,
    /// A JSON list of terms, or `None`.
    pub terms: Option<Value>,
    /// A JSON map of URI to label, or `None`.
    pub uris: Option<Value>,
}

impl CustomVocabRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            item_set_id: DB::opt_id(row, "item_set_id")?,
            owner_id: DB::opt_id(row, "owner_id")?,
            label: DB::text(row, "label")?,
            lang: DB::opt_text(row, "lang")?,
            terms: DB::opt_json(row, "terms")?,
            uris: DB::opt_json(row, "uris")?,
        })
    }
}

/// Insert input for a `custom_vocab`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCustomVocab {
    pub item_set_id: Option<ItemSetId>,
    pub owner_id: Option<UserId>,
    pub label: String,
    pub lang: Option<String>,
    pub terms: Option<Value>,
    pub uris: Option<Value>,
}
