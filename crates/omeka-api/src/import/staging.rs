//! An import's staging area: the files a person uploaded, in the store
//! (`docs/import-v2.md`, "Arrival", "Staging").
//!
//! The rows of `romeka_import_upload` are the staging area's table of
//! contents: each names one file by the path a person gave it. The bytes sit
//! in the store under `staging/<importId>/`, keyed by the upload's id and
//! never by the person's path, so a name never becomes a store path:
//!
//! - `files/<upload>`: a file whose every byte arrived;
//! - `parts/<upload>.<offset>`: the chunks of an upload still in progress,
//!   one per `PATCH`, named by the offset it starts at. They share one flat
//!   folder because the store lists files and not folders; the sweep can then
//!   find the parts of an upload that no row names ([`remove_orphan_parts`]);
//! - `meta/<upload>.json`: what a ZIP entry's name was before it was decoded
//!   or renamed ([`Meta`]), and `meta/zip-<upload>.json`, what unpacking a
//!   ZIP left out ([`Refused`]).
//!
//! A `PATCH` writes its chunk through [`Store::put_stream`] into a part of its
//! own, and the row advances only once the part is whole, so a request that
//! breaks off leaves nothing behind and the next one starts from the stored
//! offset. The last chunk joins the parts into the file before the row says
//! the upload is complete, so a reader never sees a complete upload without
//! its bytes. The store has no append, and a part per request keeps the
//! trait as it is for an object store, where an object cannot grow.
//!
//! Every function that takes an upload id expects the caller to hold that
//! upload's lock (the server keeps one per upload), so two requests for one
//! upload never interleave.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use omeka_db::Backend;
use omeka_db::DbError;
use omeka_db::ids::ImportId;
use omeka_db::repos::{UploadAdvance, UploadRepo};
use omeka_db::rows::{NewUpload, UploadRow};
use omeka_media::store::{ListedFile, Store, StoreError, new_storage_id};
use serde::{Deserialize, Serialize};
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Type};

use super::evidence::identical::identical_with;
use super::evidence::{self, Anomalies, DecodedName, FileEntry, NameEncoding};
use super::plan::{RelPath, RelPathError};

pub(crate) mod parts;
#[cfg(test)]
pub(crate) mod tests;
mod walk;

pub use parts::StoredReader;
pub use walk::walk;

use parts::write_part;

/// How much a [`StoredReader`] asks the store for at once.
const READ_CHUNK: u64 = 1024 * 1024;

/// The folder of an import's staging area in the store.
#[must_use]
pub fn staging_path(import: ImportId) -> String {
    format!("staging/{}", import.0)
}

/// Where a complete upload's bytes are.
#[must_use]
pub fn file_path(import: ImportId, upload: &str) -> String {
    parts::file_path(&staging_path(import), upload)
}

fn parts_folder(import: ImportId) -> String {
    parts::parts_folder(&staging_path(import))
}

/// The parts of one upload, in offset order.
pub(crate) fn part_files(
    store: &dyn Store,
    import: ImportId,
    upload: &str,
) -> Result<Vec<ListedFile>, StoreError> {
    parts::part_files(store, &staging_path(import), upload)
}

fn meta_folder(import: ImportId) -> String {
    format!("{}/meta", staging_path(import))
}

pub(crate) fn meta_path(import: ImportId, upload: &str) -> String {
    format!("{}/{upload}.json", meta_folder(import))
}

pub(crate) fn refused_path(import: ImportId, zip: &str) -> String {
    format!("{}/zip-{zip}.json", meta_folder(import))
}

/// What a file from a ZIP was called inside it, when its name changed on the
/// way into staging.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    /// Set when the name had no UTF-8 flag and was not UTF-8: decoded as
    /// cp437, its bytes kept in hex.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoded: Option<StoredName>,
    /// Set when an earlier entry had this name with other capitals: the name
    /// before the `~n` was added.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renamed_from: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredName {
    pub encoding: NameEncoding,
    pub raw: String,
}

/// What unpacking one ZIP left out. The walk shows it among the anomalies.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Refused {
    pub links: u64,
    /// Names that would land outside the folder, or that the path rules
    /// refuse, or that are packed in a way the reader cannot unpack.
    pub names: Vec<String>,
}

