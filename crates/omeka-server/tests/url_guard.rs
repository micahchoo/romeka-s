//! The URL guard end to end: `import_allow_private_urls` travels on
//! `/api/settings`, and a `url` media create reaches a loopback server only
//! when the operator turned it on (docs/import-v2.md "URL sources").

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use axum::Router;
use axum::http::StatusCode;
use omeka_media::store::FileStore;
use serde_json::{Value, json};

use common::{Answer, Seed, append_key, get, json_request, router_with_store, seeded_pool, send};

async fn app(dir: &std::path::Path) -> (Router, Seed) {
    let (pool, seed) = seeded_pool().await;
    (router_with_store(pool, FileStore::new(dir, "/files")), seed)
}

async fn patch_settings(app: &Router, body: &Value) -> Answer {
    send(
        app.clone(),
        json_request("PATCH", &append_key("/api/settings"), &body.to_string()),
    )
    .await
}

/// A loopback server that answers every request with a small text file and
/// counts the connections.
fn loopback_file() -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let hits = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&hits);
    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            counted.fetch_add(1, Ordering::SeqCst);
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request);
            let body = b"inside\n";
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(body);
        }
    });
    (format!("http://{addr}/inside.txt"), hits)
}

async fn create_url_media(app: &Router, seed: &Seed, url: &str) -> Answer {
    let body = json!({
        "o:ingester": "url",
        "o:item": {"o:id": seed.item.0},
        "ingest_url": url,
    });
    send(
        app.clone(),
        json_request("POST", &append_key("/api/media"), &body.to_string()),
    )
    .await
}

/// The setting answers `false` when no row is stored, and a write stores it.
#[tokio::test]
async fn the_allow_setting_defaults_off_and_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let (app, _seed) = app(dir.path()).await;
    let answer = send(app.clone(), get("/api/settings", true)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(answer.json()["import_allow_private_urls"], json!(false));

    let answer = patch_settings(&app, &json!({"import_allow_private_urls": true})).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(answer.json()["import_allow_private_urls"], json!(true));

    let answer = patch_settings(&app, &json!({"import_allow_private_urls": "yes"})).await;
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
}

/// By default a `url` create of a loopback address is refused and the server
/// sees no connection; with the setting on, the same create stores the file.
#[tokio::test]
async fn a_loopback_url_ingest_needs_the_allow_setting() {
    let dir = tempfile::tempdir().unwrap();
    let (app, seed) = app(dir.path()).await;
    let (url, hits) = loopback_file();

    let refused = create_url_media(&app, &seed, &url).await;
    assert_eq!(
        refused.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        refused.body
    );
    assert!(refused.body.contains("refused"), "{}", refused.body);
    assert_eq!(hits.load(Ordering::SeqCst), 0, "no connection was made");

    let answer = patch_settings(&app, &json!({"import_allow_private_urls": true})).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let stored = create_url_media(&app, &seed, &url).await;
    assert_eq!(stored.status, StatusCode::OK, "{}", stored.body);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}
