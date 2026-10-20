//! A ZIP among an import's uploads, unpacked into its staging area
//! (`docs/import-v2.md`, "Arrival", "ZIP").
//!
//! The job reads the archive through the store's ranges and writes each
//! entry as an upload of its own, in the folder the ZIP was uploaded to. On
//! the way it leaves out, and names, the entries it must not write: a path
//! that would leave the folder or is absolute, a symbolic link, a name the
//! path rules refuse, an entry it cannot unpack. A nested ZIP stays a file.
//! It refuses the whole upload, and keeps nothing from it, when an entry
//! unpacks to more than [`BOMB_RATIO`] times its packed size and more than
//! [`BOMB_FLOOR`] bytes, when the files would pass the import's limit, when
//! one file would pass the file limit, or when the archive holds more than
//! [`MAX_ENTRIES`] entries.
//!
//! Names: `\` becomes `/`. A name with the UTF-8 flag, or the Info-ZIP
//! Unicode Path field, is UTF-8. A name with neither is cp437, unless its
//! bytes are valid UTF-8: macOS writes UTF-8 names without the flag, and
//! cp437 bytes above 0x7F are almost never valid UTF-8, so reading those as
//! cp437 would garble every accented name such an archive holds. A decoded
//! name keeps its bytes, so the entry flags `name-encoding`. An entry whose
//! name equals an earlier one when capitals are ignored is renamed with
//! `~n` before its extension.
//!
//! Once the entries are in, the ZIP itself is removed. A file of the same
//! path uploaded earlier is replaced, as a second upload of it would be.

#[cfg(test)]
mod tests;
mod zip;

use std::collections::BTreeSet;
use std::io::{self, Read};
use std::sync::Arc;

use omeka_db::Backend;
use omeka_db::DbError;
use omeka_db::ids::ImportId;
use omeka_db::repos::{ImportRepo, UploadRepo};
use omeka_db::rows::{ImportStatus, NewUpload, UploadRow};
use omeka_media::store::{Store, StoreError, new_storage_id};
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Type};

use super::evidence::NameEncoding;
use super::evidence::names::decode_cp437;
use super::plan::RelPath;
use super::staging::{
    self, Limits, Meta, Refused, StoredName, StoredReader, file_path, hex, meta_path, refused_path,
};
use crate::messages;

pub use zip::ZipError;

/// The job class of an unpack, `job.class`; its args are
/// `{"importId": n, "upload": id}`.
pub const UNZIP_JOB: &str = "Romeka\\Job\\ImportUnzip";

/// The most entries one archive may hold.
pub const MAX_ENTRIES: u64 = 100_000;

/// An entry that unpacks to more than this many times its packed size ...
pub const BOMB_RATIO: u64 = 100;

/// ... and to more than this many bytes refuses the archive.
pub const BOMB_FLOOR: u64 = 10 * 1024 * 1024;

/// Whether an upload is a ZIP the job unpacks.
#[must_use]
pub fn is_zip(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("zip"))
}

/// An entry left out, or renamed, while the rest went in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    Escapes(String),
    Link(String),
    Unusable { name: String, reason: String },
    Unsupported(String),
    Renamed { name: String, renamed: String },
}

impl Note {
    /// The note as the job log shows it.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Escapes(name) => messages::import_zip_escapes(name),
            Self::Link(name) => messages::import_zip_link(name),
            Self::Unusable { name, reason } => messages::import_zip_unusable(name, reason),
            Self::Unsupported(name) => messages::import_zip_unsupported(name),
            Self::Renamed { name, renamed } => messages::import_zip_renamed(name, renamed),
        }
    }
}

/// Why the whole archive was refused. Nothing from it stays.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("{name} unpacks to more than {BOMB_RATIO} times its packed size")]
    Bomb { name: String },
    #[error("the files would pass {limit} bytes")]
    Full { limit: u64 },
    #[error("the archive holds {count} entries")]
    TooMany { count: u64 },
    #[error("{name} is larger than {limit} bytes")]
    FileTooLarge { name: String, limit: u64 },
    #[error("the archive cannot be read: {0}")]
    Unreadable(String),
}

