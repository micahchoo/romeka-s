//! The term catalogue: vocabulary prefixes and the `prefix:localName` parts of properties and
//! resource classes, in projections narrower than the row structs.
//!
//! A document prints a property's term and label and a class's term, so the API loads these once
//! per request (`omeka-api` `read/catalog.rs`). [`TermScope::Referenced`] keeps the terms a stored
//! row names (a value or a template property for a property, a resource for a class), which is
//! every term a read can print and far fewer rows than the whole table.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use crate::DbError;
use crate::backend::Backend;
use crate::ids::{ClassId, PropertyId, VocabularyId};

/// Which terms a catalogue read loads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermScope {
    /// The terms a stored row names.
    Referenced,
    /// Every term, for a caller that resolves a term no row names yet (a write body key, a
    /// `property[i][property]` filter).
    All,
}

/// One property as a catalogue holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyTerm {
    pub id: PropertyId,
    pub vocabulary_id: VocabularyId,
    pub local_name: String,
    pub label: String,
    /// The label of the vocabulary the property belongs to.
    pub vocabulary_label: String,
}

/// One resource class as a catalogue holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassTerm {
    pub id: ClassId,
    pub vocabulary_id: VocabularyId,
    pub local_name: String,
}

const PROPERTIES: &str = "SELECT p.id AS id, p.vocabulary_id AS vocabulary_id, \
     p.local_name AS local_name, p.label AS label, v.label AS vocabulary_label \
     FROM property p JOIN vocabulary v ON v.id = p.vocabulary_id";

const REFERENCED_PROPERTIES: &str = " WHERE p.id IN (SELECT property_id FROM value \
     UNION SELECT property_id FROM resource_template_property)";

const CLASSES: &str = "SELECT id, vocabulary_id, local_name FROM resource_class";

const REFERENCED_CLASSES: &str = " WHERE id IN (SELECT resource_class_id FROM resource \
     WHERE resource_class_id IS NOT NULL)";

/// The term catalogue.
pub struct TermRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> TermRepo<'a, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Type<DB>,
    for<'x> &'x str: ColumnIndex<DB::Row>,
{
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    /// Every vocabulary's `(id, prefix)`.
    pub async fn prefixes(&self) -> Result<Vec<(VocabularyId, String)>, DbError> {
        let rows = sqlx::query::<DB>("SELECT id, prefix FROM vocabulary ORDER BY id")
            .fetch_all(self.pool)
            .await?;
        rows.iter()
            .map(|row| Ok((DB::id(row, "id")?, DB::text(row, "prefix")?)))
            .collect()
    }

    pub async fn properties(&self, scope: TermScope) -> Result<Vec<PropertyTerm>, DbError> {
        let sql = match scope {
            TermScope::Referenced => format!("{PROPERTIES}{REFERENCED_PROPERTIES}"),
            TermScope::All => PROPERTIES.to_string(),
        };
        let rows = sqlx::query::<DB>(&sql).fetch_all(self.pool).await?;
        rows.iter()
            .map(|row| {
                Ok(PropertyTerm {
                    id: DB::id(row, "id")?,
                    vocabulary_id: DB::id(row, "vocabulary_id")?,
                    local_name: DB::text(row, "local_name")?,
                    label: DB::text(row, "label")?,
                    vocabulary_label: DB::text(row, "vocabulary_label")?,
                })
            })
            .collect()
    }

    pub async fn classes(&self, scope: TermScope) -> Result<Vec<ClassTerm>, DbError> {
        let sql = match scope {
            TermScope::Referenced => format!("{CLASSES}{REFERENCED_CLASSES}"),
            TermScope::All => CLASSES.to_string(),
        };
        let rows = sqlx::query::<DB>(&sql).fetch_all(self.pool).await?;
        rows.iter()
            .map(|row| {
                Ok(ClassTerm {
                    id: DB::id(row, "id")?,
                    vocabulary_id: DB::id(row, "vocabulary_id")?,
                    local_name: DB::text(row, "local_name")?,
                })
            })
            .collect()
    }
}
