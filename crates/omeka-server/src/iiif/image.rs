//! The Image API routes over a stored media file, and the canvas a media
//! becomes in a manifest.
//!
//! The pixel work is [`omeka_media::iiif_image`]; this module reads the media
//! as a visitor may see it, finds its file, and keeps each answer in the store
//! under `iiif/v2/<storage id>/<request path>`. A second request for the same
//! path is a read of those bytes, with no decode. The version segment keeps an
//! answer rendered before the cache learned EXIF orientation and SVG from
//! outliving the change. A media's answers are not deleted with the media; see
//! the unit report.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;
use omeka_db::ids::MediaId;
use omeka_media::iiif_image::{self, ImageRequest, Plan};
use omeka_media::kind::{KindInput, MediaKind};
use omeka_media::store::Store;
use serde_json::Value;

use super::manifest::{CanvasMedia, LocalImage, Original, Painting, Size};
use crate::AppState;

/// The media type of an Image API 3 `info.json`.
const INFO_TYPE: &str = "application/ld+json;profile=\"http://iiif.io/api/image/3/context.json\"";

/// The ready access copy a media representation reports: the URL to play and
/// its type. `None` when the original is what a browser plays, the copy is
/// still being made, or the type has no copy.
fn access_copy(media: &Value) -> Option<(String, String)> {
    let access = media.get("o-module-romeka:access")?;
    if access.get("state").and_then(Value::as_str) != Some("ready") {
        return None;
    }
    Some((
        access.get("url").and_then(Value::as_str)?.to_string(),
        access
            .get("media_type")
            .and_then(Value::as_str)?
            .to_string(),
    ))
}

/// The kind of one media representation, through the crate's one kind table.
fn kind_of(media: &Value) -> MediaKind {
    let text = |key: &str| media.get(key).and_then(Value::as_str).unwrap_or("");
    MediaKind::of(&KindInput {
        renderer: text("o:renderer"),
        ingester: text("o:ingester"),
        media_type: media.get("o:media_type").and_then(Value::as_str),
        extension: media
            .get("o:filename")
            .and_then(Value::as_str)
            .and_then(|filename| filename.rsplit_once('.'))
            .map(|(_, extension)| extension),
        has_thumbnails: media
            .get("o:thumbnail_urls")
            .and_then(|urls| urls.get("large"))
            .and_then(Value::as_str)
            .is_some(),
    })
}

/// The stored file behind a media representation: the names a canvas, an
/// extent read and the Image API all build paths from.
struct StoredFile {
    /// The filename's stem, which every derivative is named after.
    storage_id: String,
    /// The filename's extension, which `ffprobe` reads to pick a demuxer.
    extension: Option<String>,
    original_path: String,
    original_url: Option<String>,
    media_type: Option<String>,
    large_url: Option<String>,
}

impl StoredFile {
    /// The file a media representation stores, `None` for a media with no
    /// stored original.
    fn of(media: &Value) -> Option<Self> {
        let text = |key: &str| media.get(key).and_then(Value::as_str);
        let filename = text("o:filename")?;
        let (storage_id, extension) = match filename.rsplit_once('.') {
            Some((stem, extension)) => (stem.to_string(), Some(extension.to_string())),
            None => (filename.to_string(), None),
        };
        let url = |kind: &str| {
            media
                .get("o:thumbnail_urls")
                .and_then(|urls| urls.get(kind))
                .and_then(Value::as_str)
                .map(str::to_string)
        };
        Some(Self {
            storage_id,
            extension,
            original_path: format!("original/{filename}"),
            original_url: text("o:original_url").map(str::to_string),
            media_type: text("o:media_type").map(str::to_string),
            large_url: url("large"),
        })
    }
}

