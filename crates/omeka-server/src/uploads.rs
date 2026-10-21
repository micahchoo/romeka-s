//! `/api/romeka/uploads[/{upload}]`: tus 1.0 core into a user's staged
//! uploads (`briefs/media-ui`, G1).
//!
//! The rules and the bytes live in `omeka_api::uploads`; this module reads the
//! tus headers, authenticates the caller, checks the media-create grant, holds
//! one lock per upload so two `PATCH` requests never interleave, streams the
//! body into the store, and answers with the offset the next request starts
//! at. An upload belongs to the person who created it: every route names both
//! the caller and the upload id, so another account's upload is a 404.
//!
//! `GET` on the collection lists the caller's uploads, or those started for
//! one item with `?item=`, so the add panel resumes after a reload.

use std::sync::atomic::Ordering;

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::Response;
use axum::routing::{get, head};
use omeka_api::auth::Actor;
use omeka_api::messages;
use omeka_api::read::Params;
use omeka_api::routes::Operation;
use omeka_api::uploads::{self, Limit, UploadError};
use omeka_api::write::access;
use omeka_db::ids::ItemId;
use omeka_db::repos::{ItemRepo, UserUploadRepo};
use omeka_db::rows::UserUploadRow;
use omeka_db::with_db;
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::AppState;
use crate::tus::{self, BodyReader};

/// The most bytes one `PATCH` may carry, as an import's upload takes.
const MAX_CHUNK: u64 = uploads::MAX_CHUNK;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/romeka/uploads",
            get(list).post(create).options(options),
        )
        .route(
            "/api/romeka/uploads/{upload}",
            head(offset)
                .patch(patch)
                .delete(delete)
                .options(options_one),
        )
}

/// A refusal: its status, the code the admin branches on, the message and the
/// members that explain it.
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

fn shared(error: omeka_api::errors::ApiError) -> Box<Response> {
    let mut response = crate::error_response(error);
    response
        .headers_mut()
        .insert("tus-resumable", header_value());
    Box::new(response)
}

fn header_value() -> axum::http::HeaderValue {
    axum::http::HeaderValue::from_static(tus::VERSION)
}

fn db(error: omeka_db::DbError) -> Box<Response> {
    shared(omeka_api::errors::ApiError::internal(error))
}

fn upload_error(error: UploadError, upload: &str) -> Box<Response> {
    let refusal = match error {
        UploadError::Path { path, reason } => Refusal::new(
            StatusCode::BAD_REQUEST,
            Some("uploadPath"),
            messages::import_upload_path(path, reason),
        ),
        UploadError::FileTooLarge { limit } => Refusal::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            Some("fileTooLarge"),
            messages::upload_too_large(limit),
        )
        .with("limit", json!(limit)),
        UploadError::Missing => Refusal::new(
            StatusCode::NOT_FOUND,
            None,
            messages::media_upload_not_found(upload),
        ),
        UploadError::OffsetConflict { received } => {
            let mut refusal = Refusal::new(
                StatusCode::CONFLICT,
                Some("offsetConflict"),
                messages::import_upload_offset(received),
            )
            .with("received", json!(received));
            refusal.offset = Some(received);
            refusal
        }
        UploadError::PastLength { length } => Refusal::new(
            StatusCode::BAD_REQUEST,
            Some("pastLength"),
            messages::IMPORT_UPLOAD_PAST_LENGTH,
        )
        .with("length", json!(length)),
        UploadError::Interrupted(_) => Refusal::new(
            StatusCode::BAD_REQUEST,
            Some("interrupted"),
            messages::IMPORT_UPLOAD_INTERRUPTED,
        ),
        UploadError::Store(reason) => Refusal::new(StatusCode::INTERNAL_SERVER_ERROR, None, reason),
        UploadError::Db(error) => {
            Refusal::new(StatusCode::INTERNAL_SERVER_ERROR, None, error.to_string())
        }
    };
    refusal.refused()
}

