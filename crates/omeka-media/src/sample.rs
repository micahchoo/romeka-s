//! A bounded sample of a stored original: its first and last megabyte, at the
//! offsets they have in the original.
//!
//! A header read (`ffprobe`'s codecs, a TIFF's directory) seeks to where the
//! metadata sits, which is at one end of the file. Reading the whole original
//! out of an object store to answer a question about its header would copy a
//! 2 GB video in the request that created it, so the sample is a temporary
//! file of the same length whose gap is a hole. Every offset the original had
//! is where the reader finds it, which is what an MP4 whose `moov` atom trails
//! its media data needs.
//!
//! This is for reading a header. A command that decodes pixels or samples
//! needs the whole original, through its own bounded copy.

use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;

use crate::store::{Store, StoreError};

/// The bytes taken from the start of an original.
pub const HEAD: u64 = 1024 * 1024;

/// The bytes taken from its end.
pub const TAIL: u64 = 1024 * 1024;

/// The most read from the store in one call.
const CHUNK: u64 = 8 * 1024 * 1024;

/// A temporary file holding the first [`HEAD`] bytes and the last [`TAIL`]
/// bytes of the stored file at `storage_path`.
///
/// `size` is a fallback the caller already knows; the store's own answer wins,
/// so a row whose recorded size is stale cannot truncate the sample. A file no
/// longer than the two pieces is copied whole. `extension` names the temporary
/// file's suffix, because a command may lean on it to pick a demuxer (see
/// [`crate::tool::Still::make_from_bytes`]). The temporary file is deleted when
/// it drops.
///
/// # Errors
/// [`StoreError`] when the store cannot read the original or the temporary
/// file cannot be written.
pub fn stored(
    store: &dyn Store,
    storage_path: &str,
    size: Option<u64>,
    extension: Option<&str>,
) -> Result<tempfile::NamedTempFile, StoreError> {
    let size = match store.size(storage_path)? {
        Some(size) => size,
        None => size.ok_or_else(|| {
            io_error(
                storage_path,
                std::io::Error::from(std::io::ErrorKind::NotFound),
            )
        })?,
    };
    let suffix = extension
        .filter(|extension| !extension.is_empty())
        .map(|extension| format!(".{extension}"))
        .unwrap_or_default();
    let mut file = tempfile::Builder::new()
        .prefix("romeka-sample")
        .suffix(&suffix)
        .tempfile()
        .map_err(|error| io_error(storage_path, error))?;
    if size <= HEAD + TAIL {
        copy(store, storage_path, 0, size, file.as_file_mut())?;
        return Ok(file);
    }
    copy(store, storage_path, 0, HEAD, file.as_file_mut())?;
    let tail = size - TAIL;
    file.as_file_mut()
        .seek(SeekFrom::Start(tail))
        .map_err(|error| io_error(storage_path, error))?;
    copy(store, storage_path, tail, TAIL, file.as_file_mut())?;
    Ok(file)
}

/// Copy `length` bytes starting at `start` into `out`, one store read at a
/// time.
fn copy(
    store: &dyn Store,
    storage_path: &str,
    start: u64,
    length: u64,
    out: &mut std::fs::File,
) -> Result<(), StoreError> {
    let end = start.saturating_add(length);
    let mut at = start;
    while at < end {
        let stop = (at + CHUNK).min(end);
        let bytes = store.read_range(storage_path, at..stop)?;
        if bytes.is_empty() {
            break;
        }
        out.write_all(&bytes)
            .map_err(|error| io_error(storage_path, error))?;
        at += bytes.len() as u64;
    }
    Ok(())
}

fn io_error(storage_path: &str, source: std::io::Error) -> StoreError {
    StoreError::Io {
        path: PathBuf::from(storage_path),
        source,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::io::Read;

    use super::*;
    use crate::store::FileStore;

    /// A stored original longer than the two pieces gives the same first and
    /// last bytes at the same offsets, and the gap is not read.
    #[test]
    fn a_large_original_keeps_its_ends_at_their_offsets() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileStore::new(dir.path(), "/files");
        let size = 3 * 1024 * 1024;
        let mut bytes = vec![0u8; size as usize];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = (index % 251) as u8;
        }
        store.put(&bytes, "original/big.bin", None).expect("store");

        let sample = stored(&store, "original/big.bin", Some(size), None).expect("sample");
        let mut read = Vec::new();
        std::fs::File::open(sample.path())
            .expect("open")
            .read_to_end(&mut read)
            .expect("read");
        assert_eq!(read.len() as u64, size, "the length is the original's");
        assert_eq!(&read[..HEAD as usize], &bytes[..HEAD as usize], "head");
        assert_eq!(
            &read[(size - TAIL) as usize..],
            &bytes[(size - TAIL) as usize..],
            "tail"
        );
        assert!(
            read[HEAD as usize..(size - TAIL) as usize]
                .iter()
                .all(|byte| *byte == 0),
            "the gap is a hole, not the original's middle"
        );
    }

    /// A small original is copied whole, so a header at either end is there.
    #[test]
    fn a_small_original_is_copied_whole() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileStore::new(dir.path(), "/files");
        let bytes = vec![7u8; 4096];
        store
            .put(&bytes, "original/small.bin", None)
            .expect("store");
        let sample = stored(&store, "original/small.bin", None, None).expect("sample");
        assert_eq!(
            sample.as_file().metadata().expect("metadata").len(),
            bytes.len() as u64
        );
    }

    /// A path the store does not hold is a store error, not an empty sample.
    #[test]
    fn a_missing_original_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileStore::new(dir.path(), "/files");
        assert!(stored(&store, "original/gone.mp4", None, None).is_err());
    }
}
