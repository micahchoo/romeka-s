//! The WAP method handlers. Each one parses the request, calls one
//! [`Annotations`] method and copies the answer onto the wire.
//!
//! The container and page documents are built here, beside the headers they
//! answer with, so a header and the document it describes cannot drift apart.

use axum::body::{Body, Bytes};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri, header};
use axum::response::Response;
use omeka_api::annotation::{
    AnnotationError, AnnotationId, Annotations, Created, IdempotencyKey, IfMatch, Iris, Rights,
};
use omeka_api::messages;
use omeka_api::read::{Api, Params};
use omeka_db::ids::MediaId;
use omeka_db::repos::MediaRepo;
use omeka_db::with_db;

use super::{
    ALLOW_HEADERS, ALLOW_METHODS, ANNO_TYPE, ANNOTATION_LINK, CONTAINER_LINK, EXPOSE,
    container_document, if_none_match, page_document, page_tag, stored, surface_header, track_ids,
    wap_error,
};
use crate::AppState;

/// A container reader sees these methods; a writer adds POST.
const CONTAINER_READ: &str = "GET, HEAD, OPTIONS";
const CONTAINER_WRITE: &str = "GET, HEAD, POST, OPTIONS";
const PAGE_ALLOW: &str = "GET, HEAD, OPTIONS";
const ANNOTATION_ALLOW: &str = "GET, HEAD, PUT, DELETE, OPTIONS";

const SURFACE: &str = "romeka-surface";
const CANVAS: &str = "romeka-canvas";
const TRACK_IDS: &str = "romeka-track-ids";
const IDEMPOTENCY_KEY: &str = "idempotency-key";

/// `GET, HEAD, POST, OPTIONS` on a media's container, and `GET`/`HEAD` on its page.
pub(super) async fn container(
    State(state): State<AppState>,
    Path(id): Path<String>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    match method {
        Method::OPTIONS => preflight(CONTAINER_WRITE),
        Method::GET | Method::HEAD => {
            container_get(&state, &id, &uri, &headers, method == Method::HEAD).await
        }
        Method::POST => container_post(&state, &id, &uri, &headers, &body).await,
        _ => method_not_allowed(CONTAINER_WRITE),
    }
}

async fn container_get(
    state: &AppState,
    id: &str,
    uri: &Uri,
    headers: &HeaderMap,
    head: bool,
) -> Response {
    let Some(media) = media_id(id) else {
        return not_found();
    };
    let iris = Iris::new(state.base.as_str());
    let Some(origins) = super::origins(uri) else {
        return super::error_document(StatusCode::BAD_REQUEST, messages::BAD_REQUEST);
    };
    let api = match caller(state, uri, headers).await {
        Ok(api) => api,
        Err(response) => return *response,
    };
    let annotations = Annotations::new(&api, state.extents.as_ref());
    let page = match annotations.page_of(media, None, origins).await {
        Ok(page) => page,
        Err(error) => return wap_error(error, &iris),
    };
    let title = media_title(state, media).await;
    let document = container_document(
        &page,
        &iris,
        title.as_deref(),
        prefers(headers, "preferminimalcontainer"),
        prefers(headers, "prefercontainediris"),
    );
    let body = document.to_string();
    let tag = page_tag(ANNO_TYPE, &body);
    if if_none_match(header_str(headers, header::IF_NONE_MATCH), &tag) {
        return not_modified(&tag);
    }
    let mut response = super::document(StatusCode::OK, body);
    {
        let out = response.headers_mut();
        set(out, header::ETAG, &tag);
        set(
            out,
            header::ALLOW,
            match page.rights {
                Rights::ReadWrite => CONTAINER_WRITE,
                Rights::Read | Rights::ReadCaptionsOnly => CONTAINER_READ,
            },
        );
        set(out, header::LINK, CONTAINER_LINK);
        set(out, HeaderName::from_static("accept-post"), ANNO_TYPE);
        set(
            out,
            HeaderName::from_static(SURFACE),
            &surface_header(&page.media, &iris),
        );
        set(
            out,
            HeaderName::from_static(CANVAS),
            &iris.canvas(&page.media),
        );
        if let Some(ids) = track_ids(&page) {
            set(out, HeaderName::from_static(TRACK_IDS), &ids);
        }
    }
    without_body(response, head)
}