/// The caller, refused unless signed in with the media-create grant.
async fn caller(state: &AppState, uri: &Uri, headers: &HeaderMap) -> Result<Actor, Box<Response>> {
    let actor = crate::actor_for(state, uri, headers)
        .await
        .map_err(shared)?;
    let Some(user) = actor.user() else {
        return Err(Refusal::new(
            StatusCode::UNAUTHORIZED,
            None,
            messages::AUTHENTICATION_REQUIRED,
        )
        .refused());
    };
    let _ = user;
    access::adapter(&actor, "media", Operation::Create).map_err(shared)?;
    Ok(actor)
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

/// A handler's answer; the refusal is boxed, as a `Response` is large.
type Answer = Result<Response, Box<Response>>;

fn done(answer: Answer) -> Response {
    answer.unwrap_or_else(|response| *response)
}

async fn limits(state: &AppState) -> Result<Limit, Box<Response>> {
    let settings = omeka_api::settings::upload_settings(&state.pool)
        .await
        .map_err(db)?;
    Ok(Limit::from(settings))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Shown {
    id: String,
    path: String,
    media_type: Option<String>,
    item_id: Option<u64>,
    length: u64,
    received: u64,
    created: String,
    expires: String,
}

impl From<UserUploadRow> for Shown {
    fn from(row: UserUploadRow) -> Self {
        Self {
            id: row.id,
            path: row.path,
            media_type: row.media_type,
            item_id: row.item_id.map(|item| item.0),
            length: row.length,
            received: row.received,
            created: row.created.format("%Y-%m-%dT%H:%M:%S+00:00").to_string(),
            expires: row.expires.format("%Y-%m-%dT%H:%M:%S+00:00").to_string(),
        }
    }
}

/// `OPTIONS`: the protocol, its extensions and the largest file.
pub(crate) async fn options(
    State(state): State<AppState>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    done(
        async {
            caller(&state, &uri, &headers).await?;
            let limit = limits(&state).await?;
            Ok(
                tus::header(Response::builder().status(StatusCode::NO_CONTENT))
                    .header("tus-version", tus::VERSION)
                    .header("tus-max-size", limit.max_file)
                    .header("tus-extension", "creation,termination")
                    .body(Body::empty())
                    .unwrap_or_else(|_| crate::empty_internal_error()),
            )
        }
        .await,
    )
}

/// `OPTIONS` on one upload answers as on the collection.
pub(crate) async fn options_one(
    state: State<AppState>,
    Path(_upload): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    options(state, uri, headers).await
}

/// `GET`: the caller's uploads, or those started for one item.
pub(crate) async fn list(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    done(
        async {
            let actor = caller(&state, &uri, &headers).await?;
            let Some(user) = actor.user() else {
                return Err(Refusal::new(
                    StatusCode::UNAUTHORIZED,
                    None,
                    messages::AUTHENTICATION_REQUIRED,
                )
                .refused());
            };
            let params = Params::parse(uri.query().unwrap_or(""));
            let item = match params.get("item") {
                Some(raw) => Some(raw.parse::<u64>().map(ItemId).map_err(|_| item_refusal())?),
                None => None,
            };
            let rows = with_db!(&state.pool, |db| UserUploadRepo::new(db)
                .list(user, item)
                .await)
            .map_err(db)?;
            let uploads: Vec<Shown> = rows.into_iter().map(Shown::from).collect();
            Ok(tus::header(Response::builder().status(StatusCode::OK))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "uploads": uploads }).to_string()))
                .unwrap_or_else(|_| crate::empty_internal_error()))
        }
        .await,
    )
}

fn item_refusal() -> Box<Response> {
    Refusal::new(
        StatusCode::BAD_REQUEST,
        Some("uploadItem"),
        messages::UPLOAD_ITEM,
    )
    .refused()
}

