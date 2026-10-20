//! The evidence index: every source of a plan walked, stored through
//! [`EntryRepo`], and read back as the derive's [`Index`]
//! (`docs/import-v2.md`, "Evidence").
//!
//! One `romeka_import_entry` row holds one file or one sheet. Each scanned
//! source also gets one `romeka_import_scan` row: what the walk skipped (the
//! anomalies), the error when the source could not be read, and the scan's
//! token. [`EntryRepo::replace`] swaps both tables in one transaction: a
//! reader never sees the files of one scan with the anomalies of another.
//!
//! [`IndexCache`] keeps the decoded index of recent imports in memory, keyed
//! by the scan tokens, so a preview on a warm index decodes no rows.

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, NaiveDateTime, Utc};
use omeka_db::Backend;
use omeka_db::DbError;
use omeka_db::ids::ImportId;
use omeka_db::repos::{EntryRepo, ImportRepo};
use omeka_db::rows::{EntryRow, NewEntry, NewScan, ScanRow};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Type};

use super::archive::ArchiveRoots;
use super::derive::{Index, IndexedSource};
use super::evidence::{self, Anomalies, DecodedName, FileEntry, Identical, NameEncoding, Walk};
use super::plan::{ImportPlanV2, PlanError, RelPath, SourceId, SourcePlan};

/// A rescan of an index larger than this runs as a job, not in the request.
pub const SCAN_IN_REQUEST: u64 = 20_000;

/// The job class of a rescan, `job.class`; its args are `{"importId": n}`.
pub const SCAN_JOB: &str = "Romeka\\Job\\ImportScan";

/// One source as the last scan left it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceState {
    pub source: SourceId,
    /// When the source was last scanned, `None` before its first scan.
    pub scanned: Option<String>,
    pub files: usize,
    /// The sheets the walk found, by path, for the admin to read.
    pub sheet_files: Vec<String>,
    /// Why the source could not be read.
    pub error: Option<String>,
}

/// The stored index of one import.
#[derive(Debug, Clone)]
pub struct StoredIndex {
    pub index: Arc<Index>,
    /// In plan order, every plan source, scanned or not.
    pub sources: Vec<SourceState>,
}

impl StoredIndex {
    /// The number of files and sheets the index holds.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.sources
            .iter()
            .map(|source| (source.files + source.sheet_files.len()) as u64)
            .sum()
    }
}

/// The `anomalies` member of a scan row: what the walk skipped. A member a
/// row stored before it existed reads as empty, so no migration rewrites
/// old rows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct StoredAnomalies {
    symlinks: u64,
    dot: u64,
    special: u64,
    unreadable: Vec<String>,
    unusable_names: Vec<String>,
    collisions: Vec<String>,
    identical: Vec<Identical>,
}

/// The `flags` member of a stored row.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Stored {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    sheet: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    decoded: Option<StoredName>,
}

/// A name that is not UTF-8 on disk: its encoding, and its bytes in hex, so
/// a later read finds the file by the bytes the disk holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredName {
    encoding: NameEncoding,
    raw: String,
}

/// One source walked: its walk, or why it could not be.
#[derive(Debug)]
pub struct Walked {
    pub source: SourceId,
    pub walk: Result<Walk, String>,
}

/// Walk every source of a plan. Blocking: it reads the disk.
///
/// A staging source has no files until P6-upload stores them; it walks as
/// empty.
#[must_use]
pub fn walk_sources(roots: &ArchiveRoots, sources: &[SourcePlan]) -> Vec<Walked> {
    sources
        .iter()
        .map(|source| Walked {
            source: source.id().clone(),
            walk: match source {
                SourcePlan::Folder { root, path, .. } => roots
                    .source(root, path)
                    .map_err(|error| error.to_string())
                    .and_then(|dir| evidence::walk(&dir).map_err(|error| error.to_string())),
                SourcePlan::Staging { .. } => Ok(Walk::default()),
            },
        })
        .collect()
}

/// A token no other scan in this process, or a scan in another process at
/// another time, gives.
fn new_token() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "{nanos:x}-{:x}-{:x}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// The rows a scan stores: one scan record per source, and the files and sheets of the sources
/// that could be read.
#[must_use]
pub fn rows(walked: &[Walked], now: NaiveDateTime) -> (Vec<NewScan>, Vec<NewEntry>) {
    let token = new_token();
    let mut scans = Vec::new();
    let mut rows = Vec::new();
    for source in walked {
        let (error, anomalies) = match &source.walk {
            Ok(walk) => (None, stored_anomalies(&walk.anomalies)),
            Err(error) => (Some(error.clone()), StoredAnomalies::default()),
        };
        scans.push(NewScan {
            source: source.source.0.clone(),
            anomalies: serde_json::to_value(&anomalies).unwrap_or(Value::Null),
            error,
            token: token.clone(),
            scanned: now,
        });
        let Ok(walk) = &source.walk else { continue };
        let files = walk.entries.iter().map(|entry| (entry, false));
        let sheets = walk.sheets.iter().map(|entry| (entry, true));
        for (entry, sheet) in files.chain(sheets) {
            rows.push(NewEntry {
                source: source.source.0.clone(),
                path: entry.path.to_string(),
                size: entry.size,
                modified: entry.modified.map_or(epoch(), to_naive),
                media_type: Some(entry.sniffed.clone()),
                flags: to_flags(&Stored {
                    sheet,
                    decoded: entry.decoded.as_ref().map(|decoded| StoredName {
                        encoding: decoded.encoding,
                        raw: hex(&decoded.raw),
                    }),
                }),
            });
        }
    }
    (scans, rows)
}

