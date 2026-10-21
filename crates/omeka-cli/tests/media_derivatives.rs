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
use omeka_cli::jobs::{ImportSweep, Job, JobCommand, JobEnv, Runner, compose_job};
use omeka_db::DbPool;
use omeka_db::ids::{ItemId, JobId, MediaId, UserId};
use omeka_db::repos::{
    ApiKeyRepo, ImportRepo, ItemRepo, JobFilter, JobRepo, MediaDerivativeRepo, MediaRepo, Paging,
};
use omeka_db::rows::{
    DerivativeKind, DerivativeState, NewApiKey, NewDerivative, NewImport, NewItem, NewJob,
    NewMedia, NewResource,
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
        // The sweep starts a worker in its own process, which is the built
        // binary over this harness's database and store.
        jobs: Some(JobCommand {
            program: Some(PathBuf::from(env!("CARGO_BIN_EXE_romeka"))),
            database: harness._dir.path().join("romeka.db").into(),
            store: Some(harness._dir.path().join("files")),
            base_url: Some("http://localhost".to_string()),
            archive_root: None,
        }),
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

/// A camera RAW's create queues both rows, and the worker settles the access
/// copy from the embedded preview: the master is the preview at the source's own
/// size, upright, and the three thumbnails come from it.
#[tokio::test(flavor = "multi_thread")]
async fn a_raw_create_settles_an_upright_master() {
    if Tools::on_path().exiftool().is_none() {
        eprintln!("skipping: no exiftool on PATH");
        return;
    }
    let harness = harness().await;
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/media-kinds/image.nef"),
    )
    .expect("image.nef");
    let document = create_media(&harness, "photo.nef", &bytes).await;
    let id = document["o:id"].as_u64().expect("the media id");
    let storage_id = document["o:filename"]
        .as_str()
        .and_then(|name| name.rsplit_once('.'))
        .map(|(stem, _)| stem.to_string())
        .expect("the stored name");
    assert_eq!(preview(&document)["state"], json!("pending"));

    omeka_api::derivatives::drain(&harness.pool, &harness.store)
        .await
        .expect("the queue drains");

    let access = with_db!(&harness.pool, |db| MediaDerivativeRepo::new(db)
        .get(MediaId(id), DerivativeKind::AccessCopy)
        .await)
    .expect("row")
    .expect("the access row");
    assert_eq!(access.state, DerivativeState::Ready, "{access:?}");
    assert_eq!(access.tool.as_deref(), Some("exiftool"));
    let master = harness
        .store
        .read(&omeka_media::access::copy_path(
            &storage_id,
            omeka_media::access::AccessTarget::Jpeg,
        ))
        .expect("the master");
    assert_eq!(
        omeka_media::iiif_image::source_size(&master),
        Some((800, 1200)),
        "the preview's 1200x800, turned by Orientation 6"
    );
    let large = harness
        .store
        .read(&omeka_media::thumbnailer::thumbnail_path(
            &storage_id,
            omeka_media::file_render::ThumbnailType::Large,
        ))
        .expect("the large derivative");
    assert_eq!(
        omeka_media::iiif_image::source_size(&large),
        Some((533, 800)),
        "the derivative is capped at 800"
    );
    assert_eq!(
        preview(&read_media(&harness, id).await)["state"],
        json!("ready")
    );
}

