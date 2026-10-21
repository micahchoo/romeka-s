//! A staged upload above 64 MiB becomes a media without the file ever sitting
//! in memory (`briefs/media-ui`, G1 and S4).
//!
//! The upload goes in one-megabyte `PATCH` requests and the media create runs
//! the import's streaming path, so the whole file is on disk and in the store,
//! never in the process. Linux reports the process's high-water mark as
//! `VmHWM` in `/proc/self/status`; the test reads it before and after, and the
//! bound is far below the file size, so a path that reads the file whole
//! fails it. The test process is this binary's only test, so nothing else
//! moves the mark while it runs.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod imports_fixture;

use std::io::{Read, Write};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::send;
use imports_fixture::{ADMIN, fixture};
use serde_json::json;

const MIB: usize = 1024 * 1024;
/// The file is above the 64 MiB whole-file limit.
const SIZE: usize = 96 * MIB;
/// What the high-water mark may move by. A path that reads the file whole
/// would move it by at least the file's size.
const BOUND: usize = 64 * MIB;

/// The process's peak resident set in bytes, from `/proc/self/status`.
fn high_water() -> usize {
    let status = std::fs::read_to_string("/proc/self/status").expect("/proc/self/status");
    let line = status
        .lines()
        .find(|line| line.starts_with("VmHWM:"))
        .expect("VmHWM");
    let kib: usize = line
        .split_whitespace()
        .nth(1)
        .expect("a size")
        .parse()
        .expect("kilobytes");
    kib * 1024
}

fn keyed(uri: &str) -> String {
    let separator = if uri.contains('?') { '&' } else { '?' };
    format!(
        "{uri}{separator}key_identity={}&key_credential={}",
        ADMIN.0, ADMIN.1
    )
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0_u32, |n, (at, byte)| n | u32::from(*byte) << (16 - 8 * at));
        for at in 0..4 {
            if at <= chunk.len() {
                out.push(char::from(TABLE[(n >> (18 - 6 * at)) as usize & 63]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

async fn patch(app: axum::Router, location: &str, offset: usize, bytes: Vec<u8>) -> common::Answer {
    send(
        app,
        Request::builder()
            .method("PATCH")
            .uri(keyed(location))
            .header("tus-resumable", "1.0.0")
            .header("content-type", "application/offset+octet-stream")
            .header("upload-offset", offset.to_string())
            .body(Body::from(bytes))
            .unwrap(),
    )
    .await
}

#[tokio::test]
async fn a_ninety_six_mib_upload_and_create_never_hold_the_file() {
    let fixture = fixture().await;
    let item = fixture.call("GET", "/api/items", None).await.json()[0]["o:id"]
        .as_u64()
        .expect("the seeded item");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.txt");
    {
        let mut file = std::fs::File::create(&path).unwrap();
        let block = vec![b'x'; MIB];
        for _ in 0..(SIZE / MIB) {
            file.write_all(&block).unwrap();
        }
        file.flush().unwrap();
    }

    let made = send(
        fixture.app.clone(),
        Request::builder()
            .method("POST")
            .uri(keyed("/api/romeka/uploads"))
            .header("tus-resumable", "1.0.0")
            .header("upload-length", SIZE.to_string())
            .header(
                "upload-metadata",
                format!(
                    "filename {},filetype {}",
                    base64(b"big.txt"),
                    base64(b"text/plain")
                ),
            )
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(made.status, StatusCode::CREATED, "{}", made.body);
    let location = made.headers["location"].to_str().unwrap().to_string();

    let base = high_water();
    let mut file = std::fs::File::open(&path).unwrap();
    let mut sent = 0;
    while sent < SIZE {
        let mut chunk = vec![0_u8; MIB];
        file.read_exact(&mut chunk).unwrap();
        let answer = patch(fixture.app.clone(), &location, sent, chunk).await;
        assert_eq!(answer.status, StatusCode::NO_CONTENT, "{}", answer.body);
        sent += MIB;
    }
    assert_eq!(sent, SIZE, "the upload ended at {} bytes, not {SIZE}", sent);

    let created = send(
        fixture.app.clone(),
        Request::builder()
            .method("POST")
            .uri(keyed("/api/media"))
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "o:ingester": "upload",
                    "o:item": {"o:id": item},
                    "o-module-romeka:upload": location.rsplit('/').next().unwrap(),
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let media = created.json();
    assert_eq!(media["o:media_type"], "text/plain");
    assert_eq!(media["o:size"], SIZE);
    assert_eq!(media["o:source"], "big.txt");

    let peak = high_water();
    eprintln!(
        "peak resident set grew {} KiB for a {} MiB file",
        (peak.saturating_sub(base)) / 1024,
        SIZE / MIB
    );
    assert!(
        peak.saturating_sub(base) < BOUND,
        "the process peak grew by {} MiB for a {} MiB file",
        (peak - base) / MIB,
        SIZE / MIB
    );
}