fn stored_anomalies(anomalies: &Anomalies) -> StoredAnomalies {
    StoredAnomalies {
        symlinks: anomalies.symlinks,
        dot: anomalies.dot,
        special: anomalies.special,
        unreadable: anomalies.unreadable.clone(),
        unusable_names: anomalies.unusable_names.clone(),
        collisions: anomalies.collisions.clone(),
        identical: anomalies.identical.clone(),
    }
}

fn to_flags(stored: &Stored) -> Value {
    serde_json::to_value(stored).unwrap_or(Value::Null)
}

/// The index the rows hold, with each plan source's state, in plan order. A
/// row that does not decode is a fault in this module, so it fails loudly.
///
/// # Errors
/// [`DbError::Column`] for a row whose flags or path do not decode.
pub fn index_of(
    scans: Vec<ScanRow>,
    rows: Vec<EntryRow>,
    sources: &[SourcePlan],
) -> Result<StoredIndex, DbError> {
    let mut by_source: BTreeMap<String, Walk> = BTreeMap::new();
    for row in rows {
        let stored: Stored = serde_json::from_value(row.flags).map_err(|error| bad(&error))?;
        let walk = by_source.entry(row.source).or_default();
        let entry = FileEntry {
            path: RelPath::parse(&row.path).map_err(|error| bad(&error))?,
            size: row.size,
            modified: (row.modified != epoch()).then(|| from_naive(row.modified)),
            sniffed: row.media_type.unwrap_or_default(),
            decoded: stored
                .decoded
                .map(|name| {
                    Ok::<_, DbError>(DecodedName {
                        encoding: name.encoding,
                        raw: unhex(&name.raw).ok_or_else(|| bad(&"raw name"))?,
                    })
                })
                .transpose()?,
        };
        if stored.sheet {
            walk.sheets.push(entry);
        } else {
            walk.entries.push(entry);
        }
    }
    let mut scans: BTreeMap<String, ScanRow> = scans
        .into_iter()
        .map(|scan| (scan.source.clone(), scan))
        .collect();
    let mut index = Index::default();
    let mut states = Vec::new();
    for plan in sources {
        let id = plan.id();
        let Some(scan) = scans.remove(&id.0) else {
            states.push(SourceState {
                source: id.clone(),
                scanned: None,
                files: 0,
                sheet_files: Vec::new(),
                error: None,
            });
            continue;
        };
        let mut walk = by_source.remove(&id.0).unwrap_or_default();
        walk.entries.sort_by(|a, b| a.path.cmp(&b.path));
        walk.sheets.sort_by(|a, b| a.path.cmp(&b.path));
        let found: StoredAnomalies =
            serde_json::from_value(scan.anomalies).map_err(|error| bad(&error))?;
        walk.anomalies = Anomalies {
            symlinks: found.symlinks,
            dot: found.dot,
            special: found.special,
            unreadable: found.unreadable,
            unusable_names: found.unusable_names,
            collisions: found.collisions,
            identical: found.identical,
        };
        states.push(SourceState {
            source: id.clone(),
            scanned: Some(scan.scanned.format("%Y-%m-%dT%H:%M:%S+00:00").to_string()),
            files: walk.entries.len(),
            sheet_files: walk.sheets.iter().map(|s| s.path.to_string()).collect(),
            error: scan.error,
        });
        index.sources.push(IndexedSource {
            id: id.clone(),
            walk,
        });
    }
    Ok(StoredIndex {
        index: Arc::new(index),
        sources: states,
    })
}

fn bad(error: &dyn std::fmt::Display) -> DbError {
    DbError::Column {
        column: "flags",
        reason: format!("an evidence index row does not decode: {error}"),
    }
}

/// What a scan stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scanned {
    pub sources: Vec<SourceState>,
}

/// Why a scan failed.
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error("the scan stopped: {0}")]
    Panicked(String),
    #[error("no import has the id {0}")]
    Missing(u64),
    #[error(transparent)]
    Plan(#[from] PlanError),
}