async fn container_post(
    state: &AppState,
    id: &str,
    uri: &Uri,
    headers: &HeaderMap,
    body: &[u8],
) -> Response {
    let Some(media) = media_id(id) else {
        return not_found();
    };
    let iris = Iris::new(state.base.as_str());
    if !is_annotation_type(headers) {
        return wap_error(AnnotationError::UnsupportedType, &iris);
    }
    let key = match idempotency_key(headers) {
        Ok(key) => key,
        Err(response) => return *response,
    };
    let api = match caller(state, uri, headers).await {
        Ok(api) => api,
        Err(response) => return *response,
    };
    let annotations = Annotations::new(&api, state.extents.as_ref());
    match annotations.create(media, body, &iris, key.as_ref()).await {
        Ok(Created::New(annotation)) => stored(StatusCode::CREATED, &iris, &annotation),
        Ok(Created::Replayed(annotation)) => stored(StatusCode::OK, &iris, &annotation),
        Err(error) => wap_error(error, &iris),
    }
}

/// `GET`/`HEAD` one keyset page of a media's container.
pub(super) async fn page(
    State(state): State<AppState>,
    Path(id): Path<String>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    match method {
        Method::OPTIONS => preflight(PAGE_ALLOW),
        Method::GET | Method::HEAD => {
            page_get(&state, &id, &uri, &headers, method == Method::HEAD).await
        }
        _ => method_not_allowed(PAGE_ALLOW),
    }
}

async fn page_get(
    state: &AppState,
    id: &str,
    uri: &Uri,
    headers: &HeaderMap,
    head: bool,
) -> Response {
    let Some(media) = media_id(id) else {
        return not_found();
    };
    let Some(after) = cursor(uri) else {
        return super::error_document(StatusCode::BAD_REQUEST, messages::BAD_REQUEST);
    };
    let iris = Iris::new(state.base.as_str());
    let Some(origins) = super::origins(uri) else {
        return super::error_document(StatusCode::BAD_REQUEST, messages::BAD_REQUEST);
    };
    let api = match caller(state, uri, headers).await {
        Ok(api) => api,
        Err(response) => return *response,
    };
    let annotations = Annotations::new(&api, state.extents.as_ref());
    let page = match annotations.page_of(media, after, origins).await {
        Ok(page) => page,
        Err(error) => return wap_error(error, &iris),
    };
    let content_type = if accepts_iiif(headers) {
        crate::iiif::PRESENTATION_TYPE
    } else {
        ANNO_TYPE
    };
    let body = page_document(&page, &iris, false).to_string();
    let tag = page_tag(content_type, &body);
    if if_none_match(header_str(headers, header::IF_NONE_MATCH), &tag) {
        return not_modified(&tag);
    }
    let mut response = match Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(body))
    {
        Ok(response) => response,
        Err(_) => return crate::empty_internal_error(),
    };
    {
        let out = response.headers_mut();
        set(out, header::ETAG, &tag);
        set(out, header::ALLOW, PAGE_ALLOW);
        set(
            out,
            HeaderName::from_static(CANVAS),
            &iris.canvas(&page.media),
        );
        if let Some(ids) = track_ids(&page) {
            set(out, HeaderName::from_static(TRACK_IDS), &ids);
        }
    }
    without_body(super::annotated(response), head)
}

/// `GET`, `PUT` and `DELETE` one annotation.
pub(super) async fn annotation(
    State(state): State<AppState>,
    Path(id): Path<String>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    match method {
        Method::OPTIONS => preflight(ANNOTATION_ALLOW),
        Method::GET | Method::HEAD => {
            annotation_get(&state, &id, &uri, &headers, method == Method::HEAD).await
        }
        Method::PUT => annotation_put(&state, &id, &uri, &headers, &body).await,
        Method::DELETE => annotation_delete(&state, &id, &uri, &headers).await,
        _ => method_not_allowed(ANNOTATION_ALLOW),
    }
}

