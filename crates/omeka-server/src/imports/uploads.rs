//! `/api/romeka/imports/{id}/uploads[/{upload}]`: tus 1.0 core into the
//! import's staging area (`docs/import-v2.md`, "Arrival", "Staging").
//!
//! `OPTIONS` names the protocol and the size limit; `POST` creates an upload
//! (the `creation` extension) from `Upload-Length` and the `filename` of
//! `Upload-Metadata`; `HEAD` answers the stored offset; `PATCH` appends the
//! bytes that start at `Upload-Offset`; `DELETE` removes an upload (the
//! `termination` extension). `GET` on the collection is Romeka's own: the
//! uploads, the bytes they count for, the limits and the date the files are
//! kept until, so the admin resumes after a reload.
//!
//! Only the import's owner and global admins may upload, and only into a
//! draft. The rules live in `omeka_api::import::staging`; this module reads
//! the headers, holds one lock per upload so two `PATCH` requests for one
//! upload never interleave, streams the body into the store, and starts the
//! unzip job when a `.zip` arrives whole. A refusal answers the import routes'
//! error document with a `code` the admin branches on.
//!
//! An upload start dispatches the sweep job when none was dispatched in the
//! last hour; `romeka serve` also starts it every hour.

use std::sync::atomic::Ordering;
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri, header};
use axum::response::Response;
use omeka_api::auth::Actor;
use omeka_api::import::staging::{self, Limits, StagingError};
use omeka_api::import::sweep::{self, SWEEP_JOB};
use omeka_api::import::unzip::{UNZIP_JOB, is_zip};
use omeka_api::messages;
use omeka_db::ids::JobId;
use omeka_db::repos::{JobRepo, UploadRepo};
use omeka_db::rows::{ImportRow, ImportStatus, NewJob, UploadRow};
use omeka_db::with_db;
use serde::Serialize;
use serde_json::{Map, Value, json};

use super::wire::{ImportError, admin, load};
use crate::AppState;
use crate::tus::{self, BodyReader, metadata_value};

/// The header that names the unzip job a finished `.zip` started.
pub(crate) const UNZIP_HEADER: &str = "romeka-unzip-job";
/// The most bytes one `PATCH` may carry. A chunk is a part in the store, so
/// the limit bounds the work one request can ask for.
const MAX_CHUNK: u64 = 64 * 1024 * 1024;
/// How often an upload start may dispatch the sweep.
const SWEEP_EVERY: Duration = Duration::from_secs(60 * 60);

/// A refusal: its status, the contract's code, the message and members.
struct Refusal {
    status: StatusCode,
    code: Option<&'static str>,
    message: String,
    members: Map<String, Value>,
    offset: Option<u64>,
}

impl Refusal {
    fn new(status: StatusCode, code: Option<&'static str>, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            members: Map::new(),
            offset: None,
        }
    }

    fn with(mut self, key: &str, value: Value) -> Self {
        self.members.insert(key.to_string(), value);
        self
    }

    fn refused(self) -> Box<Response> {
        Box::new(self.into_response())
    }

    fn into_response(self) -> Response {
        let mut body = Map::new();
        body.insert("errors".to_string(), json!({ "error": self.message }));
        if let Some(code) = self.code {
            body.insert("code".to_string(), Value::from(code));
        }
        body.extend(self.members);
        let mut answer = tus::header(Response::builder().status(self.status))
            .header(header::CONTENT_TYPE, crate::JSONLD_CONTENT_TYPE);
        if let Some(offset) = self.offset {
            answer = answer.header("upload-offset", offset);
        }
        answer
            .body(Body::from(Value::Object(body).to_string()))
            .unwrap_or_else(|_| crate::empty_internal_error())
    }
}

/// A handler's answer; the refusal is boxed, as a `Response` is large.
type Answer = Result<Response, Box<Response>>;

fn done(answer: Answer) -> Response {
    answer.unwrap_or_else(|response| *response)
}

fn shared(error: ImportError) -> Box<Response> {
    let mut response = error.into_response();
    response
        .headers_mut()
        .insert("tus-resumable", HeaderValue::from_static(tus::VERSION));
    Box::new(response)
}

fn chunk_too_large() -> Box<Response> {
    Refusal::new(
        StatusCode::PAYLOAD_TOO_LARGE,
        Some("chunkTooLarge"),
        messages::import_upload_chunk_too_large(MAX_CHUNK),
    )
    .with("limit", json!(MAX_CHUNK))
    .refused()
}

