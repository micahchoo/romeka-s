//! The import v2 run tests' shared ground: an installed database file, the
//! server's routes over a real socket, an archive root and a file store in a
//! tempdir, and the import jobs run by an in-process dispatcher through the
//! same `Runner` that `romeka job run` uses.

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::io::Read;
use std::net::SocketAddr;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use omeka_api::import::archive::ArchiveRoots;
use omeka_api::jsonld::BaseUrl;
use omeka_cli::commands::install::{InstallOptions, run as install};
use omeka_cli::jobs::{JobEnv, Runner};
use omeka_db::ids::{ImportId, JobId};
use omeka_db::repos::{
    ImportRepo, ImportResourceRepo, ItemFilter, ItemRepo, ItemSetFilter, ItemSetRepo, MediaFilter,
    MediaRepo, Paging,
};
use omeka_db::rows::{ImportResourceRow, ImportRow, ItemRow, ItemSetRow, MediaRow};
use omeka_media::store::{Access, FileStore, ListedFile, Store, StoreError};
use omeka_server::{Config, DispatchError, JobDispatch, Services};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinHandle;

pub const PNG: &[u8] = include_bytes!("../fixtures/tiny.png");
const ORIGIN: &str = "http://localhost";

/// A real JPEG: the thumbnailer's square derivative of the PNG fixture.
pub fn jpeg() -> Vec<u8> {
    omeka_media::ImageThumbnailer::new()
        .derivative_bytes(PNG, Some("image/png"))
        .unwrap()
        .remove(0)
        .1
}

/// A real 4 by 4 grey TIFF, uncompressed: header, one IFD, then the pixels.
pub fn tiff() -> Vec<u8> {
    let (width, height) = (4u16, 4u16);
    let entries: [(u16, u16, u32); 8] = [
        (256, 3, u32::from(width)),
        (257, 3, u32::from(height)),
        (258, 3, 8),
        (259, 3, 1),
        (262, 3, 1),
        (273, 4, 0),
        (278, 3, u32::from(height)),
        (279, 4, u32::from(width) * u32::from(height)),
    ];
    let data_at = 8 + 2 + entries.len() as u32 * 12 + 4;
    let mut bytes = b"II*\0".to_vec();
    bytes.extend(8u32.to_le_bytes());
    bytes.extend((entries.len() as u16).to_le_bytes());
    for (tag, kind, value) in entries {
        bytes.extend(tag.to_le_bytes());
        bytes.extend(kind.to_le_bytes());
        bytes.extend(1u32.to_le_bytes());
        let value = if tag == 273 { data_at } else { value };
        if kind == 3 {
            bytes.extend((value as u16).to_le_bytes());
            bytes.extend([0, 0]);
        } else {
            bytes.extend(value.to_le_bytes());
        }
    }
    bytes.extend(0u32.to_le_bytes());
    bytes.extend((0..u32::from(width) * u32::from(height)).map(|n| (n * 16) as u8));
    bytes
}

/// A store that can be told to hold the n-th original it is given, as a
/// worker killed in the middle of a file would: the put blocks until the
/// test lets go, then fails, so the held file never lands.
#[derive(Debug)]
pub struct GatedStore {
    inner: Arc<dyn Store>,
    gate: Mutex<Option<Gate>>,
    /// Originals left to pass before one is slowed, and by how much.
    slow: Mutex<Option<(usize, Duration)>>,
}

#[derive(Debug)]
struct Gate {
    left: usize,
    reached: mpsc::Sender<()>,
    release: Arc<Mutex<mpsc::Receiver<()>>>,
}

/// The test's end of a gate.
pub struct GateHandle {
    pub reached: mpsc::Receiver<()>,
    pub release: mpsc::Sender<()>,
}

