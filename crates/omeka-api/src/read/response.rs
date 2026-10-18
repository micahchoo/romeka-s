//! The finished answer for one read request: status, headers, and body.
//!
//! `ApiJsonStrategy::injectResponse` sets the status from the response or the
//! exception, adds `Omeka-S-Version`, and sets
//! `Content-Type: application/ld+json` for the default `jsonld` format.
//! `ApiController::getList` adds `Link` and `Omeka-S-Total-Results`. This module
//! owns those headers so `omeka-server` only copies them onto the wire.

use http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use serde::Serialize;

use crate::errors::ApiError;
use crate::jsonld::{self, BaseUrl};
use crate::read::params::{Params, Window};

/// `ApiJsonStrategy::$formats['jsonld']`.
pub const JSONLD_CONTENT_TYPE: &str = "application/ld+json";
/// `Module::VERSION` of the frozen upstream tree.
pub const OMEKA_VERSION: &str = "4.3.0-alpha";

/// `Omeka-S-Total-Results` (HTTP header names are case-insensitive; `http`
/// stores them lowercase).
pub const TOTAL_RESULTS_HEADER: &str = "omeka-s-total-results";
/// `Omeka-S-Version`.
pub const VERSION_HEADER: &str = "omeka-s-version";

/// One API answer, ready for the HTTP layer to send.
#[derive(Debug, Clone)]
pub struct ApiResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: String,
}

impl ApiResponse {
    /// A JSON-LD document with a 200 status.
    pub fn document(body: String) -> Self {
        Self {
            status: StatusCode::OK,
            headers: base_headers(),
            body,
        }
    }

    /// A list document: sets the total-results header and generated `Link`s.
    pub fn listing(body: String, total: i64, link: HeaderValue) -> Self {
        let mut headers = base_headers();
        headers.insert(
            HeaderName::from_static(TOTAL_RESULTS_HEADER),
            HeaderValue::from_str(&total.to_string())
                .unwrap_or_else(|_| HeaderValue::from_static("0")),
        );
        headers.insert(header::LINK, link);
        Self {
            status: StatusCode::OK,
            headers,
            body,
        }
    }
}

fn base_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(JSONLD_CONTENT_TYPE),
    );
    headers.insert(
        HeaderName::from_static(VERSION_HEADER),
        HeaderValue::from_static(OMEKA_VERSION),
    );
    headers
}

/// Serialize one wire document, mapping a serializer failure to a 500.
pub fn serialize<T: Serialize>(value: &T) -> Result<String, ApiError> {
    jsonld::to_string(value).map_err(|error| ApiError::Internal {
        message: error.to_string(),
    })
}

/// The `Link` header `ApiController::getList` builds.
///
/// For each of `first`, `prev`, `next`, `last` that exists, a
/// `<url>; rel="<rel>"` entry, joined with `, `. The URL is the request path
/// with the browse-defaults query (`Params::browse_pairs`) and `page` set to
/// that page number. An empty list produces an empty header value, which is
/// what `addHeaderLine` emits when `implode` has nothing to join.
pub fn link_header(
    base: &BaseUrl,
    resource: &str,
    params: &Params,
    window: &Window,
    total: i64,
) -> HeaderValue {
    let page_count = window.page_count(total);
    let mut entries = Vec::new();
    let first = 1;
    let prev = if window.page > 1 {
        Some(window.page - 1)
    } else {
        None
    };
    let next = if window.page < page_count {
        Some(window.page + 1)
    } else {
        None
    };
    let last = if page_count >= 1 {
        Some(page_count)
    } else {
        None
    };
    for (rel, page) in [
        ("first", Some(first)),
        ("prev", prev),
        ("next", next),
        ("last", last),
    ] {
        if let Some(page) = page {
            let url = page_url(base, resource, params, page);
            entries.push(format!("<{url}>; rel=\"{rel}\""));
        }
    }
    HeaderValue::from_str(&entries.join(", ")).unwrap_or_else(|_| HeaderValue::from_static(""))
}

/// The request path with the browse-defaults query and `page` set, for one link
/// entry.
fn page_url(base: &BaseUrl, resource: &str, params: &Params, page: i64) -> String {
    let pairs = params.browse_pairs(page);
    let query: Vec<String> = pairs
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect();
    format!("{}?{}", base.api_collection(resource), query.join("&"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn a_document_carries_the_version_and_content_type() {
        let response = ApiResponse::document("{}".to_string());
        assert_eq!(
            response.headers.get(header::CONTENT_TYPE).unwrap(),
            JSONLD_CONTENT_TYPE
        );
        assert_eq!(response.headers.get(VERSION_HEADER).unwrap(), OMEKA_VERSION);
    }

    #[test]
    fn link_entries_cover_the_pages_that_exist() {
        let base = BaseUrl::parse("http://localhost").unwrap();
        let params = Params::parse("per_page=2");
        let window = Window::from_params(&params);
        let link = link_header(&base, "items", &params, &window, 5);
        assert_eq!(
            link.to_str().unwrap(),
            "<http://localhost/api/items?per_page=2&sort_by_default=&sort_order_default=\
             &sort_by=id&sort_order=asc&page=1>; rel=\"first\", \
             <http://localhost/api/items?per_page=2&sort_by_default=&sort_order_default=\
             &sort_by=id&sort_order=asc&page=2>; rel=\"next\", \
             <http://localhost/api/items?per_page=2&sort_by_default=&sort_order_default=\
             &sort_by=id&sort_order=asc&page=3>; rel=\"last\""
        );
    }

    #[test]
    fn a_single_page_has_only_first_and_last() {
        let base = BaseUrl::parse("http://localhost").unwrap();
        let params = Params::parse("");
        let window = Window::from_params(&params);
        let link = link_header(&base, "items", &params, &window, 3);
        assert_eq!(
            link.to_str().unwrap(),
            "<http://localhost/api/items?sort_by_default=&sort_order_default=&sort_by=id\
             &sort_order=asc&page=1>; rel=\"first\", \
             <http://localhost/api/items?sort_by_default=&sort_order_default=&sort_by=id\
             &sort_order=asc&page=1>; rel=\"last\""
        );
    }

    /// The plugin leaves both `_default` parameters unset when the caller sorts,
    /// and keeps the caller's values (`SetBrowseDefaults::__invoke`).
    #[test]
    fn an_explicit_sort_omits_the_default_parameters() {
        let base = BaseUrl::parse("http://localhost").unwrap();
        let params = Params::parse("item_set_id=1&sort_by=id&sort_order=desc");
        let window = Window::from_params(&params);
        let link = link_header(&base, "items", &params, &window, 4);
        assert_eq!(
            link.to_str().unwrap(),
            "<http://localhost/api/items?item_set_id=1&sort_by=id&sort_order=desc&page=1>; \
             rel=\"first\", \
             <http://localhost/api/items?item_set_id=1&sort_by=id&sort_order=desc&page=1>; \
             rel=\"last\""
        );
    }
}