/// The upload limits an import works under (`settings::upload_settings`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_file: u64,
    pub max_total: u64,
}

impl From<crate::settings::UploadSettings> for Limits {
    fn from(settings: crate::settings::UploadSettings) -> Self {
        Self {
            max_file: settings.max_file,
            max_total: settings.max_total,
        }
    }
}

/// Why an upload request was refused.
#[derive(Debug, thiserror::Error)]
pub enum StagingError {
    #[error("the file name {path:?} cannot be used: {reason}")]
    Path { path: String, reason: RelPathError },
    #[error("the file is larger than {limit} bytes")]
    FileTooLarge { limit: u64 },
    #[error("the import would pass {limit} bytes")]
    ImportFull { limit: u64, used: u64 },
    #[error("upload {upload} still holds this path")]
    InProgress { upload: String },
    #[error("no such upload")]
    Missing,
    #[error("the stored offset is {received}")]
    OffsetConflict { received: u64 },
    #[error("the bytes pass the declared length {length}")]
    PastLength { length: u64 },
    #[error("the request body ended early: {0}")]
    Interrupted(String),
    #[error("the store failed: {0}")]
    Store(String),
    #[error(transparent)]
    Db(#[from] DbError),
}

impl From<StoreError> for StagingError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

/// An import's uploads and the bytes they count for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Usage {
    pub uploads: Vec<UploadRow>,
    /// Every upload's declared length, in progress or not.
    pub used: u64,
}

pub async fn usage<DB>(pool: &Pool<DB>, import: ImportId) -> Result<Usage, DbError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let uploads = UploadRepo::new(pool).list(import).await?;
    let used = uploads.iter().map(|upload| upload.length).sum();
    Ok(Usage { uploads, used })
}

/// The path rules of the plan (`RelPath`), and a name: the empty path is the
/// staging area itself, not a file.
///
/// # Errors
/// [`StagingError::Path`].
pub fn upload_path(text: &str) -> Result<RelPath, StagingError> {
    let refused = |reason| StagingError::Path {
        path: text.to_string(),
        reason,
    };
    let path = RelPath::parse(text).map_err(refused)?;
    if path.as_str().is_empty() {
        return Err(refused(RelPathError::EmptySegment));
    }
    Ok(path)
}

/// Start an upload of `length` bytes at `path`.
///
/// # Errors
/// [`StagingError::Path`], [`StagingError::FileTooLarge`],
/// [`StagingError::ImportFull`] (uploads in progress count), and
/// [`StagingError::InProgress`] when an unfinished upload holds the path.
pub async fn create<DB>(
    pool: &Pool<DB>,
    import: ImportId,
    path: &str,
    length: u64,
    limits: Limits,
) -> Result<UploadRow, StagingError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let path = upload_path(path)?;
    if length > limits.max_file {
        return Err(StagingError::FileTooLarge {
            limit: limits.max_file,
        });
    }
    let Usage { uploads, used } = usage(pool, import).await?;
    if used.saturating_add(length) > limits.max_total {
        return Err(StagingError::ImportFull {
            limit: limits.max_total,
            used,
        });
    }
    if let Some(open) = uploads
        .iter()
        .find(|upload| !upload.is_complete() && upload.path == path.as_str())
    {
        return Err(StagingError::InProgress {
            upload: open.id.clone(),
        });
    }
    let id = new_storage_id()?;
    Ok(UploadRepo::new(pool)
        .create(&NewUpload {
            import_id: import,
            id,
            path: path.to_string(),
            length,
        })
        .await?)
}

/// What a `PATCH` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Appended {
    /// The bytes stored now: the next `PATCH` starts here.
    pub received: u64,
    /// The upload, when this request brought its last byte.
    pub completed: Option<UploadRow>,
}

