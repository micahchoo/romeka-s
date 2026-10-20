//! Decisions: corrections keyed by an entry, applied after the rule in list
//! order (`docs/import-v2.md`, "Decisions").
//!
//! The boundary does not check that a decision's source or sheet is in the
//! plan. A decision whose entry is gone is an orphan, listed and kept, so a
//! plan that drops a source still saves.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use unicode_normalization::UnicodeNormalization;

use super::plan::{FlagKind, FlagPolicy, RecordKind, RelPath, SheetId, SlotName, SourceId};
use super::values::ValueType;

/// One unit of evidence with a fixed identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum EntryRef {
    File(FileRef),
    Row(RowRef),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRef {
    pub source: SourceId,
    pub path: RelPath,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RowRef {
    pub sheet: SheetId,
    pub key: RowKey,
}

/// A row's key: its key cell, NFC and trimmed, plus `#n` for the n-th row
/// with that value; or its line when the sheet has no key column. Stored
/// NFC so a key typed in another form still finds its row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RowKey(String);

impl RowKey {
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self(text.nfc().collect())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for RowKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RowKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(|text| Self::new(&text))
    }
}

/// An entry that must be a file: only files move between groups. On the
/// wire it is an [`EntryRef`] of the `file` form.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum FileEntry {
    File(FileRef),
}

impl FileEntry {
    #[must_use]
    pub fn file(&self) -> &FileRef {
        let Self::File(file) = self;
        file
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Decision {
    /// A file joins another file's group, starts its own, or leaves.
    Move { file: FileEntry, to: MoveTo },
    /// Leave the record that holds the entry out.
    Leave { entry: EntryRef },
    /// Replace a slot's values; no values clears it.
    Set {
        entry: EntryRef,
        target: RecordKind,
        slot: SlotName,
        values: Vec<DecisionValue>,
    },
    /// Choose the existing resource a record acts on; `None` chooses none.
    Match {
        entry: EntryRef,
        target: RecordKind,
        #[serde(deserialize_with = "super::nullable")]
        resource: Option<i64>,
    },
    /// Set the policy of one flag on one record, optionally on one slot.
    Flag {
        entry: EntryRef,
        flag: FlagKind,
        #[serde(deserialize_with = "super::nullable")]
        slot: Option<SlotName>,
        policy: FlagPolicy,
    },
}

impl Decision {
    /// The entry the decision is keyed by.
    #[must_use]
    pub fn entry(&self) -> EntryRef {
        match self {
            Self::Move { file, .. } => EntryRef::File(file.file().clone()),
            Self::Leave { entry }
            | Self::Set { entry, .. }
            | Self::Match { entry, .. }
            | Self::Flag { entry, .. } => entry.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum MoveTo {
    With(MoveWith),
    Alone(MoveAlone),
}

/// By hand rather than untagged, so a bad path inside `with` reports its
/// own error instead of "no variant matched".
impl<'de> Deserialize<'de> for MoveTo {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = MoveTo;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("\"own\", \"leave\" or { with }")
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<MoveTo, E> {
                MoveAlone::deserialize(serde::de::value::StrDeserializer::new(text))
                    .map(MoveTo::Alone)
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<MoveTo, A::Error> {
                MoveWith::deserialize(serde::de::value::MapAccessDeserializer::new(map))
                    .map(MoveTo::With)
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveWith {
    pub with: FileEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MoveAlone {
    /// The file's own path becomes its group key.
    Own,
    Leave,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionValue {
    #[serde(rename = "type")]
    pub value_type: ValueType,
    pub text: String,
    #[serde(deserialize_with = "super::nullable")]
    pub lang: Option<String>,
    #[serde(deserialize_with = "super::nullable")]
    pub label: Option<String>,
}
