//! What a preview request asks of `records` and `entries`, and the pages it
//! answers (`docs/import-v2.md`, "What the admin screens need at scale",
//! "Filters").
//!
//! Every filter is a fact the model holds or [`Facts`] reads from the
//! index. A filter left out does not narrow.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::facts::{Bucket, Facts, ShapeFacts};
use crate::import2::decisions::EntryRef;
use crate::import2::model::{
    EntryState, Fate, Flag, LeftBy, Model, Record, RecordId, SetRef, Subject, TemplateRef,
};
use crate::import2::plan::{FlagKind, RecordKind, SlotName};
use crate::import2::values::fold_title;

/// A page holds at most this many entries or records.
pub const MAX_PER_PAGE: usize = 500;
pub(super) const PER_PAGE: usize = 50;

/// One page of a list, with an optional text to look for.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageQuery {
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub per_page: Option<usize>,
    /// Compared as titles.
    #[serde(default)]
    pub text: Option<String>,
}

/// Records of one kind, filtered, one page.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordsQuery {
    /// `item` when left out.
    #[serde(default)]
    pub kind: Option<RecordKind>,
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub per_page: Option<usize>,
    /// Records with a flag of this kind.
    #[serde(default)]
    pub flag: Option<FlagKind>,
    /// With `flag`, its flags on this slot; alone, records with a value in it.
    #[serde(default)]
    pub slot: Option<SlotName>,
    /// Records with this text in any slot, compared as titles.
    #[serde(default)]
    pub text: Option<String>,
    /// Items of this `shapes` group, as the section answered its facts.
    #[serde(default)]
    pub shape: Option<ShapeFacts>,
    /// Records with a file in this folder or below it.
    #[serde(default)]
    pub folder: Option<String>,
    /// Records in this item set.
    #[serde(default)]
    pub set: Option<SetFilter>,
    /// Records of this item type, or of none.
    #[serde(default)]
    pub template: Option<TemplateFilter>,
    /// Items whose file count is in this bucket.
    #[serde(default)]
    pub files: Option<Bucket>,
    /// Records left out (`true`) or kept (`false`).
    #[serde(default)]
    pub left: Option<bool>,
}

/// An item set as `resolved.sets` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum SetFilter {
    Existing(i64),
    Record(u32),
    New(usize),
}

impl SetFilter {
    fn names(self, set: SetRef) -> bool {
        match (self, set) {
            (Self::Existing(wanted), SetRef::Existing(id)) => id == wanted,
            (Self::Record(wanted), SetRef::Record(record)) => record.0 == wanted,
            (Self::New(wanted), SetRef::New(at)) => at == wanted,
            _ => false,
        }
    }
}

/// An item type as `resolved.template` names it, or none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum TemplateFilter {
    Existing(i64),
    New(usize),
    None,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntriesQuery {
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub per_page: Option<usize>,
    #[serde(default)]
    pub flag: Option<FlagKind>,
    /// Entries whose path or row key holds this text, compared as titles.
    #[serde(default)]
    pub text: Option<String>,
    /// Entries in a record the run writes (`true`), or not (`false`).
    #[serde(default)]
    pub used: Option<bool>,
    /// Entries not used for this reason.
    #[serde(default)]
    pub reason: Option<Unused>,
    /// Files in this folder or below it.
    #[serde(default)]
    pub folder: Option<String>,
    /// Sheet rows whose record has files (`true`) or none (`false`). Files
    /// are never in the answer when this is set.
    #[serde(default)]
    pub paired: Option<bool>,
}

/// Why an entry is in no record the run writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Unused {
    /// A file the pattern matched no `identifier` in.
    Unmatched,
    /// A `leave` or `move … leave` decision.
    Decision,
    /// A flag whose policy is `leave`.
    Flag,
    /// Its item is left out.
    Parent,
    /// An item whose every file is left out and that no row joined.
    NoFiles,
    /// A file with the same bytes as a file the import keeps.
    Identical,
    /// An item no list row joined.
    NoRow,
    /// In no record for another reason, such as a row the join skipped.
    NoRecord,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
    pub items: Vec<T>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    #[serde(flatten)]
    pub state: EntryState,
    pub flags: Vec<Flag>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordView {
    #[serde(flatten)]
    pub record: Record,
    pub flags: Vec<Flag>,
}

