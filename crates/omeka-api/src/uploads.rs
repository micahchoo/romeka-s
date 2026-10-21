//! A user's staged uploads: the files an editor sends from the item page
//! before a media row names them (`briefs/media-ui`, G1).
//!
//! The rows are `romeka_user_upload` (migration 0009). An upload belongs to
//! the person who started it: every lookup names both the user and the upload
//! id, so another account's id answers [`UploadError::Missing`], which the
//! route renders as 404. The bytes sit in the store under `uploads/<userId>/`
//! in the same `files/` and `parts/` layout an import's staging area uses, so
//! the mechanics in [`crate::import::staging`] serve both.
//!
//! A `PATCH` writes its chunk as a part of its own and advances the row only
//! once the part is whole; the last chunk joins the parts into the file. A
//! request that breaks off leaves the stored offset where it was, and the next
//! one starts there. Every byte received moves `expires` 24 hours forward, and
//! [`sweep`] removes a row once the time has passed.
//!
//! Every function that takes an upload id expects the caller to hold that
//! upload's lock (the server keeps one per upload), so two requests for one
//! upload never interleave.

use std::io::Read;
use std::sync::Arc;

use chrono::{NaiveDateTime, TimeDelta};
use omeka_db::Backend;
use omeka_db::DbError;
use omeka_db::ids::{ItemId, UserId};
use omeka_db::repos::{UploadAdvance, UserUploadRepo};
use omeka_db::rows::{NewUserUpload, UserUploadRow};
use omeka_media::store::{Store, new_storage_id};
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Type};

use crate::import::plan::RelPathError;
use crate::import::staging::{self, parts};

/// How long an upload stays after its last byte.
pub const KEEP: TimeDelta = TimeDelta::hours(24);

/// The most bytes one `PATCH` may carry, as an import's upload takes.
pub const MAX_CHUNK: u64 = 64 * 1024 * 1024;

/// How long one create may hold an upload. A create that dies between its claim
/// and its release leaves the upload reclaimable after this bound.
pub const CLAIM_STALE: TimeDelta = TimeDelta::hours(1);

/// The folder of a user's staged uploads in the store.
#[must_use]
pub fn folder(user: UserId) -> String {
    format!("uploads/{}", user.0)
}

/// Where a complete upload's bytes are.
#[must_use]
pub fn file_path(user: UserId, upload: &str) -> String {
    parts::file_path(&folder(user), upload)
}

/// One media create's hold on an upload. Only the create that the claim's
/// single UPDATE matched may ingest the bytes; a second create is refused until
/// the first releases, or until the claim is older than [`CLAIM_STALE`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    user_id: UserId,
    upload: String,
    /// The instant written into `claimed`; the release matches it.
    at: NaiveDateTime,
}

