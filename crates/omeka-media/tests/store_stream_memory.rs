//! `FileStore::put_stream` of 50 MB holds a bounded buffer, not the stream.
//!
//! The check reads the process's peak resident memory, so this binary holds
//! this one test and nothing else.

#![allow(clippy::unwrap_used)]

use omeka_media::store::{FileStore, conformance};

#[test]
fn file_store_streams_50_mb_without_holding_it() {
    let root = tempfile::tempdir().unwrap();
    conformance::check_put_stream_memory(
        &FileStore::new(root.path(), "/files"),
        50 * 1024 * 1024,
        8 * 1024 * 1024,
    );
}