/// A 16-bit TIFF the image crate cannot decode gets its master through `vips`,
/// at the source's own size, and the media reads ready afterwards.
#[tokio::test(flavor = "multi_thread")]
async fn a_16_bit_tiff_create_settles_a_master_through_vips() {
    if Tools::on_path().vips().is_none() {
        eprintln!("skipping: no vips on PATH");
        return;
    }
    let harness = harness().await;
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/media-kinds/image-deep.tif"),
    )
    .expect("image-deep.tif");
    let document = create_media(&harness, "deep.tif", &bytes).await;
    let id = document["o:id"].as_u64().expect("the media id");
    let storage_id = document["o:filename"]
        .as_str()
        .and_then(|name| name.rsplit_once('.'))
        .map(|(stem, _)| stem.to_string())
        .expect("the stored name");

    omeka_api::derivatives::drain(&harness.pool, &harness.store)
        .await
        .expect("the queue drains");

    let access = with_db!(&harness.pool, |db| MediaDerivativeRepo::new(db)
        .get(MediaId(id), DerivativeKind::AccessCopy)
        .await)
    .expect("row")
    .expect("the access row");
    assert_eq!(access.state, DerivativeState::Ready, "{access:?}");
    assert_eq!(access.tool.as_deref(), Some("vips"));
    let master = harness
        .store
        .read(&omeka_media::access::copy_path(
            &storage_id,
            omeka_media::access::AccessTarget::Jpeg,
        ))
        .expect("the master");
    assert_eq!(
        omeka_media::iiif_image::source_size(&master),
        Some((1200, 800))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_docx_create_queues_a_pdf_copy_and_the_worker_makes_it() {
    if Tools::on_path().soffice().is_none() {
        eprintln!("skipping: no soffice on PATH");
        return;
    }
    let harness = harness().await;
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/media-kinds/doc.docx");
    let bytes = std::fs::read(&path).expect("the DOCX fixture");
    let document = create_media(&harness, "notes.docx", &bytes).await;
    let id = document["o:id"].as_u64().expect("id");
    assert_eq!(
        state_of(&document),
        "none",
        "no decoder makes a still from a document; its preview comes from the copy"
    );
    let planned = with_db!(&harness.pool, |db| MediaDerivativeRepo::new(db)
        .get(MediaId(id), DerivativeKind::AccessCopy)
        .await)
    .expect("row")
    .expect("the access row");
    assert_eq!(planned.state, DerivativeState::Pending, "{planned:?}");
    assert_eq!(planned.tool.as_deref(), Some("soffice"));
    assert_eq!(
        document["o-module-romeka:access"]["state"], "pending",
        "{document}"
    );

    let jobs = harness.recorder.taken();
    assert_eq!(jobs.len(), 1, "the create queued one worker");
    Runner::with_env(&harness.pool, job_env(&harness))
        .run(jobs[0])
        .await
        .expect("the worker ran");

    let ready = read_media(&harness, id).await;
    let access = &ready["o-module-romeka:access"];
    assert_eq!(access["state"], "ready", "{ready}");
    assert_eq!(access["media_type"], "application/pdf", "{ready}");
    assert!(
        access["url"]
            .as_str()
            .is_some_and(|url| url.ends_with(".pdf")),
        "{ready}"
    );
    assert_eq!(ready["o:kind"], "document", "{ready}");
    // The copy's first page is the preview, which is what the reader and the
    // item card show.
    assert_eq!(preview(&ready)["state"], "ready", "{ready}");
    assert!(ready["o:thumbnail_urls"]["large"].is_string(), "{ready}");
    // The original is still the document the API names.
    assert!(
        ready["o:original_url"]
            .as_str()
            .is_some_and(|url| url.ends_with(".docx")),
        "{ready}"
    );
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
    // backfill with `--retry`. The poster is retried, and so is the access copy
    // the first worker failed on.
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
        redo: None,
        media: Some(media.0),
    })
    .await
    .expect("the backfill ran");
    assert_eq!(report.retried, 2, "{report:?}");
    assert_eq!(report.rows.len(), 2, "{report:?}");
    let preview = report
        .rows
        .iter()
        .find(|row| row.kind.is_preview())
        .expect("the poster row");
    assert_eq!(preview.state, DerivativeState::Ready, "{report:?}");
    let copy = report
        .rows
        .iter()
        .find(|row| row.kind == DerivativeKind::AccessCopy)
        .expect("the access row");
    assert_eq!(
        copy.state,
        DerivativeState::None,
        "the replaced original is a browser-playable MP4: {report:?}"
    );
    assert_eq!(copy.reason.as_deref(), Some("a browser shows the original"));
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