/// Why a create could not take the upload.
#[derive(Debug, thiserror::Error)]
pub enum ClaimError {
    /// The caller has no upload with that id.
    #[error("no such upload")]
    Missing,
    /// Another create holds it and its claim is fresh.
    #[error("the upload is being added")]
    Busy,
    #[error(transparent)]
    Db(#[from] DbError),
}

/// Why an upload request was refused.
#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    #[error("the file name {path:?} cannot be used: {reason}")]
    Path { path: String, reason: RelPathError },
    #[error("the file is larger than {limit} bytes")]
    FileTooLarge { limit: u64 },
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

impl From<staging::StagingError> for UploadError {
    fn from(error: staging::StagingError) -> Self {
        match error {
            staging::StagingError::Path { path, reason } => Self::Path { path, reason },
            staging::StagingError::FileTooLarge { limit } => Self::FileTooLarge { limit },
            staging::StagingError::Store(reason) => Self::Store(reason),
            staging::StagingError::Db(error) => Self::Db(error),
            // `upload_path` and `write_part` are the only import rules this
            // module calls, and neither answers these.
            other => Self::Store(other.to_string()),
        }
    }
}

impl From<omeka_media::store::StoreError> for UploadError {
    fn from(error: omeka_media::store::StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

/// The largest file one upload may declare, as `import_upload_max_file`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limit {
    pub max_file: u64,
}

impl From<crate::settings::UploadSettings> for Limit {
    fn from(settings: crate::settings::UploadSettings) -> Self {
        Self {
            max_file: settings.max_file,
        }
    }
}

/// Start an upload of `length` bytes. `path` is the client's file name,
/// `media_type` the type it declared, and `item` the item it was started for.
///
/// # Errors
/// [`UploadError::Path`] for a name the path rules refuse,
/// [`UploadError::FileTooLarge`] past the limit, and [`UploadError::Db`].
pub async fn create<DB>(
    pool: &Pool<DB>,
    user: UserId,
    item: Option<ItemId>,
    path: &str,
    media_type: Option<String>,
    length: u64,
    limit: Limit,
) -> Result<UserUploadRow, UploadError>
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
    let path = staging::upload_path(path)?;
    if length > limit.max_file {
        return Err(UploadError::FileTooLarge {
            limit: limit.max_file,
        });
    }
    let id = new_storage_id()?;
    let now = chrono::Utc::now().naive_utc();
    Ok(UserUploadRepo::new(pool)
        .create(&NewUserUpload {
            user_id: user,
            id,
            item_id: item,
            path: path.to_string(),
            media_type,
            length,
            expires: now + KEEP,
        })
        .await?)
}

/// What a `PATCH` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Appended {
    /// The bytes stored now: the next `PATCH` starts here.
    pub received: u64,
    /// The upload, when this request brought its last byte.
    pub completed: Option<UserUploadRow>,
}

/// Store the bytes `body` yields as the part of `upload` that starts at
/// `offset`. The part counts only when it was read to its end; the last part
/// joins every part into the file.
///
/// Blocking reads of `body` run on the blocking pool.
///
/// # Errors
/// [`UploadError::Missing`], [`UploadError::OffsetConflict`] when `offset` is
/// not the stored offset (nothing is written then),
/// [`UploadError::PastLength`], [`UploadError::Interrupted`] when the body
/// failed before its end, [`UploadError::Store`], [`UploadError::Db`].
pub async fn append<DB>(
    pool: &Pool<DB>,
    store: &Arc<dyn Store>,
    user: UserId,
    upload: &str,
    offset: u64,
    body: Box<dyn Read + Send>,
) -> Result<Appended, UploadError>
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
    let repo = UserUploadRepo::new(pool);
    let row = repo.get(user, upload).await?.ok_or(UploadError::Missing)?;
    if row.received != offset {
        return Err(UploadError::OffsetConflict {
            received: row.received,
        });
    }
    let root = folder(user);
    let part = parts::part_path(&root, upload, offset);
    let left = row.length - offset;
    let written = {
        let store = Arc::clone(store);
        let part = part.clone();
        blocking(move || parts::write_part(store.as_ref(), body, left, row.length, &part)).await??
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
            let root = root.clone();
            blocking(move || parts::join_parts(store.as_ref(), &root, &upload, row.length)).await?
        };
        if let Err(error) = joined {
            let target = parts::file_path(&root, upload);
            let _ = blocking_store(store, move |store| store.delete(&target)).await;
            return Err(error.into());
        }
    }
    match repo
        .advance(
            user,
            upload,
            offset,
            written,
            chrono::Utc::now().naive_utc() + KEEP,
        )
        .await?
    {
        UploadAdvance::Advanced(_) => {}
        UploadAdvance::Missing => return Err(UploadError::Missing),
        UploadAdvance::Conflict { received } => {
            return Err(UploadError::OffsetConflict { received });
        }
        UploadAdvance::PastLength { length } => return Err(UploadError::PastLength { length }),
    }
    if received < row.length {
        return Ok(Appended {
            received,
            completed: None,
        });
    }
    remove_parts(store, &root, upload).await;
    let complete = repo.get(user, upload).await?.ok_or(UploadError::Missing)?;
    Ok(Appended {
        received,
        completed: Some(complete),
    })
}

