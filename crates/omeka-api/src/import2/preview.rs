//! The preview: one import derived against its stored index, and the
//! sections the admin asks for (`docs/import-v2.md`, "Wire").
//!
//! [`derive_import`] gathers the derive's input: the index from the
//! database ([`scan::load`], warm from [`IndexCache`]), the catalog
//! ([`catalog_load::load`]), the plan's sheets from disk through
//! [`SheetCache`] (a staging sheet through the [`Store`]), and file tags
//! read lazily through the shared [`TagCache`]. Only a tag a slot rule names
//! opens a file. The derive runs on the blocking pool. [`preview`] then cuts
//! the sections from the model; it is pure except for the `tags` section,
//! which reads the tags of at most [`TAG_SAMPLES`] files. A section is
//! computed only when the request names it.

mod causes;
mod columns;
mod counts;
mod evidence;
mod facts;
mod identical;
mod names;
mod query;
mod sample;
mod sheets;
mod unpaired;

use std::collections::BTreeMap;
use std::sync::Arc;

use omeka_db::Backend;
use omeka_db::DbError;
use omeka_db::ids::ImportId;
use omeka_media::store::Store;
use omeka_media::validation::FileValidation;
use serde::{Deserialize, Serialize};
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Type};

pub use causes::{FlagCount, FlagSample};
pub use columns::{ColumnMatches, SheetMatches};
pub use counts::{COUNTED, SlotValues, ValueCount, ValuesQuery};
pub use evidence::{
    ColumnView, SheetGuess, SheetRead, SheetView, TagSample, TagView, WorkbookView,
};
pub use facts::{Bucket, BucketCount, MediaKind, PAGE, SAMPLES, ShapeFacts, ShapeGroup, Shapes};
pub use identical::{IdenticalFiles, IdenticalView};
pub use names::{NameWords, NameWordsQuery, WordCount};
pub use query::{
    EntriesQuery, EntryView, MAX_PER_PAGE, Page, PageQuery, RecordView, RecordsQuery, SetFilter,
    TemplateFilter, Unused,
};
pub use sheets::{MAX_BYTES, MAX_SHEETS, ReadSheet, SheetCache, staging_path};
pub use unpaired::{Unpaired, UnpairedFile, UnpairedQuery, UnpairedRow};

use super::archive::{ArchiveRoots, SourceDir};
use super::catalog::Catalog;
use super::catalog_load::{self, Owners};
use super::check::{Check, check};
use super::decisions::EntryRef;
use super::derive::{FileTags, Sheets, derive};
use super::evidence::FileEntry;
use super::model::{Anomaly, Model, OrphanReason, PatternState};
use super::plan::{ImportPlanV2, SheetId, SourceId, SourcePlan, TagId};
use super::scan::{self, IndexCache, SourceState, StoredIndex};
use super::tags::{Embedded, TagCache};
use facts::Facts;
use query::FlagsBy;

/// How many files the `tags` section reads tags from.
pub const TAG_SAMPLES: usize = 5;

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
    Shapes,
    Histogram,
    Identical,
    Unpaired,
    Values,
    NameWords,
    ColumnMatches,
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
    /// The page of `shapes` groups.
    #[serde(default)]
    pub shapes: Option<PageQuery>,
    #[serde(default)]
    pub identical: Option<PageQuery>,
    #[serde(default)]
    pub unpaired: Option<UnpairedQuery>,
    #[serde(default)]
    pub values: Option<ValuesQuery>,
    #[serde(default)]
    pub name_words: Option<NameWordsQuery>,
    /// Draws the sample records of `shapes` and `flags`; 0 when left out.
    #[serde(default)]
    pub seed: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrphanView {
    pub decision: usize,
    pub reason: OrphanReason,
    /// The entry the reason is about, not the whole decision: a `many`
    /// decision holds thousands of entries.
    pub entry: EntryRef,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shapes: Option<Shapes>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub histogram: Option<Vec<BucketCount>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identical: Option<IdenticalFiles>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unpaired: Option<Unpaired>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<SlotValues>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_words: Option<NameWords>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_matches: Option<Vec<SheetMatches>>,
}

/// File tags read from disk through the shared cache.
pub struct DiskTags {
    caches: Arc<Caches>,
    dirs: BTreeMap<SourceId, SourceDir>,
}

