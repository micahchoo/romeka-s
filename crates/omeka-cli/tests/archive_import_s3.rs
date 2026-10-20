//! The archive import on an S3 store: a run through the server's routes and
//! the job runner puts every original and thumbnail in the bucket and nothing
//! on the local disk, the API names them with signed and plain URLs, and an
//! undo removes them again.
//!
//! Gated on `ROMEKA_S3_TEST_ENDPOINT`, as `omeka-media`'s S3 suite is
//! (`crates/omeka-media/tests/s3_fixture/mod.rs` shows the MinIO command);
//! `ROMEKA_S3_TEST_ACCESS_KEY` and `ROMEKA_S3_TEST_SECRET_KEY` default to
//! `minioadmin`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use omeka_api::jsonld::BaseUrl;
use omeka_cli::commands::install::{InstallOptions, run as install};
use omeka_cli::jobs::{JobEnv, Runner};
use omeka_db::ids::JobId;
use omeka_media::store::{S3Config, S3Store, Store};
use omeka_server::{Config, DispatchError, JobDispatch, Services};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const PNG: &[u8] = include_bytes!("fixtures/tiny.png");
const ORIGIN: &str = "http://localhost";

struct InProcess {
    pool: SqlitePool,
    env: JobEnv,
}

impl JobDispatch for InProcess {
    fn start(&self, job: JobId) -> Result<(), DispatchError> {
        let (pool, env) = (self.pool.clone(), self.env.clone());
        tokio::spawn(async move {
            let _ = Runner::with_env(&pool, env).run(job).await;
        });
        Ok(())
    }
}

fn s3_store() -> Option<Arc<S3Store>> {
    let Ok(endpoint) = std::env::var("ROMEKA_S3_TEST_ENDPOINT") else {
        eprintln!("SKIP: ROMEKA_S3_TEST_ENDPOINT is not set, so the S3 import run did not run");
        return None;
    };
    let key = |name: &str| std::env::var(name).unwrap_or_else(|_| "minioadmin".to_string());
    let bucket = format!(
        "romeka-import-{}",
        &omeka_media::store::new_storage_id().unwrap()[..16]
    );
    let store = S3Store::new(S3Config {
        endpoint: endpoint.parse().unwrap(),
        bucket,
        region: "us-east-1".to_string(),
        access_key: key("ROMEKA_S3_TEST_ACCESS_KEY"),
        secret_key: key("ROMEKA_S3_TEST_SECRET_KEY"),
        public_base: None,
        url_ttl: Duration::from_secs(900),
    })
    .unwrap();
    store.create_bucket().unwrap();
    Some(Arc::new(store))
}

fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

struct Site {
    addr: SocketAddr,
    cookie: String,
    pool: SqlitePool,
}

struct Reply {
    status: u16,
    set_cookie: Option<String>,
    body: String,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|_| panic!("not JSON: {}", self.body))
    }
}

impl Site {
    async fn start(dir: &Path, store: Arc<dyn Store>) -> Self {
        let database = dir.join("romeka.db");
        install(&InstallOptions {
            database: database.clone(),
            email: "admin@example.org".to_string(),
            name: "Admin".to_string(),
            password: "a long test password".to_string(),
            title: "Test".to_string(),
            time_zone: "UTC".to_string(),
            locale: "en".to_string(),
        })
        .await
        .expect("install");
        let archive = dir.join("archive");
        write(&archive, "Box/Letters/L_001_front.png", PNG);
        write(&archive, "Box/Letters/L_001_back.png", PNG);
        write(&archive, "Box/Photos/P_001.png", PNG);
        write(
            &archive,
            "Box/register.csv",
            b"id,Title\nL_001,First letter\nP_001,Harbour\n",
        );
        let pool = omeka_db::open_file(&database).await.unwrap();
        let base = BaseUrl::parse(ORIGIN).unwrap();
        let config = Config {
            base_url: base.clone(),
            archive_root: Some(archive.clone()),
            ..Config::default()
        };
        let env = JobEnv {
            store,
            base_url: base,
            archive_root: Some(archive),
        };
        let jobs = InProcess {
            pool: pool.clone(),
            env: env.clone(),
        };
        let services = Services::new(env.store.clone()).with_jobs(Arc::new(jobs));
        let app = omeka_server::build_router_with_services(pool.clone(), &config, services);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await;
        });
        let mut site = Self {
            addr,
            cookie: String::new(),
            pool,
        };
        let login = site
            .send(
                "POST",
                "/api/login",
                Some(json!({ "email": "admin@example.org", "password": "a long test password" })),
            )
            .await;
        assert_eq!(login.status, 200, "{}", login.body);
        site.cookie = login
            .set_cookie
            .expect("a session cookie")
            .split(';')
            .next()
            .unwrap()
            .to_string();
        site
    }

    async fn send(&self, method: &str, path: &str, body: Option<Value>) -> Reply {
        let body = body.map(|body| body.to_string()).unwrap_or_default();
        let mut request = format!(
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nOrigin: {ORIGIN}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        );
        if !self.cookie.is_empty() {
            request.push_str(&format!("Cookie: {}\r\n", self.cookie));
        }
        request.push_str("\r\n");
        request.push_str(&body);
        let mut stream = tokio::net::TcpStream::connect(self.addr).await.unwrap();
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut raw = String::new();
        stream.read_to_string(&mut raw).await.unwrap();
        let (head, body) = raw.split_once("\r\n\r\n").expect("a head");
        let set_cookie = head
            .lines()
            .find(|line| line.to_ascii_lowercase().starts_with("set-cookie:"))
            .map(|line| line["set-cookie:".len()..].trim().to_string());
        Reply {
            status: head[9..12].parse().unwrap(),
            set_cookie,
            body: body.to_string(),
        }
    }

    async fn wait_for(&self, id: u64, want: &str) -> Value {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let import = self
                .send("GET", &format!("/api/romeka/imports/{id}"), None)
                .await
                .json();
            if import["status"] == want {
                return import;
            }
            assert!(Instant::now() < deadline, "import {id} stayed {import}");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Each media row's original and, when it has them, its three thumbnails.
    async fn expected_files(&self) -> Vec<String> {
        let rows: Vec<(String, Option<String>, bool, bool)> =
            sqlx::query_as("SELECT storage_id, extension, has_original, has_thumbnails FROM media")
                .fetch_all(&self.pool)
                .await
                .unwrap();
        let mut files = Vec::new();
        for (id, extension, has_original, has_thumbnails) in rows {
            if has_original {
                files.push(format!("original/{id}.{}", extension.unwrap()));
            }
            if has_thumbnails {
                for kind in ["large", "medium", "square"] {
                    files.push(format!("{kind}/{id}.jpg"));
                }
            }
        }
        files.sort();
        files
    }
}