/// Store the bytes `body` yields as the part of `upload` that starts at
/// `offset`. The part counts only when it was read to its end; the last part
/// joins every part into the file, and an earlier complete upload of the
/// same path is then removed, so a file uploaded again replaces the old one.
///
/// Blocking reads of `body` run on the blocking pool.
///
/// # Errors
/// [`StagingError::Missing`], [`StagingError::OffsetConflict`] when `offset`
/// is not the stored offset (nothing is written then),
/// [`StagingError::PastLength`], [`StagingError::Interrupted`] when the body
/// failed before its end, [`StagingError::Store`].
pub async fn append<DB>(
    pool: &Pool<DB>,
    store: &Arc<dyn Store>,
    import: ImportId,
    upload: &str,
    offset: u64,
    body: Box<dyn Read + Send>,
) -> Result<Appended, StagingError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let repo = UploadRepo::new(pool);
    let row = repo
        .get(import, upload)
        .await?
        .ok_or(StagingError::Missing)?;
    if row.received != offset {
        return Err(StagingError::OffsetConflict {
            received: row.received,
        });
    }
    let part = parts::part_path(&staging_path(import), upload, offset);
    let left = row.length - offset;
    let written = {
        let store = Arc::clone(store);
        let part = part.clone();
        blocking(move || write_part(store.as_ref(), body, left, row.length, &part)).await??
    };
    if written == 0 {
        let _ = blocking_store(store, move |store| store.delete(&part)).await;
        return Ok(Appended {
            received: offset,
            completed: None,
        });
    }
    let received = offset + written;
    if received == row.length {
        let joined = {
            let store = Arc::clone(store);
            let upload = upload.to_string();
            let root = staging_path(import);
            blocking(move || parts::join_parts(store.as_ref(), &root, &upload, row.length)).await?
        };
        if let Err(error) = joined {
            let target = file_path(import, upload);
            let _ = blocking_store(store, move |store| store.delete(&target)).await;
            return Err(error);
        }
    }
    match repo.advance(import, upload, offset, written).await? {
        UploadAdvance::Advanced(_) => {}
        UploadAdvance::Missing => return Err(StagingError::Missing),
        UploadAdvance::Conflict { received } => {
            return Err(StagingError::OffsetConflict { received });
        }
        UploadAdvance::PastLength { length } => return Err(StagingError::PastLength { length }),
    }
    if received < row.length {
        return Ok(Appended {
            received,
            completed: None,
        });
    }
    remove_parts(store, import, upload).await;
    let complete = repo
        .get(import, upload)
        .await?
        .ok_or(StagingError::Missing)?;
    for older in repo.list(import).await? {
        if older.id != complete.id && older.is_complete() && older.path == complete.path {
            remove(pool, store, import, &older.id).await?;
        }
    }
    Ok(Appended {
        received,
        completed: Some(complete),
    })
}

/// Remove one upload: its row, its parts, its file and its name record. The
/// row goes first, so a reader never finds a complete row without bytes.
///
/// # Errors
/// [`DbError`] when the row cannot be removed.
pub async fn remove<DB>(
    pool: &Pool<DB>,
    store: &Arc<dyn Store>,
    import: ImportId,
    upload: &str,
) -> Result<bool, DbError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let removed = UploadRepo::new(pool).delete(import, upload).await?;
    remove_parts(store, import, upload).await;
    let file = file_path(import, upload);
    let meta = meta_path(import, upload);
    let _ = blocking_store(store, move |store| {
        store.delete(&file)?;
        store.delete(&meta)
    })
    .await;
    Ok(removed)
}

/// Remove an import's whole staging area: every upload and every file the
/// store lists under it, including files no row names any more.
///
/// # Errors
/// [`DbError`] when the rows cannot be read or removed.
pub async fn remove_all<DB>(
    pool: &Pool<DB>,
    store: &Arc<dyn Store>,
    import: ImportId,
) -> Result<usize, DbError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let uploads = UploadRepo::new(pool).list(import).await?;
    for upload in &uploads {
        remove(pool, store, import, &upload.id).await?;
    }
    let folders = [
        format!("{}/files", staging_path(import)),
        parts_folder(import),
        meta_folder(import),
    ];
    let _ = blocking_store(store, move |store| {
        for folder in &folders {
            for file in store.list(folder)? {
                store.delete(&file.storage_path)?;
            }
        }
        Ok(true)
    })
    .await;
    Ok(uploads.len())
}

async fn remove_parts(store: &Arc<dyn Store>, import: ImportId, upload: &str) {
    let upload = upload.to_string();
    let _ = blocking_store(store, move |store| {
        for part in part_files(store, import, &upload)? {
            store.delete(&part.storage_path)?;
        }
        Ok(true)
    })
    .await;
}

