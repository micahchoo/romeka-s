//! `S3Store::put_stream` of 50 MB goes up in multipart parts and holds one
//! part, not the stream.
//!
//! The check reads the process's peak resident memory, so this binary holds
//! this one test and nothing else. See `s3_fixture` for the server.

#![allow(clippy::unwrap_used)]

mod s3_fixture;

use omeka_media::store::conformance;

#[test]
fn s3_store_streams_50_mb_without_holding_it() {
    let Some(store) = s3_fixture::store("the S3 put_stream memory test") else {
        return;
    };
    conformance::check_put_stream_memory(&store, 50 * 1024 * 1024, 8 * 1024 * 1024);
}