fn listed(store: &dyn Store) -> Vec<String> {
    let mut files: Vec<String> = ["original", "large", "medium", "square"]
        .into_iter()
        .flat_map(|prefix| store.list(prefix).unwrap())
        .map(|file| file.storage_path)
        .collect();
    files.sort();
    files
}

fn plan() -> Value {
    json!({
        "pattern": r"^(?<set>[^/]+)/(?<identifier>[A-Z]_\d{3})(?:_(?<part>[a-z]+))?\.png$",
        "example": null, "setLevel": "level1", "sets": {}, "typeFor": {},
        "csv": { "file": "register.csv", "delimiter": ",", "headerRow": 1, "windows1252": false,
                 "trim": false, "joinColumn": "id", "columns": { "Title": "dcterms:title" } },
        "meta": [], "priority": "csv", "edits": {}, "skip": [], "withoutFiles": [], "csvRow": {},
        "problemRows": "leave"
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn a_run_stores_originals_and_thumbnails_in_the_bucket_and_an_undo_removes_them() {
    let Some(store) = s3_store() else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let site = Site::start(dir.path(), store.clone()).await;

    let created = site
        .send(
            "POST",
            "/api/romeka/imports",
            Some(json!({ "folder": "Box" })),
        )
        .await;
    assert_eq!(created.status, 200, "{}", created.body);
    let id = created.json()["id"].as_u64().unwrap();
    let saved = site
        .send(
            "PUT",
            &format!("/api/romeka/imports/{id}"),
            Some(json!({ "plan": plan() })),
        )
        .await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let run = site
        .send("POST", &format!("/api/romeka/imports/{id}/run"), None)
        .await;
    assert_eq!(run.status, 200, "{}", run.body);
    let done = site.wait_for(id, "done").await;
    assert_eq!(done["summary"]["items"], 2, "{done}");

    let expected = site.expected_files().await;
    assert_eq!(
        expected.len(),
        12,
        "3 originals and 9 thumbnails: {expected:?}"
    );
    assert_eq!(
        listed(store.as_ref()),
        expected,
        "the bucket holds them all"
    );
    assert!(
        !dir.path().join("files").exists(),
        "nothing is written to the local store"
    );
    let original = expected
        .iter()
        .find(|path| path.starts_with("original/"))
        .unwrap();
    assert_eq!(
        store.read(original).unwrap(),
        PNG,
        "the original is byte for byte"
    );

    let (media_id, public): (i64, bool) = sqlx::query_as(
        "SELECT media.id, m.is_public AND i.is_public FROM media \
         JOIN resource m ON m.id = media.id JOIN resource i ON i.id = media.item_id \
         ORDER BY media.id LIMIT 1",
    )
    .fetch_one(&site.pool)
    .await
    .unwrap();
    let media = site
        .send("GET", &format!("/api/media/{media_id}"), None)
        .await;
    assert_eq!(media.status, 200, "{}", media.body);
    let media = media.json();
    let url = media["o:original_url"].as_str().unwrap();
    assert!(
        url.contains("X-Amz-Signature="),
        "an original is signed: {url}"
    );
    let square = media["o:thumbnail_urls"]["square"].as_str().unwrap();
    assert_eq!(
        square.contains("X-Amz-Signature="),
        !public,
        "a thumbnail is signed exactly when its media or item is private: {square}"
    );

    let undo = site
        .send("POST", &format!("/api/romeka/imports/{id}/undo"), None)
        .await;
    assert_eq!(undo.status, 200, "{}", undo.body);
    site.wait_for(id, "undone").await;
    assert!(
        listed(store.as_ref()).is_empty(),
        "the undo deletes every file"
    );
}