fn staging_refusal(error: StagingError) -> Box<Response> {
    let refusal = match error {
        StagingError::Path { path, reason } => Refusal::new(
            StatusCode::BAD_REQUEST,
            Some("uploadPath"),
            messages::import_upload_path(path, reason),
        ),
        StagingError::FileTooLarge { limit } => Refusal::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            Some("fileTooLarge"),
            messages::import_upload_too_large(limit),
        )
        .with("limit", json!(limit)),
        StagingError::ImportFull { limit, used } => Refusal::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            Some("importFull"),
            messages::import_upload_full(limit),
        )
        .with("limit", json!(limit))
        .with("used", json!(used)),
        StagingError::InProgress { upload } => Refusal::new(
            StatusCode::CONFLICT,
            Some("uploadInProgress"),
            messages::import_upload_in_progress(&upload),
        )
        .with("upload", json!(upload)),
        StagingError::Missing => Refusal::new(
            StatusCode::NOT_FOUND,
            None,
            messages::import_upload_not_found(""),
        ),
        StagingError::OffsetConflict { received } => {
            let mut refusal = Refusal::new(
                StatusCode::CONFLICT,
                Some("offsetConflict"),
                messages::import_upload_offset(received),
            )
            .with("received", json!(received));
            refusal.offset = Some(received);
            refusal
        }
        StagingError::PastLength { length } => Refusal::new(
            StatusCode::BAD_REQUEST,
            Some("pastLength"),
            messages::IMPORT_UPLOAD_PAST_LENGTH,
        )
        .with("length", json!(length)),
        StagingError::Interrupted(_) => Refusal::new(
            StatusCode::BAD_REQUEST,
            Some("interrupted"),
            messages::IMPORT_UPLOAD_INTERRUPTED,
        ),
        StagingError::Store(reason) => {
            Refusal::new(StatusCode::INTERNAL_SERVER_ERROR, None, reason)
        }
        StagingError::Db(error) => {
            Refusal::new(StatusCode::INTERNAL_SERVER_ERROR, None, error.to_string())
        }
    };
    Box::new(refusal.into_response())
}

/// The caller and the import, refused unless the caller may upload into it.
async fn uploader(
    state: &AppState,
    id: &str,
    uri: &Uri,
    headers: &HeaderMap,
    needs_draft: bool,
) -> Result<(Actor, ImportRow), Box<Response>> {
    let actor = admin(state, uri, headers).await.map_err(shared)?;
    let row = load(state, id).await.map_err(shared)?;
    let owns = actor.user().is_some() && row.owner_id == actor.user();
    if actor.role() != Some("global_admin") && !owns {
        return Err(Refusal::new(
            StatusCode::FORBIDDEN,
            None,
            messages::IMPORT_UPLOAD_OWNER_ONLY,
        )
        .refused());
    }
    if needs_draft && row.status != ImportStatus::Draft {
        return Err(Refusal::new(
            StatusCode::CONFLICT,
            Some("notDraft"),
            messages::IMPORT_UPLOAD_NOT_DRAFT,
        )
        .with("status", json!(row.status.as_str()))
        .refused());
    }
    Ok((actor, row))
}

/// Refuse a tus request that does not speak 1.0.0 (412, as the protocol says).
fn speaks_tus(headers: &HeaderMap) -> Result<(), Box<Response>> {
    if tus::speaks(headers) {
        return Ok(());
    }
    Err(Box::new(
        tus::header(Response::builder().status(StatusCode::PRECONDITION_FAILED))
            .header("tus-version", tus::VERSION)
            .header(header::CONTENT_TYPE, crate::JSONLD_CONTENT_TYPE)
            .body(Body::from(
                json!({ "errors": { "error": messages::IMPORT_UPLOAD_VERSION } }).to_string(),
            ))
            .unwrap_or_else(|_| crate::empty_internal_error()),
    ))
}

async fn limits(state: &AppState) -> Result<omeka_api::settings::UploadSettings, Box<Response>> {
    omeka_api::settings::upload_settings(&state.pool)
        .await
        .map_err(|error| shared(error.into()))
}

/// `OPTIONS`: the protocol, its extensions and the largest file.
pub(super) async fn options(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    done(
        async {
            uploader(&state, &id, &uri, &headers, false).await?;
            let settings = limits(&state).await?;
            Ok(
                tus::header(Response::builder().status(StatusCode::NO_CONTENT))
                    .header("tus-version", tus::VERSION)
                    .header("tus-max-size", settings.max_file)
                    .header("tus-extension", "creation,termination")
                    .body(Body::empty())
                    .unwrap_or_else(|_| crate::empty_internal_error()),
            )
        }
        .await,
    )
}

