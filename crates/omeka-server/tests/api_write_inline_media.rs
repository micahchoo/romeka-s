//! Inline `o:media` on an item create, and the `o:` members a write ignores.
//!
//! `ItemAdapter::hydrate` (`Api/Adapter/ItemAdapter.php:296-345`) lets a create
//! carry its media, each entry naming an ingester. The tests here drive the
//! router, so an entry runs the same ingester a standalone `POST /api/media`
//! runs. The `o:` member tests freeze what upstream does with a member no
//! hydrator reads: it is ignored, never a validation error.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use axum::http::StatusCode;
use common::{
    append_key, get, json_request, multipart_asset_request, router_with_store, seeded_pool,
    seeded_router, send,
};
use omeka_api::settings::IMPORT_ALLOW_PRIVATE_URLS;
use omeka_db::repos::SettingRepo;
use omeka_media::store::{FileStore, Store};
use serde_json::{Value, json};

fn keyed(uri: &str) -> String {
    append_key(uri)
}

/// A loopback HTTP/1.1 server that accepts one request, reports that it did,
/// waits `delay`, then answers. The report lets a test know the client is
/// stalled inside the fetch before it drives anything else.
fn stalling_server(delay: Duration) -> (String, mpsc::Receiver<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (started, started_rx) = mpsc::channel();
    thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut request = [0u8; 2048];
        let _ = stream.read(&mut request);
        let _ = started.send(());
        thread::sleep(delay);
        let body = b"STALLED\n";
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(body);
        let _ = stream.flush();
    });
    (format!("http://{addr}/slow.txt"), started_rx)
}

/// Drive a create that fetches a stalled URL, then prove the same server answers
/// a plain `GET /health` while the fetch is still in flight.
///
/// One worker, so a synchronous fetch inside the handler would leave the health
/// request queued behind it for the whole stall. The health future is spawned
/// after the fetch has started, so the bound it is measured against is the
/// server's own liveness and not the caller's thread.
fn assert_stays_responsive(
    runtime: &tokio::runtime::Runtime,
    app: axum::Router,
    path: &str,
    body: impl FnOnce(&str) -> String,
) {
    const STALL: Duration = Duration::from_secs(3);
    const BOUND: Duration = Duration::from_secs(1);
    let (url, started) = stalling_server(STALL);
    let path = path.to_string();
    let create = runtime.spawn({
        let app = app.clone();
        let body = body(&url);
        async move { send(app, json_request("POST", &keyed(&path), &body)).await }
    });
    started
        .recv_timeout(BOUND)
        .expect("the fetch did not start inside the bound");

    let health = runtime.spawn({
        let app = app.clone();
        async move { send(app, get("/health", false)).await }
    });
    let ready = Instant::now();
    let health = runtime.block_on(health).expect("the health task ran");
    let waited = ready.elapsed();

    assert_eq!(health.status, StatusCode::OK, "{}", health.body);
    assert!(
        waited < BOUND,
        "GET /health waited {waited:?}, the server was blocked by the stalled ingest"
    );
    assert_eq!(
        runtime
            .block_on(create)
            .expect("the create task ran")
            .status,
        StatusCode::OK,
        "the stalled ingest did not finish"
    );
}

fn one_worker_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("runtime")
}

/// A loopback HTTP/1.1 server that answers one request with `body` and then
/// stops. The client under test is real, so the network ingester needs a real
/// socket (AGENTS.md: tests bind an ephemeral port). The handle is dropped: a
/// create that never fetches leaves the thread waiting, and the assertions on
/// the stored bytes are what prove the fetch happened.
fn serve_once(body: &'static [u8], content_type: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut request = [0u8; 2048];
        let _ = stream.read(&mut request);
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(body);
        let _ = stream.flush();
    });
    format!("http://{addr}/hello.txt")
}

/// The seeded pool with a store rooted in `dir`, so an ingester's bytes land
/// somewhere the test can read and the checkout is untouched. The operator's
/// `import_allow_private_urls` is on, because every fetch here goes to a
/// loopback server the URL guard refuses by default.
async fn router_in(dir: &std::path::Path) -> (axum::Router, common::Seed) {
    let (pool, seed) = seeded_pool().await;
    SettingRepo::new(&pool)
        .set(IMPORT_ALLOW_PRIVATE_URLS, &json!(true))
        .await
        .expect("allow private URLs");
    let store = FileStore::new(dir, "/files");
    (router_with_store(pool, store), seed)
}

