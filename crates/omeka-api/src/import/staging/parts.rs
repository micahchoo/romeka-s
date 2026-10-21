//! The bytes of the staging area: a part written per `PATCH`, the parts
//! joined into the file, and a stored file read back a chunk at a time.
//!
//! A folder root is the area's own folder in the store: `staging/<importId>`
//! for an import, `uploads/<userId>` for a user's staged upload. The layout
//! under it is `files/<upload>` and `parts/<upload>.<offset>` for both, so
//! the mechanics are shared and no caller hands this module a table id.

use std::io::{self, Read, Seek, SeekFrom};
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use omeka_media::store::{ListedFile, Store, StoreError};

use super::{READ_CHUNK, StagingError};

/// Where a complete upload's bytes are, under its area's root.
#[must_use]
pub(crate) fn file_path(root: &str, upload: &str) -> String {
    format!("{root}/files/{upload}")
}

/// The folder of an area's parts under its root.
#[must_use]
pub(crate) fn parts_folder(root: &str) -> String {
    format!("{root}/parts")
}

/// Zero-padded, so the store lists the parts in offset order. An upload id
/// is hex, so the first `.` ends it.
#[must_use]
pub(crate) fn part_path(root: &str, upload: &str, offset: u64) -> String {
    format!("{}/{upload}.{offset:020}", parts_folder(root))
}

/// The parts of one upload, in offset order.
///
/// # Errors
/// [`StoreError`] when the store cannot list the folder.
pub(crate) fn part_files(
    store: &dyn Store,
    root: &str,
    upload: &str,
) -> Result<Vec<ListedFile>, StoreError> {
    let folder = parts_folder(root);
    let prefix = format!("{folder}/{upload}.");
    Ok(store
        .list(&folder)?
        .into_iter()
        .filter(|part| part.storage_path.starts_with(&prefix))
        .collect())
}

/// Write one part; the byte count, or why the body did not arrive whole.
pub(crate) fn write_part(
    store: &dyn Store,
    body: Box<dyn Read + Send>,
    left: u64,
    length: u64,
    part: &str,
) -> Result<u64, StagingError> {
    let over = Arc::new(AtomicBool::new(false));
    let mut limited = Limited {
        inner: body,
        left,
        over: Arc::clone(&over),
    };
    match store.put_stream(&mut limited, None, part, None) {
        Ok(written) => Ok(written),
        Err(_) if over.load(Ordering::Relaxed) => Err(StagingError::PastLength { length }),
        Err(StoreError::Io { source, .. }) => Err(StagingError::Interrupted(source.to_string())),
        Err(error) => Err(error.into()),
    }
}

/// A reader that fails once it would yield more than `left` bytes.
struct Limited {
    inner: Box<dyn Read + Send>,
    left: u64,
    over: Arc<AtomicBool>,
}

impl Read for Limited {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buffer)?;
        let read_bytes = read as u64;
        if read_bytes > self.left {
            self.over.store(true, Ordering::Relaxed);
            return Err(io::Error::other("past the declared length"));
        }
        self.left -= read_bytes;
        Ok(read)
    }
}

/// Join the parts of a finished upload into its file. The parts stay until
/// the row says the upload is complete.
pub(crate) fn join_parts(
    store: &dyn Store,
    root: &str,
    upload: &str,
    length: u64,
) -> Result<(), StagingError> {
    let parts: Vec<String> = part_files(store, root, upload)?
        .into_iter()
        .map(|part| part.storage_path)
        .collect();
    let mut joined = Joined {
        store,
        parts: parts.into_iter(),
        current: None,
    };
    let written = store.put_stream(&mut joined, Some(length), &file_path(root, upload), None)?;
    if written != length {
        return Err(StagingError::Store(format!(
            "the parts of upload {upload} hold {written} of its {length} bytes"
        )));
    }
    Ok(())
}

/// Every part, one after another.
struct Joined<'s, I> {
    store: &'s dyn Store,
    parts: I,
    current: Option<StoredReader<'s>>,
}

impl<I: Iterator<Item = String>> Read for Joined<'_, I> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            if let Some(reader) = &mut self.current {
                let read = reader.read(buffer)?;
                if read > 0 || buffer.is_empty() {
                    return Ok(read);
                }
            }
            match self.parts.next() {
                Some(part) => self.current = Some(StoredReader::new(self.store, part, 0..u64::MAX)),
                None => return Ok(0),
            }
        }
    }
}

/// A stored file read through [`Store::read_range`], [`READ_CHUNK`] at a
/// time, so a large file never sits in memory.
pub struct StoredReader<'s> {
    store: &'s dyn Store,
    path: String,
    at: u64,
    end: u64,
    held: Vec<u8>,
    taken: usize,
}

impl<'s> StoredReader<'s> {
    /// Read `range` of the file at `path`; the end may pass the file's end.
    #[must_use]
    pub fn new(store: &'s dyn Store, path: String, range: Range<u64>) -> Self {
        Self {
            store,
            path,
            at: range.start,
            end: range.end,
            held: Vec::new(),
            taken: 0,
        }
    }
}

impl Read for StoredReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.taken == self.held.len() {
            if self.at >= self.end {
                return Ok(0);
            }
            let to = self.end.min(self.at.saturating_add(READ_CHUNK));
            self.held = self
                .store
                .read_range(&self.path, self.at..to)
                .map_err(io::Error::other)?;
            self.taken = 0;
            if self.held.is_empty() {
                self.end = self.at;
                return Ok(0);
            }
            self.at += self.held.len() as u64;
        }
        let count = buffer.len().min(self.held.len() - self.taken);
        buffer[..count].copy_from_slice(&self.held[self.taken..self.taken + count]);
        self.taken += count;
        Ok(count)
    }
}

/// Seeking lets the image decoder re-read a stored original for its
/// thumbnails. A seek inside the chunk already held keeps it, because the
/// decoder makes many short seeks and each refetch is a whole chunk.
impl Seek for StoredReader<'_> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let held_start = self.at - self.held.len() as u64;
        let position = held_start + self.taken as u64;
        let target = match from {
            SeekFrom::Start(offset) => Some(offset),
            SeekFrom::Current(delta) => position.checked_add_signed(delta),
            SeekFrom::End(delta) => self.end.checked_add_signed(delta),
        }
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek before the start"))?;
        if target >= held_start && target <= self.at && !self.held.is_empty() {
            self.taken = (target - held_start) as usize;
        } else {
            self.held.clear();
            self.taken = 0;
            self.at = target;
        }
        Ok(target)
    }
}