impl Refusal {
    /// The refusal as the job log shows it.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Bomb { name } => messages::import_zip_bomb(name),
            Self::Full { limit } => messages::import_zip_full(limit),
            Self::TooMany { count } => messages::import_zip_too_many(count),
            Self::FileTooLarge { name, limit } => messages::import_zip_file_too_large(name, limit),
            Self::Unreadable(reason) => messages::import_zip_unreadable(reason),
        }
    }
}

/// Why an unpack did not run or did not finish.
#[derive(Debug, thiserror::Error)]
pub enum UnzipError {
    #[error("{}", .0.message())]
    Refused(Refusal),
    #[error("the upload {0} is not a finished upload of this import")]
    Missing(String),
    #[error("the import is no longer a draft")]
    NotDraft,
    #[error(transparent)]
    Db(#[from] DbError),
    #[error("the store failed: {0}")]
    Store(String),
}

/// What an unpack wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unpacked {
    pub files: usize,
    pub notes: Vec<Note>,
}

/// Unpack upload `upload` of `import`, a finished `.zip`, into the staging
/// area, then remove it. A refused archive is removed too.
///
/// # Errors
/// [`UnzipError::Refused`] with the reason, [`UnzipError::Missing`],
/// [`UnzipError::NotDraft`], [`UnzipError::Db`].
pub async fn unzip_upload<DB>(
    pool: &Pool<DB>,
    store: &Arc<dyn Store>,
    import: ImportId,
    upload: &str,
    limits: Limits,
) -> Result<Unpacked, UnzipError>
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
    let status = ImportRepo::new(pool)
        .get(import)
        .await?
        .map(|row| row.status);
    if status != Some(ImportStatus::Draft) {
        return Err(UnzipError::NotDraft);
    }
    let repo = UploadRepo::new(pool);
    let others: Vec<UploadRow> = repo
        .list(import)
        .await?
        .into_iter()
        .filter(|row| row.id != upload)
        .collect();
    let zip = repo
        .get(import, upload)
        .await?
        .filter(UploadRow::is_complete)
        .ok_or_else(|| UnzipError::Missing(upload.to_string()))?;
    let used: u64 = others.iter().map(|row| row.length).sum();
    let budget = limits.max_total.saturating_sub(used);
    let unpacking = {
        let store = Arc::clone(store);
        let zip = zip.clone();
        tokio::task::spawn_blocking(move || unpack(store.as_ref(), import, &zip, budget, limits))
            .await
            .map_err(|error| UnzipError::Store(error.to_string()))?
    };
    let unpacked = match unpacking {
        Ok(unpacked) => unpacked,
        Err(refusal) => {
            staging::remove(pool, store, import, upload).await?;
            return Err(UnzipError::Refused(refusal));
        }
    };
    if let Err(error) = repo.insert_complete(&unpacked.rows).await {
        let written = unpacked.written.clone();
        let store = Arc::clone(store);
        let _ = tokio::task::spawn_blocking(move || discard(store.as_ref(), &written)).await;
        return Err(error.into());
    }
    let new_paths: BTreeSet<&str> = unpacked.rows.iter().map(|row| row.path.as_str()).collect();
    for older in &others {
        if older.is_complete() && new_paths.contains(older.path.as_str()) {
            staging::remove(pool, store, import, &older.id).await?;
        }
    }
    if unpacked.refused != Refused::default() {
        let record = serde_json::to_vec(&unpacked.refused)
            .map_err(|error| UnzipError::Store(error.to_string()))?;
        let path = refused_path(import, upload);
        let store = Arc::clone(store);
        tokio::task::spawn_blocking(move || store.put(&record, &path))
            .await
            .map_err(|error| UnzipError::Store(error.to_string()))?
            .map_err(|error| UnzipError::Store(error.to_string()))?;
    }
    staging::remove(pool, store, import, upload).await?;
    Ok(Unpacked {
        files: unpacked.rows.len(),
        notes: unpacked.notes,
    })
}

/// What [`unpack`] wrote, for the caller to record.
#[derive(Debug)]
struct Written {
    rows: Vec<NewUpload>,
    notes: Vec<Note>,
    refused: Refused,
    /// Every store path written, so a failure can take them back.
    written: Vec<String>,
}

