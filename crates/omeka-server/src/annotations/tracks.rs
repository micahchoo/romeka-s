//! The caption routes: import, list, remove, dry run and `captions.vtt`
//! (contract 6.5, S5 of the media UI plan).
//!
//! A caption is a `supplementing` time row, so the routes are projections of the
//! annotation container. The server is the only cue parser: the admin posts a
//! file's bytes and reads the counts back.

use axum::body::Body;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri, header};
use axum::response::Response;
use omeka_api::annotation::{
    Annotations, Iris, KeptSummary, TrackFormat, TrackImport, TrackSummary, TrackUndo, iso,
};
use omeka_api::read::Params;
use omeka_db::ids::MediaId;
use serde_json::{Value, json};

use super::kept::KeptFiles;
use super::{if_none_match, page_tag, wap_error};
use crate::AppState;
use crate::annotations::wap;

/// The methods the caption routes admit.
const TRACKS_ALLOW: &str = "GET, HEAD, POST, DELETE, OPTIONS";
const CAPTIONS_ALLOW: &str = "GET, HEAD, OPTIONS";
const UNDO_ALLOW: &str = "POST, OPTIONS";
/// A `text/vtt` answer, in UTF-8.
const VTT_TYPE: &str = "text/vtt; charset=utf-8";

const TRACK_IDS: &str = "romeka-track-ids";

/// `GET`, `POST` and `DELETE` a media's caption languages.
pub(super) async fn tracks(
    State(state): State<AppState>,
    Path(id): Path<String>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    match method {
        Method::OPTIONS => wap::preflight(TRACKS_ALLOW),
        Method::GET | Method::HEAD => {
            track_list(&state, &id, &uri, &headers, method == Method::HEAD).await
        }
        Method::POST => track_post(&state, &id, &uri, &headers, &body).await,
        Method::DELETE => track_delete(&state, &id, &uri, &headers).await,
        _ => wap::method_not_allowed(TRACKS_ALLOW),
    }
}

/// `GET` a media's captions as WebVTT, one language or the default.
pub(super) async fn captions(
    State(state): State<AppState>,
    Path(id): Path<String>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    match method {
        Method::OPTIONS => wap::preflight(CAPTIONS_ALLOW),
        Method::GET | Method::HEAD => {
            captions_get(&state, &id, &uri, &headers, method == Method::HEAD).await
        }
        _ => wap::method_not_allowed(CAPTIONS_ALLOW),
    }
}

/// `POST` an undo of the last replace of one language (G6).
pub(super) async fn undo(
    State(state): State<AppState>,
    Path(id): Path<String>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    match method {
        Method::OPTIONS => wap::preflight(UNDO_ALLOW),
        Method::POST => undo_post(&state, &id, &uri, &headers).await,
        _ => wap::method_not_allowed(UNDO_ALLOW),
    }
}

async fn undo_post(state: &AppState, id: &str, uri: &Uri, headers: &HeaderMap) -> Response {
    let Some(media) = wap::media_id(id) else {
        return wap::not_found();
    };
    let iris = Iris::new(state.base.as_str());
    let params = Params::parse(uri.query().unwrap_or(""));
    let api = match wap::caller(state, uri, headers).await {
        Ok(api) => api,
        Err(response) => return *response,
    };
    let kept = KeptFiles::new(std::sync::Arc::clone(&state.store));
    let annotations = Annotations::keeping(&api, state.extents.as_ref(), &kept);
    match annotations.undo_track(media, params.get("lang")).await {
        Ok(undone) => json_document(StatusCode::OK, &undo_document(&undone)),
        Err(error) => wap_error(error, &iris),
    }
}

async fn track_post(
    state: &AppState,
    id: &str,
    uri: &Uri,
    headers: &HeaderMap,
    body: &[u8],
) -> Response {
    let Some(media) = wap::media_id(id) else {
        return wap::not_found();
    };
    let iris = Iris::new(state.base.as_str());
    let Some(format) = track_format(headers) else {
        return super::error_document(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "A caption file is text/vtt or application/x-subrip.",
        );
    };
    let params = Params::parse(uri.query().unwrap_or(""));
    let lang = params.get("lang");
    let api = match wap::caller(state, uri, headers).await {
        Ok(api) => api,
        Err(response) => return *response,
    };
    let kept = KeptFiles::new(std::sync::Arc::clone(&state.store));
    let annotations = Annotations::keeping(&api, state.extents.as_ref(), &kept);
    if params.bool("dry_run").unwrap_or(false) {
        match annotations.preview_track(media, lang, body, format).await {
            Ok(preview) => json_document(
                StatusCode::OK,
                &json!({
                    "cues": preview.cues,
                    "start": preview.start_ms,
                    "end": preview.end_ms,
                    "replaces": preview.replaces,
                    "handEdited": preview.hand_edited,
                }),
            ),
            Err(error) => wap_error(error, &iris),
        }
    } else {
        match annotations.import_track(media, lang, body, format).await {
            Ok(import) => imported(&iris, media, &import),
            Err(error) => wap_error(error, &iris),
        }
    }
}

