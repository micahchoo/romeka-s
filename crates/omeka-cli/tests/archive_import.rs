//! The archive import end to end: the server's routes over a real socket, an
//! installed database file, a real file store and archive folder, and the
//! `Romeka\Job\ImportArchive` and `Romeka\Job\UndoImport` jobs run by an
//! in-process dispatcher through the same `Runner` `romeka job run` uses.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use omeka_api::jsonld::BaseUrl;
use omeka_cli::commands::install::{InstallOptions, run as install};
use omeka_cli::jobs::{JobEnv, Runner};
use omeka_db::ids::JobId;
use omeka_media::store::FileStore;
use omeka_server::{Config, DispatchError, JobDispatch, Services};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const PNG: &[u8] = include_bytes!("fixtures/tiny.png");
const ORIGIN: &str = "http://localhost";

/// Runs each job on the server's runtime, as `romeka job run` would in a child.
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

struct Site {
    _dir: TempDir,
    addr: SocketAddr,
    cookie: String,
    pool: SqlitePool,
    store: PathBuf,
}

fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

async fn site() -> Site {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("romeka.db");
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
    let archive = dir.path().join("archive");
    write(&archive, "Box/Letters/L_001_front.png", PNG);
    write(&archive, "Box/Letters/L_001_back.png", PNG);
    write(&archive, "Box/Photos/P_001.png", PNG);
    write(
        &archive,
        "Box/register.csv",
        b"id,Title\nL_001,First letter\nP_001,Harbour\n",
    );
    let store = dir.path().join("files");
    let pool = omeka_db::open_file(&database).await.unwrap();
    let base = BaseUrl::parse(ORIGIN).unwrap();
    let config = Config {
        base_url: base.clone(),
        archive_root: Some(archive.clone()),
        ..Config::default()
    };
    let env = JobEnv {
        store: FileStore::new(&store, omeka_server::FILES_URI),
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
    let mut site = Site {
        _dir: dir,
        addr,
        cookie: String::new(),
        pool,
        store,
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

struct Reply {
    status: u16,
    content_type: String,
    set_cookie: Option<String>,
    body: String,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|_| panic!("not JSON: {}", self.body))
    }
}

impl Site {
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
        let header = |name: &str| {
            head.lines()
                .find(|line| line.to_ascii_lowercase().starts_with(&format!("{name}:")))
                .map(|line| line[name.len() + 1..].trim().to_string())
        };
        Reply {
            status: head[9..12].parse().unwrap(),
            content_type: header("content-type").unwrap_or_default(),
            set_cookie: header("set-cookie"),
            body: body.to_string(),
        }
    }

    async fn draft(&self, plan: &Value) -> u64 {
        let created = self
            .send(
                "POST",
                "/api/romeka/imports",
                Some(json!({ "folder": "Box" })),
            )
            .await;
        assert_eq!(created.status, 200, "{}", created.body);
        let id = created.json()["id"].as_u64().unwrap();
        let saved = self
            .send(
                "PUT",
                &format!("/api/romeka/imports/{id}"),
                Some(json!({ "plan": plan })),
            )
            .await;
        assert_eq!(saved.status, 200, "{}", saved.body);
        id
    }

    async fn import(&self, id: u64) -> Value {
        self.send("GET", &format!("/api/romeka/imports/{id}"), None)
            .await
            .json()
    }

    /// Poll the import until its status is `want`, or fail after 30 seconds.
    async fn wait_for(&self, id: u64, want: &str) -> Value {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let import = self.import(id).await;
            if import["status"] == want {
                return import;
            }
            assert!(Instant::now() < deadline, "import {id} stayed {import}");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    async fn count(&self, table: &str) -> i64 {
        sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    fn stored_files(&self) -> usize {
        fn walk(path: &Path) -> usize {
            std::fs::read_dir(path).map_or(0, |entries| {
                entries
                    .flatten()
                    .map(|entry| {
                        let path = entry.path();
                        if path.is_dir() { walk(&path) } else { 1 }
                    })
                    .sum()
            })
        }
        walk(&self.store)
    }
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
async fn a_run_imports_the_folder_and_only_the_last_import_can_be_undone() {
    let site = site().await;
    let items_before = site.count("item").await;
    let first = site.draft(&plan()).await;

    let preview = site
        .send(
            "POST",
            &format!("/api/romeka/imports/{first}/preview"),
            Some(json!({ "include": ["files", "sets", "csv", "rows", "check"] })),
        )
        .await;
    assert_eq!(preview.status, 200, "{}", preview.body);
    let preview = preview.json();
    assert_eq!(preview["files"]["matched"], 3);
    assert_eq!(preview["sets"][0]["items"], 1);
    assert_eq!(preview["csv"]["joined"], 2);
    assert_eq!(preview["rows"]["total"], 2);
    assert_eq!(preview["check"]["create"], 2);
    assert_eq!(preview["check"]["canRun"], true, "{preview}");

    let run = site
        .send("POST", &format!("/api/romeka/imports/{first}/run"), None)
        .await;
    assert_eq!(run.status, 200, "{}", run.body);
    let job = run.json()["jobId"].as_u64().unwrap();
    let done = site.wait_for(first, "done").await;
    assert_eq!(done["undoable"], true);
    assert_eq!(done["summary"]["items"], 2, "{done}");
    assert_eq!(site.count("item").await, items_before + 2);
    assert_eq!(site.count("media").await, 3);
    assert!(
        site.stored_files() >= 3,
        "originals and thumbnails are stored"
    );

    let log = site
        .send("GET", &format!("/api/romeka/jobs/{job}/log"), None)
        .await;
    assert_eq!(log.status, 200);
    assert!(
        log.content_type.starts_with("text/plain"),
        "{}",
        log.content_type
    );
    assert!(log.body.lines().count() >= 3, "{}", log.body);
    assert!(log.body.contains("imported 2 items"), "{}", log.body);

    let again = site
        .send("POST", &format!("/api/romeka/imports/{first}/run"), None)
        .await;
    assert_eq!(again.status, 409, "an import never runs twice");

    // Importing the folder again skips the two items it already made, so
    // the second import has one new letter to create.
    write(
        &site._dir.path().join("archive"),
        "Box/Letters/L_002_front.png",
        PNG,
    );
    let second = site.draft(&plan()).await;
    let run = site
        .send("POST", &format!("/api/romeka/imports/{second}/run"), None)
        .await;
    assert_eq!(run.status, 200, "{}", run.body);
    let done = site.wait_for(second, "done").await;
    assert_eq!(done["summary"]["existing"], 2, "{done}");
    assert_eq!(
        site.import(first).await["undoable"],
        false,
        "the next run closed the first window"
    );
    assert_eq!(site.count("item").await, items_before + 3);

    let closed = site
        .send("POST", &format!("/api/romeka/imports/{first}/undo"), None)
        .await;
    assert_eq!(closed.status, 409, "{}", closed.body);

    let undo = site
        .send("POST", &format!("/api/romeka/imports/{second}/undo"), None)
        .await;
    assert_eq!(undo.status, 200, "{}", undo.body);
    let undo_job = undo.json()["jobId"].as_u64().unwrap();
    let undone = site.wait_for(second, "undone").await;
    assert_eq!(undone["undoable"], false);
    assert_eq!(site.count("item").await, items_before + 2);
    assert_eq!(site.count("media").await, 3);
    let log = site
        .send("GET", &format!("/api/romeka/jobs/{undo_job}/log"), None)
        .await;
    assert!(log.body.contains("undone: 1 items"), "{}", log.body);

    let history = site.send("GET", "/api/romeka/imports", None).await.json();
    let statuses: Vec<&str> = history
        .as_array()
        .unwrap()
        .iter()
        .map(|import| import["status"].as_str().unwrap())
        .collect();
    assert_eq!(statuses, ["undone", "done"], "newest first");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_run_whose_job_died_shows_as_failed_and_can_be_undone() {
    let site = site().await;
    let id = site.draft(&plan()).await;
    let run = site
        .send("POST", &format!("/api/romeka/imports/{id}/run"), None)
        .await;
    let job = run.json()["jobId"].as_u64().unwrap();
    site.wait_for(id, "done").await;
    // A worker killed mid-run leaves the import `running` and its job `error`.
    sqlx::query("UPDATE romeka_import SET status = 'running', finished = NULL WHERE id = ?")
        .bind(id as i64)
        .execute(&site.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE job SET status = 'error' WHERE id = ?")
        .bind(job as i64)
        .execute(&site.pool)
        .await
        .unwrap();
    assert_eq!(site.import(id).await["status"], "failed");

    let undo = site
        .send("POST", &format!("/api/romeka/imports/{id}/undo"), None)
        .await;
    assert_eq!(undo.status, 200, "{}", undo.body);
    site.wait_for(id, "undone").await;
    assert_eq!(site.count("media").await, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_killed_worker_that_stopped_beating_is_recorded_failed_and_undone() {
    let site = site().await;
    let id = site.draft(&plan()).await;
    let run = site
        .send("POST", &format!("/api/romeka/imports/{id}/run"), None)
        .await;
    let job = run.json()["jobId"].as_u64().unwrap();
    site.wait_for(id, "done").await;
    // A worker killed mid-run: its job row still says `in_progress`, and its
    // last heartbeat is ten minutes old.
    sqlx::query(
        "UPDATE romeka_import SET status = 'running', finished = NULL,
         summary = json_set(summary, '$.heartbeat', datetime('now', '-10 minutes'))
         WHERE id = ?",
    )
    .bind(id as i64)
    .execute(&site.pool)
    .await
    .unwrap();
    sqlx::query("UPDATE job SET status = 'in_progress', ended = NULL WHERE id = ?")
        .bind(job as i64)
        .execute(&site.pool)
        .await
        .unwrap();

    let shown = site.import(id).await;
    assert_eq!(shown["status"], "failed", "{shown}");
    assert!(shown["died"].is_string(), "{shown}");
    let undo = site
        .send("POST", &format!("/api/romeka/imports/{id}/undo"), None)
        .await;
    assert_eq!(undo.status, 200, "{}", undo.body);
    let undone = site.wait_for(id, "undone").await;
    assert_eq!(undone["died"], Value::Null, "the undo ended normally");
    assert_eq!(site.count("media").await, 0);
}
