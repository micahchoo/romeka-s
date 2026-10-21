//! Content Search 2.0 over the notes of one item.
//!
//! `GET /iiif/search/2/item/<id>?q=<words>` answers an `AnnotationPage` of the
//! matching notes, as the canvas's annotation page describes them, so a viewer's
//! search panel can jump to the note on the canvas. Only `q` filters: the other
//! parameters the specification names are reported back in `ignored`, because a
//! caller that sent them is otherwise left believing they applied.
//!
//! The service reads as the caller of a public page (a session cookie or no
//! one), and the annotation service decides what that caller may see: a media
//! it may not read contributes no hit, and the switch leaves only captions.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;
use omeka_api::annotation::{AnnotationError, AnnotationId, Annotations, Iris};
use omeka_db::ids::ItemId;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::AppState;

/// The most hits one answer holds. Mirador asks page by page after that.
const HITS: u32 = 100;

/// The Content Search 2.0 context and document type.
const CONTEXT: &str = "http://iiif.io/api/search/2/context.json";
const TYPE: &str = "application/ld+json;profile=\"http://iiif.io/api/search/2/context.json\"";

/// The parameters this service answers and the ones it reports as ignored.
const IGNORED: [&str; 3] = ["motivation", "date", "user"];

#[derive(Deserialize)]
pub(super) struct SearchQuery {
    q: Option<String>,
    after: Option<i64>,
    motivation: Option<String>,
    date: Option<String>,
    user: Option<String>,
}

impl SearchQuery {
    /// The parameters this request sent that the service does not apply.
    fn ignored(&self) -> Vec<&'static str> {
        let sent = [
            self.motivation.is_some(),
            self.date.is_some(),
            self.user.is_some(),
        ];
        IGNORED
            .into_iter()
            .zip(sent)
            .filter_map(|(name, sent)| sent.then_some(name))
            .collect()
    }
}

/// `GET /iiif/search/2/item/<id>?q=[&after=<id>]`.
pub(super) async fn item_search(
    State(state): State<AppState>,
    Path(item): Path<u64>,
    Query(query): Query<SearchQuery>,
    headers: HeaderMap,
) -> Response {
    let Some(api) = super::visitor_api(&state, &headers).await else {
        return crate::empty_internal_error();
    };
    let iris = Iris::new(state.base.as_str());
    let words = query.q.as_deref();
    let annotations = Annotations::new(&api, state.extents.as_ref());
    let found = annotations
        .search_item(
            ItemId(item),
            words.unwrap_or_default(),
            query.after.map(AnnotationId),
            HITS,
        )
        .await;
    let (items, next) = match found {
        Ok(found) => found,
        Err(AnnotationError::NotFound) => return super::not_found(),
        Err(error) => {
            tracing::error!(%error, "a search over an item's notes failed");
            return crate::empty_internal_error();
        }
    };
    let service = iris.search(ItemId(item));
    let mut document = Map::new();
    document.insert("@context".into(), json!(CONTEXT));
    document.insert("type".into(), json!("AnnotationPage"));
    document.insert("id".into(), json!(address(&service, words, None)));
    document.insert(
        "items".into(),
        Value::Array(
            items
                .iter()
                .map(|annotation| omeka_api::annotation::render(annotation, &iris))
                .collect(),
        ),
    );
    if let Some(next) = next {
        document.insert("next".into(), json!(address(&service, words, Some(next))));
    }
    let ignored = query.ignored();
    if !ignored.is_empty() {
        document.insert("ignored".into(), json!(ignored));
    }
    answer(&Value::Object(document).to_string())
}

/// The search IRI a request asked for, and the one its next page asks for. A
/// request without `q` keeps no `q` on its own IRI.
fn address(service: &str, words: Option<&str>, after: Option<AnnotationId>) -> String {
    let mut pairs = Vec::new();
    if let Some(words) = words {
        pairs.push(("q".to_string(), words.to_string()));
    }
    if let Some(AnnotationId(id)) = after {
        pairs.push(("after".to_string(), id.to_string()));
    }
    match omeka_site::html::query_string(&pairs) {
        Some(query) => format!("{service}?{query}"),
        None => service.to_string(),
    }
}

/// A search answer. It names the caller's own hits, so no shared cache keeps it.
fn answer(body: &str) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, TYPE)
        .header(header::CACHE_CONTROL, "no-cache")
        .header(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        )
        .body(axum::body::Body::from(body.to_string()))
        .unwrap_or_else(|_| crate::empty_internal_error())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    /// A request that sends the parameters the service ignores gets them named
    /// back, in the specification's order, and only the ones it sent.
    #[test]
    fn the_ignored_list_names_what_the_caller_sent() {
        let query = SearchQuery {
            q: Some("shield".to_string()),
            after: None,
            motivation: Some("commenting".to_string()),
            date: None,
            user: Some("12".to_string()),
        };
        assert_eq!(query.ignored(), vec!["motivation", "user"]);
        let bare = SearchQuery {
            q: Some("shield".to_string()),
            after: None,
            motivation: None,
            date: None,
            user: None,
        };
        assert!(bare.ignored().is_empty());
    }

    /// The next page's IRI keeps the words and appends the keyset cursor.
    #[test]
    fn the_next_address_keeps_the_query_and_the_cursor() {
        assert_eq!(
            address(
                "http://localhost/iiif/search/2/item/8",
                Some("red shield"),
                Some(AnnotationId(41))
            ),
            "http://localhost/iiif/search/2/item/8?q=red+shield&after=41"
        );
        assert_eq!(
            address(
                "http://localhost/iiif/search/2/item/8",
                Some("shield"),
                None
            ),
            "http://localhost/iiif/search/2/item/8?q=shield"
        );
        assert_eq!(
            address("http://localhost/iiif/search/2/item/8", None, None),
            "http://localhost/iiif/search/2/item/8"
        );
    }
}
