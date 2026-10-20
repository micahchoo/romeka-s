//! Sheets for the derive, read from disk per request through a small cache.
//!
//! The index lists sheets but never stores their rows, so each derive reads
//! the plan's sheets again. [`SheetCache`] keeps the last reads, keyed by
//! the file (canonical path, modified time, size) and the read options. A
//! changed file, or a change of encoding, delimiter, header line or key
//! column, is a new key. The cache is bounded twice: at most
//! [`MAX_SHEETS`] reads and [`MAX_BYTES`] of sheet bytes, the least
//! recently used read going first.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use crate::import2::archive::{ArchiveError, SourceDir};
use crate::import2::plan::{Encoding, SheetPlan};
use crate::import2::sheet::{self, Guess, MAX_SHEET_BYTES, Sheet, SheetError};

/// The most reads the cache keeps.
pub const MAX_SHEETS: usize = 16;
/// The most sheet bytes the cache keeps; the parsed rows take a small
/// multiple of this.
pub const MAX_BYTES: u64 = 128 * 1024 * 1024;
/// How much of a sheet the Sheets section shows as raw lines.
const HEAD_BYTES: usize = 16 * 1024;
/// How many raw lines the Sheets section shows.
const HEAD_LINES: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Key {
    path: PathBuf,
    modified: Option<SystemTime>,
    size: u64,
    read: String,
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
    held: Mutex<Vec<(Key, Arc<ReadSheet>)>>,
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
        let key = Key {
            path,
            modified: metadata.modified().ok(),
            size: metadata.len(),
            read: serde_json::to_string(&(&plan.read, sheet::key_column(plan))).unwrap_or_default(),
        };
        if let Some(found) = self.take(&key) {
            return Some(found);
        }
        let read = Arc::new(read_sheet(&key, plan));
        self.put(key, Arc::clone(&read));
        Some(read)
    }

    fn take(&self, key: &Key) -> Option<Arc<ReadSheet>> {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        let at = held.iter().position(|(held_key, _)| held_key == key)?;
        let entry = held.remove(at);
        let found = Arc::clone(&entry.1);
        held.push(entry);
        Some(found)
    }

    fn put(&self, key: Key, read: Arc<ReadSheet>) {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        held.retain(|(held_key, _)| held_key.path != key.path || held_key.read != key.read);
        held.push((key, read));
        while held.len() > MAX_SHEETS
            || (held.len() > 1 && held.iter().map(|(key, _)| key.size).sum::<u64>() > MAX_BYTES)
        {
            held.remove(0);
        }
    }
}

fn unread(error: SheetError) -> ReadSheet {
    ReadSheet {
        sheet: Err(error),
        guess: sheet::guess(&[]),
        lines: Vec::new(),
    }
}

fn read_sheet(key: &Key, plan: &SheetPlan) -> ReadSheet {
    if key.size > MAX_SHEET_BYTES {
        return unread(SheetError::TooLarge(key.size));
    }
    let bytes = match std::fs::read(&key.path) {
        Ok(bytes) => bytes,
        Err(error) => return unread(SheetError::Io(error.to_string())),
    };
    let head = &bytes[..bytes.len().min(HEAD_BYTES)];
    let sheet = sheet::read(&bytes, &plan.read, sheet::key_column(plan));
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
        assert!(held[0].0.path.ends_with("1.csv"), "the oldest went first");
    }
}