/// The MJPEG AVI the corpus holds: a video no browser plays.
fn avi() -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/media-kinds/mjpeg.avi");
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// An AVI created through the route gets a playable MP4 under `access/`, the
/// media document says where it is, and the original is still there untouched.
#[tokio::test(flavor = "multi_thread")]
async fn an_avi_gets_a_playable_copy_and_the_original_is_kept() {
    if Tools::on_path().ffmpeg().is_none() || Tools::on_path().ffprobe().is_none() {
        eprintln!("skipping: no ffmpeg or ffprobe on PATH");
        return;
    }
    let harness = harness().await;
    let document = create_media(&harness, "reel.avi", &avi()).await;
    let id = document["o:id"].as_u64().expect("the media id");
    let access = |id: u64| {
        let pool = harness.pool.clone();
        async move {
            with_db!(&pool, |db| MediaDerivativeRepo::new(db)
                .get(MediaId(id), DerivativeKind::AccessCopy)
                .await)
            .expect("the row")
            .expect("the access row")
        }
    };
    let queued = access(id).await;
    assert_eq!(queued.state, DerivativeState::Pending);
    assert_eq!(queued.tool.as_deref(), Some("ffmpeg"));

    let jobs = harness.recorder.taken();
    assert_eq!(jobs.len(), 1, "one worker for one create");
    Runner::with_env(&harness.pool, job_env(&harness))
        .run(jobs[0])
        .await
        .expect("the worker ran");

    let media = with_db!(&harness.pool, |db| MediaRepo::new(db)
        .get(MediaId(id))
        .await)
    .expect("media")
    .expect("the row");
    let storage_id = media.storage_id.clone().expect("a storage id");
    let copy = format!("access/{storage_id}.mp4");
    assert!(
        harness.store.exists(&copy).expect("the store answers"),
        "the copy is under its own prefix"
    );
    assert!(
        harness
            .store
            .exists(&format!("original/{storage_id}.avi"))
            .expect("the store answers"),
        "the original is untouched"
    );
    let ready = access(id).await;
    assert_eq!(ready.state, DerivativeState::Ready);
    assert_eq!(ready.progress, Some(100));

    let document = read_media(&harness, id).await;
    let member = &document["o-module-romeka:access"];
    assert_eq!(member["state"], "ready", "{document}");
    assert_eq!(member["media_type"], "video/mp4", "{document}");
    let url = member["url"].as_str().unwrap_or_default();
    assert!(url.ends_with(&copy), "{url}");
    assert!(
        document["o:original_url"]
            .as_str()
            .is_some_and(|original| original.ends_with(&format!("original/{storage_id}.avi"))),
        "the preservation master is still the original: {document}"
    );
}