/// Read the archive and write its entries. Blocking. On a refusal every file
/// written so far is removed again.
fn unpack(
    store: &dyn Store,
    import: ImportId,
    zip: &UploadRow,
    budget: u64,
    limits: Limits,
) -> Result<Written, Refusal> {
    let mut written = Written {
        rows: Vec::new(),
        notes: Vec::new(),
        refused: Refused::default(),
        written: Vec::new(),
    };
    match unpack_into(store, import, zip, budget, limits, &mut written) {
        Ok(()) => Ok(written),
        Err(refusal) => {
            discard(store, &written.written);
            Err(refusal)
        }
    }
}

fn discard(store: &dyn Store, written: &[String]) {
    for path in written {
        let _ = store.delete(path);
    }
}

fn unpack_into(
    store: &dyn Store,
    import: ImportId,
    zip: &UploadRow,
    budget: u64,
    limits: Limits,
    out: &mut Written,
) -> Result<(), Refusal> {
    let archive = file_path(import, &zip.id);
    let read =
        |range: std::ops::Range<u64>| store.read_range(&archive, range).map_err(io::Error::other);
    let unreadable = |error: ZipError| Refusal::Unreadable(error.to_string());
    let directory = zip::directory(&read, zip.length).map_err(unreadable)?;
    if directory.entries > MAX_ENTRIES {
        return Err(Refusal::TooMany {
            count: directory.entries,
        });
    }
    let entries = zip::entries(&read, directory).map_err(unreadable)?;
    let folder = zip.path.rsplit_once('/').map(|(folder, _)| folder);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut total: u64 = 0;
    for entry in entries {
        let (name, decoded) = entry_name(&entry);
        if entry.is_link {
            out.refused.links += 1;
            out.notes.push(Note::Link(name));
            continue;
        }
        if entry.is_folder() {
            continue;
        }
        let path = match normalised(&name, folder) {
            Ok(path) => path,
            Err(note) => {
                out.refused.names.push(name);
                out.notes.push(note);
                continue;
            }
        };
        if entry.encrypted || !matches!(entry.method, 0 | 8) {
            out.refused.names.push(name.clone());
            out.notes.push(Note::Unsupported(name));
            continue;
        }
        if entry.size > limits.max_file {
            return Err(Refusal::FileTooLarge {
                name,
                limit: limits.max_file,
            });
        }
        if is_bomb(entry.size, entry.compressed) {
            return Err(Refusal::Bomb { name });
        }
        let (path, renamed_from) = unique(path, &mut seen);
        if let Some(from) = &renamed_from {
            out.notes.push(Note::Renamed {
                name: from.clone(),
                renamed: path.to_string(),
            });
        }
        let start = zip::data_start(&read, &entry).map_err(unreadable)?;
        let packed = StoredReader::new(store, archive.clone(), start..start + entry.compressed);
        let unpacked = zip::unpacked(&entry, Box::new(packed))
            .ok_or_else(|| Refusal::Unreadable(format!("{name}: unknown method")))?;
        let id = new_storage_id().map_err(|error| Refusal::Unreadable(error.to_string()))?;
        let target = file_path(import, &id);
        let mut guard = Guard {
            inner: unpacked,
            out: 0,
            packed: entry.compressed,
            room: budget.saturating_sub(total),
            limits,
            name: name.clone(),
            crc: flate2::Crc::new(),
        };
        out.written.push(target.clone());
        let size = match store.put_stream(&mut guard, Some(entry.size), &target) {
            Ok(size) => size,
            Err(StoreError::Io { source, .. }) => {
                return Err(source
                    .get_ref()
                    .and_then(|inner| inner.downcast_ref::<Refusal>())
                    .cloned()
                    .unwrap_or_else(|| Refusal::Unreadable(format!("{name}: {source}"))));
            }
            Err(error) => return Err(Refusal::Unreadable(error.to_string())),
        };
        if size != entry.size || guard.crc.sum() != entry.crc32 {
            return Err(Refusal::Unreadable(format!(
                "{name}: the bytes are damaged"
            )));
        }
        total += size;
        let meta = Meta {
            decoded: decoded.clone(),
            renamed_from,
        };
        if meta != Meta::default() {
            let record = meta_path(import, &id);
            let bytes = serde_json::to_vec(&meta)
                .map_err(|error| Refusal::Unreadable(error.to_string()))?;
            out.written.push(record.clone());
            store
                .put(&bytes, &record)
                .map_err(|error| Refusal::Unreadable(error.to_string()))?;
        }
        out.rows.push(NewUpload {
            import_id: import,
            id,
            path: path.to_string(),
            length: size,
        });
    }
    Ok(())
}

