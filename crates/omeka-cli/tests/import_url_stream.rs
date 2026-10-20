//! A URL source over 64 MB reaches the store as a stream (`docs/import-v2.md`,
//! "URL sources"): the run's peak resident memory grows by far less than the
//! body.
//!
//! The check reads the process's peak resident memory, so this binary holds
//! this one test and nothing else.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod import_site;

use import_site::{col, rule, sheet_plan, site};
use omeka_db::repos::SettingRepo;
use omeka_db::rows::ImportStatus;
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const LEN: usize = 70 * 1024 * 1024;
const MAX_GROWTH: u64 = 24 * 1024 * 1024;
const CHUNK: usize = 1024 * 1024;

/// `VmHWM`, the process's peak resident set, in bytes.
fn peak_resident_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmHWM:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

/// The n-th megabyte of the body: a TIFF head, then bytes that differ per
/// megabyte, made on the fly so the test never holds the whole body.
fn chunk(n: usize) -> Vec<u8> {
    let mut chunk = vec![u8::try_from(n % 251).unwrap(); CHUNK];
    if n == 0 {
        chunk[..8].copy_from_slice(b"II*\0\x08\0\0\0");
    }
    chunk
}

/// A loopback server that answers with `LEN` bytes and their length.
async fn big_server() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buffer = [0u8; 4096];
                let _ = stream.read(&mut buffer).await;
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: image/tiff\r\nContent-Length: {LEN}\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(head.as_bytes()).await;
                for n in 0..LEN / CHUNK {
                    if stream.write_all(&chunk(n)).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    format!("http://{addr}")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_url_body_over_64_mb_streams_into_the_store() {
    let site = site().await;
    SettingRepo::new(&site.pool)
        .set(omeka_api::settings::IMPORT_ALLOW_PRIVATE_URLS, &json!(true))
        .await
        .unwrap();
    let base = big_server().await;
    let mut hasher = Sha256::new();
    for n in 0..LEN / CHUNK {
        hasher.update(chunk(n));
    }
    let digest: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    site.write(
        "rows.csv",
        format!("Kind,Id,Item,URL\nitem,BIG,,\nmedia,,BIG,{base}/big.tif\n").as_bytes(),
    );
    let id = site
        .draft(|plan| {
            plan["sheets"] = json!([sheet_plan("rows", &["Kind", "Id", "Item", "URL"])]);
            plan["sheets"][0]["rowKind"] = json!({ "column": { "index": 0, "header": "Kind" } });
            plan["sheets"][0]["itemKey"] = json!({ "index": 2, "header": "Item" });
            plan["slots"]["item"]["dcterms:identifier"] =
                rule(json!([col("rows", 1, "Id")]), "first");
            plan["slots"]["media"]["ingester"] =
                rule(json!([{ "from": "constant", "value": "url" }]), "first");
            plan["slots"]["media"]["mediaSource"] = rule(json!([col("rows", 3, "URL")]), "first");
        })
        .await;
    let before = peak_resident_bytes();

    let row = site.run(id).await;

    let after = peak_resident_bytes();
    assert_eq!(row.status, ImportStatus::Done, "{:?}", row.summary);
    let media = site.media().await;
    assert_eq!(media.len(), 1, "{:?}", row.summary);
    assert_eq!(media[0].ingester, "url");
    assert_eq!(media[0].size, Some(LEN as i64));
    assert_eq!(media[0].sha256.as_deref(), Some(digest.as_str()));
    assert_eq!(media[0].media_type.as_deref(), Some("image/tiff"));
    assert_eq!(
        media[0].source.as_deref(),
        Some(format!("{base}/big.tif").as_str())
    );
    assert!(
        !media[0].has_thumbnails,
        "no thumbnail from a streamed body"
    );
    if let (Some(before), Some(after)) = (before, after) {
        let growth = after.saturating_sub(before);
        eprintln!("peak resident memory grew by {growth} bytes for a {LEN} byte body");
        assert!(
            growth <= MAX_GROWTH,
            "peak resident memory grew by {growth} bytes for a {LEN} byte body"
        );
    }
}
