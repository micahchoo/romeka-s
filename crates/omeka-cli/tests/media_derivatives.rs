//! The derivative queue end to end: a media created through `/api/media`
//! answers `pending`, the worker makes its preview, and the document answers
//! `ready`.
//!
//! The server under test is the real router with a dispatcher that records the
//! composed job instead of starting a process, so a test can assert both that
//! the request queued work and what the worker then did with it. The import
//! that holds the queue and the CLI backfill are exercised through the same
//! interfaces their callers use.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use omeka_api::derivatives::DERIVATIVES_JOB;
use omeka_cli::commands::derivatives::{self, DerivativeOptions};
use omeka_cli::commands::install::{InstallOptions, run as install};
use omeka_cli::jobs::{ImportSweep, Job, JobEnv, Runner, compose_job};
use omeka_db::DbPool;
use omeka_db::ids::{ItemId, JobId, MediaId, UserId};
use omeka_db::repos::{
    ApiKeyRepo, ImportRepo, ItemRepo, JobFilter, JobRepo, MediaDerivativeRepo, MediaRepo, Paging,
};
use omeka_db::rows::{
    DerivativeKind, DerivativeState, NewApiKey, NewImport, NewItem, NewJob, NewMedia, NewResource,
};
use omeka_db::with_db;
use omeka_media::derivative::{DerivativeWork, PreviewSource, make_preview};
use omeka_media::store::{
    Access, FileStore, ListedFile, Store, StoreError, StorePrefix, storage_path,
};
use omeka_media::tool::Tools;
use omeka_server::{Config, DispatchError, JobDispatch, Services};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

const IDENTITY: &str = "0123456789abcdef0123456789abcdef";
const CREDENTIAL: &str = "fedcba9876543210fedcba9876543210";

/// The real MP4 the corpus holds.
fn video() -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/media-kinds/video.mp4");
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// A real PNG, for the type that needs no tool.
fn png() -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny.png");
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// Records each composed job instead of starting a process.
struct Recorder {
    jobs: Mutex<Vec<JobId>>,
}

impl Recorder {
    fn new() -> Self {
        Self {
            jobs: Mutex::new(Vec::new()),
        }
    }

    fn taken(&self) -> Vec<JobId> {
        self.jobs.lock().unwrap().clone()
    }
}

impl JobDispatch for Recorder {
    fn start(&self, job: JobId) -> Result<(), DispatchError> {
        self.jobs.lock().unwrap().push(job);
        Ok(())
    }
}

/// A store the worker can only read through the `Store` trait, as an object
/// store is: no local path, and a record of whether the bytes moved in ranges.
#[derive(Debug)]
struct CountingStore {
    inner: FileStore,
    ranges: Mutex<usize>,
}

