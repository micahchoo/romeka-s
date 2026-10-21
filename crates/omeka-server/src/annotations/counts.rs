//! `GET /annotations/item/{id}/counts`: the note count of every file of an
//! item the caller may read, with the write each file's container would allow.
//!
//! The admin item page draws a card's count and its action, its first file's
//! summary and "All notes in this item" from this one document, so a visit
//! reads once instead of reading one WAP container per visible file. The
//! annotation service applies the same visibility as the container: the public
//! notes switch, the caller's rights, and the notes view's own rows, so an
//! imported caption cue is a row the container lists but not a note this counts.
//!
//! This route is Romeka's own, like the rest of `/annotations`; upstream has no
//! annotation server. `docs/compat.md` records it.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::Response;
use omeka_api::annotation::{Annotations, Rights};
use omeka_db::ids::ItemId;
use serde_json::{Map, Value, json};

use super::wap::caller;
use crate::AppState;

/// The media type of the counts document.
const COUNTS_TYPE: &str = "application/json";

/// `GET /annotations/item/{id}/counts`.
pub(super) async fn counts(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let Some(item) = item_id(&id) else {
        return super::wap::not_found();
    };
    let api = match caller(&state, &uri, &headers).await {
        Ok(api) => api,
        Err(response) => return *response,
    };
    let annotations = Annotations::new(&api, state.extents.as_ref());
    let counts = match annotations.item_counts(item).await {
        Ok(counts) => counts,
        Err(error) => {
            tracing::error!(%error, item = item.0, "the item's note counts could not be read");
            return crate::empty_internal_error();
        }
    };
    let mut media = Map::with_capacity(counts.len());
    for count in counts {
        media.insert(
            count.media.0.to_string(),
            json!({
                "count": count.count,
                "writable": count.rights == Rights::ReadWrite,
            }),
        );
    }
    let body = json!({ "media": Value::Object(media) }).to_string();
    match Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, COUNTS_TYPE)
        // A count follows every write, and it answers one caller's rights.
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(body))
    {
        Ok(response) => response,
        Err(_) => crate::empty_internal_error(),
    }
}

fn item_id(raw: &str) -> Option<ItemId> {
    raw.trim()
        .parse::<u64>()
        .ok()
        .filter(|id| *id > 0)
        .map(ItemId)
}