/// A stored image this server can serve through the Image API, read from a
/// media representation.
struct StoredImage {
    storage_id: String,
    /// The file the Image API decodes: the original, or for camera RAW, whose
    /// sensor data no decoder here reads, the preview-derived large derivative.
    display_path: String,
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
        let file = StoredFile::of(media)?;
        let square_url = media
            .get("o:thumbnail_urls")
            .and_then(|urls| urls.get("square"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let media_type = file.media_type?;
        let display_path = if omeka_media::media_type::is_raw_media_type(&media_type) {
            format!("large/{}.jpg", file.storage_id)
        } else {
            file.original_path
        };
        Some(Self {
            storage_id: file.storage_id,
            display_path,
            original_url: file.original_url?,
            media_type,
            large_url: file.large_url?,
            square_url,
            is_public: media.get("o:is_public").and_then(Value::as_bool) == Some(true),
        })
    }

    fn large_path(&self) -> String {
        format!("large/{}.jpg", self.storage_id)
    }
}

/// The canvas a media representation becomes, `None` for a media no canvas
/// can paint.
///
/// A time-based canvas needs a duration, so an audio or video media whose
/// extent is unknown gets none; its notes still work through the annotation
/// service.
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
            let extents = &state.extents;
            let size = extents.image(&image.display_path).await?;
            let large_size = extents.image(&image.large_path()).await?;
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
        MediaKind::Pdf { first_page: true } => {
            let file = StoredFile::of(media)?;
            let page = format!("large/{}.jpg", file.storage_id);
            let size = state.extents.image(&page).await?;
            Painting::Still {
                url: file.large_url?,
                size,
                pdf_url: file.original_url?,
            }
        }
        MediaKind::Audio | MediaKind::Video => {
            let file = StoredFile::of(media)?;
            let extent = state
                .extents
                .av(MediaId(id), &file.original_path, file.extension.as_deref())
                .await;
            let duration_s = extent.duration_ms? as f64 / 1000.0;
            let original = Original {
                url: file.original_url?,
                media_type: file.media_type?,
            };
            // A ready access copy is what a viewer plays; the original stays
            // linked through `rendering`. The duration is the probe's, which the
            // copy keeps, so a time-based note still names a moment inside it.
            let (url, media_type, original) = match access_copy(media) {
                Some((url, media_type)) => (url, media_type, Some(original)),
                None => (original.url.clone(), original.media_type.clone(), None),
            };
            match kind {
                MediaKind::Audio => Painting::Sound {
                    url,
                    media_type,
                    duration_s,
                    original,
                },
                _ => Painting::Video {
                    url,
                    media_type,
                    duration_s,
                    size: extent.size,
                    poster: file.large_url,
                    original,
                },
            }
        }
        _ => return None,
    };
    Some(CanvasMedia {
        id,
        label,
        painting,
        annotation_page: None,
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
    let size = state.extents.image(&image.display_path).await?;
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
    // A version segment, so an answer rendered before this cache learned
    // EXIF orientation (or SVG) is never served in place of a fresh one.
    let cached = format!(
        "iiif/v2/{}/{region}/{size}/{rotation}/{file}",
        image.storage_id
    );
    let store = Arc::clone(&state.store);
    let display = image.display_path.clone();
    let format = request.format;
    let bytes = tokio::task::spawn_blocking(move || {
        cached_or_rendered(store.as_ref(), &cached, &display, &plan, format)
    })
    .await;
    match bytes {
        Ok(Some(bytes)) => answer(format.media_type(), image.is_public, Body::from(bytes)),
        _ => crate::empty_internal_error(),
    }
}

/// The answer stored at `cached`, or a fresh one rendered from the display
/// file and stored there. A failed write still answers; the next request
/// renders again.
fn cached_or_rendered(
    store: &dyn Store,
    cached: &str,
    display: &str,
    plan: &Plan,
    format: iiif_image::ImageFormat,
) -> Option<Vec<u8>> {
    if store.exists(cached).unwrap_or(false)
        && let Ok(bytes) = store.read(cached)
    {
        return Some(bytes);
    }
    let source = iiif_image::decode(&store.read(display).ok()?).ok()?;
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