/// What a flag is on, as a map key without cloning the entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum SubjectKey<'a> {
    Entry(&'a EntryRef),
    Record(u32),
}

impl<'a> SubjectKey<'a> {
    pub(super) fn of(subject: &'a Subject) -> Option<Self> {
        match subject {
            Subject::Entry(entry) => Some(Self::Entry(entry)),
            Subject::Record(record) => Some(Self::Record(record.0)),
            Subject::Sheet(_) | Subject::Plan => None,
        }
    }
}

/// The model's flags by what they are on.
pub(super) struct FlagsBy<'m>(HashMap<SubjectKey<'m>, Vec<&'m Flag>>);

impl<'m> FlagsBy<'m> {
    pub(super) fn new(model: &'m Model) -> Self {
        let mut by: HashMap<SubjectKey<'m>, Vec<&'m Flag>> = HashMap::new();
        for flag in &model.flags {
            if let Some(key) = SubjectKey::of(&flag.subject) {
                by.entry(key).or_default().push(flag);
            }
        }
        Self(by)
    }

    pub(super) fn on<'s>(&'s self, key: SubjectKey<'s>) -> &'s [&'s Flag] {
        self.0.get(&key).map_or(&[], Vec::as_slice)
    }

    pub(super) fn cloned(&self, key: SubjectKey<'_>) -> Vec<Flag> {
        self.on(key).iter().map(|flag| (*flag).clone()).collect()
    }

    pub(super) fn record_view(&self, record: &Record) -> RecordView {
        RecordView {
            record: record.clone(),
            flags: self.cloned(SubjectKey::Record(record.id.0)),
        }
    }
}

pub(super) fn records(
    model: &Model,
    query: &RecordsQuery,
    flags: &FlagsBy<'_>,
    facts: Option<&Facts<'_>>,
) -> Page<RecordView> {
    let kind = query.kind.unwrap_or(RecordKind::Item);
    let text = query.text.as_deref().map(fold_title);
    let matching = model.records.iter().filter(|record| {
        record.kind == kind
            && flagged(record, query, flags)
            && text.as_ref().is_none_or(|text| {
                record
                    .slots
                    .values()
                    .flatten()
                    .any(|value| fold_title(&value.value.text).contains(text.as_str()))
            })
            && query.shape.as_ref().is_none_or(|shape| {
                facts.is_some_and(|facts| facts.shape(record).as_ref() == Some(shape))
            })
            && query.folder.as_deref().is_none_or(|folder| {
                record
                    .files
                    .iter()
                    .any(|file| in_folder(file.path.as_str(), folder))
            })
            && query
                .set
                .is_none_or(|filter| record.resolved.sets.iter().any(|set| filter.names(*set)))
            && query
                .template
                .is_none_or(|filter| template_is(record.resolved.template, filter))
            && query
                .files
                .is_none_or(|bucket| Bucket::of(record.files.len()) == bucket)
            && query
                .left
                .is_none_or(|left| matches!(record.fate, Fate::Left(_)) == left)
    });
    page(matching, query.page, query.per_page, |record| {
        flags.record_view(record)
    })
}

fn flagged(record: &Record, query: &RecordsQuery, flags: &FlagsBy<'_>) -> bool {
    match (query.flag, &query.slot) {
        (Some(kind), slot) => flags
            .on(SubjectKey::Record(record.id.0))
            .iter()
            .any(|flag| {
                flag.kind == kind
                    && slot
                        .as_ref()
                        .is_none_or(|slot| flag.slot.as_ref() == Some(slot))
            }),
        (None, Some(slot)) => record.slots.contains_key(slot),
        (None, None) => true,
    }
}

fn template_is(template: Option<TemplateRef>, filter: TemplateFilter) -> bool {
    match (template, filter) {
        (Some(TemplateRef::Existing(id)), TemplateFilter::Existing(wanted)) => id == wanted,
        (Some(TemplateRef::New(at)), TemplateFilter::New(wanted)) => at == wanted,
        (None, TemplateFilter::None) => true,
        _ => false,
    }
}