/// A create with two inline media answers the item with both media, in array
/// order, the first as the primary media (`ItemRepresentation::primaryMedia`
/// falls back to the first when none is stored).
#[tokio::test]
async fn an_item_create_builds_its_inline_media() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (app, seed) = router_in(dir.path()).await;
    let body = json!({
        "o:is_public": true,
        "dcterms:title": [{"property_id": "auto", "type": "literal", "@value": "Two media"}],
        "o:media": [
            {"o:ingester": "html", "html": "<p>first</p>", "o:is_public": true},
            {"o:ingester": "html", "html": "<p>second</p>", "o:is_public": false},
        ],
    })
    .to_string();
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/items"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let item = created.json();
    let media = item["o:media"].as_array().expect("media array");
    assert_eq!(media.len(), 2, "{}", created.body);
    assert_eq!(item["o:primary_media"]["o:id"], media[0]["o:id"]);

    let first = send(
        app.clone(),
        get(&format!("/api/media/{}", media[0]["o:id"]), true),
    )
    .await
    .json();
    assert_eq!(first["o:ingester"], "html");
    assert_eq!(first["o:renderer"], "html");
    assert_eq!(first["o:item"]["o:id"], item["o:id"]);
    assert_eq!(first["o:owner"]["o:id"], seed.user.0);
    assert_eq!(first["o:is_public"], true);
    assert_eq!(first["o-cnt:chars"], "<p>first</p>");
    assert_eq!(
        first["data"],
        json!({"o:ingester": "html", "html": "<p>first</p>", "o:is_public": true})
    );

    let second = send(app, get(&format!("/api/media/{}", media[1]["o:id"]), true)).await;
    assert_eq!(second.status, StatusCode::OK, "{}", second.body);
    let second = second.json();
    assert_eq!(second["o:is_public"], false);
    assert_eq!(second["o-cnt:chars"], "<p>second</p>");
}

/// The `position` member of an entry wins over the array index, and the item's
/// media read back in position order.
#[tokio::test]
async fn an_inline_entry_position_wins_over_its_index() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (app, _seed) = router_in(dir.path()).await;
    let body = json!({
        "o:is_public": true,
        "o:media": [
            {"o:ingester": "html", "html": "one", "position": 5},
            {"o:ingester": "html", "html": "two", "position": 9},
            {"o:ingester": "html", "html": "three"},
        ],
    })
    .to_string();
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/items"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let item = created.json();
    let ids: Vec<u64> = item["o:media"]
        .as_array()
        .expect("media")
        .iter()
        .map(|media| media["o:id"].as_u64().expect("id"))
        .collect();
    assert_eq!(ids.len(), 3);
    let chars = |id: &u64| {
        let app = app.clone();
        let id = *id;
        async move {
            send(app, get(&format!("/api/media/{id}"), false))
                .await
                .json()["o-cnt:chars"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        }
    };
    // Position order is 3 (index 3), 5 (index 1), 9 (index 2).
    assert_eq!(chars(&ids[0]).await, "three");
    assert_eq!(chars(&ids[1]).await, "one");
    assert_eq!(chars(&ids[2]).await, "two");
    assert_eq!(item["o:primary_media"]["o:id"], ids[0]);
}

/// A `url` entry names its URL with a top-level `ingest_url`, and the create
/// downloads it: the row carries the source, the sniffed media type, the size
/// and checksum, and the bytes are in the store.
#[tokio::test]
async fn an_inline_url_entry_downloads_and_stores_the_file() {
    const BODY: &[u8] = b"HELLO URL INGESTER\n";
    let url = serve_once(BODY, "text/plain");
    let dir = tempfile::tempdir().expect("tempdir");
    let (app, _seed) = router_in(dir.path()).await;
    let body = json!({
        "o:is_public": true,
        "o:media": [{"o:ingester": "url", "ingest_url": url}],
    })
    .to_string();
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/items"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let id = created.json()["o:media"][0]["o:id"]
        .as_u64()
        .expect("media id");
    let media = send(app, get(&format!("/api/media/{id}"), false))
        .await
        .json();
    assert_eq!(media["o:source"], url);
    assert_eq!(media["o:media_type"], "text/plain");
    assert_eq!(media["o:size"], BODY.len());
    assert_eq!(media["o:sha256"], omeka_media::store::sha256_hex(BODY));
    // The url ingester stores no payload, and `Doctrine 2`'s `JsonArrayType`
    // answers `[]` for the NULL column, so the read does too.
    assert_eq!(media["data"], json!([]));
    let filename = media["o:filename"].as_str().expect("filename");
    assert!(filename.ends_with(".txt"), "{filename}");
    assert_eq!(
        media["o:original_url"],
        format!("http://localhost/files/original/{filename}")
    );
    let stored = FileStore::new(dir.path(), "/files")
        .read(&format!("original/{filename}"))
        .expect("stored bytes");
    assert_eq!(stored, BODY);
}

