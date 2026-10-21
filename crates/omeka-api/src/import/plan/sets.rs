//! Set sources: how a level or a free set names its item set. The shape
//! says what the text means; the engine never reads meaning from the text.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::{Combine, SourceRef};

/// How a level or a free set names its item set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SetSource {
    /// `{ "id": 42 }`: the install's set #42.
    Id(SetById),
    /// `{ "title": "2018", "description": null }`: the set with this title.
    Title(SetByTitle),
    /// `{ "each": { … } }`: one set per value a rule gives.
    Each(EachValue),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetById {
    pub id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetByTitle {
    /// Not empty after trim. Compared as a title (trim, NFC, case fold).
    pub title: String,
    /// Written as `dcterms:description` when the run makes the set. A set
    /// that exists keeps its own description.
    #[serde(deserialize_with = "crate::import::nullable")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EachValue {
    pub each: EachSet,
}

/// A slot rule whose values each name a set, and what the values are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EachSet {
    pub sources: Vec<SourceRef>,
    pub combine: Combine,
    /// The v2 value map. A value mapped to `""` adds no set.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub map: IndexMap<String, String>,
    pub names: SetNames,
}

/// What each value of an `each` rule is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SetNames {
    /// A title, compared as a title. No match makes a new set.
    Title,
    /// The id of a set in the install. No such set flags `unknown-set`.
    Id,
    /// The item key of one of this import's item-set records (rows).
    Key,
    /// v2's reading: an id that exists, else a record key, else a title.
    /// Only the lift writes it. The admin offers to change it.
    Text,
}