impl FileTags for DiskTags {
    fn tag(&self, source: &SourceId, entry: &FileEntry, tag: TagId) -> Option<Embedded> {
        self.caches.tags.tag(self.dirs.get(source)?, entry, tag)
    }
}

/// One import derived, with what the sections read besides the model.
pub struct Derived {
    pub plan: Arc<ImportPlanV2>,
    pub model: Model,
    pub catalog: Catalog,
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

/// Derive `plan` against import `import`'s stored index. A sheet in a
/// staging source is read from `store`, under the import's staging area.
///
/// # Errors
/// [`PreviewError::Db`] when the index or the catalog cannot be read.
#[allow(clippy::too_many_arguments)]
pub async fn derive_import<DB>(
    pool: &Pool<DB>,
    roots: &ArchiveRoots,
    store: &Arc<dyn Store>,
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
    let store = Arc::clone(store);
    tokio::task::spawn_blocking(move || {
        let dirs = scan::source_dirs(&roots, &plan.sources);
        let read: BTreeMap<SheetId, Arc<ReadSheet>> = plan
            .sheets
            .iter()
            .filter_map(|sheet| {
                let read = match plan
                    .sources
                    .iter()
                    .find(|source| *source.id() == sheet.source)?
                {
                    SourcePlan::Folder { .. } => {
                        caches.sheets.read(sheet, dirs.get(&sheet.source)?)
                    }
                    SourcePlan::Staging { .. } => caches.sheets.read_stored(
                        sheet,
                        store.as_ref(),
                        &staging_path(import, &sheet.file),
                    ),
                }?;
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
            catalog,
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
    let seed = request.seed.unwrap_or(0);
    let flags = (wants(Section::Entries) || wants(Section::Records) || wants(Section::Shapes))
        .then(|| FlagsBy::new(model));
    let records_query = request.records.clone().unwrap_or_default();
    let facts = (wants(Section::Shapes)
        || (wants(Section::Records) && records_query.shape.is_some()))
    .then(|| Facts::new(&derived.plan, &derived.stored));
    let flags_for = |section| flags.as_ref().filter(|_| wants(section));
    Preview {
        sources: derived.stored.sources.clone(),
        pattern: model.pattern.clone(),
        entries: flags_for(Section::Entries).map(|flags| {
            query::entries(model, &request.entries.clone().unwrap_or_default(), flags)
        }),
        sheets: wants(Section::Sheets).then(|| evidence::sheet_views(derived)),
        tags: wants(Section::Tags).then(|| evidence::tag_views(derived)),
        records: flags_for(Section::Records)
            .map(|flags| query::records(model, &records_query, flags, facts.as_ref())),
        flags: wants(Section::Flags).then(|| causes::flag_counts(model, seed)),
        anomalies: wants(Section::Anomalies).then(|| model.anomalies.clone()),
        orphans: wants(Section::Orphans).then(|| {
            model
                .orphans
                .iter()
                .map(|orphan| OrphanView {
                    decision: orphan.decision,
                    reason: orphan.reason,
                    entry: orphan.entry.clone(),
                })
                .collect()
        }),
        check: wants(Section::Check).then(|| check(&derived.plan, model)),
        shapes: flags_for(Section::Shapes)
            .zip(facts.as_ref())
            .map(|(flags, facts)| {
                let query = request.shapes.clone().unwrap_or_default();
                facts::shapes(model, facts, flags, &query, seed)
            }),
        histogram: wants(Section::Histogram).then(|| facts::histogram(model)),
        identical: wants(Section::Identical).then(|| {
            identical::identical(
                &derived.stored,
                &request.identical.clone().unwrap_or_default(),
            )
        }),
        unpaired: wants(Section::Unpaired)
            .then(|| unpaired::unpaired(model, &request.unpaired.clone().unwrap_or_default())),
        values: wants(Section::Values)
            .then(|| counts::values(model, &request.values.clone().unwrap_or_default())),
        name_words: wants(Section::NameWords).then(|| {
            names::name_words(
                &derived.stored,
                &request.name_words.clone().unwrap_or_default(),
            )
        }),
        column_matches: wants(Section::ColumnMatches).then(|| columns::column_matches(derived)),
    }
}

#[cfg(test)]
mod facts_tests;
#[cfg(test)]
mod names_tests;
#[cfg(test)]
mod tests;