/// Take `upload` for one media create. The row's `claimed` column is set by one
/// UPDATE that matches only an unclaimed row, or a claim at or before
/// `now - CLAIM_STALE`, so exactly one of two creates can answer `Ok`.
///
/// # Errors
/// [`ClaimError::Missing`] when the caller has no such upload,
/// [`ClaimError::Busy`] when another create holds a fresh claim, and
/// [`ClaimError::Db`].
pub async fn claim<DB>(
    pool: &Pool<DB>,
    user: UserId,
    upload: &str,
    now: NaiveDateTime,
) -> Result<Claim, ClaimError>
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
    let repo = UserUploadRepo::new(pool);
    let stale_before = now - CLAIM_STALE;
    if repo.claim(user, upload, now, stale_before).await? {
        return Ok(Claim {
            user_id: user,
            upload: upload.to_string(),
            at: now,
        });
    }
    if repo.get(user, upload).await?.is_none() {
        return Err(ClaimError::Missing);
    }
    Err(ClaimError::Busy)
}

/// Give a claimed upload back after a create that could not finish.
///
/// # Errors
/// [`DbError`] when the release cannot be written. The stale bound then frees
/// the upload.
pub async fn release<DB>(pool: &Pool<DB>, claim: &Claim) -> Result<(), DbError>
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
    UserUploadRepo::new(pool)
        .release(claim.user_id, &claim.upload, claim.at)
        .await
        .map(|_| ())
}

/// Remove one upload: its row, its parts and its file. The row goes first, so
/// a reader never finds a complete row without bytes.
///
/// # Errors
/// [`DbError`] when the row cannot be removed.
pub async fn remove<DB>(
    pool: &Pool<DB>,
    store: &Arc<dyn Store>,
    user: UserId,
    upload: &str,
) -> Result<bool, DbError>
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
    let removed = UserUploadRepo::new(pool).delete(user, upload).await?;
    let root = folder(user);
    remove_parts(store, &root, upload).await;
    let file = parts::file_path(&root, upload);
    let _ = blocking_store(store, move |store| store.delete(&file)).await;
    Ok(removed)
}

/// Remove every upload whose 24 hours have passed. The sweep tick calls this
/// with the clock of now; the upload's own file and parts go with its row.
///
/// # Errors
/// [`DbError`] when the rows cannot be read or removed.
pub async fn sweep<DB>(
    pool: &Pool<DB>,
    store: &Arc<dyn Store>,
    now: NaiveDateTime,
) -> Result<usize, DbError>
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
    let mut removed = 0;
    for row in UserUploadRepo::new(pool).expired(now).await? {
        if remove(pool, store, row.user_id, &row.id).await? {
            removed += 1;
        }
    }
    Ok(removed)
}

async fn remove_parts(store: &Arc<dyn Store>, root: &str, upload: &str) {
    let upload = upload.to_string();
    let root = root.to_string();
    let _ = blocking_store(store, move |store| {
        for part in parts::part_files(store, &root, &upload)? {
            store.delete(&part.storage_path)?;
        }
        Ok(true)
    })
    .await;
}

async fn blocking<T: Send + 'static>(
    task: impl FnOnce() -> T + Send + 'static,
) -> Result<T, UploadError> {
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|error| UploadError::Store(error.to_string()))
}

async fn blocking_store<T: Send + 'static>(
    store: &Arc<dyn Store>,
    task: impl FnOnce(&dyn Store) -> Result<T, omeka_media::store::StoreError> + Send + 'static,
) -> Result<T, UploadError> {
    let store = Arc::clone(store);
    blocking(move || task(store.as_ref()))
        .await?
        .map_err(Into::into)
}