/// The evidence index in memory: the decoded index of the imports previewed
/// last, each kept while its scan tokens stay the same. A preview of a large
/// import then costs the derive, not a decode of every row.
#[derive(Default)]
pub struct IndexCache {
    held: Mutex<HashMap<ImportId, (String, StoredIndex)>>,
}

/// How many imports the cache holds: a few admins editing a few drafts.
const CACHED_IMPORTS: usize = 8;

impl IndexCache {
    fn get(&self, import: ImportId, tokens: &str) -> Option<StoredIndex> {
        let held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        held.get(&import)
            .filter(|(held_tokens, _)| held_tokens == tokens)
            .map(|(_, index)| index.clone())
    }

    fn put(&self, import: ImportId, tokens: String, index: StoredIndex) {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        if held.len() >= CACHED_IMPORTS && !held.contains_key(&import) {
            held.clear();
        }
        held.insert(import, (tokens, index));
    }
}

/// Walk the sources and replace the import's index. The walk runs on the
/// blocking pool.
///
/// # Errors
/// [`ScanError::Db`] when the index cannot be written.
pub async fn scan<DB>(
    pool: &Pool<DB>,
    roots: &ArchiveRoots,
    import: ImportId,
    sources: &[SourcePlan],
) -> Result<Scanned, ScanError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let walk_roots = roots.clone();
    let walk_sources_of = sources.to_vec();
    let now = Utc::now().naive_utc();
    let (scans, rows) = tokio::task::spawn_blocking(move || {
        rows(&walk_sources(&walk_roots, &walk_sources_of), now)
    })
    .await
    .map_err(|error| ScanError::Panicked(error.to_string()))?;
    let repo = EntryRepo::new(pool);
    repo.replace(import, &scans, &rows).await?;
    let stored = index_of(repo.scans(import).await?, repo.all(import).await?, sources)?;
    Ok(Scanned {
        sources: stored.sources,
    })
}

/// Rescan import `import` from its plan (the frozen one once it ran): the
/// body of the [`SCAN_JOB`] job.
///
/// # Errors
/// [`ScanError::Missing`], [`ScanError::Plan`] for a stored plan that does
/// not parse, and [`ScanError::Db`].
pub async fn scan_import<DB>(
    pool: &Pool<DB>,
    roots: &ArchiveRoots,
    import: ImportId,
) -> Result<Scanned, ScanError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let row = ImportRepo::new(pool)
        .get(import)
        .await?
        .ok_or(ScanError::Missing(import.0))?;
    let stored = row.ran_plan.as_ref().unwrap_or(&row.plan);
    let plan = ImportPlanV2::from_json(&stored.to_string())?;
    scan(pool, roots, import, &plan.sources).await
}

/// The import's stored index, from the cache when no source was scanned
/// since it was decoded.
///
/// # Errors
/// [`DbError`] when the rows cannot be read or decoded.
pub async fn load<DB>(
    pool: &Pool<DB>,
    cache: &IndexCache,
    import: ImportId,
    sources: &[SourcePlan],
) -> Result<StoredIndex, DbError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let repo = EntryRepo::new(pool);
    let scans = repo.scans(import).await?;
    let mut tokens = String::new();
    for source in sources {
        let token = scans
            .iter()
            .find(|scan| scan.source == source.id().0)
            .map_or("", |scan| scan.token.as_str());
        tokens.push_str(&source.id().0);
        tokens.push('=');
        tokens.push_str(token);
        tokens.push(';');
    }
    if let Some(found) = cache.get(import, &tokens) {
        return Ok(found);
    }
    let stored = index_of(scans, repo.all(import).await?, sources)?;
    cache.put(import, tokens, stored.clone());
    Ok(stored)
}

fn epoch() -> NaiveDateTime {
    DateTime::<Utc>::UNIX_EPOCH.naive_utc()
}

/// Whole seconds, as the index stores them.
fn to_naive(time: SystemTime) -> NaiveDateTime {
    let seconds = time.duration_since(UNIX_EPOCH).map_or(0, |since| {
        i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
    });
    DateTime::<Utc>::from_timestamp(seconds, 0).map_or_else(epoch, |time| time.naive_utc())
}

fn from_naive(time: NaiveDateTime) -> SystemTime {
    let seconds = u64::try_from(time.and_utc().timestamp()).unwrap_or(0);
    UNIX_EPOCH + Duration::from_secs(seconds)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(text.get(at..at + 2)?, 16).ok())
        .collect()
}

/// Where a source's files are on disk, for the readers that open them.
#[must_use]
pub fn source_dirs(
    roots: &ArchiveRoots,
    sources: &[SourcePlan],
) -> BTreeMap<SourceId, super::archive::SourceDir> {
    sources
        .iter()
        .filter_map(|source| match source {
            SourcePlan::Folder { id, root, path } => {
                roots.source(root, path).ok().map(|dir| (id.clone(), dir))
            }
            SourcePlan::Staging { .. } => None,
        })
        .collect()
}