/// An `upload` entry takes the multipart `file` part, named by `file_index`.
#[tokio::test]
async fn an_inline_upload_entry_stores_the_multipart_file() {
    const BYTES: &[u8] = b"an uploaded body";
    let dir = tempfile::tempdir().expect("tempdir");
    let (app, _seed) = router_in(dir.path()).await;
    let data = json!({
        "o:is_public": true,
        "o:media": [{"o:ingester": "upload", "file_index": 0}],
    })
    .to_string();
    let created = send(
        app.clone(),
        multipart_asset_request(&keyed("/api/items"), "notes.txt", BYTES, &data),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let id = created.json()["o:media"][0]["o:id"]
        .as_u64()
        .expect("media id");
    let media = send(app, get(&format!("/api/media/{id}"), false))
        .await
        .json();
    assert_eq!(media["o:ingester"], "upload");
    assert_eq!(media["o:source"], "notes.txt");
    assert_eq!(media["o:size"], BYTES.len());
    let filename = media["o:filename"].as_str().expect("filename");
    let stored = FileStore::new(dir.path(), "/files")
        .read(&format!("original/{filename}"))
        .expect("stored bytes");
    assert_eq!(stored, BYTES);
}

/// Every failing entry answers under `o:media`, one entry per failure: a plain
/// message for an entry naming an `o:id`, the entry's own error store for one
/// the ingester refused.
#[tokio::test]
async fn the_inline_media_errors_are_one_entry_per_failure() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (app, _seed) = router_in(dir.path()).await;
    let body = json!({
        "o:is_public": true,
        "o:media": [
            {"o:ingester": "url"},
            {"o:ingester": "bogus"},
            {"o:id": 999999, "o:ingester": "html", "html": "x"},
        ],
    })
    .to_string();
    let answer = send(app, json_request("POST", &keyed("/api/items"), &body)).await;
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
    assert_eq!(
        answer.json()["errors"],
        json!({"o:media": [
            {"error": ["No ingest URL specified"]},
            {"o:ingester": ["Media must set a valid ingester."]},
            "Media must belong to this item.",
        ]})
    );
}

/// `isset($mediaData['o:id'])`: a present `null` names no media, so the entry is
/// a new one.
#[tokio::test]
async fn a_null_o_id_names_no_media() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (app, _seed) = router_in(dir.path()).await;
    let body = json!({
        "o:is_public": true,
        "o:media": [{"o:id": Value::Null, "o:ingester": "html", "html": "built"}],
    })
    .to_string();
    let created = send(app, json_request("POST", &keyed("/api/items"), &body)).await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    assert_eq!(created.json()["o:media"].as_array().map(Vec::len), Some(1));
}

/// A media the ingester could not build leaves no item behind.
#[tokio::test]
async fn a_failed_inline_media_leaves_no_item() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (app, _seed) = router_in(dir.path()).await;
    let before = send(app.clone(), get(&keyed("/api/items"), true)).await;
    let before_total = before.headers.get("omeka-s-total-results").cloned();
    let body = json!({"o:is_public": true, "o:media": [{"o:ingester": "url"}]}).to_string();
    let answer = send(
        app.clone(),
        json_request("POST", &keyed("/api/items"), &body),
    )
    .await;
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
    let after = send(app, get(&keyed("/api/items"), true)).await;
    assert_eq!(
        after.body, before.body,
        "the failed create changed the item listing"
    );
    assert_eq!(
        after.headers.get("omeka-s-total-results"),
        before_total.as_ref()
    );
}

/// `o:primary_media` on a create names a media the item cannot have yet, so a
/// numeric id is the validation error and any other shape is dropped.
#[tokio::test]
async fn a_create_primary_media_member_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (app, _seed) = router_in(dir.path()).await;
    let refused = send(
        app.clone(),
        json_request(
            "POST",
            &keyed("/api/items"),
            r#"{"o:is_public":true,"o:primary_media":{"o:id":1}}"#,
        ),
    )
    .await;
    assert_eq!(
        refused.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        refused.body
    );
    assert_eq!(
        refused.body,
        r#"{"errors":{"o:primary_media":["Primary media must belong to this item."]}}"#
    );
    let accepted = send(
        app,
        json_request(
            "POST",
            &keyed("/api/items"),
            r#"{"o:is_public":true,"o:primary_media":{"o:id":"abc"}}"#,
        ),
    )
    .await;
    assert_eq!(accepted.status, StatusCode::OK, "{}", accepted.body);
    assert_eq!(accepted.json()["o:primary_media"], Value::Null);
}

