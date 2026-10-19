//! The check the upstream write paths run before they change a published list or vocabulary.
//!
//! A write proposes the terms the row will have afterwards; [`term_list`] and [`vocabulary`] turn
//! that into the next snapshot and run the same [`check`] a publish runs, so the API refuses with
//! a 422 what a publish would refuse. A draft (no version yet) changes freely.

use omeka_core::publication::{SchemeKind, Source, SourceTerm, TermKind, check, draft, retire};
use omeka_db::ids::{CustomVocabId, VocabularyId};
use omeka_db::rows::{CustomVocabRow, PublicationTarget};

use super::admin::refusal;
use super::{record, refused, source};
use crate::errors::ApiError;
use crate::messages;
use crate::read::Api;

/// What a write does to a vocabulary.
pub(crate) enum VocabularyChange<'a> {
    Delete,
    /// The vocabulary's namespace after the write, and each member list it rewrites (`None`
    /// keeps the stored list).
    Update {
        namespace: &'a str,
        classes: Option<&'a [String]>,
        properties: Option<&'a [String]>,
    },
    /// One member leaves under `from`, and comes back under `to` when it stays in the vocabulary.
    Member {
        from: &'a str,
        to: Option<&'a str>,
    },
}

/// Refuse a term list write that would break a published promise. `proposed` is the row after
/// the write, `None` for a delete.
pub(crate) async fn term_list(
    api: &Api<'_>,
    id: CustomVocabId,
    proposed: Option<&CustomVocabRow>,
) -> Result<(), ApiError> {
    let record = record(api.pool(), PublicationTarget::TermList(id)).await?;
    let Some(latest) = record.latest else {
        return Ok(());
    };
    let Some(proposed) = proposed else {
        return Err(not_deletable());
    };
    let keys = source::term_list(proposed)
        .map(|source| source.terms.into_iter().map(|term| term.key).collect())
        .unwrap_or_default();
    let next = keyed(
        SchemeKind::TermList,
        &latest.snapshot.namespace,
        keys,
        &record.states,
    );
    check(&latest.snapshot, &next).map_err(refusal)
}

/// What a merge of `from` into `to` keeps of a published term list.
#[derive(Debug, Default)]
pub(crate) struct Retired {
    /// The spellings that stay in the list, deprecated and replaced by `to`.
    pub keys: Vec<String>,
    /// The term state to store with the merge; `None` when it does not change.
    pub states: Option<serde_json::Value>,
}

/// The spellings a merge must keep because a version of the list carries them
/// (`omeka_core::publication::retire`). A draft list keeps none.
pub(crate) async fn retire_merged(
    api: &Api<'_>,
    id: CustomVocabId,
    from: &[String],
    to: &str,
) -> Result<Retired, ApiError> {
    let mut record = record(api.pool(), PublicationTarget::TermList(id)).await?;
    let Some(latest) = &record.latest else {
        return Ok(Retired::default());
    };
    let before = record.states.clone();
    let keys = retire(&latest.snapshot, &mut record.states, from, to);
    let states = (record.states != before)
        .then(|| serde_json::to_value(&record.states).map_err(ApiError::internal))
        .transpose()?;
    Ok(Retired { keys, states })
}

/// Refuse a vocabulary or member write that would break a published promise.
pub(crate) async fn vocabulary(
    api: &Api<'_>,
    id: VocabularyId,
    change: VocabularyChange<'_>,
) -> Result<(), ApiError> {
    let record = record(api.pool(), PublicationTarget::Vocabulary(id)).await?;
    let Some(latest) = record.latest else {
        return Ok(());
    };
    let stored = |kind: TermKind| -> Vec<String> {
        latest
            .snapshot
            .terms
            .iter()
            .filter(|term| term.kind == kind)
            .map(|term| term.key.clone())
            .collect()
    };
    let (namespace, keys) = match change {
        VocabularyChange::Delete => return Err(not_deletable()),
        VocabularyChange::Update {
            namespace,
            classes,
            properties,
        } => {
            let mut keys = classes.map_or_else(|| stored(TermKind::Class), <[String]>::to_vec);
            keys.extend(properties.map_or_else(|| stored(TermKind::Property), <[String]>::to_vec));
            (namespace, keys)
        }
        VocabularyChange::Member { from, to } => {
            let keys = latest
                .snapshot
                .terms
                .iter()
                .map(|term| term.key.as_str())
                .filter(|key| *key != from)
                .chain(to)
                .map(str::to_string)
                .collect();
            (latest.snapshot.namespace.as_str(), keys)
        }
    };
    let next = keyed(SchemeKind::Vocabulary, namespace, keys, &record.states);
    check(&latest.snapshot, &next).map_err(refusal)
}

/// The next snapshot as far as the rule reads it: the namespace and each key with its state.
fn keyed(
    kind: SchemeKind,
    namespace: &str,
    keys: Vec<String>,
    states: &omeka_core::publication::TermStates,
) -> omeka_core::publication::Snapshot {
    let source = Source {
        kind,
        title: String::new(),
        description: None,
        lang: None,
        terms: keys
            .into_iter()
            .map(|key| SourceTerm {
                label: key.clone(),
                key,
                kind: TermKind::Concept,
                comment: None,
                link: None,
            })
            .collect(),
    };
    draft(&source, namespace, states).0
}

fn not_deletable() -> ApiError {
    refused("o:id", messages::PUBLICATION_NOT_DELETABLE.to_string())
}