impl Store for CountingStore {
    fn put(&self, bytes: &[u8], path: &str, media_type: Option<&str>) -> Result<(), StoreError> {
        self.inner.put(bytes, path, media_type)
    }
    fn put_stream(
        &self,
        reader: &mut dyn std::io::Read,
        size_hint: Option<u64>,
        path: &str,
        media_type: Option<&str>,
    ) -> Result<u64, StoreError> {
        self.inner.put_stream(reader, size_hint, path, media_type)
    }
    fn read(&self, path: &str) -> Result<Vec<u8>, StoreError> {
        self.inner.read(path)
    }
    fn read_range(&self, path: &str, range: Range<u64>) -> Result<Vec<u8>, StoreError> {
        *self.ranges.lock().unwrap() += 1;
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

struct Harness {
    _dir: TempDir,
    pool: DbPool,
    store: Arc<dyn Store>,
    app: Router,
    recorder: Arc<Recorder>,
    user: UserId,
    item: ItemId,
}

async fn harness() -> Harness {
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
    let pool = DbPool::from(omeka_db::open_file(&database).await.expect("pool"));
    let files = dir.path().join("files");
    let store: Arc<dyn Store> = Arc::new(FileStore::new(&files, omeka_server::FILES_URI));
    let user = with_db!(&pool, |db| omeka_db::repos::UserRepo::new(db)
        .get_by_email("admin@example.org")
        .await)
    .expect("user")
    .expect("the installed admin")
    .id;
    with_db!(&pool, |db| ApiKeyRepo::new(db)
        .create(&NewApiKey {
            id: IDENTITY.to_string(),
            owner_id: user,
            label: "Test key".to_string(),
            credential_hash: bcrypt::hash(CREDENTIAL, 4).unwrap(),
            last_ip: None,
            last_accessed: None,
            created: None,
        })
        .await)
    .expect("api key");
    let item = with_db!(&pool, |db| ItemRepo::new(db)
        .create(&NewItem::default())
        .await)
    .expect("item");
    let recorder = Arc::new(Recorder::new());
    let config = Config {
        base_url: omeka_api::jsonld::BaseUrl::parse("http://localhost").unwrap(),
        ..Config::default()
    };
    let services = Services::new(Arc::clone(&store)).with_jobs(recorder.clone());
    let app = omeka_server::build_router_with_services(pool.clone(), &config, services);
    Harness {
        _dir: dir,
        pool,
        store,
        app,
        recorder,
        user,
        item: item.id(),
    }
}

fn credentials() -> String {
    format!("key_identity={IDENTITY}&key_credential={CREDENTIAL}")
}

/// One multipart `POST /api/media` with the payload in `data` and one upload.
fn multipart(filename: &str, bytes: &[u8], data: &str) -> (String, Vec<u8>) {
    const BOUNDARY: &str = "romeka-derivative-boundary";
    let mut body =
        format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"data\"\r\n\r\n{data}\r\n")
            .into_bytes();
    body.extend_from_slice(
        format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={BOUNDARY}"), body)
}

async fn send(
    app: &Router,
    method: Method,
    uri: String,
    content_type: Option<&str>,
    body: Vec<u8>,
) -> (StatusCode, String) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(content_type) = content_type {
        builder = builder.header("content-type", content_type);
    }
    let request = builder.body(Body::from(body)).unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// Create one media through the route, as the admin's file panel does.
async fn create_media(harness: &Harness, filename: &str, bytes: &[u8]) -> Value {
    let data = json!({
        "o:ingester": "upload",
        "o:item": {"o:id": harness.item.0},
        "file_index": 0,
    });
    let (content_type, body) = multipart(filename, bytes, &data.to_string());
    let (status, answer) = send(
        &harness.app,
        Method::POST,
        format!("/api/media?{}", credentials()),
        Some(&content_type),
        body,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    serde_json::from_str(&answer).expect("a media document")
}

async fn read_media(harness: &Harness, id: u64) -> Value {
    let (status, answer) = send(
        &harness.app,
        Method::GET,
        format!("/api/media/{id}?{}", credentials()),
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    serde_json::from_str(&answer).expect("a media document")
}

fn preview(document: &Value) -> &Value {
    &document["o-module-romeka:preview"]
}

/// A media row inserted directly, for a state the create route cannot make.
async fn insert_media(
    harness: &Harness,
    media_type: &str,
    extension: &str,
    bytes: &[u8],
) -> (MediaId, String) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let storage_id = format!("direct-{}", NEXT.fetch_add(1, Ordering::Relaxed));
    let path = storage_path(StorePrefix::Original, &storage_id, Some(extension));
    harness
        .store
        .put(bytes, &path, Some(media_type))
        .expect("store");
    let id = with_db!(&harness.pool, |db| MediaRepo::new(db)
        .create(&NewMedia {
            resource: NewResource::default(),
            item_id: harness.item,
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            data: None,
            source: Some(format!("direct.{extension}")),
            media_type: Some(media_type.to_string()),
            storage_id: Some(storage_id.clone()),
            extension: Some(extension.to_string()),
            sha256: Some(String::new()),
            size: Some(bytes.len() as i64),
            has_original: true,
            has_thumbnails: false,
            position: None,
            lang: None,
            alt_text: None,
        })
        .await)
    .expect("media row")
    .id();
    (id, storage_id)
}

fn job_env(harness: &Harness) -> JobEnv {
    JobEnv {
        store: Arc::clone(&harness.store),
        base_url: omeka_api::jsonld::BaseUrl::parse("http://localhost").unwrap(),
        archive_roots: Default::default(),
        jobs: None,
    }
}

/// The preview a document answers, as a word, for a readable assertion.
fn state_of(document: &Value) -> &str {
    preview(document)["state"].as_str().unwrap_or("missing")
}

#[tokio::test(flavor = "multi_thread")]
async fn an_api_created_video_answers_pending_then_ready_after_the_worker_runs() {
    let harness = harness().await;
    let document = create_media(&harness, "reel.mp4", &video()).await;
    assert_eq!(state_of(&document), "pending", "{document}");
    assert_eq!(document["o:thumbnail_urls"]["large"], Value::Null);

    // The request queued one worker, and the row names the poster work. The
    // second row is the access decision, and the worker's queue does not
    // include it.
    let jobs = harness.recorder.taken();
    assert_eq!(jobs.len(), 1, "one worker for one create");
    let id = document["o:id"].as_u64().expect("id");
    let rows = with_db!(&harness.pool, |db| MediaDerivativeRepo::new(db)
        .for_media(MediaId(id))
        .await)
    .expect("rows");
    assert_eq!(rows.len(), 2, "{rows:?}");
    let poster = rows
        .iter()
        .find(|row| row.kind == DerivativeKind::Poster)
        .expect("the poster row");
    assert_eq!(poster.state, DerivativeState::Pending);
    let access = rows
        .iter()
        .find(|row| row.kind == DerivativeKind::AccessCopy)
        .expect("the access row");
    if Tools::on_path().ffprobe().is_some() {
        assert_eq!(
            access.state,
            DerivativeState::None,
            "a browser plays this MP4"
        );
        assert_eq!(
            access.reason.as_deref(),
            Some("a browser shows the original")
        );
    }

    Runner::with_env(&harness.pool, job_env(&harness))
        .run(jobs[0])
        .await
        .expect("the worker ran");

    let ready = read_media(&harness, id).await;
    assert_eq!(state_of(&ready), "ready", "{ready}");
    assert!(ready["o:thumbnail_urls"]["large"].is_string(), "{ready}");
    let media = with_db!(&harness.pool, |db| MediaRepo::new(db)
        .get(MediaId(id))
        .await)
    .expect("media")
    .expect("the row");
    assert!(media.has_thumbnails, "the flag follows the files");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_png_create_records_that_the_original_stands() {
    let harness = harness().await;
    let document = create_media(&harness, "tiny.png", &png()).await;
    assert_eq!(state_of(&document), "pending", "{document}");
    let id = document["o:id"].as_u64().expect("id");
    let access = with_db!(&harness.pool, |db| MediaDerivativeRepo::new(db)
        .get(MediaId(id), DerivativeKind::AccessCopy)
        .await)
    .expect("row")
    .expect("the access row");
    assert_eq!(access.state, DerivativeState::None);
    assert_eq!(
        access.reason.as_deref(),
        Some("a browser shows the original")
    );
    assert!(access.tool.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_raw_create_records_the_copy_it_needs_and_the_tool() {
    let harness = harness().await;
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/media-kinds/image.nef");
    let bytes = std::fs::read(&path).expect("the NEF fixture");
    let document = create_media(&harness, "photo.nef", &bytes).await;
    let id = document["o:id"].as_u64().expect("id");
    let access = with_db!(&harness.pool, |db| MediaDerivativeRepo::new(db)
        .get(MediaId(id), DerivativeKind::AccessCopy)
        .await)
    .expect("row")
    .expect("the access row");
    let tools = Tools::on_path();
    match (tools.exiftool(), tools.libraw()) {
        (Some(_), _) => {
            assert_eq!(access.state, DerivativeState::Pending);
            assert_eq!(access.tool.as_deref(), Some("exiftool"));
        }
        (None, Some(_)) => {
            assert_eq!(access.state, DerivativeState::Pending);
            assert_eq!(access.tool.as_deref(), Some("dcraw_emu"));
        }
        (None, None) => {
            assert_eq!(access.state, DerivativeState::None);
            assert_eq!(
                access.reason.as_deref(),
                Some("needs exiftool, not installed")
            );
        }
    }
    assert!(access.finished.is_some() || access.state == DerivativeState::Pending);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tool_that_fails_settles_failed_and_the_backfill_retries_it() {
    let harness = harness().await;
    let (media, storage_id) =
        insert_media(&harness, "video/mp4", "mp4", b"this is not a video").await;
    omeka_api::derivatives::enqueue(&harness.pool, &harness.store, &{
        with_db!(&harness.pool, |db| MediaRepo::new(db).get(media).await)
            .expect("media")
            .expect("the row")
    })
    .await
    .expect("queue");
    let job = with_db!(&harness.pool, |db| JobRepo::new(db)
        .create(&omeka_db::rows::NewJob {
            owner_id: None,
            pid: None,
            status: Some("starting".to_string()),
            class: DERIVATIVES_JOB.to_string(),
            args: Some(json!({})),
            log: None,
            started: None,
        })
        .await)
    .expect("job");
    Runner::with_env(&harness.pool, job_env(&harness))
        .run(job.id)
        .await
        .expect("the worker ran");
    let row = with_db!(&harness.pool, |db| MediaDerivativeRepo::new(db)
        .get(media, DerivativeKind::Poster)
        .await)
    .expect("row")
    .expect("the row");
    assert_eq!(row.state, DerivativeState::Failed);
    assert!(
        row.reason.as_deref().unwrap_or_default().contains("ffmpeg"),
        "{row:?}"
    );
    assert_eq!(row.attempts, 1);

    // The retry path: replace the broken original with a real one and run the
    // backfill with `--retry`.
    let path = storage_path(StorePrefix::Original, &storage_id, Some("mp4"));
    harness
        .store
        .put(&video(), &path, Some("video/mp4"))
        .expect("store");
    let database = PathBuf::from(&harness._dir.path().join("romeka.db"));
    let report = derivatives::run(&DerivativeOptions {
        database: database.into(),
        store: Some(harness._dir.path().join("files")),
        retry: true,
        media: Some(media.0),
    })
    .await
    .expect("the backfill ran");
    assert_eq!(report.retried, 1, "{report:?}");
    assert_eq!(report.rows.len(), 2, "{report:?}");
    let preview = report
        .rows
        .iter()
        .find(|row| row.kind.is_preview())
        .expect("the poster row");
    assert_eq!(preview.state, DerivativeState::Ready, "{report:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_running_import_holds_the_worker_off_the_queue() {
    let harness = harness().await;
    let (media, _) = insert_media(&harness, "image/png", "png", &png()).await;
    let row = with_db!(&harness.pool, |db| MediaRepo::new(db).get(media).await)
        .expect("media")
        .expect("the row");
    omeka_api::derivatives::enqueue(&harness.pool, &harness.store, &row)
        .await
        .expect("queue");
    let import = with_db!(&harness.pool, |db| ImportRepo::new(db)
        .create_draft(&NewImport {
            owner_id: Some(harness.user),
            sources: json!([]),
            plan: json!({}),
            copied_from: None,
        })
        .await)
    .expect("draft");
    with_db!(&harness.pool, |db| ImportRepo::new(db)
        .start_run(import.id, import.revision)
        .await)
    .expect("the import started");
    let job = omeka_api::derivatives::queue_worker(&harness.pool, None)
        .await
        .expect("queue worker")
        .expect("a worker");
    let task = tokio::spawn({
        let pool = harness.pool.clone();
        let env = job_env(&harness);
        async move { Runner::with_env(&pool, env).run(job).await }
    });
    // Wait until the worker says it is holding, then ask it to stop.
    let started = Instant::now();
    loop {
        let text = with_db!(&harness.pool, |db| JobRepo::new(db).get(job).await)
            .expect("job")
            .and_then(|row| row.log)
            .unwrap_or_default();
        if text.contains("waiting: import") {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(10), "{text}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    with_db!(&harness.pool, |db| JobRepo::new(db).request_stop(job).await).expect("stop");
    let status = task.await.expect("worker task").expect("the worker ended");
    assert!(
        matches!(status, omeka_cli::jobs::JobStatus::Stopped),
        "{status:?}"
    );
    let untouched = with_db!(&harness.pool, |db| MediaDerivativeRepo::new(db)
        .get(media, DerivativeKind::Thumbnail)
        .await)
    .expect("row")
    .expect("the row");
    assert_eq!(untouched.state, DerivativeState::Pending, "nothing started");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_backfill_queues_and_makes_a_media_the_import_left_bare() {
    let harness = harness().await;
    // The shape the import leaves for a staged file over the whole-file limit:
    // the bytes arrived through `put_stream`, and no derivative was made.
    let (media, storage_id) = insert_media(&harness, "video/mp4", "mp4", &[]).await;
    let path = storage_path(StorePrefix::Original, &storage_id, Some("mp4"));
    harness
        .store
        .put_stream(
            &mut std::io::Cursor::new(video()),
            Some(video().len() as u64),
            &path,
            Some("video/mp4"),
        )
        .expect("the staged original");

    let report = derivatives::run(&DerivativeOptions {
        database: harness._dir.path().join("romeka.db").into(),
        store: Some(harness._dir.path().join("files")),
        retry: false,
        media: Some(media.0),
    })
    .await
    .expect("the backfill ran");
    assert_eq!(report.queued, 1, "{report:?}");
    assert_eq!(report.rows.len(), 2, "{report:?}");
    let preview = report
        .rows
        .iter()
        .find(|row| row.kind.is_preview())
        .expect("the poster row");
    assert_eq!(preview.state, DerivativeState::Ready, "{report:?}");
}

#[test]
fn a_stored_original_with_no_local_path_is_streamed_not_read_whole() {
    let dir = tempfile::tempdir().unwrap();
    let counting = CountingStore {
        inner: FileStore::new(dir.path(), "/files"),
        ranges: Mutex::new(0),
    };
    let bytes = video();
    counting
        .put(&bytes, "original/streamed.mp4", Some("video/mp4"))
        .expect("store");
    let row = omeka_db::rows::MediaRow {
        resource: omeka_db::rows::ResourceRow {
            id: omeka_db::ids::ResourceId(1),
            owner_id: None,
            resource_class_id: None,
            resource_template_id: None,
            thumbnail_id: None,
            title: None,
            is_public: true,
            created: chrono::NaiveDateTime::default(),
            modified: None,
            resource_type: omeka_db::rows::resource_type::MEDIA.to_string(),
        },
        item_id: ItemId(1),
        ingester: "upload".to_string(),
        renderer: "file".to_string(),
        data: None,
        source: Some("streamed.mp4".to_string()),
        media_type: Some("video/mp4".to_string()),
        storage_id: Some("streamed".to_string()),
        extension: Some("mp4".to_string()),
        sha256: None,
        size: Some(bytes.len() as i64),
        has_original: true,
        has_thumbnails: false,
        position: None,
        lang: None,
        alt_text: None,
    };
    let source = PreviewSource {
        storage_id: row.storage_id.clone().unwrap_or_default(),
        extension: row.extension.clone(),
        media_type: row.media_type.clone(),
        has_original: row.has_original,
    };
    let work = make_preview(&counting, &source, false);
    assert!(matches!(work, DerivativeWork::Ready { .. }), "{work:?}");
    assert!(
        *counting.ranges.lock().unwrap() > 0,
        "the bytes moved in ranges"
    );
}

/// The hourly sweep is the queue's timer: a pending preview must not wait for
/// the next create. It starts one worker when the queue has work and no worker
/// holds it, and nothing while an import runs.
#[tokio::test(flavor = "multi_thread")]
async fn the_hourly_sweep_starts_one_derivative_worker_and_never_during_an_import() {
    let harness = harness().await;
    let worker_rows = || {
        let pool = harness.pool.clone();
        async move {
            with_db!(&pool, |db| JobRepo::new(db)
                .list(
                    &JobFilter {
                        class: Some(DERIVATIVES_JOB.to_string()),
                        ..JobFilter::default()
                    },
                    &Paging::default(),
                )
                .await)
            .expect("worker rows")
        }
    };
    let sweep = || {
        let pool = harness.pool.clone();
        let env = job_env(&harness);
        async move {
            let job = compose_job(&pool, ImportSweep.class(), &json!({}), None)
                .await
                .expect("sweep row");
            Runner::with_env(&pool, env)
                .run(job)
                .await
                .expect("sweep ran")
        }
    };

    // A queued preview and no worker: one worker starts and settles it.
    let (first, _) = insert_media(&harness, "image/png", "png", &png()).await;
    let row = with_db!(&harness.pool, |db| MediaRepo::new(db).get(first).await)
        .expect("media")
        .expect("the row");
    omeka_api::derivatives::enqueue(&harness.pool, &harness.store, &row)
        .await
        .expect("queue");
    sweep().await;
    assert_eq!(worker_rows().await.len(), 1, "one worker");
    let settled = with_db!(&harness.pool, |db| MediaDerivativeRepo::new(db)
        .get(first, DerivativeKind::Thumbnail)
        .await)
    .expect("row")
    .expect("the row");
    assert_eq!(settled.state, DerivativeState::Ready, "the worker made it");

    // A worker is already active: the sweep starts none.
    let (second, _) = insert_media(&harness, "image/png", "png", &png()).await;
    let row = with_db!(&harness.pool, |db| MediaRepo::new(db).get(second).await)
        .expect("media")
        .expect("the row");
    omeka_api::derivatives::enqueue(&harness.pool, &harness.store, &row)
        .await
        .expect("queue");
    let active = with_db!(&harness.pool, |db| JobRepo::new(db)
        .create(&NewJob {
            owner_id: None,
            pid: None,
            status: Some("starting".to_string()),
            class: DERIVATIVES_JOB.to_string(),
            args: Some(json!({})),
            log: None,
            started: None,
        })
        .await)
    .expect("active worker row");
    sweep().await;
    assert_eq!(
        worker_rows().await.len(),
        2,
        "the active worker and the first"
    );
    let waiting = with_db!(&harness.pool, |db| MediaDerivativeRepo::new(db)
        .get(second, DerivativeKind::Thumbnail)
        .await)
    .expect("row")
    .expect("the row");
    assert_eq!(
        waiting.state,
        DerivativeState::Pending,
        "left for the active worker"
    );
    with_db!(&harness.pool, |db| JobRepo::new(db)
        .finish(active.id, "completed")
        .await)
    .expect("finish the active worker");

    // An import is running: the sweep starts none, even with rows queued.
    let (third, _) = insert_media(&harness, "image/png", "png", &png()).await;
    let row = with_db!(&harness.pool, |db| MediaRepo::new(db).get(third).await)
        .expect("media")
        .expect("the row");
    omeka_api::derivatives::enqueue(&harness.pool, &harness.store, &row)
        .await
        .expect("queue");
    let import = with_db!(&harness.pool, |db| ImportRepo::new(db)
        .create_draft(&NewImport {
            owner_id: Some(harness.user),
            sources: json!([]),
            plan: json!({}),
            copied_from: None,
        })
        .await)
    .expect("draft");
    with_db!(&harness.pool, |db| ImportRepo::new(db)
        .start_run(import.id, import.revision)
        .await)
    .expect("the import started");
    sweep().await;
    assert_eq!(worker_rows().await.len(), 2, "no worker during an import");
    let untouched = with_db!(&harness.pool, |db| MediaDerivativeRepo::new(db)
        .get(third, DerivativeKind::Thumbnail)
        .await)
    .expect("row")
    .expect("the row");
    assert_eq!(untouched.state, DerivativeState::Pending, "nothing started");
}