fn is_bomb(out: u64, packed: u64) -> bool {
    out > BOMB_FLOOR && out > packed.max(1).saturating_mul(BOMB_RATIO)
}

/// The entry's name as text, and the bytes it was decoded from when it was
/// not UTF-8.
fn entry_name(entry: &zip::ZipEntry) -> (String, Option<StoredName>) {
    if let Some(name) = &entry.unicode_name {
        return (name.clone(), None);
    }
    match std::str::from_utf8(&entry.name) {
        Ok(name) => (name.to_string(), None),
        Err(_) => (
            decode_cp437(&entry.name),
            Some(StoredName {
                encoding: NameEncoding::Cp437,
                raw: hex(&entry.name),
            }),
        ),
    }
}

/// The entry's path in staging: `\` as `/`, `.` and empty segments dropped,
/// `..` resolved, under the ZIP's own folder. An absolute path, or one that
/// climbs out, is a [`Note::Escapes`].
fn normalised(name: &str, folder: Option<&str>) -> Result<RelPath, Note> {
    let slashed = name.replace('\\', "/");
    let bytes = slashed.as_bytes();
    let drive = bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic();
    if slashed.starts_with('/') || drive {
        return Err(Note::Escapes(name.to_string()));
    }
    let mut segments: Vec<&str> = Vec::new();
    for segment in slashed.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if segments.pop().is_none() {
                    return Err(Note::Escapes(name.to_string()));
                }
            }
            other => segments.push(other),
        }
    }
    let inner = segments.join("/");
    let full = match folder {
        Some(folder) => format!("{folder}/{inner}"),
        None => inner,
    };
    RelPath::parse(&full).map_err(|reason| Note::Unusable {
        name: name.to_string(),
        reason: reason.to_string(),
    })
}

/// `path`, or `path` with `~n` before its extension when an earlier entry's
/// path equals it with capitals ignored; and the name before the rename.
fn unique(path: RelPath, seen: &mut BTreeSet<String>) -> (RelPath, Option<String>) {
    let fold = |text: &str| text.to_lowercase();
    if seen.insert(fold(path.as_str())) {
        return (path, None);
    }
    let text = path.as_str();
    let (folder, name) = text.rsplit_once('/').unwrap_or(("", text));
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => (stem, Some(extension)),
        _ => (name, None),
    };
    for n in 1.. {
        let renamed = match extension {
            Some(extension) => format!("{stem}~{n}.{extension}"),
            None => format!("{stem}~{n}"),
        };
        let candidate = if folder.is_empty() {
            renamed
        } else {
            format!("{folder}/{renamed}")
        };
        if seen.insert(fold(&candidate))
            && let Ok(renamed) = RelPath::parse(&candidate)
        {
            return (renamed, Some(text.to_string()));
        }
    }
    unreachable!("a free ~n name exists")
}

/// The unpacked bytes of one entry, refused as soon as they pass a limit.
struct Guard<'r> {
    inner: Box<dyn Read + 'r>,
    out: u64,
    packed: u64,
    room: u64,
    limits: Limits,
    name: String,
    crc: flate2::Crc,
}

impl Read for Guard<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.out += read as u64;
        self.crc.update(&buffer[..read]);
        let refused = if is_bomb(self.out, self.packed) {
            Some(Refusal::Bomb {
                name: self.name.clone(),
            })
        } else if self.out > self.limits.max_file {
            Some(Refusal::FileTooLarge {
                name: self.name.clone(),
                limit: self.limits.max_file,
            })
        } else if self.out > self.room {
            Some(Refusal::Full {
                limit: self.limits.max_total,
            })
        } else {
            None
        };
        match refused {
            Some(refusal) => Err(io::Error::other(refusal)),
            None => Ok(read),
        }
    }
}
