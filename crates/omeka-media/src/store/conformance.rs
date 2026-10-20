//! The behaviour every [`Store`] owes its callers, as checks a test runs
//! against one implementation.
//!
//! Each check writes under its own `conformance-<name>` folder, so the checks
//! share one empty store. The S3 store runs the same checks, so nothing here
//! may depend on a local filesystem.
//!
//! [`check_put_stream_memory`] measures the process's peak resident memory, so
//! it must be the only test in its test binary.

use std::io::Read;
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::{Access, Store, StoreError};

/// Run every check except [`check_put_stream_memory`].
pub fn check_store(store: &dyn Store) {
    put_then_read_round_trips(store);
    exists_and_delete_report_presence(store);
    read_range_clips_to_the_file(store);
    list_names_the_files_directly_in_a_folder(store);
    put_stream_stores_every_byte(store);
    illegal_paths_are_refused(store);
    url_names_the_storage_path(store);
}

/// `put` then `read` returns the bytes; a second `put` replaces them.
pub fn put_then_read_round_trips(store: &dyn Store) {
    let path = "conformance-put/a.txt";
    ok(store.put(b"first", path), "put");
    assert_eq!(ok(store.read(path), "read"), b"first");
    ok(store.put(b"second and longer", path), "overwrite");
    assert_eq!(ok(store.read(path), "read"), b"second and longer");
    assert!(
        store.read("conformance-put/missing.txt").is_err(),
        "reading a missing file is an error"
    );
}

/// `exists` follows `put` and `delete`; deleting a missing file answers
/// `false`, as upstream's `Local::delete` treats it as success.
pub fn exists_and_delete_report_presence(store: &dyn Store) {
    let path = "conformance-delete/a.bin";
    assert!(!ok(store.exists(path), "exists before put"));
    ok(store.put(b"x", path), "put");
    assert!(ok(store.exists(path), "exists after put"));
    assert!(ok(store.delete(path), "delete"));
    assert!(!ok(store.exists(path), "exists after delete"));
    assert!(!ok(store.delete(path), "second delete"));
}

/// `read_range` answers the bytes in the range, clipped at the end of the
/// file, and nothing for a range that starts past it.
pub fn read_range_clips_to_the_file(store: &dyn Store) {
    let path = "conformance-range/digits.txt";
    ok(store.put(b"0123456789", path), "put");
    assert_eq!(ok(store.read_range(path, 2..5), "inside"), b"234");
    assert_eq!(ok(store.read_range(path, 7..100), "past the end"), b"789");
    assert_eq!(ok(store.read_range(path, 0..10), "whole"), b"0123456789");
    assert!(ok(store.read_range(path, 10..20), "at the end").is_empty());
    assert!(ok(store.read_range(path, 4..4), "empty").is_empty());
    assert!(
        store.read_range("conformance-range/missing", 0..1).is_err(),
        "a range of a missing file is an error"
    );
}

/// `list(prefix)` names the files directly inside the folder, sorted, as
/// storage paths. A nested folder's files and a sibling folder are not listed,
/// and a folder that does not exist lists nothing.
pub fn list_names_the_files_directly_in_a_folder(store: &dyn Store) {
    for path in [
        "conformance-list/b.jpg",
        "conformance-list/a.jpg",
        "conformance-list/nested/c.jpg",
        "conformance-list-sibling/d.jpg",
    ] {
        ok(store.put(b"x", path), "put");
    }
    let listed: Vec<String> = ok(store.list("conformance-list"), "list")
        .into_iter()
        .map(|file| file.storage_path)
        .collect();
    assert_eq!(listed, ["conformance-list/a.jpg", "conformance-list/b.jpg"]);
    assert!(ok(store.list("conformance-absent"), "absent folder").is_empty());
}

/// `put_stream` stores every byte of the reader and answers the count.
pub fn put_stream_stores_every_byte(store: &dyn Store) {
    let path = "conformance-stream/small.bin";
    let mut source = Generated::new(300_000);
    let written = ok(store.put_stream(&mut source, Some(300_000), path), "stream");
    assert_eq!(written, 300_000);
    assert_eq!(ok(store.read(path), "read"), Generated::bytes(300_000));
    let mut empty: &[u8] = &[];
    let path = "conformance-stream/empty.bin";
    assert_eq!(ok(store.put_stream(&mut empty, None, path), "empty"), 0);
    assert!(ok(store.read(path), "read empty").is_empty());
}