async fn track_list(
    state: &AppState,
    id: &str,
    uri: &Uri,
    headers: &HeaderMap,
    head: bool,
) -> Response {
    let Some(media) = wap::media_id(id) else {
        return wap::not_found();
    };
    let iris = Iris::new(state.base.as_str());
    let api = match wap::caller(state, uri, headers).await {
        Ok(api) => api,
        Err(response) => return *response,
    };
    let kept = KeptFiles::new(std::sync::Arc::clone(&state.store));
    let annotations = Annotations::keeping(&api, state.extents.as_ref(), &kept);
    match annotations.tracks(media).await {
        Ok(tracks) => {
            let listed: Vec<_> = tracks.iter().map(summary_document).collect();
            let response = json_document(StatusCode::OK, &json!(listed));
            wap::without_body(response, head)
        }
        Err(error) => wap_error(error, &iris),
    }
}

async fn track_delete(state: &AppState, id: &str, uri: &Uri, headers: &HeaderMap) -> Response {
    let Some(media) = wap::media_id(id) else {
        return wap::not_found();
    };
    let iris = Iris::new(state.base.as_str());
    let params = Params::parse(uri.query().unwrap_or(""));
    let api = match wap::caller(state, uri, headers).await {
        Ok(api) => api,
        Err(response) => return *response,
    };
    let kept = KeptFiles::new(std::sync::Arc::clone(&state.store));
    let annotations = Annotations::keeping(&api, state.extents.as_ref(), &kept);
    match annotations.remove_track(media, params.get("lang")).await {
        Ok(()) => match Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Body::empty())
        {
            Ok(response) => super::annotated(response),
            Err(_) => crate::empty_internal_error(),
        },
        Err(error) => wap_error(error, &iris),
    }
}

async fn captions_get(
    state: &AppState,
    id: &str,
    uri: &Uri,
    headers: &HeaderMap,
    head: bool,
) -> Response {
    let Some(media) = wap::media_id(id) else {
        return wap::not_found();
    };
    let iris = Iris::new(state.base.as_str());
    let params = Params::parse(uri.query().unwrap_or(""));
    let api = match wap::caller(state, uri, headers).await {
        Ok(api) => api,
        Err(response) => return *response,
    };
    let annotations = Annotations::new(&api, state.extents.as_ref());
    let body = match annotations.captions_vtt(media, params.get("lang")).await {
        Ok(Some(body)) => body,
        Ok(None) => return wap::not_found(),
        Err(error) => return wap_error(error, &iris),
    };
    let tag = page_tag(VTT_TYPE, &body);
    if if_none_match(
        headers
            .get(header::IF_NONE_MATCH)
            .and_then(|value| value.to_str().ok()),
        &tag,
    ) {
        return wap::not_modified(&tag);
    }
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, VTT_TYPE)
        .header(header::ETAG, tag)
        .body(Body::from(body));
    match response {
        Ok(response) => wap::without_body(super::annotated(response), head),
        Err(_) => crate::empty_internal_error(),
    }
}

/// The import answer: the summary, the captions location and, for a small
/// enough language, the track ids.
fn imported(iris: &Iris, media: MediaId, import: &TrackImport) -> Response {
    let mut response = json_document(
        StatusCode::OK,
        &json!({
            "lang": import.lang,
            "cues": import.cues,
            "replaced": import.replaced,
        }),
    );
    let headers = response.headers_mut();
    set(
        headers,
        header::LOCATION,
        &iris.captions(media, import.lang.as_deref()),
    );
    if !import.ids.is_empty() {
        let ids: Vec<String> = import.ids.iter().map(i64::to_string).collect();
        set(headers, HeaderName::from_static(TRACK_IDS), &ids.join(" "));
    }
    response
}

fn summary_document(summary: &TrackSummary) -> Value {
    json!({
        "lang": summary.lang.clone().unwrap_or_default(),
        "cues": summary.cues,
        "imported": summary.imported.map(iso),
        "handEdited": summary.hand_edited,
        "undo": summary.undo.map(kept_document),
    })
}

/// The edition an undo would bring back: its size and when it was imported.
fn kept_document(kept: KeptSummary) -> Value {
    json!({
        "cues": kept.cues,
        "imported": kept.imported.map(iso),
    })
}

/// The answer to an undo: the edition now live again.
fn undo_document(undone: &TrackUndo) -> Value {
    json!({
        "lang": undone.lang,
        "cues": undone.cues,
        "imported": undone.imported.map(iso),
    })
}

fn json_document(status: StatusCode, value: &serde_json::Value) -> Response {
    let response = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(value.to_string()));
    match response {
        Ok(response) => super::annotated(response),
        Err(_) => crate::empty_internal_error(),
    }
}

/// `text/vtt` is WebVTT, `application/x-subrip` is SubRip; anything else is a
/// 415 (contract 6.5).
fn track_format(headers: &HeaderMap) -> Option<TrackFormat> {
    let raw = headers.get(header::CONTENT_TYPE)?.to_str().ok()?;
    let media_type = raw.split(';').next().unwrap_or(raw).trim();
    if media_type.eq_ignore_ascii_case("text/vtt") {
        Some(TrackFormat::Vtt)
    } else if media_type.eq_ignore_ascii_case("application/x-subrip") {
        Some(TrackFormat::Srt)
    } else {
        None
    }
}

fn set(headers: &mut HeaderMap, name: HeaderName, value: &str) {
    if let Ok(value) = HeaderValue::from_str(value) {
        headers.insert(name, value);
    }
}