impl GatedStore {
    /// Hold the original put after `after` have passed.
    pub fn hold_after(&self, after: usize) -> GateHandle {
        let (reached_tx, reached_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        *self.gate.lock().unwrap() = Some(Gate {
            left: after,
            reached: reached_tx,
            release: Arc::new(Mutex::new(release_rx)),
        });
        GateHandle {
            reached: reached_rx,
            release: release_tx,
        }
    }

    /// Slow the original put after `after` have passed by `by`, so the run
    /// spends a measurable time between two records.
    pub fn slow_after(&self, after: usize, by: Duration) {
        *self.slow.lock().unwrap() = Some((after, by));
    }

    fn pass(&self, path: &str) -> Result<(), StoreError> {
        if !path.starts_with("original/") {
            return Ok(());
        }
        let slow = {
            let mut slow = self.slow.lock().unwrap();
            match slow.as_mut() {
                Some((left, _)) if *left > 0 => {
                    *left -= 1;
                    None
                }
                Some(_) => slow.take().map(|(_, by)| by),
                None => None,
            }
        };
        if let Some(by) = slow {
            std::thread::sleep(by);
        }
        let held = {
            let mut gate = self.gate.lock().unwrap();
            match gate.as_mut() {
                Some(open) if open.left > 0 => {
                    open.left -= 1;
                    None
                }
                Some(_) => gate.take(),
                None => None,
            }
        };
        let Some(gate) = held else {
            return Ok(());
        };
        let _ = gate.reached.send(());
        let _ = gate.release.lock().unwrap().recv();
        Err(StoreError::IllegalPath("the worker was killed".into()))
    }
}

impl Store for GatedStore {
    fn put(&self, bytes: &[u8], path: &str, media_type: Option<&str>) -> Result<(), StoreError> {
        self.pass(path)?;
        self.inner.put(bytes, path, media_type)
    }
    fn put_stream(
        &self,
        reader: &mut dyn Read,
        size_hint: Option<u64>,
        path: &str,
        media_type: Option<&str>,
    ) -> Result<u64, StoreError> {
        self.pass(path)?;
        self.inner.put_stream(reader, size_hint, path, media_type)
    }
    fn read(&self, path: &str) -> Result<Vec<u8>, StoreError> {
        self.inner.read(path)
    }
    fn read_range(&self, path: &str, range: Range<u64>) -> Result<Vec<u8>, StoreError> {
        self.inner.read_range(path, range)
    }
    fn delete(&self, path: &str) -> Result<bool, StoreError> {
        self.inner.delete(path)
    }
    fn exists(&self, path: &str) -> Result<bool, StoreError> {
        self.inner.exists(path)
    }
    fn size(&self, path: &str) -> Result<Option<u64>, StoreError> {
        self.inner.size(path)
    }
    fn list(&self, prefix: &str) -> Result<Vec<ListedFile>, StoreError> {
        self.inner.list(prefix)
    }
    fn url(&self, path: &str, access: Access) -> String {
        self.inner.url(path, access)
    }
    fn url_ttl(&self) -> Duration {
        self.inner.url_ttl()
    }
}

/// Runs each job on the test's runtime, as `romeka job run` would in a
/// child, and keeps the task so a test can kill the worker.
pub struct InProcess {
    pool: omeka_db::DbPool,
    env: JobEnv,
    pub tasks: Mutex<Vec<JoinHandle<()>>>,
}

impl JobDispatch for InProcess {
    fn start(&self, job: JobId) -> Result<(), DispatchError> {
        let (pool, env) = (self.pool.clone(), self.env.clone());
        let task = tokio::spawn(async move {
            let _ = Runner::with_env(&pool, env).run(job).await;
        });
        self.tasks.lock().unwrap().push(task);
        Ok(())
    }
}

pub struct Site {
    pub dir: TempDir,
    pub addr: SocketAddr,
    cookie: String,
    pub pool: SqlitePool,
    pub archive: PathBuf,
    pub files: PathBuf,
    pub store: Arc<GatedStore>,
    pub jobs: Arc<InProcess>,
}

pub struct Reply {
    pub status: u16,
    pub set_cookie: Option<String>,
    pub body: String,
}

impl Reply {
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|_| panic!("not JSON: {}", self.body))
    }
}

pub fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// A site whose store is a local folder at `files` in the tempdir.
pub async fn site() -> Site {
    site_with(|files| Arc::new(FileStore::new(files, omeka_server::FILES_URI))).await
}

