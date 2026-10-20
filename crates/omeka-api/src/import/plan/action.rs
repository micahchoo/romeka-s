//! The action: what records do to the install, and how each kind finds the
//! resource it acts on.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{FlagPolicy, Term};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionPlan {
    pub action: Action,
    #[serde(rename = "match")]
    pub match_by: MatchPlan,
    pub unmatched: Unmatched,
}

/// One action per import (upstream CSVImport parity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Create,
    Append,
    Revise,
    Update,
    Replace,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MatchPlan {
    pub item: MatchRule,
    pub media: MatchRule,
    pub item_set: MatchRule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatchRule {
    pub key: MatchKey,
    pub scope: Scope,
}

/// The slot a record is matched on: a term's values, or the internal id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MatchKey {
    InternalId,
    Term(Term),
}

impl Serialize for MatchKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::InternalId => serializer.serialize_str("internalId"),
            Self::Term(term) => term.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for MatchKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        if text == "internalId" {
            return Ok(Self::InternalId);
        }
        Term::parse(&text)
            .map(Self::Term)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Scope {
    Named(NamedScope),
    Sets(SetsScope),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NamedScope {
    /// Items: the sets this import's records name. Item sets: this import's
    /// item-set records. Media: the media of the matched item.
    ImportSets,
    All,
}

/// Resources in these item sets, by id.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetsScope {
    pub sets: Vec<i64>,
}

/// What a record with no match does on a non-create action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Unmatched {
    Create,
    Leave,
}

impl Unmatched {
    /// The policy of the `unmatched` flag when the plan names none.
    #[must_use]
    pub fn policy(self) -> FlagPolicy {
        match self {
            Self::Create => FlagPolicy::Keep,
            Self::Leave => FlagPolicy::Leave,
        }
    }
}
