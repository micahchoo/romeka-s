//! The `Store` conformance checks against the local `FileStore`.

#![allow(clippy::unwrap_used)]

use omeka_media::store::{FileStore, conformance};

#[test]
fn file_store_conforms_to_the_store_trait() {
    let root = tempfile::tempdir().unwrap();
    conformance::check_store(&FileStore::new(root.path(), "/files"));
}
