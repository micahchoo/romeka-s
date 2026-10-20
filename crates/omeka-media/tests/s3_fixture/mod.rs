//! An S3 store on a fresh bucket of the server `ROMEKA_S3_TEST_ENDPOINT`
//! names, such as a MinIO container:
//!
//! ```sh
//! docker run -d --name romeka-minio -p 127.0.0.1:19000:9000 \
//!   -e MINIO_ROOT_USER=minioadmin -e MINIO_ROOT_PASSWORD=minioadmin \
//!   quay.io/minio/minio server /data
//! ROMEKA_S3_TEST_ENDPOINT=http://127.0.0.1:19000 cargo test -p omeka-media --test store_s3
//! ```
//!
//! `ROMEKA_S3_TEST_ACCESS_KEY` and `ROMEKA_S3_TEST_SECRET_KEY` default to
//! MinIO's `minioadmin`. Without the endpoint the tests print SKIP and pass,
//! as the MySQL suites do.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use omeka_media::store::{S3Config, S3Store};

/// The settings for a new, empty bucket, or `None` (after a SKIP line) when no
/// endpoint is set.
pub fn config(test: &str) -> Option<S3Config> {
    let Ok(endpoint) = std::env::var("ROMEKA_S3_TEST_ENDPOINT") else {
        eprintln!("SKIP: ROMEKA_S3_TEST_ENDPOINT is not set, so {test} did not run");
        return None;
    };
    let access_key =
        std::env::var("ROMEKA_S3_TEST_ACCESS_KEY").unwrap_or_else(|_| "minioadmin".into());
    let secret_key =
        std::env::var("ROMEKA_S3_TEST_SECRET_KEY").unwrap_or_else(|_| "minioadmin".into());
    let bucket = format!(
        "romeka-test-{}",
        &omeka_media::store::new_storage_id().unwrap()[..16]
    );
    let config = S3Config {
        endpoint: endpoint.parse().unwrap(),
        bucket,
        region: "us-east-1".to_string(),
        access_key,
        secret_key,
        public_base: None,
        url_ttl: Duration::from_secs(15 * 60),
    };
    S3Store::new(config.clone())
        .unwrap()
        .create_bucket()
        .unwrap();
    Some(config)
}

/// A store on a new bucket.
pub fn store(test: &str) -> Option<S3Store> {
    config(test).map(|config| S3Store::new(config).unwrap())
}

/// Fetch a URL with no credentials, as a browser would: the status and body.
pub fn fetch(url: &str) -> (u16, Vec<u8>) {
    let agent: ureq::Agent = ureq::config::Config::builder()
        .http_status_as_error(false)
        .build()
        .into();
    let mut response = agent.get(url).call().unwrap();
    let status = response.status().as_u16();
    (status, response.body_mut().read_to_vec().unwrap())
}
