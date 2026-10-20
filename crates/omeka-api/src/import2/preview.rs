//! The preview: one import derived against its stored index, and the
//! sections the admin asks for (`docs/import-v2.md`, "Wire").
//!
//! [`derive_import`] gathers the derive's input: the index from the
//! database ([`scan::load`], warm from [`IndexCache`]), the catalog
//! ([`catalog_load::load`]), the plan's sheets from disk through
//! [`SheetCache`], and file tags read lazily through the shared
//! [`TagCache`]. Only a tag a slot rule names opens a file. The derive runs
//! on the blocking pool. [`preview`] then cuts the sections from the model;
//! it is pure except for the `tags` section, which reads the tags of at
//! most [`TAG_SAMPLES`] files.

mod evidence;
mod sheets;

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use omeka_db::Backend;
use omeka_db::DbError;
use omeka_db::ids::ImportId;
use omeka_media::validation::FileValidation;
use serde::{Deserialize, Serialize};
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Type};

pub use evidence::{ColumnView, SheetGuess, SheetRead, SheetView, TagSample, TagView};
pub use sheets::{MAX_BYTES, MAX_SHEETS, ReadSheet, SheetCache};

use super::archive::{ArchiveRoots, SourceDir};
use super::catalog_load::{self, Owners};
use super::check::{Check, check};
use super::decisions::{Decision, EntryRef};
use super::derive::{FileTags, Sheets, derive};
use super::evidence::FileEntry;
use super::model::{Anomaly, EntryState, Flag, Model, OrphanReason, PatternState, Record, Subject};
use super::plan::{
    FlagKind, FlagPolicy, ImportPlanV2, RecordKind, SheetId, SlotName, SourceId, TagId,
};
use super::scan::{self, IndexCache, SourceState, StoredIndex};
use super::tags::TagCache;
use super::values::fold_title;

/// How many files the `tags` section reads tags from.
pub const TAG_SAMPLES: usize = 5;
/// A page holds at most this many entries or records.
pub const MAX_PER_PAGE: usize = 500;
const PER_PAGE: usize = 50;

/// The caches one process shares across previews.
#[derive(Default)]
pub struct Caches {
    pub tags: TagCache,
    pub sheets: SheetCache,
    pub index: IndexCache,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Section {
    Entries,
    Sheets,
    Tags,
    Records,
    Flags,
    Anomalies,
    Orphans,
    Check,
}

/// `POST /api/romeka/imports/{id}/preview`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewRequest {
    pub include: Vec<Section>,
    /// A plan to preview in place of the stored one; it is not saved.
    #[serde(default)]
    pub plan: Option<serde_json::Value>,
    #[serde(default)]
    pub records: Option<RecordsQuery>,
    #[serde(default)]
    pub entries: Option<EntriesQuery>,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlagCount {
    pub kind: FlagKind,
    pub slot: Option<SlotName>,
    pub policy: Option<FlagPolicy>,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrphanView {
    pub decision: usize,
    pub reason: OrphanReason,
    pub detail: Option<Decision>,
}

/// The answer: every source's scan state and the pattern's state, then
/// each section the request named.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub sources: Vec<SourceState>,
    pub pattern: PatternState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries: Option<Page<EntryView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheets: Option<Vec<SheetView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<TagView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub records: Option<Page<RecordView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<Vec<FlagCount>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anomalies: Option<Vec<Anomaly>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orphans: Option<Vec<OrphanView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check: Option<Check>,
}

/// File tags read from disk through the shared cache.
pub struct DiskTags {
    caches: Arc<Caches>,
    dirs: BTreeMap<SourceId, SourceDir>,
}

impl FileTags for DiskTags {
    fn tag(&self, source: &SourceId, entry: &FileEntry, tag: TagId) -> Option<String> {
        self.caches.tags.value(self.dirs.get(source)?, entry, tag)
    }
}

/// One import derived, with what the sections read besides the model.
pub struct Derived {
    pub plan: Arc<ImportPlanV2>,
    pub model: Model,
    pub stored: StoredIndex,
    pub sheets: BTreeMap<SheetId, Arc<ReadSheet>>,
    pub tags: DiskTags,
}

#[derive(Debug, thiserror::Error)]
pub enum PreviewError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error("the derive stopped: {0}")]
    Panicked(String),
}

/// Derive `plan` against import `import`'s stored index.
///
/// # Errors
/// [`PreviewError::Db`] when the index or the catalog cannot be read.
pub async fn derive_import<DB>(
    pool: &Pool<DB>,
    roots: &ArchiveRoots,
    caches: &Arc<Caches>,
    import: ImportId,
    plan: Arc<ImportPlanV2>,
    owners: Owners,
    files: FileValidation,
) -> Result<Derived, PreviewError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> f64: Decode<'x, DB> + Type<DB>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
    for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let stored = scan::load(pool, &caches.index, import, &plan.sources).await?;
    let catalog = catalog_load::load(pool, import, &plan, owners, files).await?;
    let roots = roots.clone();
    let caches = Arc::clone(caches);
    tokio::task::spawn_blocking(move || {
        let dirs = scan::source_dirs(&roots, &plan.sources);
        let read: BTreeMap<SheetId, Arc<ReadSheet>> = plan
            .sheets
            .iter()
            .filter_map(|sheet| {
                let read = caches.sheets.read(sheet, dirs.get(&sheet.source)?)?;
                Some((sheet.id.clone(), read))
            })
            .collect();
        let sheets: Sheets = read
            .iter()
            .map(|(id, read)| (id.clone(), read.sheet.clone()))
            .collect();
        let tags = DiskTags { caches, dirs };
        let model = derive(&stored.index, &sheets, &plan, &catalog, &tags);
        Derived {
            plan,
            model,
            stored,
            sheets: read,
            tags,
        }
    })
    .await
    .map_err(|error| PreviewError::Panicked(error.to_string()))
}