/// A path with a `..` component, a leading `/` or no name is refused before
/// the store touches anything.
pub fn illegal_paths_are_refused(store: &dyn Store) {
    for path in [
        "../escape.txt",
        "original/../../etc/passwd",
        "/etc/passwd",
        "",
    ] {
        let illegal = |result: Result<(), StoreError>, call: &str| {
            assert!(
                matches!(result, Err(StoreError::IllegalPath(_))),
                "{call}({path:?}) must be refused, got {result:?}"
            );
        };
        illegal(store.put(b"x", path), "put");
        illegal(store.read(path).map(drop), "read");
        illegal(store.read_range(path, 0..1).map(drop), "read_range");
        illegal(store.delete(path).map(drop), "delete");
        illegal(store.exists(path).map(drop), "exists");
        illegal(
            store.put_stream(&mut &b"x"[..], Some(1), path).map(drop),
            "put_stream",
        );
    }
    for prefix in ["..", "conformance/../..", "/etc"] {
        assert!(
            matches!(store.list(prefix), Err(StoreError::IllegalPath(_))),
            "list({prefix:?}) must be refused"
        );
    }
}

/// Both kinds of URL carry the storage path; a public one ends with it.
pub fn url_names_the_storage_path(store: &dyn Store) {
    let path = "original/abc123.jpg";
    assert!(
        store
            .url(path, Access::Public)
            .ends_with("/original/abc123.jpg")
    );
    let signed = store.url(
        path,
        Access::Signed {
            ttl: Duration::from_secs(300),
        },
    );
    assert!(signed.contains("/original/abc123.jpg"), "{signed}");
}

/// `put_stream` of `len` generated bytes stores them all without holding the
/// stream in memory: the process's peak resident memory may grow by at most
/// `max_growth` bytes, and the bytes read back hash the same.
///
/// The peak is read from `/proc/self/status`, so on a system without it only
/// the content is checked. Run this check alone in its own test binary: another
/// test's allocations would count against the bound.
pub fn check_put_stream_memory(store: &dyn Store, len: u64, max_growth: u64) {
    let path = "conformance-memory/large.bin";
    let before = peak_resident_bytes();
    let mut source = Generated::new(len);
    let written = ok(store.put_stream(&mut source, Some(len), path), "stream");
    let after = peak_resident_bytes();
    assert_eq!(written, len);
    if let (Some(before), Some(after)) = (before, after) {
        let growth = after.saturating_sub(before);
        assert!(
            growth <= max_growth,
            "peak resident memory grew by {growth} bytes for a {len} byte stream"
        );
    }

    let mut stored = Sha256::new();
    let chunk = 4 * 1024 * 1024;
    let mut offset = 0;
    while offset < len {
        let bytes = ok(store.read_range(path, offset..offset + chunk), "read_range");
        assert!(!bytes.is_empty(), "the stored file ends at {offset}");
        offset += bytes.len() as u64;
        stored.update(&bytes);
    }
    assert_eq!(offset, len);
    assert_eq!(stored.finalize(), Generated::digest(len));
    assert!(ok(store.delete(path), "delete"));
}

fn ok<T>(result: Result<T, StoreError>, what: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{what}: {error}"),
    }
}

/// `VmHWM`, the process's peak resident set, in bytes.
fn peak_resident_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmHWM:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

/// A reader of `len` deterministic bytes that it makes as it is read, so the
/// source itself holds nothing.
struct Generated {
    position: u64,
    len: u64,
}

impl Generated {
    fn new(len: u64) -> Self {
        Self { position: 0, len }
    }

    /// Position-dependent, so a reordered or repeated chunk changes the hash.
    fn byte(position: u64) -> u8 {
        (position.wrapping_mul(31) ^ (position >> 13)) as u8
    }

    fn bytes(len: u64) -> Vec<u8> {
        (0..len).map(Self::byte).collect()
    }

    fn digest(len: u64) -> sha2::digest::Output<Sha256> {
        let mut hasher = Sha256::new();
        let mut source = Self::new(len);
        let mut buffer = vec![0; 64 * 1024];
        loop {
            let read = source.read(&mut buffer).unwrap_or_default();
            if read == 0 {
                return hasher.finalize();
            }
            hasher.update(&buffer[..read]);
        }
    }
}

impl Read for Generated {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let left = self.len - self.position;
        let count = usize::try_from(left).map_or(buffer.len(), |left| left.min(buffer.len()));
        for slot in &mut buffer[..count] {
            *slot = Self::byte(self.position);
            self.position += 1;
        }
        Ok(count)
    }
}