/// `OPTIONS` on one upload answers as on the collection.
pub(super) async fn options_one(
    state: State<AppState>,
    Path((id, _)): Path<(String, String)>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    options(state, Path(id), uri, headers).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Shown {
    id: String,
    path: String,
    length: u64,
    received: u64,
}

impl From<UploadRow> for Shown {
    fn from(row: UploadRow) -> Self {
        Self {
            id: row.id,
            path: row.path,
            length: row.length,
            received: row.received,
        }
    }
}

/// `GET`: the uploads, the bytes they count for, the limits, and until when
/// a draft's files are kept.
pub(super) async fn list(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    done(
        async {
            let (_, row) = uploader(&state, &id, &uri, &headers, false).await?;
            let settings = limits(&state).await?;
            let usage = with_db!(&state.pool, |db| staging::usage(db, row.id).await)
                .map_err(|error| shared(error.into()))?;
            let keep_until = sweep::kept_until(&row, &usage.uploads, settings.into())
                .map(|until| until.format("%Y-%m-%dT%H:%M:%S+00:00").to_string());
            let uploads: Vec<Shown> = usage.uploads.into_iter().map(Shown::from).collect();
            let body = json!({
                "uploads": uploads,
                "used": usage.used,
                "limit": { "file": settings.max_file, "total": settings.max_total },
                "keepUntil": keep_until,
            });
            Ok(tus::header(Response::builder().status(StatusCode::OK))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap_or_else(|_| crate::empty_internal_error()))
        }
        .await,
    )
}

/// Serializes upload starts, so two starts cannot both pass the import's limit.
static CREATING: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));

/// `POST`: start an upload.
pub(super) async fn create(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    done(
        async {
            speaks_tus(&headers)?;
            let (actor, row) = uploader(&state, &id, &uri, &headers, true).await?;
            let Some(length) = tus::number(&headers, "upload-length") else {
                return Err(Refusal::new(
                    StatusCode::BAD_REQUEST,
                    Some("uploadLength"),
                    messages::IMPORT_UPLOAD_LENGTH,
                )
                .refused());
            };
            let Some(path) = headers
                .get("upload-metadata")
                .and_then(|value| value.to_str().ok())
                .and_then(|metadata| metadata_value(metadata, "filename"))
            else {
                return Err(Refusal::new(
                    StatusCode::BAD_REQUEST,
                    Some("uploadMetadata"),
                    messages::IMPORT_UPLOAD_METADATA,
                )
                .refused());
            };
            let settings = limits(&state).await?;
            let made = {
                let _held = CREATING.lock().await;
                with_db!(&state.pool, |db| staging::create(
                    db,
                    row.id,
                    &path,
                    length,
                    Limits::from(settings)
                )
                .await)
            }
            .map_err(staging_refusal)?;
            maybe_sweep(&state, &actor).await;
            let location = format!("/api/romeka/imports/{}/uploads/{}", row.id.0, made.id);
            Ok(tus::header(Response::builder().status(StatusCode::CREATED))
                .header(header::LOCATION, location)
                .header("upload-offset", 0)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!(Shown::from(made)).to_string()))
                .unwrap_or_else(|_| crate::empty_internal_error()))
        }
        .await,
    )
}

/// `HEAD /{upload}`: the stored offset.
pub(super) async fn head(
    State(state): State<AppState>,
    Path((id, upload)): Path<(String, String)>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    done(
        async {
            speaks_tus(&headers)?;
            let (_, row) = uploader(&state, &id, &uri, &headers, false).await?;
            let found = with_db!(&state.pool, |db| UploadRepo::new(db)
                .get(row.id, &upload)
                .await)
            .map_err(|error| shared(error.into()))?;
            let Some(found) = found else {
                return Ok(
                    tus::header(Response::builder().status(StatusCode::NOT_FOUND))
                        .header(header::CACHE_CONTROL, "no-store")
                        .body(Body::empty())
                        .unwrap_or_else(|_| crate::empty_internal_error()),
                );
            };
            Ok(tus::header(Response::builder().status(StatusCode::OK))
                .header("upload-offset", found.received)
                .header("upload-length", found.length)
                .header(header::CACHE_CONTROL, "no-store")
                .body(Body::empty())
                .unwrap_or_else(|_| crate::empty_internal_error()))
        }
        .await,
    )
}