/// The sections `request` names, cut from `derived`.
#[must_use]
pub fn preview(derived: &Derived, request: &PreviewRequest) -> Preview {
    let model = &derived.model;
    let wants = |section| request.include.contains(&section);
    let mut by_subject: HashMap<SubjectKey<'_>, Vec<&Flag>> = HashMap::new();
    if wants(Section::Entries) || wants(Section::Records) {
        for flag in &model.flags {
            if let Some(key) = SubjectKey::of(&flag.subject) {
                by_subject.entry(key).or_default().push(flag);
            }
        }
    }
    let flags_of = |key: SubjectKey<'_>| -> Vec<Flag> {
        by_subject
            .get(&key)
            .map(|flags| flags.iter().map(|flag| (*flag).clone()).collect())
            .unwrap_or_default()
    };
    Preview {
        sources: derived.stored.sources.clone(),
        pattern: model.pattern.clone(),
        entries: wants(Section::Entries).then(|| {
            let query = request.entries.clone().unwrap_or_default();
            let text = query.text.as_deref().map(fold_title);
            let matching = model.entries.iter().filter(|state| {
                let flagged = query.flag.is_none_or(|kind| {
                    by_subject
                        .get(&SubjectKey::Entry(&state.entry))
                        .is_some_and(|flags| flags.iter().any(|flag| flag.kind == kind))
                });
                flagged
                    && text
                        .as_ref()
                        .is_none_or(|text| entry_text(&state.entry).contains(text))
            });
            page(matching, query.page, query.per_page, |state| EntryView {
                state: state.clone(),
                flags: flags_of(SubjectKey::Entry(&state.entry)),
            })
        }),
        sheets: wants(Section::Sheets).then(|| evidence::sheet_views(derived)),
        tags: wants(Section::Tags).then(|| evidence::tag_views(derived)),
        records: wants(Section::Records).then(|| {
            let query = request.records.clone().unwrap_or_default();
            let kind = query.kind.unwrap_or(RecordKind::Item);
            let text = query.text.as_deref().map(fold_title);
            let matching = model.records.iter().filter(|record| {
                let flags = by_subject.get(&SubjectKey::Record(record.id.0));
                let flagged = match (query.flag, &query.slot) {
                    (Some(kind), slot) => flags.is_some_and(|flags| {
                        flags.iter().any(|flag| {
                            flag.kind == kind
                                && slot
                                    .as_ref()
                                    .is_none_or(|slot| flag.slot.as_ref() == Some(slot))
                        })
                    }),
                    (None, Some(slot)) => record.slots.contains_key(slot),
                    (None, None) => true,
                };
                record.kind == kind
                    && flagged
                    && text.as_ref().is_none_or(|text| {
                        record
                            .slots
                            .values()
                            .flatten()
                            .any(|value| fold_title(&value.value.text).contains(text.as_str()))
                    })
            });
            page(matching, query.page, query.per_page, |record| RecordView {
                record: record.clone(),
                flags: flags_of(SubjectKey::Record(record.id.0)),
            })
        }),
        flags: wants(Section::Flags).then(|| flag_counts(&model.flags)),
        anomalies: wants(Section::Anomalies).then(|| model.anomalies.clone()),
        orphans: wants(Section::Orphans).then(|| {
            model
                .orphans
                .iter()
                .map(|orphan| OrphanView {
                    decision: orphan.decision,
                    reason: orphan.reason,
                    detail: derived.plan.decisions.get(orphan.decision).cloned(),
                })
                .collect()
        }),
        check: wants(Section::Check).then(|| check(&derived.plan, model)),
    }
}

/// What a flag is on, as a map key without cloning the entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SubjectKey<'a> {
    Entry(&'a EntryRef),
    Record(u32),
}

impl<'a> SubjectKey<'a> {
    fn of(subject: &'a Subject) -> Option<Self> {
        match subject {
            Subject::Entry(entry) => Some(Self::Entry(entry)),
            Subject::Record(record) => Some(Self::Record(record.0)),
            Subject::Sheet(_) | Subject::Plan => None,
        }
    }
}

fn entry_text(entry: &EntryRef) -> String {
    match entry {
        EntryRef::File(file) => fold_title(file.path.as_str()),
        EntryRef::Row(row) => fold_title(row.key.as_str()),
    }
}

fn page<'a, T: 'a, V>(
    items: impl Iterator<Item = &'a T>,
    page: Option<usize>,
    per_page: Option<usize>,
    view: impl Fn(&T) -> V,
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

fn flag_counts(flags: &[Flag]) -> Vec<FlagCount> {
    let mut counts: Vec<FlagCount> = Vec::new();
    for flag in flags {
        match counts.iter_mut().find(|count| {
            count.kind == flag.kind && count.slot == flag.slot && count.policy == flag.policy
        }) {
            Some(count) => count.count += 1,
            None => counts.push(FlagCount {
                kind: flag.kind,
                slot: flag.slot.clone(),
                policy: flag.policy,
                count: 1,
            }),
        }
    }
    counts.sort_by(|a, b| {
        (a.kind, a.slot.as_ref().map(ToString::to_string))
            .cmp(&(b.kind, b.slot.as_ref().map(ToString::to_string)))
    });
    counts
}

#[cfg(test)]
mod tests;