/// A non-array `o:media` answers the upstream error key, which is `o:item_set`
/// (a copy of the `o:item_set` check in `ItemAdapter::validateRequest`).
#[tokio::test]
async fn a_non_array_o_media_answers_under_o_item_set() {
    let (app, _seed) = seeded_router().await;
    let answer = send(
        app,
        json_request(
            "POST",
            &keyed("/api/items"),
            r#"{"o:is_public":true,"o:media":"x"}"#,
        ),
    )
    .await;
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
    assert_eq!(
        answer.body,
        r#"{"errors":{"o:item_set":["Media must be an array"]}}"#
    );
}

/// `o:renderer` is a media member the hydrator never reads, so a media create
/// carrying it is not refused. A term the catalog does not know still is.
#[tokio::test]
async fn a_media_create_ignores_o_renderer_and_refuses_an_unknown_term() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (app, seed) = router_in(dir.path()).await;
    let body = json!({
        "o:ingester": "html",
        "o:renderer": "html",
        "o:item": {"o:id": seed.item.0},
        "html": "<p>rendered</p>",
    })
    .to_string();
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/media"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    // The renderer comes from the ingester, not the body.
    assert_eq!(created.json()["o:renderer"], "html");

    let refused = send(
        app,
        json_request(
            "POST",
            &keyed("/api/media"),
            &format!(
                r#"{{"o:ingester":"html","o:item":{{"o:id":{}}},"dcterms:nonsense":[]}}"#,
                seed.item.0
            ),
        ),
    )
    .await;
    assert_eq!(
        refused.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        refused.body
    );
    assert_eq!(
        refused.json()["errors"]["dcterms:nonsense"][0],
        "The property \"dcterms:nonsense\" is not recognized."
    );
}

/// An `o:` member no hydrator reads says nothing about the values, on an item,
/// an item set or a media create.
#[tokio::test]
async fn an_unknown_o_member_is_ignored_on_every_resource_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (app, seed) = router_in(dir.path()).await;
    let cases = [
        (
            "/api/items",
            r#"{"o:is_public":true,"o:zzz":{"a":1}}"#.to_string(),
        ),
        (
            "/api/item_sets",
            r#"{"o:is_public":true,"o:zzz":["a"]}"#.to_string(),
        ),
        (
            "/api/media",
            format!(
                r#"{{"o:ingester":"html","o:item":{{"o:id":{}}},"o:zzz":"a","html":"x"}}"#,
                seed.item.0
            ),
        ),
    ];
    for (path, body) in cases {
        let answer = send(app.clone(), json_request("POST", &keyed(path), &body)).await;
        assert_eq!(answer.status, StatusCode::OK, "{path}: {}", answer.body);
    }
}

/// `MediaRepresentation` answers `data` from the stored column through Doctrine
/// 2's `JsonArrayType`, which maps a NULL column to `[]` (`Data/JsonArrayType`,
/// read from the oracle's `media.data` as NULL and answered as `[]`). A media
/// with no payload therefore reads `[]`, and not `null`.
#[tokio::test]
async fn a_media_with_no_payload_reads_an_empty_data_array() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (app, seed) = router_in(dir.path()).await;
    let body = json!({"o:ingester": "html", "o:item": {"o:id": seed.item.0}}).to_string();
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/media"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let document = created.json();
    assert_eq!(document["data"], json!([]), "{}", created.body);
    let id = document["o:id"].as_u64().expect("media id");

    let read = send(app, get(&format!("/api/media/{id}"), true)).await;
    assert_eq!(read.json()["data"], json!([]), "{}", read.body);
}

/// A `url` ingest on an item create must not hold the server's worker while it
/// fetches.
#[test]
fn a_stalled_inline_url_ingest_leaves_the_server_responsive() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runtime = one_worker_runtime();
    let (app, _seed) = runtime.block_on(router_in(dir.path()));
    assert_stays_responsive(&runtime, app, "/api/items", |url| {
        json!({
            "o:is_public": true,
            "o:media": [{"o:ingester": "url", "ingest_url": url}],
        })
        .to_string()
    });
}

/// A `url` ingest on `POST /api/media` runs through the same path, so it must
/// not hold the worker either.
#[test]
fn a_stalled_media_create_leaves_the_server_responsive() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runtime = one_worker_runtime();
    let (app, seed) = runtime.block_on(router_in(dir.path()));
    assert_stays_responsive(&runtime, app, "/api/media", |url| {
        json!({
            "o:ingester": "url",
            "o:item": {"o:id": seed.item.0},
            "ingest_url": url,
        })
        .to_string()
    });
}
