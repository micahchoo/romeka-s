//! The `Store` conformance checks against the local `FileStore`, and against a
//! store that keeps the trait's default `rename`.

#![allow(clippy::unwrap_used)]

use std::io::Read;
use std::ops::Range;

use omeka_media::store::{Access, FileStore, ListedFile, Store, StoreError, conformance};

#[test]
fn file_store_conforms_to_the_store_trait() {
    let root = tempfile::tempdir().unwrap();
    conformance::check_store(&FileStore::new(root.path(), "/files"));
}

/// A store with every method but `rename`, as a test double has.
#[derive(Debug)]
struct Plain(FileStore);

impl Store for Plain {
    fn put(&self, bytes: &[u8], path: &str, media_type: Option<&str>) -> Result<(), StoreError> {
        self.0.put(bytes, path, media_type)
    }
    fn put_stream(
        &self,
        reader: &mut dyn Read,
        size_hint: Option<u64>,
        path: &str,
        media_type: Option<&str>,
    ) -> Result<u64, StoreError> {
        self.0.put_stream(reader, size_hint, path, media_type)
    }
    fn read(&self, path: &str) -> Result<Vec<u8>, StoreError> {
        self.0.read(path)
    }
    fn read_range(&self, path: &str, range: Range<u64>) -> Result<Vec<u8>, StoreError> {
        self.0.read_range(path, range)
    }
    fn delete(&self, path: &str) -> Result<bool, StoreError> {
        self.0.delete(path)
    }
    fn exists(&self, path: &str) -> Result<bool, StoreError> {
        self.0.exists(path)
    }
    fn size(&self, path: &str) -> Result<Option<u64>, StoreError> {
        self.0.size(path)
    }
    fn list(&self, prefix: &str) -> Result<Vec<ListedFile>, StoreError> {
        self.0.list(prefix)
    }
    fn url(&self, path: &str, access: Access) -> String {
        self.0.url(path, access)
    }
}

#[test]
fn the_default_rename_conforms_too() {
    let root = tempfile::tempdir().unwrap();
    conformance::check_store(&Plain(FileStore::new(root.path(), "/files")));
}
