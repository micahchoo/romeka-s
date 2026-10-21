//! The Image API routes over a stored media file, and the canvas a media
//! becomes in a manifest.
//!
//! The pixel work is [`omeka_media::iiif_image`]; this module reads the media
//! as a visitor may see it, finds its file, and keeps each answer in the store
//! under `iiif/<storage id>/<request path>`. A second request for the same path
//! is a read of those bytes, with no decode. A media's answers are not deleted
//! with the media; see the unit report.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;
use omeka_media::iiif_image::{self, ImageRequest, Plan};
use omeka_media::kind::{KindInput, MediaKind};
use omeka_media::store::Store;
use serde_json::Value;

use super::manifest::{CanvasMedia, LocalImage, Painting, Size};
use crate::AppState;

/// The media type of an Image API 3 `info.json`.
const INFO_TYPE: &str = "application/ld+json;profile=\"http://iiif.io/api/image/3/context.json\"";

/// The kind of one media representation, through the crate's one kind table.
fn kind_of(media: &Value) -> MediaKind {
    let text = |key: &str| media.get(key).and_then(Value::as_str).unwrap_or("");
    MediaKind::of(&KindInput {
        renderer: text("o:renderer"),
        ingester: text("o:ingester"),
        media_type: media.get("o:media_type").and_then(Value::as_str),
        has_thumbnails: media
            .get("o:thumbnail_urls")
            .and_then(|urls| urls.get("large"))
            .and_then(Value::as_str)
            .is_some(),
    })
}

/// A stored image this server can serve through the Image API, read from a
/// media representation.
struct StoredImage {
    storage_id: String,
    original_path: String,
    original_url: String,
    media_type: String,
    large_url: String,
    square_url: Option<String>,
    is_public: bool,
}

impl StoredImage {
    /// The image a media representation stores, `None` for a media that is
    /// not a local image with derivatives.
    fn of(media: &Value, kind: MediaKind) -> Option<Self> {
        if kind != MediaKind::LocalImage {
            return None;
        }
        let text = |key: &str| media.get(key).and_then(Value::as_str);
        let thumbnail = |kind: &str| {
            media
                .get("o:thumbnail_urls")
                .and_then(|urls| urls.get(kind))
                .and_then(Value::as_str)
                .map(str::to_string)
        };
        let media_type = text("o:media_type")?;
        let large = thumbnail("large");
        let filename = text("o:filename")?;
        Some(Self {
            storage_id: filename
                .rsplit_once('.')
                .map_or(filename, |(stem, _)| stem)
                .to_string(),
            original_path: format!("original/{filename}"),
            original_url: text("o:original_url")?.to_string(),
            media_type: media_type.to_string(),
            large_url: large?,
            square_url: thumbnail("square"),
            is_public: media.get("o:is_public").and_then(Value::as_bool) == Some(true),
        })
    }

    fn large_path(&self) -> String {
        format!("large/{}.jpg", self.storage_id)
    }
}

/// The canvas a media representation becomes, `None` for a media no canvas
/// can paint.
pub(super) async fn canvas_media(state: &AppState, media: &Value) -> Option<CanvasMedia> {
    let id = media.get("o:id").and_then(Value::as_u64)?;
    let label = ["o:title", "o:source"]
        .into_iter()
        .find_map(|key| {
            media
                .get(key)
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
        })
        .unwrap_or("[Untitled]")
        .to_string();
    let kind = kind_of(media);
    let painting = match kind {
        MediaKind::LocalImage => {
            let image = StoredImage::of(media, kind)?;
            let dimensions = &state.iiif_dimensions;
            let size = dimensions.of(&state.store, &image.original_path).await?;
            let large_size = dimensions.of(&state.store, &image.large_path()).await?;
            Painting::Local(LocalImage {
                original_url: image.original_url,
                media_type: image.media_type,
                size,
                large_url: image.large_url,
                large_size,
                square_url: image.square_url,
                service: service_id(state, id),
            })
        }
        MediaKind::IiifImage => Painting::Remote(media.get("data")?.clone()),
        _ => return None,
    };
    Some(CanvasMedia {
        id,
        label,
        painting,
    })
}

