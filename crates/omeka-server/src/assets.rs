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

use axum::Router;
use axum::body::Body;
use axum::extract::Path;
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::Response;
use axum::routing::any;

use omeka_site::theme::{ThemeAsset, theme_asset, themes};

use crate::AppState;

/// How long a client may reuse a served asset.
///
/// The asset URL carries no version query, so the value balances reuse against
/// picking up a redeploy: a day. The response also carries the weak `ETag` the
/// cache layer adds (`crate::http::conditional_get`), so a client that holds the
/// bytes can revalidate instead of waiting the day out.
const CACHE_CONTROL: &str = "public, max-age=86400";

/// The `/assets` routes, mounted into [`crate::build_router`].
pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/assets/{*path}", any(handle))
}

async fn handle(method: Method, Path(path): Path<String>) -> Response {
    if method != Method::GET {
        return method_not_allowed();
    }
    match asset(&path) {
        Some(asset) => asset_response(asset),
        None => crate::not_found().await,
    }
}

/// The bundled asset at a manifest path, the first theme in registry order that
/// ships it.
fn asset(path: &str) -> Option<&'static ThemeAsset> {
    themes()
        .iter()
        .find_map(|theme| theme_asset(theme.info().name, path))
}

fn asset_response(asset: &ThemeAsset) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, asset.kind.media_type())
        .header(
            header::CACHE_CONTROL,
            HeaderValue::from_static(CACHE_CONTROL),
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
