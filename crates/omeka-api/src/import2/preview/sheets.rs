//! Sheets for the derive, read per request through a small cache: from disk
//! for a folder source, from the [`Store`] for the import's staging area
//! (the lists a person uploads, `POST …/sheets`).
//!
//! The index lists sheets but never stores their rows, so each derive reads
//! the plan's sheets again. [`SheetCache`] keeps the last reads, keyed by
//! the file (canonical path, modified time, size; or storage path and
//! modified time) and the read options. A changed file, or a change of
//! encoding, delimiter, header line or key column, is a new key. The cache is bounded twice: at most
//! [`MAX_SHEETS`] reads and [`MAX_BYTES`] of sheet bytes, the least
//! recently used read going first.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use omeka_db::ids::ImportId;
use omeka_media::store::Store;

use crate::import2::archive::{ArchiveError, SourceDir};
use crate::import2::plan::{Encoding, RelPath, SheetPlan};
use crate::import2::sheet::{self, Guess, MAX_SHEET_BYTES, Sheet, SheetError};
use crate::import2::template;

/// The most reads the cache keeps.
pub const MAX_SHEETS: usize = 16;
/// The most sheet bytes the cache keeps; the parsed rows take a small
/// multiple of this.
pub const MAX_BYTES: u64 = 128 * 1024 * 1024;
/// How much of a sheet the Sheets section shows as raw lines.
const HEAD_BYTES: usize = 16 * 1024;
/// How many raw lines the Sheets section shows.
const HEAD_LINES: usize = 20;