/// A running copy yields to an import: the command is killed, the row goes back
/// to the queue and any partial copy is removed. A stand-in `ffmpeg` prints
/// progress and sleeps, so the import starts while the copy is running.
#[tokio::test(flavor = "multi_thread")]
async fn a_starting_import_sends_a_running_copy_back_to_the_queue() {
    use omeka_api::derivatives::{Settled, settle_next_with};

    let harness = harness().await;
    let (media, storage_id) = insert_media(&harness, "video/x-msvideo", "avi", &avi()).await;
    let row = with_db!(&harness.pool, |db| MediaRepo::new(db).get(media).await)
        .expect("media")
        .expect("the row");
    omeka_api::derivatives::enqueue(&harness.pool, &harness.store, &row)
        .await
        .expect("queue");
    let access = |media: MediaId| {
        let pool = harness.pool.clone();
        async move {
            with_db!(&pool, |db| MediaDerivativeRepo::new(db)
                .get(media, DerivativeKind::AccessCopy)
                .await)
            .expect("the row")
            .expect("the access row")
        }
    };
    assert_eq!(access(media).await.state, DerivativeState::Pending);

    // A partial file from an earlier run must be gone after the stop.
    let copy = format!("access/{storage_id}.mp4");
    harness
        .store
        .put(b"half a copy", &copy, Some("video/mp4"))
        .expect("a partial copy");

    let dir = tempfile::tempdir().expect("tempdir");
    let ffmpeg = dir.path().join("ffmpeg");
    std::fs::write(
        &ffmpeg,
        "#!/bin/sh\n\
         for last in \"$@\"; do out=$last; done\n\
         printf 'out_time_us=1000000\\nprogress=continue\\n'\n\
         sleep 10\n\
         printf 'x' > \"$out\"\n",
    )
    .expect("the stand-in ffmpeg");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&ffmpeg, std::fs::Permissions::from_mode(0o755))
            .expect("make it executable");
    }
    let ffprobe = dir.path().join("ffprobe");
    std::fs::write(
        &ffprobe,
        "#!/bin/sh\nprintf '%s' '{\"streams\":[{\"codec_type\":\"video\",\"codec_name\":\"mjpeg\",\"width\":96,\"height\":64}],\"format\":{\"duration\":\"2.0\"}}'\n",
    )
    .expect("the stand-in ffprobe");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&ffprobe, std::fs::Permissions::from_mode(0o755))
            .expect("make it executable");
    }
    let tools = Tools::none().with_ffmpeg(&ffmpeg).with_ffprobe(&ffprobe);

    let task = tokio::spawn({
        let pool = harness.pool.clone();
        let store = Arc::clone(&harness.store);
        async move { settle_next_with(&pool, &store, &tools, None).await }
    });
    let started = Instant::now();
    let running = loop {
        let row = access(media).await;
        if row.state == DerivativeState::Running && row.progress.is_some_and(|p| p > 0) {
            break row;
        }
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the copy did not start: {row:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert_eq!(
        running.progress,
        Some(50),
        "one second of a two second file"
    );

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

    let settled = task
        .await
        .expect("the worker task")
        .expect("the worker ran");
    assert!(
        matches!(settled, Some(Settled::Queued { .. })),
        "{settled:?}"
    );
    let queued = access(media).await;
    assert_eq!(
        queued.state,
        DerivativeState::Pending,
        "the row waits for the next worker"
    );
    assert!(
        !harness.store.exists(&copy).expect("the store answers"),
        "the partial copy was removed"
    );
}

/// A media whose original yielded no poster takes the copy's frame: with no
/// preview row at all, the access copy's work makes the three thumbnails.
#[tokio::test(flavor = "multi_thread")]
async fn a_copy_makes_the_poster_when_the_original_yielded_none() {
    if Tools::on_path().ffmpeg().is_none() || Tools::on_path().ffprobe().is_none() {
        eprintln!("skipping: no ffmpeg or ffprobe on PATH");
        return;
    }
    let harness = harness().await;
    let (media, storage_id) = insert_media(&harness, "video/x-msvideo", "avi", &avi()).await;
    // Only the copy is planned, so the still the original would have made is
    // never queued.
    let planned = with_db!(&harness.pool, |db| MediaDerivativeRepo::new(db)
        .plan(
            &NewDerivative {
                media_id: media,
                kind: DerivativeKind::AccessCopy,
            },
            DerivativeState::Pending,
            None,
            Some("ffmpeg"),
        )
        .await)
    .expect("the access row");
    assert!(planned);
    let job = with_db!(&harness.pool, |db| JobRepo::new(db)
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
    .expect("job");
    Runner::with_env(&harness.pool, job_env(&harness))
        .run(job.id)
        .await
        .expect("the worker ran");

    for kind in omeka_media::file_render::ThumbnailType::ALL {
        let path = omeka_media::thumbnailer::thumbnail_path(&storage_id, kind);
        assert!(
            harness.store.exists(&path).expect("the store answers"),
            "{path} was not made from the copy"
        );
    }
    let updated = with_db!(&harness.pool, |db| MediaRepo::new(db).get(media).await)
        .expect("media")
        .expect("the row");
    assert!(updated.has_thumbnails, "the flag follows the files");
    let copy = with_db!(&harness.pool, |db| MediaDerivativeRepo::new(db)
        .get(media, DerivativeKind::AccessCopy)
        .await)
    .expect("the row")
    .expect("the access row");
    assert_eq!(copy.state, DerivativeState::Ready);
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
        redo: None,
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

/// The demo's gap: a media whose only work is an access copy waited for the
/// hourly sweep, because the command only ran a worker when it had queued a
/// preview. The run makes the copy itself and its summary counts the access row.
#[tokio::test(flavor = "multi_thread")]
async fn the_backfill_makes_an_access_copy_and_counts_its_row() {
    if Tools::on_path().ffmpeg().is_none() || Tools::on_path().ffprobe().is_none() {
        eprintln!("skipping: no ffmpeg or ffprobe on PATH");
        return;
    }
    let harness = harness().await;
    let (media, storage_id) = insert_media(&harness, "video/x-msvideo", "avi", &avi()).await;

    let report = derivatives::run(&DerivativeOptions {
        database: harness._dir.path().join("romeka.db").into(),
        store: Some(harness._dir.path().join("files")),
        retry: false,
        redo: None,
        media: Some(media.0),
    })
    .await
    .expect("the backfill ran");

    assert_eq!(
        report.queued, 2,
        "the access copy and the poster: {report:?}"
    );
    let copy = report
        .rows
        .iter()
        .find(|row| row.kind == DerivativeKind::AccessCopy)
        .expect("the access row");
    assert_eq!(copy.state, DerivativeState::Ready, "{report:?}");
    assert!(
        harness
            .store
            .exists(&format!("access/{storage_id}.mp4"))
            .expect("the store answers"),
        "the run made the copy, not the next sweep"
    );
}

/// A row a build before the words named the reader stores "a browser shows the
/// original" for a TIFF or JPEG 2000. The backfill re-decides and corrects the
/// words without touching the state: the image server is what reads it.
#[tokio::test(flavor = "multi_thread")]
async fn the_backfill_corrects_the_words_a_copy_row_was_given() {
    if Tools::on_path().vips().is_none() {
        eprintln!("skipping: no vips on PATH");
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let jp2 = dir.path().join("scan.jp2");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/media-kinds/image.png");
    let status = std::process::Command::new("vips")
        .args(["copy"])
        .arg(source)
        .arg(&jp2)
        .status()
        .expect("vips runs");
    assert!(status.success(), "vips wrote the JPEG 2000");
    let bytes = std::fs::read(&jp2).expect("the JPEG 2000");

    let harness = harness().await;
    let (media, _) = insert_media(&harness, "image/jp2", "jp2", &bytes).await;
    // The words an older build stored for this original.
    with_db!(&harness.pool, |db| MediaDerivativeRepo::new(db)
        .plan(
            &NewDerivative {
                media_id: media,
                kind: DerivativeKind::AccessCopy,
            },
            DerivativeState::None,
            Some("a browser shows the original"),
            None,
        )
        .await)
    .expect("the old row");

    let report = derivatives::run(&DerivativeOptions {
        database: harness._dir.path().join("romeka.db").into(),
        store: Some(harness._dir.path().join("files")),
        retry: false,
        redo: None,
        media: Some(media.0),
    })
    .await
    .expect("the backfill ran");

    let copy = with_db!(&harness.pool, |db| MediaDerivativeRepo::new(db)
        .get(media, DerivativeKind::AccessCopy)
        .await)
    .expect("the row")
    .expect("the access row");
    assert_eq!(copy.state, DerivativeState::None, "{copy:?}");
    assert_eq!(
        copy.reason.as_deref(),
        Some("the image server reads the original"),
        "the words name what reads this original: {report:?}"
    );
}

/// A ready copy can be made again: `--redo` queues it, the worker writes the
/// new file over the same path, and the original is untouched. The old copy is
/// never removed first, so a worker that fails leaves the copy that played.
#[tokio::test(flavor = "multi_thread")]
async fn a_redo_makes_a_ready_copy_again_and_a_failed_redo_keeps_the_old_one() {
    if Tools::on_path().ffmpeg().is_none() || Tools::on_path().ffprobe().is_none() {
        eprintln!("skipping: no ffmpeg or ffprobe on PATH");
        return;
    }
    use omeka_api::derivatives::settle_next_with;

    let harness = harness().await;
    let (media, storage_id) = insert_media(&harness, "video/x-msvideo", "avi", &avi()).await;
    let copy = format!("access/{storage_id}.mp4");
    let made = derivatives::run(&DerivativeOptions {
        database: harness._dir.path().join("romeka.db").into(),
        store: Some(harness._dir.path().join("files")),
        retry: false,
        redo: None,
        media: Some(media.0),
    })
    .await
    .expect("the first run");
    assert_eq!(made.redone, 0, "{made:?}");
    let first = harness.store.read(&copy).expect("the first copy");

    // `--redo video` queues the ready copy and the worker replaces it.
    let again = derivatives::run(&DerivativeOptions {
        database: harness._dir.path().join("romeka.db").into(),
        store: Some(harness._dir.path().join("files")),
        retry: false,
        redo: Some(derivatives::Redo::Kind(derivatives::RedoKind::Video)),
        media: Some(media.0),
    })
    .await
    .expect("the redo run");
    assert_eq!(again.redone, 1, "{again:?}");
    assert_eq!(
        harness.store.read(&copy).expect("the new copy"),
        first,
        "the same source makes the same copy"
    );

    // The old file stays while the new one is made: a redo on its own leaves
    // the copy where it is, and a worker that fails leaves it there too.
    let before = harness
        .store
        .read(&copy)
        .expect("the copy before the failure");
    assert!(
        with_db!(&harness.pool, |db| MediaDerivativeRepo::new(db)
            .redo(media, DerivativeKind::AccessCopy)
            .await)
        .expect("the second redo")
    );
    assert_eq!(
        harness.store.read(&copy).expect("the old copy"),
        before,
        "the old copy stays until the new one is written"
    );
    let dir = tempfile::tempdir().expect("tempdir");
    let ffmpeg = dir.path().join("ffmpeg");
    std::fs::write(&ffmpeg, "#!/bin/sh\nexit 1\n").expect("the stand-in ffmpeg");
    let ffprobe = dir.path().join("ffprobe");
    std::fs::write(
        &ffprobe,
        "#!/bin/sh\nprintf '%s' '{\"streams\":[{\"codec_type\":\"video\",\"codec_name\":\"mjpeg\",\"width\":96,\"height\":64}],\"format\":{\"duration\":\"2.0\"}}'\n",
    )
    .expect("the stand-in ffprobe");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        for tool in [&ffmpeg, &ffprobe] {
            std::fs::set_permissions(tool, std::fs::Permissions::from_mode(0o755))
                .expect("make it executable");
        }
    }
    let tools = Tools::none().with_ffmpeg(&ffmpeg).with_ffprobe(&ffprobe);
    settle_next_with(&harness.pool, &harness.store, &tools, None)
        .await
        .expect("the failing worker");
    let row = with_db!(&harness.pool, |db| MediaDerivativeRepo::new(db)
        .get(media, DerivativeKind::AccessCopy)
        .await)
    .expect("the row")
    .expect("the access row");
    assert_eq!(row.state, DerivativeState::Failed, "{row:?}");
    assert_eq!(
        harness.store.read(&copy).expect("the old copy"),
        before,
        "a failed redo keeps the copy that played"
    );
    assert!(
        harness
            .store
            .exists(&format!("original/{storage_id}.avi"))
            .expect("the store answers"),
        "the original is never changed"
    );
}

/// An undo during a copy: the worker writes `access/<id>.mp4`, then finds the
/// media row gone and removes its output rather than leaving a file nothing
/// names.
#[tokio::test(flavor = "multi_thread")]
async fn an_undo_during_a_copy_removes_the_copy_it_wrote() {
    use omeka_api::derivatives::settle_next_with;

    let harness = harness().await;
    let (media, storage_id) = insert_media(&harness, "video/x-msvideo", "avi", &avi()).await;
    let row = with_db!(&harness.pool, |db| MediaRepo::new(db).get(media).await)
        .expect("media")
        .expect("the row");
    omeka_api::derivatives::enqueue(&harness.pool, &harness.store, &row)
        .await
        .expect("queue");

    let dir = tempfile::tempdir().expect("tempdir");
    let ffmpeg = dir.path().join("ffmpeg");
    std::fs::write(
        &ffmpeg,
        "#!/bin/sh\n\
         for last in \"$@\"; do out=$last; done\n\
         printf 'out_time_us=1000000\\nprogress=continue\\n'\n\
         sleep 1\n\
         printf 'a copy' > \"$out\"\n",
    )
    .expect("the stand-in ffmpeg");
    let ffprobe = dir.path().join("ffprobe");
    std::fs::write(
        &ffprobe,
        "#!/bin/sh\nprintf '%s' '{\"streams\":[{\"codec_type\":\"video\",\"codec_name\":\"mjpeg\",\"width\":96,\"height\":64}],\"format\":{\"duration\":\"2.0\"}}'\n",
    )
    .expect("the stand-in ffprobe");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        for tool in [&ffmpeg, &ffprobe] {
            std::fs::set_permissions(tool, std::fs::Permissions::from_mode(0o755))
                .expect("make it executable");
        }
    }
    let tools = Tools::none().with_ffmpeg(&ffmpeg).with_ffprobe(&ffprobe);

    let task = tokio::spawn({
        let pool = harness.pool.clone();
        let store = Arc::clone(&harness.store);
        async move { settle_next_with(&pool, &store, &tools, None).await }
    });
    let started = Instant::now();
    loop {
        let running = with_db!(&harness.pool, |db| MediaDerivativeRepo::new(db)
            .get(media, DerivativeKind::AccessCopy)
            .await)
        .expect("the row");
        if running
            .as_ref()
            .is_some_and(|row| row.state == DerivativeState::Running)
        {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the copy did not start: {running:?}"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    // The undo: the media row goes, and the derivative rows cascade with it.
    with_db!(&harness.pool, |db| MediaRepo::new(db).delete(media).await)
        .expect("the media was removed");
    task.await
        .expect("the worker task")
        .expect("the worker ran");

    assert!(
        !harness
            .store
            .exists(&format!("access/{storage_id}.mp4"))
            .expect("the store answers"),
        "the copy nothing names was removed"
    );
}

/// The first page a document's preview paints comes from its copy, so the copy
/// is not announced `ready` until those thumbnails exist. The store below
/// records the row's state every time the thumbnails read the copy.
#[tokio::test(flavor = "multi_thread")]
async fn a_document_copy_settles_after_its_first_page_exists() {
    if Tools::on_path().soffice().is_none() || Tools::on_path().pdftoppm().is_none() {
        eprintln!("skipping: no soffice or pdftoppm on PATH");
        return;
    }
    use omeka_api::derivatives::settle_next_with;

    #[derive(Debug)]
    struct WatchingStore {
        inner: FileStore,
        pool: DbPool,
        media: MediaId,
        handle: tokio::runtime::Handle,
        seen: Arc<Mutex<Vec<DerivativeState>>>,
    }

    impl Store for WatchingStore {
        fn put(
            &self,
            bytes: &[u8],
            path: &str,
            media_type: Option<&str>,
        ) -> Result<(), StoreError> {
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
            self.watch(path);
            self.inner.read(path)
        }
        fn read_range(&self, path: &str, range: Range<u64>) -> Result<Vec<u8>, StoreError> {
            self.watch(path);
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

    impl WatchingStore {
        /// Record the access row's state while its copy is read for thumbnails.
        /// The read runs on a blocking thread, so it may wait on the database
        /// here while the worker holds the row.
        fn watch(&self, path: &str) {
            if !path.starts_with("access/") {
                return;
            }
            let pool = self.pool.clone();
            let media = self.media;
            let row = self.handle.block_on(async move {
                with_db!(&pool, |db| MediaDerivativeRepo::new(db)
                    .get(media, DerivativeKind::AccessCopy)
                    .await)
                .ok()
                .flatten()
            });
            if let Some(row) = row {
                self.seen.lock().unwrap().push(row.state);
            }
        }
    }

    let harness = harness().await;
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/media-kinds/doc.docx"),
    )
    .expect("the document");
    let (media, _) = insert_media(
        &harness,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "docx",
        &bytes,
    )
    .await;
    let row = with_db!(&harness.pool, |db| MediaRepo::new(db).get(media).await)
        .expect("media")
        .expect("the row");
    omeka_api::derivatives::enqueue(&harness.pool, &harness.store, &row)
        .await
        .expect("queue");

    let seen = Arc::new(Mutex::new(Vec::new()));
    let watching: std::sync::Arc<dyn Store> = std::sync::Arc::new(WatchingStore {
        inner: FileStore::new(harness._dir.path().join("files"), omeka_server::FILES_URI),
        pool: harness.pool.clone(),
        media,
        handle: tokio::runtime::Handle::current(),
        seen: Arc::clone(&seen),
    });
    settle_next_with(&harness.pool, &watching, Tools::on_path(), None)
        .await
        .expect("the worker");

    let seen = seen.lock().unwrap().clone();
    assert!(
        !seen.is_empty(),
        "the thumbnails read the copy, so the row was observed"
    );
    assert!(
        seen.iter().all(|state| *state == DerivativeState::Running),
        "the copy is not announced ready while its first page is made: {seen:?}"
    );
    let done = with_db!(&harness.pool, |db| MediaDerivativeRepo::new(db)
        .get(media, DerivativeKind::AccessCopy)
        .await)
    .expect("the row")
    .expect("the access row");
    assert_eq!(done.state, DerivativeState::Ready, "{done:?}");
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
/// the next create. It starts one worker in its own process when the queue has
/// work and no worker holds it, and nothing while an import runs.
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
    // The sweep no longer runs its worker in this process: the row it composed
    // is settled by the child, so wait for that child before judging the queue.
    let wait_for_workers = || {
        let pool = harness.pool.clone();
        async move {
            let started = Instant::now();
            loop {
                let rows = with_db!(&pool, |db| JobRepo::new(db)
                    .list(
                        &JobFilter {
                            class: Some(DERIVATIVES_JOB.to_string()),
                            ..JobFilter::default()
                        },
                        &Paging::default(),
                    )
                    .await)
                .expect("worker rows");
                if rows.iter().all(|row| {
                    matches!(
                        row.status.as_deref(),
                        Some("completed" | "stopped" | "error")
                    )
                }) {
                    return rows;
                }
                assert!(
                    started.elapsed() < Duration::from_secs(60),
                    "a derivative worker did not end: {rows:?}"
                );
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    };

    // A queued preview and no worker: one worker starts, in its own process, and
    // settles it.
    let (first, _) = insert_media(&harness, "image/png", "png", &png()).await;
    let row = with_db!(&harness.pool, |db| MediaRepo::new(db).get(first).await)
        .expect("media")
        .expect("the row");
    omeka_api::derivatives::enqueue(&harness.pool, &harness.store, &row)
        .await
        .expect("queue");
    sweep().await;
    let started = worker_rows().await;
    assert_eq!(started.len(), 1, "one worker");
    let finished = wait_for_workers().await;
    assert_eq!(finished[0].status.as_deref(), Some("completed"));
    assert!(
        finished[0]
            .log
            .as_deref()
            .unwrap_or_default()
            .contains("Derivatives:"),
        "the child did the work: {:?}",
        finished[0].log
    );
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
