//! The IIIF routes: the viewer page, the Presentation 3 documents for the
//! site's own items and item sets, and the Image API over their stored images.
//!
//! - `GET /iiif-viewer?url=&mirador_config=` is upstream's viewer route
//!   (`routes.config.php:370-379`), rendered by [`omeka_site::iiif_viewer`].
//! - `GET /iiif/3/item/<id>/manifest` and `GET /iiif/3/item-set/<id>/collection`
//!   ([`manifest`]) and `GET /iiif/image/3/<media>/...` ([`image`]) are Romeka
//!   additions; upstream needs the IIIF Server module for them.
//!
//! Every document is read through the API as the caller of a public page
//! reads it: the session cookie or no one, never a key in the query, so a
//! private item or media is a 404 to a visitor exactly as its `/s/` page is.
//! The documents are fetched from other origins by viewers, so they answer
//! with `Access-Control-Allow-Origin: *`. That shares no private document,
//! because a cross-origin fetch carries no cookie.

mod dimensions;
mod image;
mod manifest;

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;
use axum::routing::get;
use omeka_api::auth::Actor;
use omeka_api::read::{Api, Params};
use serde::Deserialize;
use serde_json::Value;

use crate::AppState;

pub(crate) use dimensions::Dimensions;

/// The viewer page's policy. Mirador and its starter come from this origin and
/// no inline script runs. Mirador styles itself through injected `<style>`
/// elements, so inline styles are allowed. A manifest and its images may live
/// on any https origin, which is what a remote manifest needs.
const VIEWER_CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
     img-src 'self' https: data: blob:; connect-src 'self' https:; font-src 'self' data:; \
     frame-ancestors 'self'; base-uri 'none'; form-action 'none'";

/// The media type of a Presentation 3 document.
const PRESENTATION_TYPE: &str =
    "application/ld+json;profile=\"http://iiif.io/api/presentation/3/context.json\"";

/// The IIIF routes, mounted into [`crate::build_router`].
pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/iiif-viewer", get(viewer))
        .route("/iiif/3/item/{id}/manifest", get(item_manifest))
        .route("/iiif/3/item-set/{id}/collection", get(item_set_collection))
        .route("/iiif/image/3/{media}/info.json", get(image::info))
        .route(
            "/iiif/image/3/{media}/{region}/{size}/{rotation}/{file}",
            get(image::image),
        )
}

#[derive(Deserialize)]
struct ViewerQuery {
    url: Option<String>,
    mirador_config: Option<String>,
}

/// `IiifViewerController::indexAction`.
async fn viewer(Query(query): Query<ViewerQuery>) -> Response {
    let config = omeka_site::iiif_viewer::mirador_config(
        query.url.as_deref(),
        query.mirador_config.as_deref(),
    );
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header("content-security-policy", VIEWER_CSP)
        .body(Body::from(omeka_site::iiif_viewer::viewer_page(
            &config, "/assets",
        )))
        .unwrap_or_else(|_| crate::empty_internal_error())
}

/// `GET /iiif/3/item/<id>/manifest`.
async fn item_manifest(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    headers: HeaderMap,
) -> Response {
    let Some(api) = visitor_api(&state, &headers).await else {
        return crate::empty_internal_error();
    };
    let Some(item) = read(&api, "items", id).await else {
        return not_found();
    };
    let mut media = Vec::new();
    for media_id in member_ids(&item, "o:media") {
        if let Some(row) = read(&api, "media", media_id).await
            && let Some(canvas) = image::canvas_media(&state, &row).await
        {
            media.push(canvas);
        }
    }
    let manifest_id = format!("{}/iiif/3/item/{id}/manifest", state.base.as_str());
    document(&manifest::item_manifest(&manifest_id, &item, &media))
}

/// `GET /iiif/3/item-set/<id>/collection`: one manifest per item the caller
/// may see, in id order.
async fn item_set_collection(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    headers: HeaderMap,
) -> Response {
    let Some(api) = visitor_api(&state, &headers).await else {
        return crate::empty_internal_error();
    };
    let Some(item_set) = read(&api, "item_sets", id).await else {
        return not_found();
    };
    let base = state.base.as_str();
    let mut items = Vec::new();
    for page in 1.. {
        let params = Params::from_decoded(vec![
            ("item_set_id".to_string(), id.to_string()),
            ("per_page".to_string(), PAGE.to_string()),
            ("page".to_string(), page.to_string()),
        ]);
        let Ok(answer) = api.search("items", &params).await else {
            return crate::empty_internal_error();
        };
        let found: Vec<Value> = serde_json::from_str(&answer.body).unwrap_or_default();
        let last = found.len() < PAGE;
        for item in found {
            if let Some(item_id) = item.get("o:id").and_then(Value::as_u64) {
                items.push((format!("{base}/iiif/3/item/{item_id}/manifest"), item));
            }
        }
        if last {
            break;
        }
    }
    let collection_id = format!("{base}/iiif/3/item-set/{id}/collection");
    document(&manifest::item_set_collection(
        &collection_id,
        &item_set,
        &items,
    ))
}

/// The search page size the collection reads with.
const PAGE: usize = 100;

/// The read surface for the request's session, or a visitor's when it has
/// none. `None` only when the session store fails.
async fn visitor_api<'a>(state: &'a AppState, headers: &HeaderMap) -> Option<Api<'a>> {
    let actor = crate::session_actor(state, headers)
        .await
        .ok()?
        .unwrap_or_else(Actor::anonymous);
    Api::load_with_store(
        &state.pool,
        state.base.clone(),
        actor,
        Some(state.store.clone()),
    )
    .await
    .ok()
}

/// One member's representation, `None` when the caller may not see it.
async fn read(api: &Api<'_>, resource: &str, id: u64) -> Option<Value> {
    let answer = api.read_member(resource, &id.to_string()).await.ok()?;
    if answer.status != StatusCode::OK {
        return None;
    }
    serde_json::from_str(&answer.body).ok()
}

/// The `o:id` of every reference in a list member such as `o:media`.
fn member_ids(resource: &Value, key: &str) -> Vec<u64> {
    resource
        .get(key)
        .and_then(Value::as_array)
        .map(|refs| {
            refs.iter()
                .filter_map(|reference| reference.get("o:id").and_then(Value::as_u64))
                .collect()
        })
        .unwrap_or_default()
}

fn document(document: &Value) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, PRESENTATION_TYPE)
        .header(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        )
        .body(Body::from(document.to_string()))
        .unwrap_or_else(|_| crate::empty_internal_error())
}

fn not_found() -> Response {
    crate::document_response(
        StatusCode::NOT_FOUND,
        &omeka_api::errors::ErrorDocument::general(omeka_api::messages::NOT_FOUND),
    )
}