async fn annotation_get(
    state: &AppState,
    id: &str,
    uri: &Uri,
    headers: &HeaderMap,
    head: bool,
) -> Response {
    let Some(id) = annotation_id(id) else {
        return not_found();
    };
    let iris = Iris::new(state.base.as_str());
    let api = match caller(state, uri, headers).await {
        Ok(api) => api,
        Err(response) => return *response,
    };
    let annotations = Annotations::new(&api, state.extents.as_ref());
    match annotations.get(id).await {
        Ok(annotation) => {
            let mut response = stored(StatusCode::OK, &iris, &annotation);
            set(response.headers_mut(), header::LINK, ANNOTATION_LINK);
            without_body(response, head)
        }
        Err(error) => wap_error(error, &iris),
    }
}

async fn annotation_put(
    state: &AppState,
    id: &str,
    uri: &Uri,
    headers: &HeaderMap,
    body: &[u8],
) -> Response {
    let Some(id) = annotation_id(id) else {
        return not_found();
    };
    let iris = Iris::new(state.base.as_str());
    if !is_annotation_type(headers) {
        return wap_error(AnnotationError::UnsupportedType, &iris);
    }
    let api = match caller(state, uri, headers).await {
        Ok(api) => api,
        Err(response) => return *response,
    };
    let annotations = Annotations::new(&api, state.extents.as_ref());
    let if_match = match precondition(headers, &annotations, id, &iris).await {
        Ok(if_match) => if_match,
        Err(response) => return *response,
    };
    match annotations.replace(id, &if_match, body, &iris).await {
        Ok(annotation) => stored(StatusCode::OK, &iris, &annotation),
        Err(error) => wap_error(error, &iris),
    }
}

async fn annotation_delete(state: &AppState, id: &str, uri: &Uri, headers: &HeaderMap) -> Response {
    let Some(id) = annotation_id(id) else {
        return not_found();
    };
    let iris = Iris::new(state.base.as_str());
    let api = match caller(state, uri, headers).await {
        Ok(api) => api,
        Err(response) => return *response,
    };
    let annotations = Annotations::new(&api, state.extents.as_ref());
    let if_match = match precondition(headers, &annotations, id, &iris).await {
        Ok(if_match) => if_match,
        Err(response) => return *response,
    };
    match annotations.delete(id, &if_match).await {
        Ok(()) => no_content(),
        Err(error) => wap_error(error, &iris),
    }
}

fn no_content() -> Response {
    match Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(Body::empty())
    {
        Ok(response) => super::annotated(response),
        Err(_) => crate::empty_internal_error(),
    }
}

/// The `If-Match` a write requires. An absent or empty header is 428; a header
/// that names no revision (a weak tag) never matches, so it is 412 with the
/// current note, which is the editor's conflict view.
async fn precondition(
    headers: &HeaderMap,
    annotations: &Annotations<'_>,
    id: AnnotationId,
    iris: &Iris,
) -> Result<IfMatch, Box<Response>> {
    let Some(text) = header_str(headers, header::IF_MATCH) else {
        return Err(Box::new(if_match_required()));
    };
    if text.trim().is_empty() {
        return Err(Box::new(if_match_required()));
    }
    match IfMatch::parse(text) {
        Some(if_match) => Ok(if_match),
        None => Err(Box::new(match annotations.get(id).await {
            Ok(current) => stored(StatusCode::PRECONDITION_FAILED, iris, &current),
            Err(error) => wap_error(error, iris),
        })),
    }
}

fn if_match_required() -> Response {
    super::error_document(
        StatusCode::PRECONDITION_REQUIRED,
        &AnnotationError::PreconditionRequired.to_string(),
    )
}

/// The caller of the request: session cookie, else the `/api` key pair.
pub(super) async fn caller<'a>(
    state: &'a AppState,
    uri: &Uri,
    headers: &HeaderMap,
) -> Result<Api<'a>, Box<Response>> {
    crate::open(state, uri, headers)
        .await
        .map_err(|error| Box::new(super::annotated(crate::error_response(error))))
}