/// `PATCH /{upload}`: the bytes from `Upload-Offset` on.
pub(super) async fn patch(
    State(state): State<AppState>,
    Path((id, upload)): Path<(String, String)>,
    uri: Uri,
    headers: HeaderMap,
    body: Body,
) -> Response {
    done(
        async {
            speaks_tus(&headers)?;
            let (actor, row) = uploader(&state, &id, &uri, &headers, true).await?;
            if !tus::is_offset_type(&headers) {
                return Err(Refusal::new(
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    None,
                    messages::IMPORT_UPLOAD_CONTENT_TYPE,
                )
                .refused());
            }
            let Some(offset) = tus::number(&headers, "upload-offset") else {
                return Err(Refusal::new(
                    StatusCode::BAD_REQUEST,
                    Some("uploadOffset"),
                    messages::IMPORT_UPLOAD_OFFSET_MISSING,
                )
                .refused());
            };
            if tus::number(&headers, "content-length").is_some_and(|sent| sent > MAX_CHUNK) {
                return Err(chunk_too_large());
            }
            let reader = BodyReader::spawn(body, MAX_CHUNK);
            let over = reader.over();
            let work = {
                let state = state.clone();
                let upload = upload.clone();
                let import = row.id;
                async move {
                    with_db!(&state.pool, |db| staging::append(
                        db,
                        &state.store,
                        import,
                        &upload,
                        offset,
                        Box::new(reader),
                    )
                    .await)
                }
            };
            let appended = tus::locked(format!("import/{}/{upload}", row.id.0), work)
                .await
                .ok_or_else(|| Box::new(crate::empty_internal_error()))?;
            let appended = match appended {
                Ok(appended) => appended,
                Err(StagingError::Missing) => {
                    return Err(Refusal::new(
                        StatusCode::NOT_FOUND,
                        None,
                        messages::import_upload_not_found(&upload),
                    )
                    .refused());
                }
                Err(StagingError::Interrupted(_)) if over.load(Ordering::Relaxed) => {
                    return Err(chunk_too_large());
                }
                Err(error) => return Err(staging_refusal(error)),
            };
            let mut answer = tus::header(Response::builder().status(StatusCode::NO_CONTENT))
                .header("upload-offset", appended.received);
            if let Some(complete) = appended.completed
                && is_zip(&complete.path)
            {
                let job = start_job(
                    &state,
                    &actor,
                    UNZIP_JOB,
                    json!({ "importId": row.id.0, "upload": complete.id }),
                )
                .await?;
                answer = answer.header(UNZIP_HEADER, job.0);
            }
            Ok(answer
                .body(Body::empty())
                .unwrap_or_else(|_| crate::empty_internal_error()))
        }
        .await,
    )
}

/// `DELETE /{upload}`: remove an upload, finished or not.
pub(super) async fn delete(
    State(state): State<AppState>,
    Path((id, upload)): Path<(String, String)>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    done(
        async {
            speaks_tus(&headers)?;
            let (_, row) = uploader(&state, &id, &uri, &headers, true).await?;
            let work = {
                let state = state.clone();
                let upload = upload.clone();
                let import = row.id;
                async move {
                    with_db!(&state.pool, |db| staging::remove(
                        db,
                        &state.store,
                        import,
                        &upload
                    )
                    .await)
                }
            };
            let removed = tus::locked(format!("import/{}/{upload}", row.id.0), work)
                .await
                .ok_or_else(|| Box::new(crate::empty_internal_error()))?;
            match removed {
                Ok(true) => Ok(
                    tus::header(Response::builder().status(StatusCode::NO_CONTENT))
                        .body(Body::empty())
                        .unwrap_or_else(|_| crate::empty_internal_error()),
                ),
                Ok(false) => Err(Refusal::new(
                    StatusCode::NOT_FOUND,
                    None,
                    messages::import_upload_not_found(&upload),
                )
                .refused()),
                Err(error) => Err(shared(error.into())),
            }
        }
        .await,
    )
}

/// Compose a job row and hand it to the runner. A start that fails ends
/// the job `error`, and the upload is still stored.
async fn start_job(
    state: &AppState,
    actor: &Actor,
    class: &str,
    args: Value,
) -> Result<JobId, Box<Response>> {
    let job = with_db!(&state.pool, |db| JobRepo::new(db)
        .create(&NewJob {
            owner_id: actor.user(),
            pid: None,
            status: Some("starting".to_string()),
            class: class.to_string(),
            args: Some(args),
            log: None,
            started: None,
        })
        .await)
    .map_err(|error| shared(error.into()))?;
    if let Err(error) = state.jobs.start(job.id) {
        let _ = with_db!(&state.pool, |db| JobRepo::new(db)
            .append_log(job.id, &error.to_string())
            .await);
        let _ = with_db!(&state.pool, |db| JobRepo::new(db)
            .finish(job.id, "error")
            .await);
        return Err(shared(ImportError::Internal(
            messages::import_job_not_started(error),
        )));
    }
    Ok(job.id)
}

static LAST_SWEEP: LazyLock<Mutex<Option<Instant>>> = LazyLock::new(|| Mutex::new(None));

/// Dispatch the sweep job unless one was dispatched within [`SWEEP_EVERY`].
async fn maybe_sweep(state: &AppState, actor: &Actor) {
    {
        let mut last = LAST_SWEEP.lock().unwrap_or_else(PoisonError::into_inner);
        if last.is_some_and(|at| at.elapsed() < SWEEP_EVERY) {
            return;
        }
        *last = Some(Instant::now());
    }
    let _ = start_job(state, actor, SWEEP_JOB, json!({})).await;
}