/// `POST`: start an upload.
pub(crate) async fn create(
    State(state): State<AppState>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    done(
        async {
            speaks_tus(&headers)?;
            let actor = caller(&state, &uri, &headers).await?;
            let Some(user) = actor.user() else {
                return Err(Refusal::new(
                    StatusCode::UNAUTHORIZED,
                    None,
                    messages::AUTHENTICATION_REQUIRED,
                )
                .refused());
            };
            let Some(length) = tus::number(&headers, "upload-length") else {
                return Err(Refusal::new(
                    StatusCode::BAD_REQUEST,
                    Some("uploadLength"),
                    messages::IMPORT_UPLOAD_LENGTH,
                )
                .refused());
            };
            let metadata = headers
                .get("upload-metadata")
                .and_then(|value| value.to_str().ok());
            let Some(path) = metadata.and_then(|value| tus::metadata_value(value, "filename"))
            else {
                return Err(Refusal::new(
                    StatusCode::BAD_REQUEST,
                    Some("uploadMetadata"),
                    messages::IMPORT_UPLOAD_METADATA,
                )
                .refused());
            };
            let media_type = metadata.and_then(|value| tus::metadata_value(value, "filetype"));
            let item = match metadata.and_then(|value| tus::metadata_value(value, "item")) {
                Some(raw) => {
                    let id: u64 = raw.trim().parse().map_err(|_| item_refusal())?;
                    if with_db!(&state.pool, |db| ItemRepo::new(db).get(ItemId(id)).await)
                        .map_err(db)?
                        .is_none()
                    {
                        return Err(item_refusal());
                    }
                    Some(ItemId(id))
                }
                None => None,
            };
            let limit = limits(&state).await?;
            let made = with_db!(&state.pool, |db| uploads::create(
                db, user, item, &path, media_type, length, limit,
            )
            .await)
            .map_err(|error| upload_error(error, &path))?;
            let location = format!("/api/romeka/uploads/{}", made.id);
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
pub(crate) async fn offset(
    State(state): State<AppState>,
    Path(upload): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    done(
        async {
            speaks_tus(&headers)?;
            let actor = caller(&state, &uri, &headers).await?;
            let Some(user) = actor.user() else {
                return Err(Refusal::new(
                    StatusCode::UNAUTHORIZED,
                    None,
                    messages::AUTHENTICATION_REQUIRED,
                )
                .refused());
            };
            let found = with_db!(&state.pool, |db| UserUploadRepo::new(db)
                .get(user, &upload)
                .await)
            .map_err(db)?;
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
pub(crate) async fn patch(
    State(state): State<AppState>,
    Path(upload): Path<String>,
    uri: Uri,
    headers: HeaderMap,
    body: Body,
) -> Response {
    done(
        async {
            speaks_tus(&headers)?;
            let actor = caller(&state, &uri, &headers).await?;
            let Some(user) = actor.user() else {
                return Err(Refusal::new(
                    StatusCode::UNAUTHORIZED,
                    None,
                    messages::AUTHENTICATION_REQUIRED,
                )
                .refused());
            };
            if !tus::is_offset_type(&headers) {
                return Err(Refusal::new(
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    None,
                    messages::IMPORT_UPLOAD_CONTENT_TYPE,
                )
                .refused());
            }
            let Some(start) = tus::number(&headers, "upload-offset") else {
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
            let key = format!("user/{}/{upload}", user.0);
            let lock = tus::lock_of(&key);
            let held = lock.lock().await;
            let reader = BodyReader::spawn(body, MAX_CHUNK);
            let over = reader.over();
            let appended = with_db!(&state.pool, |db| uploads::append(
                db,
                &state.store,
                user,
                &upload,
                start,
                Box::new(reader),
            )
            .await);
            drop(held);
            let appended = match appended {
                Ok(appended) => appended,
                Err(UploadError::Missing) => {
                    tus::forget_lock(&key);
                    return Err(upload_error(UploadError::Missing, &upload));
                }
                Err(_) if over.load(Ordering::Relaxed) => {
                    return Err(chunk_too_large());
                }
                Err(error) => return Err(upload_error(error, &upload)),
            };
            if appended.completed.is_some() {
                tus::forget_lock(&key);
            }
            Ok(
                tus::header(Response::builder().status(StatusCode::NO_CONTENT))
                    .header("upload-offset", appended.received)
                    .body(Body::empty())
                    .unwrap_or_else(|_| crate::empty_internal_error()),
            )
        }
        .await,
    )
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

/// `DELETE /{upload}`: remove an upload, finished or not.
pub(crate) async fn delete(
    State(state): State<AppState>,
    Path(upload): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    done(
        async {
            speaks_tus(&headers)?;
            let actor = caller(&state, &uri, &headers).await?;
            let Some(user) = actor.user() else {
                return Err(Refusal::new(
                    StatusCode::UNAUTHORIZED,
                    None,
                    messages::AUTHENTICATION_REQUIRED,
                )
                .refused());
            };
            let key = format!("user/{}/{upload}", user.0);
            let lock = tus::lock_of(&key);
            let held = lock.lock().await;
            let removed = with_db!(&state.pool, |db| uploads::remove(
                db,
                &state.store,
                user,
                &upload
            )
            .await);
            drop(held);
            tus::forget_lock(&key);
            match removed {
                Ok(true) => Ok(
                    tus::header(Response::builder().status(StatusCode::NO_CONTENT))
                        .body(Body::empty())
                        .unwrap_or_else(|_| crate::empty_internal_error()),
                ),
                Ok(false) => Err(Refusal::new(
                    StatusCode::NOT_FOUND,
                    None,
                    messages::media_upload_not_found(&upload),
                )
                .refused()),
                Err(error) => Err(db(error)),
            }
        }
        .await,
    )
}