/// Where a staging file of import `import` lies in the store. Staging is
/// never under the public `/files` prefix (`docs/import-v2.md`, "Arrival").
#[must_use]
pub fn staging_path(import: ImportId, file: &RelPath) -> String {
    format!("staging/{}/{}", import.0, file.as_str())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Key {
    file: FileKey,
    read: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FileKey {
    Disk {
        path: PathBuf,
        modified: Option<SystemTime>,
        size: u64,
    },
    Stored {
        path: String,
        modified: SystemTime,
    },
}

/// One sheet as read.
#[derive(Debug)]
pub struct ReadSheet {
    pub sheet: Result<Sheet, SheetError>,
    /// What the reader would choose for this file on its own.
    pub guess: Guess,
    /// The first lines, decoded as the sheet was.
    pub lines: Vec<String>,
}

#[derive(Default)]
pub struct SheetCache {
    /// Each read with its byte count, least recently used first.
    held: Mutex<Vec<(Key, u64, Arc<ReadSheet>)>>,
}

impl SheetCache {
    /// The sheet `plan` names in `dir`. `None` when it is not there: the
    /// derive reads a missing sheet as `sheet-unread`.
    #[must_use]
    pub fn read(&self, plan: &SheetPlan, dir: &SourceDir) -> Option<Arc<ReadSheet>> {
        let path = match dir.locate(std::path::Path::new(plan.file.as_str())) {
            Ok(path) => path,
            Err(
                ArchiveError::NotFound(_) | ArchiveError::NotAFile(_) | ArchiveError::Outside(_),
            ) => return None,
            Err(error) => {
                return Some(Arc::new(unread(SheetError::Io(error.to_string()))));
            }
        };
        let metadata = match std::fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => return Some(Arc::new(unread(SheetError::Io(error.to_string())))),
        };
        let size = metadata.len();
        let key = Key {
            file: FileKey::Disk {
                path: path.clone(),
                modified: metadata.modified().ok(),
                size,
            },
            read: read_key(plan),
        };
        if let Some(found) = self.take(&key) {
            return Some(found);
        }
        let read = Arc::new(if size > MAX_SHEET_BYTES {
            unread(SheetError::TooLarge(size))
        } else {
            match std::fs::read(&path) {
                Ok(bytes) => read_bytes(&bytes, plan),
                Err(error) => unread(SheetError::Io(error.to_string())),
            }
        });
        self.put(key, size, Arc::clone(&read));
        Some(read)
    }

    /// The sheet `plan` names, stored at `storage_path`. `None` when the
    /// store holds no such file.
    #[must_use]
    pub fn read_stored(
        &self,
        plan: &SheetPlan,
        store: &dyn Store,
        storage_path: &str,
    ) -> Option<Arc<ReadSheet>> {
        let folder = storage_path
            .rsplit_once('/')
            .map_or("", |(folder, _)| folder);
        let listed = match store.list(folder) {
            Ok(listed) => listed,
            Err(error) => return Some(Arc::new(unread(SheetError::Io(error.to_string())))),
        };
        let modified = listed
            .into_iter()
            .find(|file| file.storage_path == storage_path)?
            .modified;
        let key = Key {
            file: FileKey::Stored {
                path: storage_path.to_string(),
                modified,
            },
            read: read_key(plan),
        };
        if let Some(found) = self.take(&key) {
            return Some(found);
        }
        let (read, size) = match store.read(storage_path) {
            Ok(bytes) if bytes.len() as u64 > MAX_SHEET_BYTES => (
                unread(SheetError::TooLarge(bytes.len() as u64)),
                bytes.len() as u64,
            ),
            Ok(bytes) => {
                // Only the staging area holds lists the template wrote.
                let mut read = read_bytes(&bytes, plan);
                if let Ok(sheet) = &mut read.sheet {
                    template::unescape_list(sheet, sheet::key_column(plan));
                }
                (read, bytes.len() as u64)
            }
            Err(error) => (unread(SheetError::Io(error.to_string())), 0),
        };
        let read = Arc::new(read);
        self.put(key, size, Arc::clone(&read));
        Some(read)
    }

    fn take(&self, key: &Key) -> Option<Arc<ReadSheet>> {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        let at = held.iter().position(|(held_key, _, _)| held_key == key)?;
        let entry = held.remove(at);
        let found = Arc::clone(&entry.2);
        held.push(entry);
        Some(found)
    }

    fn put(&self, key: Key, size: u64, read: Arc<ReadSheet>) {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        held.retain(|(held_key, _, _)| {
            held_key.read != key.read || held_key.file.path() != key.file.path()
        });
        held.push((key, size, read));
        while held.len() > MAX_SHEETS
            || (held.len() > 1 && held.iter().map(|(_, size, _)| size).sum::<u64>() > MAX_BYTES)
        {
            held.remove(0);
        }
    }
}

impl FileKey {
    /// The file, whatever its version.
    fn path(&self) -> &std::path::Path {
        match self {
            Self::Disk { path, .. } => path,
            Self::Stored { path, .. } => std::path::Path::new(path),
        }
    }
}

fn read_key(plan: &SheetPlan) -> String {
    serde_json::to_string(&(&plan.read, sheet::key_column(plan))).unwrap_or_default()
}

fn unread(error: SheetError) -> ReadSheet {
    ReadSheet {
        sheet: Err(error),
        guess: sheet::guess(&[]),
        lines: Vec::new(),
    }
}

fn read_bytes(bytes: &[u8], plan: &SheetPlan) -> ReadSheet {
    let head = &bytes[..bytes.len().min(HEAD_BYTES)];
    let sheet = sheet::read(bytes, &plan.read, sheet::key_column(plan));
    let encoding = match &sheet {
        Ok(sheet) => Some(sheet.encoding.encoding),
        Err(_) => None,
    };
    ReadSheet {
        guess: sheet::guess(head),
        lines: head_lines(head, encoding),
        sheet,
    }
}

/// The first lines of `head`, decoded with `encoding` (lossy UTF-8 when the
/// sheet did not read). The last line may be cut short.
fn head_lines(head: &[u8], encoding: Option<Encoding>) -> Vec<String> {
    let text = match encoding {
        Some(Encoding::Utf16Le) => encoding_rs::UTF_16LE.decode(head).0.into_owned(),
        Some(Encoding::Utf16Be) => encoding_rs::UTF_16BE.decode(head).0.into_owned(),
        Some(Encoding::Windows1252) => encoding_rs::WINDOWS_1252.decode(head).0.into_owned(),
        Some(Encoding::Iso88591) => head.iter().map(|byte| char::from(*byte)).collect(),
        Some(Encoding::Utf8) | None => encoding_rs::UTF_8.decode(head).0.into_owned(),
    };
    text.split("\r\n")
        .flat_map(|line| line.split(['\r', '\n']))
        .take(HEAD_LINES)
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use crate::import2::plan::{RelPath, SheetPlan};

    fn plan(file: &str) -> SheetPlan {
        serde_json::from_value(serde_json::json!({
            "id": "r", "source": "s1", "file": file, "hash": null,
            "read": { "encoding": "auto", "delimiter": "auto", "quote": "\"",
                      "headerLine": 1, "trim": true },
            "rowKind": "item", "join": null, "itemKey": null, "columns": []
        }))
        .unwrap()
    }

    #[test]
    fn a_read_is_kept_until_the_file_or_its_read_options_change() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("r.csv"), "Id,Title\r\nA1,Lamp\r\n").unwrap();
        let source = SourceDir::standalone(dir.path()).unwrap();
        let cache = SheetCache::default();
        let first = cache.read(&plan("r.csv"), &source).expect("read");
        assert_eq!(first.sheet.as_ref().unwrap().rows.len(), 1);
        assert_eq!(first.lines[..2], ["Id,Title", "A1,Lamp"]);
        let again = cache.read(&plan("r.csv"), &source).expect("read");
        assert!(Arc::ptr_eq(&first, &again));

        let mut other = plan("r.csv");
        other.read.header_line = std::num::NonZeroU32::new(2).unwrap();
        let reread = cache.read(&other, &source).expect("read");
        assert!(!Arc::ptr_eq(&first, &reread), "other read options");

        assert!(cache.read(&plan("missing.csv"), &source).is_none());
        assert!(RelPath::parse("r.csv").is_ok());
    }

    #[test]
    fn a_staging_sheet_is_read_from_the_store_and_kept_until_it_changes() {
        let dir = tempfile::tempdir().unwrap();
        let store = omeka_media::store::FileStore::new(dir.path(), "/files");
        let path = staging_path(ImportId(4), &RelPath::parse("lists/1.csv").unwrap());
        assert_eq!(path, "staging/4/lists/1.csv");
        let cache = SheetCache::default();
        assert!(
            cache
                .read_stored(&plan("lists/1.csv"), &store, &path)
                .is_none()
        );
        store.put("Id,Title\nA1,दीपक\n".as_bytes(), &path).unwrap();
        let first = cache
            .read_stored(&plan("lists/1.csv"), &store, &path)
            .expect("stored");
        assert_eq!(first.sheet.as_ref().unwrap().rows[0].cells[1], "दीपक");
        let again = cache
            .read_stored(&plan("lists/1.csv"), &store, &path)
            .expect("stored");
        assert!(Arc::ptr_eq(&first, &again), "warm");
    }

    #[test]
    fn the_cache_holds_at_most_its_count_of_reads() {
        let dir = tempfile::tempdir().unwrap();
        let source = SourceDir::standalone(dir.path()).unwrap();
        let cache = SheetCache::default();
        for n in 0..=MAX_SHEETS {
            let name = format!("{n}.csv");
            std::fs::write(dir.path().join(&name), "Id\nA\n").unwrap();
            cache.read(&plan(&name), &source).expect("read");
        }
        let held = cache.held.lock().unwrap();
        assert_eq!(held.len(), MAX_SHEETS);
        assert!(
            held[0].0.file.path().ends_with("1.csv"),
            "the oldest went first"
        );
    }
}