fn service_id(state: &AppState, media: u64) -> String {
    format!("{}/iiif/image/3/{media}", state.base.as_str())
}

/// The stored image behind a media id and its size, as the request's caller
/// may see it.
async fn visible_image(
    state: &AppState,
    headers: &HeaderMap,
    media: u64,
) -> Option<(StoredImage, Size)> {
    let api = super::visitor_api(state, headers).await?;
    let media = super::read(&api, "media", media).await?;
    let image = StoredImage::of(&media, kind_of(&media))?;
    let size = state
        .iiif_dimensions
        .of(&state.store, &image.original_path)
        .await?;
    Some((image, size))
}

/// `GET /iiif/image/3/<media>/info.json`.
pub(super) async fn info(
    State(state): State<AppState>,
    Path(media): Path<u64>,
    headers: HeaderMap,
) -> Response {
    let Some((image, (width, height))) = visible_image(&state, &headers, media).await else {
        return super::not_found();
    };
    let info = iiif_image::info(&service_id(&state, media), width, height);
    answer(INFO_TYPE, image.is_public, Body::from(info.to_string()))
}

/// `GET /iiif/image/3/<media>/<region>/<size>/<rotation>/<quality>.<format>`.
pub(super) async fn image(
    State(state): State<AppState>,
    Path((media, region, size, rotation, file)): Path<(u64, String, String, String, String)>,
    headers: HeaderMap,
) -> Response {
    let request = match ImageRequest::parse(&region, &size, &rotation, &file) {
        Ok(request) => request,
        Err(error) => return bad_request(&error.to_string()),
    };
    let Some((image, (width, height))) = visible_image(&state, &headers, media).await else {
        return super::not_found();
    };
    let plan = match request.plan(width, height) {
        Ok(plan) => plan,
        Err(error) => return bad_request(&error.to_string()),
    };
    let cached = format!(
        "iiif/{}/{region}/{size}/{rotation}/{file}",
        image.storage_id
    );
    let store = Arc::clone(&state.store);
    let original = image.original_path.clone();
    let format = request.format;
    let bytes = tokio::task::spawn_blocking(move || {
        cached_or_rendered(store.as_ref(), &cached, &original, &plan, format)
    })
    .await;
    match bytes {
        Ok(Some(bytes)) => answer(format.media_type(), image.is_public, Body::from(bytes)),
        _ => crate::empty_internal_error(),
    }
}

/// The answer stored at `cached`, or a fresh one rendered from the original
/// and stored there. A failed write still answers; the next request renders
/// again.
fn cached_or_rendered(
    store: &dyn Store,
    cached: &str,
    original: &str,
    plan: &Plan,
    format: iiif_image::ImageFormat,
) -> Option<Vec<u8>> {
    if store.exists(cached).unwrap_or(false)
        && let Ok(bytes) = store.read(cached)
    {
        return Some(bytes);
    }
    let source = iiif_image::decode(&store.read(original).ok()?).ok()?;
    let bytes = iiif_image::render(&source, plan, format).ok()?;
    if let Err(error) = store.put(&bytes, cached, Some(format.media_type())) {
        tracing::warn!(%error, cached, "the IIIF image cache could not keep an answer");
    }
    Some(bytes)
}

/// An Image API answer. A public media's answer may sit in any cache for a
/// day; a private one stays in the browser.
fn answer(media_type: &str, is_public: bool, body: Body) -> Response {
    let cache = if is_public {
        "public, max-age=86400"
    } else {
        "private, max-age=86400"
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, media_type)
        .header(header::CACHE_CONTROL, cache)
        .header(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        )
        .body(body)
        .unwrap_or_else(|_| crate::empty_internal_error())
}

fn bad_request(message: &str) -> Response {
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        )
        .body(Body::from(message.to_string()))
        .unwrap_or_else(|_| crate::empty_internal_error())
}
