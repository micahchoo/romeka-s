//! `romeka_publication` and `romeka_publication_version`: Romeka's own tables for publishing term
//! lists and vocabularies (migration 0003). Upstream has neither.
//!
//! `terms` and `snapshot` are JSON. The row keeps them as JSON; `omeka-core::publication` owns what
//! they mean, so this crate stays the storage boundary and nothing else.

use chrono::NaiveDateTime;
use serde_json::Value;

use crate::backend::Backend;
use crate::error::DbError;
use crate::ids::{CustomVocabId, PublicationId, VocabularyId};

/// What a publication publishes: a term list or a vocabulary, never both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationTarget {
    TermList(CustomVocabId),
    Vocabulary(VocabularyId),
}

impl PublicationTarget {
    /// The column that names this kind of target.
    pub(crate) fn column(self) -> &'static str {
        match self {
            Self::TermList(_) => "custom_vocab_id",
            Self::Vocabulary(_) => "vocabulary_id",
        }
    }

    pub(crate) fn raw_id(self) -> i64 {
        match self {
            Self::TermList(id) => crate::repos::bind_id(id),
            Self::Vocabulary(id) => crate::repos::bind_id(id),
        }
    }
}

/// One row of `romeka_publication`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationRow {
    pub id: PublicationId,
    /// `None` once upstream deleted the list or vocabulary; the versions still serve.
    pub target: Option<PublicationTarget>,
    /// The path segment under `/vocab/`, set at the first publish.
    pub slug: Option<String>,
    /// The namespace the terms are minted in, set at the first publish.
    pub namespace_uri: Option<String>,
    /// Each term's Romeka state, keyed by the term's key.
    pub terms: Value,
}

impl PublicationRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        let list: Option<CustomVocabId> = DB::opt_id(row, "custom_vocab_id")?;
        let vocabulary: Option<VocabularyId> = DB::opt_id(row, "vocabulary_id")?;
        let target = match (list, vocabulary) {
            (Some(id), None) => Some(PublicationTarget::TermList(id)),
            (None, Some(id)) => Some(PublicationTarget::Vocabulary(id)),
            (None, None) => None,
            (Some(_), Some(_)) => {
                return Err(DbError::Column {
                    column: "custom_vocab_id",
                    reason: "a publication names both a term list and a vocabulary".to_string(),
                });
            }
        };
        Ok(Self {
            id: DB::id(row, "id")?,
            target,
            slug: DB::opt_text(row, "slug")?,
            namespace_uri: DB::opt_text(row, "namespace_uri")?,
            terms: DB::json(row, "terms")?,
        })
    }
}

/// One row of `romeka_publication_version`: one publish, never changed afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationVersionRow {
    pub publication_id: PublicationId,
    /// 1 for the first publish, then one more each time.
    pub version: u32,
    pub created: NaiveDateTime,
    /// The published scheme as JSON.
    pub snapshot: Value,
}

impl PublicationVersionRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        let version = DB::int(row, "version")?;
        Ok(Self {
            publication_id: DB::id(row, "publication_id")?,
            version: u32::try_from(version).map_err(|_| DbError::Column {
                column: "version",
                reason: format!("not a version number: {version}"),
            })?,
            created: DB::datetime(row, "created")?,
            snapshot: DB::json(row, "snapshot")?,
        })
    }
}
