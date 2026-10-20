//! A term list or vocabulary as `omeka_core::publication` reads it: its [`Source`], from the
//! upstream rows.

use omeka_core::publication::{SchemeKind, Source, SourceTerm, TermKind};
use omeka_core::term_list::{TermList, TermListKind};
use omeka_db::ids::{CustomVocabId, VocabularyId};
use omeka_db::repos::{CustomVocabRepo, VocabularyRepo};
use omeka_db::rows::{CustomVocabRow, PublicationTarget, VocabularyRow};
use omeka_db::with_db;

use crate::errors::ApiError;
use crate::messages;
use crate::read::{Api, db_error, not_found, parse_id};
use crate::write::vocab_member::{self, Member};

pub(super) const LIST_CLASS: &str = "CustomVocab\\Entity\\CustomVocab";
pub(super) const VOCABULARY_CLASS: &str = "Omeka\\Entity\\Vocabulary";

/// The upstream row a publication is about.
pub(super) enum Upstream {
    TermList(CustomVocabRow),
    Vocabulary(VocabularyRow),
}

impl Upstream {
    pub(super) fn target(&self) -> PublicationTarget {
        match self {
            Self::TermList(row) => PublicationTarget::TermList(row.id),
            Self::Vocabulary(row) => PublicationTarget::Vocabulary(row.id),
        }
    }
}

/// The row a `/api/<resource>/<id>/publication` path names.
pub(super) async fn load(
    api: &Api<'_>,
    resource: &str,
    raw_id: &str,
) -> Result<Upstream, ApiError> {
    match resource {
        "custom_vocabs" => {
            let id = parse_id(LIST_CLASS, raw_id)?;
            with_db!(api.pool(), |db| CustomVocabRepo::new(db)
                .get(CustomVocabId(id))
                .await)
            .map_err(db_error)?
            .map(Upstream::TermList)
            .ok_or_else(|| not_found(LIST_CLASS, raw_id))
        }
        "vocabularies" => {
            let id = parse_id(VOCABULARY_CLASS, raw_id)?;
            with_db!(api.pool(), |db| VocabularyRepo::new(db)
                .get(VocabularyId(id))
                .await)
            .map_err(db_error)?
            .map(Upstream::Vocabulary)
            .ok_or_else(|| not_found(VOCABULARY_CLASS, raw_id))
        }
        other => Err(ApiError::not_found(messages::unknown_resource(other))),
    }
}

/// The scheme the row describes now. A list of items has no terms of its own to publish.
pub(super) async fn source(api: &Api<'_>, upstream: &Upstream) -> Result<Source, ApiError> {
    match upstream {
        Upstream::TermList(row) => term_list(row).ok_or_else(|| {
            super::refused("o:item_set", messages::PUBLICATION_ITEMS_LIST.to_string())
        }),
        Upstream::Vocabulary(row) => vocabulary(api, row).await,
    }
}

/// A term list's words or links, in list order; `None` for a list of items.
pub(super) fn term_list(row: &CustomVocabRow) -> Option<Source> {
    let list = TermList::from_columns(
        row.label.clone(),
        row.lang.clone(),
        row.item_set_id.map(|id| id.0),
        row.terms.as_ref(),
        row.uris.as_ref(),
    );
    let concept = |key: &str, label: &str, link: Option<&str>| SourceTerm {
        key: key.to_string(),
        kind: TermKind::Concept,
        label: label.to_string(),
        comment: None,
        link: link.map(str::to_string),
    };
    let terms = match list.kind() {
        Some(TermListKind::Items) => return None,
        Some(TermListKind::Uris) => list
            .uris
            .iter()
            .map(|(uri, label)| {
                let label = if label.is_empty() { uri } else { label };
                concept(uri, label, Some(uri))
            })
            .collect(),
        Some(TermListKind::Terms) => list
            .terms
            .iter()
            .map(|word| concept(word, word, None))
            .collect(),
        None => Vec::new(),
    };
    Some(Source {
        kind: SchemeKind::TermList,
        title: list.label,
        description: None,
        lang: list.lang,
        terms,
    })
}

async fn vocabulary(api: &Api<'_>, row: &VocabularyRow) -> Result<Source, ApiError> {
    let mut terms = Vec::new();
    for (member, kind) in [
        (Member::Class, TermKind::Class),
        (Member::Property, TermKind::Property),
    ] {
        for entry in vocab_member::list(api, member, row.id).await? {
            terms.push(SourceTerm {
                key: entry.local_name,
                kind,
                label: entry.label,
                comment: entry.comment,
                link: None,
            });
        }
    }
    Ok(Source {
        kind: SchemeKind::Vocabulary,
        title: row.label.clone(),
        description: row.comment.clone(),
        lang: None,
        terms,
    })
}
