//! The check itself: hash one stored original, and walk every media with one.

use std::sync::Arc;

use omeka_db::DbPool;
use omeka_db::ids::JobId;
use omeka_db::repos::{JobRepo, MediaFilter, MediaRepo, Paging};
use omeka_db::rows::MediaRow;
use omeka_db::with_db;
use omeka_media::store::{Store, StorePrefix, storage_path};
use sha2::{Digest, Sha256};

use super::record::{CheckRecord, Outcome};
use crate::errors::ApiError;
use crate::read::db_error;

/// How many bytes one read asks the store for. Memory stays at one chunk whatever the file's
/// size, and an object store answers each chunk with one ranged GET.
pub const CHUNK: u64 = 8 * 1024 * 1024;

/// How often the check writes a progress line to the job log.
const PROGRESS_EVERY: u64 = 500;

/// What reading one original found, and for a file that is not `ok`, why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCheck {
    pub outcome: Outcome,
    pub reason: Option<String>,
}

impl FileCheck {
    fn of(outcome: Outcome, reason: impl Into<String>) -> Self {
        Self {
            outcome,
            reason: Some(reason.into()),
        }
    }
}

/// Hash the file at `path` in [`CHUNK`]s and compare it with `expected`, a hex SHA-256.
///
/// `missing` only when the store answers that nothing is there; any other failure, before or
/// during the read, is `unreadable`. `ok` and `differs` both mean every byte was hashed.
#[must_use]
pub fn check_file(store: &dyn Store, path: &str, expected: &str) -> FileCheck {
    match store.size(path) {
        Ok(None) => return FileCheck::of(Outcome::Missing, format!("no file at {path}")),
        Err(error) => return FileCheck::of(Outcome::Unreadable, error.to_string()),
        Ok(Some(_)) => {}
    }
    let mut hasher = Sha256::new();
    let mut offset = 0u64;
    loop {
        let bytes = match store.read_range(path, offset..offset.saturating_add(CHUNK)) {
            Ok(bytes) => bytes,
            Err(error) => return FileCheck::of(Outcome::Unreadable, error.to_string()),
        };
        hasher.update(&bytes);
        let read = bytes.len() as u64;
        offset += read;
        if read < CHUNK {
            break;
        }
    }
    let found = omeka_media::text::hex_encode(&hasher.finalize());
    if found.eq_ignore_ascii_case(expected.trim()) {
        FileCheck {
            outcome: Outcome::Ok,
            reason: None,
        }
    } else {
        FileCheck::of(
            Outcome::Differs,
            format!("SHA-256 is {found}, the record says {expected}"),
        )
    }
}

/// What a whole check did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckRun {
    pub record: CheckRecord,
    /// Media with an original and no recorded SHA-256: nothing to compare, so not checked.
    pub unrecorded: u64,
    /// The job was asked to stop before the last file.
    pub stopped: bool,
}

/// Check every media that has an original, in id order, as job `job`.
///
/// Each problem file gets a log line, every [`PROGRESS_EVERY`] files a progress line, and the end
/// a summary line. The record is written into the job's `args` at the end, and also when a stop
/// request ends the walk early, so a stopped check still states what it read.
///
/// # Errors
/// An [`ApiError`] when the database cannot be read or written. A file that cannot be read is an
/// outcome, not an error.
pub async fn check_all(
    pool: &DbPool,
    store: &Arc<dyn Store>,
    job: JobId,
) -> Result<CheckRun, ApiError> {
    let mut media = with_db!(pool, |db| MediaRepo::new(db)
        .list(&MediaFilter::default(), &Paging::default())
        .await)
    .map_err(db_error)?;
    media.retain(|row| row.has_original);
    media.sort_by_key(MediaRow::id);
    let total = media.len();
    let mut run = CheckRun::default();
    let mut read = 0u64;
    for row in media {
        if stop_asked(pool, job).await? {
            run.stopped = true;
            log(pool, job, "stopped on request.").await?;
            break;
        }
        let id = row.id().0;
        let Some(expected) = row.sha256.clone() else {
            run.unrecorded += 1;
            log(
                pool,
                job,
                &format!("media {id}: no SHA-256 recorded, not checked"),
            )
            .await?;
            continue;
        };
        let checked = check_row(store, &row, expected).await?;
        if let Some(reason) = &checked.reason {
            let line = format!("media {id}: {} ({reason})", checked.outcome.as_str());
            log(pool, job, &line).await?;
        }
        run.record.push(id, checked.outcome);
        read += 1;
        if read.is_multiple_of(PROGRESS_EVERY) {
            log(pool, job, &format!("checked {read} of {total} files")).await?;
        }
    }
    write_record(pool, job, &run.record).await?;
    log(pool, job, &summary_line(&run)).await?;
    Ok(run)
}

async fn check_row(
    store: &Arc<dyn Store>,
    row: &MediaRow,
    expected: String,
) -> Result<FileCheck, ApiError> {
    let Some(storage_id) = &row.storage_id else {
        return Ok(FileCheck::of(
            Outcome::Missing,
            "the record names no stored file",
        ));
    };
    let path = storage_path(StorePrefix::Original, storage_id, row.extension.as_deref());
    let store = Arc::clone(store);
    tokio::task::spawn_blocking(move || check_file(store.as_ref(), &path, &expected))
        .await
        .map_err(|error| ApiError::internal(format!("the file check did not finish: {error}")))
}

fn summary_line(run: &CheckRun) -> String {
    let record = &run.record;
    let checked: u64 = Outcome::ALL.iter().map(|o| record.runs(*o).len()).sum();
    let noun = if checked == 1 { "file" } else { "files" };
    let mut line = format!(
        "Checked {checked} {noun}: {} ok, {} differ, {} missing, {} unreadable.",
        record.ok.len(),
        record.differs.len(),
        record.missing.len(),
        record.unreadable.len()
    );
    if run.unrecorded > 0 {
        line.push_str(&format!(" {} had no SHA-256 to compare.", run.unrecorded));
    }
    line
}

/// Keep the job's other args and put the record beside them.
async fn write_record(pool: &DbPool, job: JobId, record: &CheckRecord) -> Result<(), ApiError> {
    let row = with_db!(pool, |db| JobRepo::new(db).get(job).await).map_err(db_error)?;
    let mut args = match row.and_then(|row| row.args) {
        Some(serde_json::Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };
    let value = serde_json::to_value(record).map_err(ApiError::internal)?;
    args.insert(super::RESULT_ARG.to_string(), value);
    with_db!(pool, |db| JobRepo::new(db)
        .set_args(job, &serde_json::Value::Object(args))
        .await)
    .map_err(db_error)?;
    Ok(())
}

async fn stop_asked(pool: &DbPool, job: JobId) -> Result<bool, ApiError> {
    let status = with_db!(pool, |db| JobRepo::new(db).status(job).await).map_err(db_error)?;
    Ok(status.as_deref() == Some("stopping"))
}

async fn log(pool: &DbPool, job: JobId, line: &str) -> Result<(), ApiError> {
    with_db!(pool, |db| JobRepo::new(db).append_log(job, line).await).map_err(db_error)?;
    Ok(())
}
