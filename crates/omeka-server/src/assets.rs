//! The theme asset routes (`/assets/<path>`).
//!
//! The bundled theme's document frame links every file in its manifest under
//! `RenderContext::asset_base`, which defaults to `/assets`, and the file bytes
//! live in `omeka-site`'s theme registry (`theme::ThemeAsset`). The port ships
//! no asset directory, so this route serves the bytes from the registry instead
//! of from a filesystem.
//!
//! The path after the prefix is the manifest path, `css/style.css`. The media
//! type comes from [`AssetKind::media_type`](omeka_site::theme::AssetKind::media_type),
//! so a stylesheet is `text/css` and a script `application/javascript`. With one
//! bundled theme the lookup is unambiguous; a second theme that ships the same
//! path would need the theme in the URL, which is a change to the theme
//! contract rather than to this route.
//!
//! The core assets the frame links (`css/fonts.css`, `css/iconfonts.css`,
//! `js/global.js`, `vendor/jquery/jquery.min.js`) are upstream admin assets the
//! port does not ship (the inventory records them as `unported-admin-asset`),
//! so they stay a 404 here.
//!
//! `/application/asset/thumbnails/<kind>.png` serves upstream's four fallback
//! thumbnails, copied byte for byte from
//! `application/asset/thumbnails/` at the pinned upstream commit. The API links
//! them for a media with no thumbnail of its own
//! (`omeka_api::jsonld::BaseUrl::fallback_thumbnail_url`).

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, RawQuery};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::Response;
use axum::routing::any;

use omeka_site::theme::{ThemeAsset, theme_asset, themes};

use crate::AppState;

/// The cache header for a URL whose `?v=` names the bytes it gets.
///
/// The frame links every head asset as `<path>?v=<ThemeAsset::version>`, a
/// digest of the bytes, so a deploy that changes a sheet changes the URL every
/// page links. One URL then always means one set of bytes, and a client may
/// keep them for a year without asking again.
const IMMUTABLE: &str = "public, max-age=31536000, immutable";

/// The cache header for any other request: no version, or a version that is
/// not the current bytes (a page cached before a deploy). The client keeps the
/// bytes but revalidates them with the weak `ETag` the cache layer adds
/// (`crate::http::conditional_get`), so a stale copy is never shown.
const REVALIDATE: &str = "no-cache";

/// The `/assets` routes, mounted into [`crate::build_router`].
pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/assets/{*path}", any(handle)).route(
        "/application/asset/thumbnails/{name}",
        any(fallback_thumbnail),
    )
}

/// Upstream's fallback thumbnails by file name. The names are the ones
/// `thumbnails.fallbacks` in upstream's `module.config.php` lists.
const FALLBACK_THUMBNAILS: [(&str, &[u8]); 4] = [
    (
        "default.png",
        include_bytes!("../assets/thumbnails/default.png"),
    ),
    (
        "image.png",
        include_bytes!("../assets/thumbnails/image.png"),
    ),
    (
        "video.png",
        include_bytes!("../assets/thumbnails/video.png"),
    ),
    (
        "audio.png",
        include_bytes!("../assets/thumbnails/audio.png"),
    ),
];

async fn fallback_thumbnail(method: Method, Path(name): Path<String>) -> Response {
    if method != Method::GET {
        return method_not_allowed();
    }
    let Some((_, bytes)) = FALLBACK_THUMBNAILS.iter().find(|(file, _)| *file == name) else {
        return crate::not_found().await;
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .header(header::CACHE_CONTROL, HeaderValue::from_static(REVALIDATE))
        .body(Body::from(*bytes))
        .unwrap_or_else(|_| crate::empty_internal_error())
}

async fn handle(method: Method, Path(path): Path<String>, RawQuery(query): RawQuery) -> Response {
    if method != Method::GET {
        return method_not_allowed();
    }
    match asset(&path) {
        Some(asset) => {
            let current = requested_version(query.as_deref()) == Some(asset.version().as_str());
            asset_response(asset, if current { IMMUTABLE } else { REVALIDATE })
        }
        None => crate::not_found().await,
    }
}

/// The `v` member of a query string.
fn requested_version(query: Option<&str>) -> Option<&str> {
    query?.split('&').find_map(|pair| pair.strip_prefix("v="))
}

/// The bundled asset at a manifest path, the first theme in registry order that
/// ships it.
fn asset(path: &str) -> Option<&'static ThemeAsset> {
    themes()
        .iter()
        .find_map(|theme| theme_asset(theme.info().name, path))
}

fn asset_response(asset: &ThemeAsset, cache_control: &'static str) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, asset.kind.media_type())
        .header(
            header::CACHE_CONTROL,
            HeaderValue::from_static(cache_control),
        )
        .body(Body::from(asset.content))
        .unwrap_or_else(|_| crate::empty_internal_error())
}

/// A 405 with `Allow: GET`; only `GET` serves an asset.
fn method_not_allowed() -> Response {
    Response::builder()
        .status(StatusCode::METHOD_NOT_ALLOWED)
        .header(header::ALLOW, "GET")
        .body(Body::empty())
        .unwrap_or_else(|_| crate::empty_internal_error())
}