/// A site whose store is the one `store` makes; it is handed the local
/// `files` folder, which an object store leaves empty.
pub async fn site_with(store: impl FnOnce(&Path) -> Arc<dyn Store>) -> Site {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("romeka.db");
    install(&InstallOptions {
        database: database.clone().into(),
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
    std::fs::create_dir_all(&archive).unwrap();
    let archive = archive.canonicalize().unwrap();
    let files = dir.path().join("files");
    let pool = omeka_db::open_file(&database).await.unwrap();
    let base = BaseUrl::parse(ORIGIN).unwrap();
    let roots = ArchiveRoots::new([("archive".to_string(), archive.clone())]).unwrap();
    let config = Config {
        base_url: base.clone(),
        archive_roots: roots.clone(),
        ..Config::default()
    };
    let store = Arc::new(GatedStore {
        inner: store(&files),
        gate: Mutex::new(None),
        slow: Mutex::new(None),
    });
    let env = JobEnv {
        store: store.clone(),
        base_url: base,
        archive_roots: roots,
    };
    let jobs = Arc::new(InProcess {
        pool: pool.clone().into(),
        env: env.clone(),
        tasks: Mutex::new(Vec::new()),
    });
    let services = Services::new(env.store.clone()).with_jobs(jobs.clone());
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
        dir,
        addr,
        cookie: String::new(),
        pool,
        archive,
        files,
        store,
        jobs,
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

impl Site {
    pub async fn send(&self, method: &str, path: &str, body: Option<Value>) -> Reply {
        let body = body.map(|body| body.to_string()).unwrap_or_default();
        self.send_raw(method, path, "application/json", body.as_bytes())
            .await
    }

    /// Uploads `bytes` as the one file of a multipart form to the import's
    /// `POST …/sheets`.
    pub async fn upload_list(&self, id: u64, name: &str, bytes: &[u8]) -> Reply {
        const BOUNDARY: &str = "romeka-list-boundary";
        let mut body = format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: text/csv\r\n\r\n"
        )
        .into_bytes();
        body.extend_from_slice(bytes);
        body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
        self.send_raw(
            "POST",
            &format!("/api/romeka/imports/{id}/sheets"),
            &format!("multipart/form-data; boundary={BOUNDARY}"),
            &body,
        )
        .await
    }

    async fn send_raw(&self, method: &str, path: &str, content_type: &str, body: &[u8]) -> Reply {
        let mut request = format!(
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nOrigin: {ORIGIN}\r\nConnection: close\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n",
            body.len()
        );
        if !self.cookie.is_empty() {
            request.push_str(&format!("Cookie: {}\r\n", self.cookie));
        }
        request.push_str("\r\n");
        let mut request = request.into_bytes();
        request.extend_from_slice(body);
        let mut stream = tokio::net::TcpStream::connect(self.addr).await.unwrap();
        stream.write_all(&request).await.unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).await.unwrap();
        let raw = String::from_utf8_lossy(&raw).into_owned();
        let (head, body) = raw.split_once("\r\n\r\n").expect("a head");
        let header = |name: &str| {
            head.lines()
                .find(|line| line.to_ascii_lowercase().starts_with(&format!("{name}:")))
                .map(|line| line[name.len() + 1..].trim().to_string())
        };
        let chunked = header("transfer-encoding").is_some_and(|value| value == "chunked");
        Reply {
            status: head[9..12].parse().unwrap(),
            set_cookie: header("set-cookie"),
            body: if chunked {
                unchunk(body)
            } else {
                body.to_string()
            },
        }
    }

    pub fn write(&self, relative: &str, bytes: &[u8]) {
        write(&self.archive, relative, bytes);
    }

    /// Write the file with its path after its bytes. Files of distinct
    /// objects never share their bytes, and the derive keeps only one of
    /// identical files (`anomalies.identical: keepOne`).
    pub fn write_object(&self, relative: &str, bytes: &[u8]) {
        self.write(relative, &[bytes, relative.as_bytes()].concat());
    }

    /// A draft on the whole archive root with `edit` applied to its plan,
    /// saved and scanned. Answers the import id.
    pub async fn draft(&self, edit: impl FnOnce(&mut Value)) -> u64 {
        let made = self
            .send(
                "POST",
                "/api/romeka/imports",
                Some(json!({ "sources": [{ "kind": "folder", "id": "s1", "root": "archive", "path": "" }] })),
            )
            .await;
        assert_eq!(made.status, 201, "{}", made.body);
        let draft = made.json();
        let id = draft["id"].as_u64().unwrap();
        let mut plan = draft["plan"].clone();
        edit(&mut plan);
        let saved = self
            .send(
                "PUT",
                &format!("/api/romeka/imports/{id}"),
                Some(json!({ "plan": plan, "revision": draft["revision"] })),
            )
            .await;
        assert_eq!(saved.status, 200, "{}", saved.body);
        let scanned = self
            .send("POST", &format!("/api/romeka/imports/{id}/scan"), None)
            .await;
        assert_eq!(scanned.status, 200, "{}", scanned.body);
        id
    }

    pub async fn check(&self, id: u64) -> Value {
        let preview = self
            .send(
                "POST",
                &format!("/api/romeka/imports/{id}/preview"),
                Some(json!({ "include": ["check"] })),
            )
            .await;
        assert_eq!(preview.status, 200, "{}", preview.body);
        preview.json()["check"].clone()
    }

    /// Check, then press Run with the digest Check showed. Answers the job.
    pub async fn start(&self, id: u64) -> u64 {
        let check = self.check(id).await;
        assert!(check["canRun"].as_bool().unwrap(), "{check}");
        let started = self
            .send(
                "POST",
                &format!("/api/romeka/imports/{id}/run"),
                Some(json!({ "digest": check["digest"] })),
            )
            .await;
        assert_eq!(started.status, 200, "{}", started.body);
        started.json()["jobId"].as_u64().unwrap()
    }

    /// Run and wait for the import to end.
    pub async fn run(&self, id: u64) -> ImportRow {
        self.start(id).await;
        self.ended(id).await
    }

    pub async fn row(&self, id: u64) -> ImportRow {
        ImportRepo::new(&self.pool)
            .get(ImportId(id))
            .await
            .unwrap()
            .unwrap()
    }

    /// Poll the import until it is no longer running, or fail after 60 s.
    pub async fn ended(&self, id: u64) -> ImportRow {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let row = self.row(id).await;
            if row.status.as_str() != "running" {
                return row;
            }
            assert!(Instant::now() < deadline, "import {id} stayed running");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    pub async fn ledger(&self, id: u64) -> Vec<ImportResourceRow> {
        ImportResourceRepo::new(&self.pool)
            .list(ImportId(id), None)
            .await
            .unwrap()
    }

    pub async fn items(&self) -> Vec<ItemRow> {
        ItemRepo::new(&self.pool)
            .list(&ItemFilter::default(), &Paging::default())
            .await
            .unwrap()
    }

    pub async fn item_sets(&self) -> Vec<ItemSetRow> {
        ItemSetRepo::new(&self.pool)
            .list(&ItemSetFilter::default(), &Paging::default())
            .await
            .unwrap()
    }

    pub async fn media(&self) -> Vec<MediaRow> {
        MediaRepo::new(&self.pool)
            .list(&MediaFilter::default(), &Paging::default())
            .await
            .unwrap()
    }

    /// The item whose title is `title`.
    pub async fn item(&self, title: &str) -> ItemRow {
        self.items()
            .await
            .into_iter()
            .find(|item| item.resource.title.as_deref() == Some(title))
            .unwrap_or_else(|| panic!("no item titled {title}"))
    }

    /// An item's media, by position.
    pub async fn media_of(&self, item: &ItemRow) -> Vec<MediaRow> {
        let mut media: Vec<MediaRow> = self
            .media()
            .await
            .into_iter()
            .filter(|media| media.item_id == item.id())
            .collect();
        media.sort_by_key(|media| media.position);
        media
    }

    /// Every file under the store's root, by path relative to it.
    pub fn stored_files(&self) -> Vec<String> {
        fn walk(root: &Path, path: &Path, out: &mut Vec<String>) {
            for entry in std::fs::read_dir(path).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(root, &path, out);
                } else {
                    out.push(path.strip_prefix(root).unwrap().display().to_string());
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.files, &self.files, &mut out);
        out.sort();
        out
    }
}

fn unchunk(body: &str) -> String {
    let mut out = String::new();
    let mut rest = body;
    while let Some((size, after)) = rest.split_once("\r\n") {
        let Ok(size) = usize::from_str_radix(size.trim(), 16) else {
            break;
        };
        if size == 0 {
            break;
        }
        out.push_str(&after[..size]);
        rest = &after[size + 2..];
    }
    out
}

/// One sheet column plan, literal values.
pub fn sheet_plan(id: &str, headers: &[&str]) -> Value {
    let columns: Vec<Value> = headers
        .iter()
        .enumerate()
        .map(|(index, header)| {
            json!({
                "column": { "index": index, "header": header },
                "options": { "split": null, "type": "literal", "lang": null, "private": false }
            })
        })
        .collect();
    json!({
        "id": id, "source": "s1", "file": format!("{id}.csv"), "hash": null,
        "read": { "encoding": "utf-8", "delimiter": ",", "quote": "\"", "headerLine": 1, "trim": false },
        "rowKind": "item", "join": null, "itemKey": null, "columns": columns
    })
}

pub fn col(sheet: &str, index: u32, header: &str) -> Value {
    json!({ "from": "column", "sheet": sheet, "column": { "index": index, "header": header } })
}

pub fn rule(sources: Value, combine: &str) -> Value {
    json!({ "sources": sources, "combine": combine })
}