/// The media row's title, for the container's label. `None` when the row or its
/// title is gone, which the page cannot show anyway.
async fn media_title(state: &AppState, media: MediaId) -> Option<String> {
    with_db!(&state.pool, |db| MediaRepo::new(db).get(media).await)
        .ok()
        .flatten()
        .and_then(|row| row.resource.title)
}

pub(super) fn preflight(allow: &'static str) -> Response {
    let response = Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header(header::ALLOW, allow)
        .header(header::ACCESS_CONTROL_ALLOW_METHODS, ALLOW_METHODS)
        .header(header::ACCESS_CONTROL_ALLOW_HEADERS, ALLOW_HEADERS)
        .header(header::ACCESS_CONTROL_EXPOSE_HEADERS, EXPOSE)
        .body(Body::empty());
    match response {
        Ok(response) => super::annotated(response),
        Err(_) => crate::empty_internal_error(),
    }
}

pub(super) fn method_not_allowed(allow: &'static str) -> Response {
    let mut response =
        super::error_document(StatusCode::METHOD_NOT_ALLOWED, messages::METHOD_NOT_ALLOWED);
    set(response.headers_mut(), header::ALLOW, allow);
    response
}

pub(super) fn not_found() -> Response {
    super::error_document(StatusCode::NOT_FOUND, messages::NOT_FOUND)
}

pub(super) fn not_modified(tag: &str) -> Response {
    let response = Response::builder()
        .status(StatusCode::NOT_MODIFIED)
        .header(header::ETAG, tag)
        .body(Body::empty());
    match response {
        Ok(response) => super::annotated(response),
        Err(_) => crate::empty_internal_error(),
    }
}

/// A write body must be the Web Annotation JSON type (6.7).
fn is_annotation_type(headers: &HeaderMap) -> bool {
    header_str(headers, header::CONTENT_TYPE).is_some_and(|value| {
        let media_type = value.split(';').next().unwrap_or(value).trim();
        media_type.eq_ignore_ascii_case("application/ld+json")
            || media_type.eq_ignore_ascii_case("application/json")
    })
}

fn idempotency_key(headers: &HeaderMap) -> Result<Option<IdempotencyKey>, Box<Response>> {
    let Some(value) = headers.get(HeaderName::from_static(IDEMPOTENCY_KEY)) else {
        return Ok(None);
    };
    match value.to_str().ok().and_then(IdempotencyKey::parse) {
        Some(key) => Ok(Some(key)),
        None => Err(Box::new(super::error_document(
            StatusCode::BAD_REQUEST,
            messages::BAD_REQUEST,
        ))),
    }
}

/// The keyset cursor, `None` when `after` is present but names no annotation.
fn cursor(uri: &Uri) -> Option<Option<AnnotationId>> {
    let params = Params::parse(uri.query().unwrap_or(""));
    match params.get("after") {
        None => Some(None),
        Some(raw) => raw
            .trim()
            .parse::<i64>()
            .ok()
            .filter(|id| *id > 0)
            .map(|id| Some(AnnotationId(id))),
    }
}

fn prefers(headers: &HeaderMap, token: &str) -> bool {
    header_str(headers, HeaderName::from_static("prefer"))
        .is_some_and(|value| value.to_ascii_lowercase().contains(token))
}

fn accepts_iiif(headers: &HeaderMap) -> bool {
    header_str(headers, header::ACCEPT)
        .is_some_and(|value| value.contains("iiif.io/api/presentation"))
}

pub(super) fn media_id(raw: &str) -> Option<MediaId> {
    raw.trim()
        .parse::<u64>()
        .ok()
        .filter(|id| *id > 0)
        .map(MediaId)
}

fn annotation_id(raw: &str) -> Option<AnnotationId> {
    raw.trim()
        .parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .map(AnnotationId)
}

fn header_str(headers: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

fn set(headers: &mut HeaderMap, name: HeaderName, value: &str) {
    if let Ok(value) = HeaderValue::from_str(value) {
        headers.insert(name, value);
    }
}

pub(super) fn without_body(mut response: Response, head: bool) -> Response {
    if head {
        *response.body_mut() = Body::empty();
    }
    response
}
