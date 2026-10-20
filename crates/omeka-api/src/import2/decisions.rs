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
    /// One change to the records that hold each of these entries, as if it
    /// were one decision per entry at this place in the list.
    Many {
        entries: Vec<EntryRef>,
        change: Change,
    },
    /// A list row joined to a file's record by hand. It wins over the
    /// sheet's join rule for that row and that file.
    Pair { row: RowRef, file: FileRef },
}

/// A decision without its entry: what a `many` decision does to each of
/// its entries. It is never itself `many` or `pair`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Change {
    Move {
        to: MoveTo,
    },
    Leave {},
    Set {
        target: RecordKind,
        slot: SlotName,
        values: Vec<DecisionValue>,
    },
    Match {
        target: RecordKind,
        #[serde(deserialize_with = "super::nullable")]
        resource: Option<i64>,
    },
    Flag {
        flag: FlagKind,
        #[serde(deserialize_with = "super::nullable")]
        slot: Option<SlotName>,
        policy: FlagPolicy,
    },
}

/// What a `leave`, `set`, `match` or `flag` decision does, single or part of
/// a `many`.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Act<'p> {
    Leave,
    Set {
        target: RecordKind,
        slot: &'p SlotName,
        values: &'p [DecisionValue],
    },
    Match {
        target: RecordKind,
        resource: Option<i64>,
    },
    Flag {
        flag: FlagKind,
        slot: Option<&'p SlotName>,
        policy: FlagPolicy,
    },
}

impl Decision {
    /// What the decision does to each entry it names; `None` for `move`
    /// and `pair`, which act before records exist.
    pub(crate) fn act(&self) -> Option<Act<'_>> {
        Some(match self {
            Self::Leave { .. }
            | Self::Many {
                change: Change::Leave {},
                ..
            } => Act::Leave,
            Self::Set {
                target,
                slot,
                values,
                ..
            }
            | Self::Many {
                change:
                    Change::Set {
                        target,
                        slot,
                        values,
                    },
                ..
            } => Act::Set {
                target: *target,
                slot,
                values,
            },
            Self::Match {
                target, resource, ..
            }
            | Self::Many {
                change: Change::Match { target, resource },
                ..
            } => Act::Match {
                target: *target,
                resource: *resource,
            },
            Self::Flag {
                flag, slot, policy, ..
            }
            | Self::Many {
                change: Change::Flag { flag, slot, policy },
                ..
            } => Act::Flag {
                flag: *flag,
                slot: slot.as_ref(),
                policy: *policy,
            },
            Self::Move { .. }
            | Self::Pair { .. }
            | Self::Many {
                change: Change::Move { .. },
                ..
            } => return None,
        })
    }

    fn entries(&self) -> &[EntryRef] {
        match self {
            Self::Leave { entry }
            | Self::Set { entry, .. }
            | Self::Match { entry, .. }
            | Self::Flag { entry, .. } => std::slice::from_ref(entry),
            Self::Many { entries, .. } => entries,
            Self::Move { .. } | Self::Pair { .. } => &[],
        }
    }
}

/// One decision on one entry, at its decision's place in the list.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Single<'p> {
    pub decision: usize,
    pub entry: &'p EntryRef,
    pub act: Act<'p>,
}

/// Every `leave`, `set`, `match` and `flag` decision as one decision per
/// entry, in list order: a `many` expands in place, in its entries' order,
/// so the later of two decisions on one entry still wins.
pub(crate) fn singles(decisions: &[Decision]) -> impl Iterator<Item = Single<'_>> {
    decisions
        .iter()
        .enumerate()
        .filter_map(|(decision, choice)| Some((decision, choice.act()?, choice.entries())))
        .flat_map(|(decision, act, entries)| {
            entries.iter().map(move |entry| Single {
                decision,
                entry,
                act,
            })
        })
}

/// One file's move, at its decision's place in the list.
pub(crate) struct Moved<'p> {
    pub decision: usize,
    pub file: &'p FileRef,
    pub to: &'p MoveTo,
}

/// Every `move`, a `many` move expanded in place. The boundary refuses a
/// row in a `many` move, so only files are here.
pub(crate) fn moves(decisions: &[Decision]) -> impl Iterator<Item = Moved<'_>> {
    decisions.iter().enumerate().flat_map(|(decision, choice)| {
        let (files, to): (Vec<&FileRef>, _) = match choice {
            Decision::Move { file, to } => (vec![file.file()], Some(to)),
            Decision::Many {
                entries,
                change: Change::Move { to },
            } => (
                entries
                    .iter()
                    .filter_map(|entry| match entry {
                        EntryRef::File(file) => Some(file),
                        EntryRef::Row(_) => None,
                    })
                    .collect(),
                Some(to),
            ),
            _ => (Vec::new(), None),
        };
        files.into_iter().filter_map(move |file| {
            Some(Moved {
                decision,
                file,
                to: to?,
            })
        })
    })
}

/// Every `pair`, in list order.
pub(crate) fn pairs(decisions: &[Decision]) -> impl Iterator<Item = (usize, &RowRef, &FileRef)> {
    decisions
        .iter()
        .enumerate()
        .filter_map(|(decision, choice)| match choice {
            Decision::Pair { row, file } => Some((decision, row, file)),
            _ => None,
        })
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