/// Whether `path` lies in `folder` or below it; `""` is the top of the
/// source, which holds every path.
pub(super) fn in_folder(path: &str, folder: &str) -> bool {
    let folder = folder.trim_matches('/');
    folder.is_empty()
        || path
            .strip_prefix(folder)
            .is_some_and(|rest| rest.starts_with('/'))
}

pub(super) fn entries(model: &Model, query: &EntriesQuery, flags: &FlagsBy<'_>) -> Page<EntryView> {
    let text = query.text.as_deref().map(fold_title);
    let matching = model.entries.iter().filter(|state| {
        query.flag.is_none_or(|kind| {
            flags
                .on(SubjectKey::Entry(&state.entry))
                .iter()
                .any(|flag| flag.kind == kind)
        }) && text
            .as_ref()
            .is_none_or(|text| entry_text(&state.entry).contains(text))
            && query
                .used
                .is_none_or(|used| unused(model, state).is_none() == used)
            && query
                .reason
                .is_none_or(|reason| unused(model, state) == Some(reason))
            && query
                .folder
                .as_deref()
                .is_none_or(|folder| match &state.entry {
                    EntryRef::File(file) => in_folder(file.path.as_str(), folder),
                    EntryRef::Row(_) => false,
                })
            && query.paired.is_none_or(|paired| match &state.entry {
                EntryRef::Row(_) => state
                    .record
                    .is_some_and(|record| !model.record(record).files.is_empty() == paired),
                EntryRef::File(_) => false,
            })
    });
    page(matching, query.page, query.per_page, |state| EntryView {
        state: state.clone(),
        flags: flags.cloned(SubjectKey::Entry(&state.entry)),
    })
}

/// Why the entry is in no record the run writes; `None` when it is in one.
pub(super) fn unused(model: &Model, state: &EntryState) -> Option<Unused> {
    let by = |left: LeftBy| match left {
        LeftBy::Decision(_) => Unused::Decision,
        LeftBy::Flag(_) => Unused::Flag,
        LeftBy::Parent => Unused::Parent,
        LeftBy::NoFiles => Unused::NoFiles,
        LeftBy::Identical => Unused::Identical,
        LeftBy::NoRow => Unused::NoRow,
    };
    if let Some(left) = state.left {
        return Some(by(left));
    }
    let Some(record) = state.record else {
        return Some(if state.unmatched {
            Unused::Unmatched
        } else {
            Unused::NoRecord
        });
    };
    match model.record(record).fate {
        Fate::Left(left) => Some(by(left)),
        Fate::Create | Fate::Act(_) => None,
    }
}

fn entry_text(entry: &EntryRef) -> String {
    match entry {
        EntryRef::File(file) => fold_title(file.path.as_str()),
        EntryRef::Row(row) => fold_title(row.key.as_str()),
    }
}

pub(super) fn page<T, V>(
    items: impl Iterator<Item = T>,
    page: Option<usize>,
    per_page: Option<usize>,
    view: impl Fn(T) -> V,
) -> Page<V> {
    let per_page = per_page.unwrap_or(PER_PAGE).clamp(1, MAX_PER_PAGE);
    let page = page.unwrap_or(1).max(1);
    let mut total = 0;
    let mut out = Vec::new();
    let skip = (page - 1).saturating_mul(per_page);
    for item in items {
        if total >= skip && out.len() < per_page {
            out.push(view(item));
        }
        total += 1;
    }
    Page {
        total,
        page,
        per_page,
        items: out,
    }
}

/// The record a flag's subject lands in: the record itself, or the record
/// an entry lands in.
pub(super) fn record_of(
    subject: &Subject,
    entries: &HashMap<&EntryRef, Option<RecordId>>,
) -> Option<RecordId> {
    match subject {
        Subject::Record(record) => Some(*record),
        Subject::Entry(entry) => entries.get(entry).copied().flatten(),
        Subject::Sheet(_) | Subject::Plan => None,
    }
}