/// Remove the parts of `import` whose upload no row names and that were
/// last written before `before`: what a crash between a row's removal and its
/// parts' removal leaves. The age keeps a part that a request is writing
/// for a row created a moment ago. Answers how many uploads' parts went.
///
/// # Errors
/// [`DbError`] when the rows cannot be read.
pub async fn remove_orphan_parts<DB>(
    pool: &Pool<DB>,
    store: &Arc<dyn Store>,
    import: ImportId,
    before: SystemTime,
) -> Result<usize, DbError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let named: BTreeSet<String> = UploadRepo::new(pool)
        .list(import)
        .await?
        .into_iter()
        .map(|upload| upload.id)
        .collect();
    let swept = blocking_store(store, move |store| {
        let folder = parts_folder(import);
        let mut gone = BTreeSet::new();
        for part in store.list(&folder)? {
            let name = part.storage_path.rsplit('/').next().unwrap_or_default();
            let upload = name.split('.').next().unwrap_or_default();
            if !named.contains(upload) && part.modified < before {
                store.delete(&part.storage_path)?;
                gone.insert(upload.to_string());
            }
        }
        Ok(gone.len())
    })
    .await;
    Ok(swept.unwrap_or(0))
}

/// Each complete upload's store path, by its path in staging: where the run
/// reads a staged file from.
///
/// # Errors
/// [`DbError`] when the rows cannot be read.
pub async fn stored_paths<DB>(
    pool: &Pool<DB>,
    import: ImportId,
) -> Result<BTreeMap<String, String>, DbError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    Ok(UploadRepo::new(pool)
        .list(import)
        .await?
        .into_iter()
        .filter(UploadRow::is_complete)
        .map(|upload| (upload.path, file_path(import, &upload.id)))
        .collect())
}

/// A stored time as the walk reports it.
pub(crate) fn system_time(time: chrono::NaiveDateTime) -> SystemTime {
    let seconds = u64::try_from(time.and_utc().timestamp()).unwrap_or(0);
    UNIX_EPOCH + Duration::from_secs(seconds)
}

async fn blocking<T: Send + 'static>(
    task: impl FnOnce() -> T + Send + 'static,
) -> Result<T, StagingError> {
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|error| StagingError::Store(error.to_string()))
}

async fn blocking_store<T: Send + 'static>(
    store: &Arc<dyn Store>,
    task: impl FnOnce(&dyn Store) -> Result<T, StoreError> + Send + 'static,
) -> Result<T, StagingError> {
    let store = Arc::clone(store);
    blocking(move || task(store.as_ref()))
        .await?
        .map_err(Into::into)
}

/// The entry a staged file is in the walk, decoded name included.
pub(crate) fn entry(
    path: RelPath,
    size: u64,
    created: chrono::NaiveDateTime,
    head: &[u8],
    meta: Option<&Meta>,
) -> Option<(FileEntry, bool)> {
    let decoded = match meta.and_then(|meta| meta.decoded.as_ref()) {
        Some(name) => Some(DecodedName {
            encoding: name.encoding,
            raw: unhex(&name.raw)?,
        }),
        None => None,
    };
    let (sniffed, is_sheet) = evidence::sniff(&path, head);
    Some((
        FileEntry {
            path,
            size,
            modified: Some(system_time(created)),
            sniffed,
            decoded,
        },
        is_sheet,
    ))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
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

/// The identical sets among staged files, read through the store.
pub(crate) fn identical(
    store: &dyn Store,
    stored: &BTreeMap<String, String>,
    entries: &[FileEntry],
) -> Vec<evidence::Identical> {
    identical_with(entries, |entry, limit| {
        let path = stored
            .get(entry.path.as_str())
            .ok_or_else(|| io::Error::other("not staged"))?;
        Ok(Box::new(StoredReader::new(
            store,
            path.clone(),
            0..limit.unwrap_or(u64::MAX),
        )))
    })
}

/// The anomalies one ZIP's [`Refused`] record adds.
pub(crate) fn add_refused(anomalies: &mut Anomalies, refused: Refused) {
    anomalies.symlinks += refused.links;
    anomalies.unusable_names.extend(refused.names);
}
