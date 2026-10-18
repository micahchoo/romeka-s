//! The same-origin check a cookie-authenticated write must pass.
//!
//! `SameSite=Lax` stops a cross-site `POST` from carrying the session cookie in
//! a current browser, but it is a browser default rather than a server check,
//! and it does not cover a same-site subdomain, a browser that ignores the
//! attribute, or a *safe-method* escalation such as a cross-site form post to a
//! route that treats `GET` as a write. The server therefore checks the request's
//! own origin on every write that carries the session cookie.
//!
//! The header to compare is `Origin`, which the Fetch standard sends on every
//! non-`GET`/`HEAD` request, including same-origin ones. `Referer` is the
//! fallback for a client that sends no `Origin`; a request that sends neither is
//! refused, because a browser-attached cookie would have had to come from
//! somewhere and nothing in the request says where.
//!
//! `Origin: null` (a sandboxed iframe, a `data:` URL) parses to no origin and is
//! refused. Only the scheme, host and effective port are compared, so a base URL
//! with a path (`https://museum.example/omeka`) still matches the origin the
//! browser reports, and an explicit default port (`:443`) equals its absence.
//!
//! Upstream has no equivalent check: `LoginController` and the admin controllers
//! rely on the Laminas CSRF form element for a form post, and the port's API is
//! reachable with a JSON body from a script instead, so the origin check is the
//! port's own boundary.

use std::str::FromStr;

use http::HeaderMap;
use http::header::{ORIGIN, REFERER};
use http::uri::Uri;

use crate::jsonld::BaseUrl;

/// Whether the request came from the configured base URL's origin.
#[must_use]
pub fn is_same_origin(headers: &HeaderMap, base: &BaseUrl) -> bool {
    let Some(base) = Origin::from_str(base.as_str()).ok() else {
        return false;
    };
    if let Some(origin) = header(headers, ORIGIN) {
        // The specification allows a list; a browser sends one.
        return origin
            .split_whitespace()
            .any(|candidate| Origin::from_str(candidate).is_ok_and(|value| value == base));
    }
    if let Some(referer) = header(headers, REFERER) {
        return Origin::from_str(referer).is_ok_and(|value| value == base);
    }
    false
}

fn header(headers: &HeaderMap, name: http::header::HeaderName) -> Option<&str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

/// A request origin: scheme, host and effective port.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Origin {
    scheme: String,
    host: String,
    port: u16,
}

impl FromStr for Origin {
    type Err = ();

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let uri = Uri::from_str(raw).map_err(|_| ())?;
        let scheme = uri.scheme_str().ok_or(())?.to_ascii_lowercase();
        let authority = uri.authority().ok_or(())?;
        let host = authority.host().to_ascii_lowercase();
        if host.is_empty() {
            return Err(());
        }
        let port = match authority.port_u16() {
            Some(port) => port,
            None if scheme == "https" => 443,
            None => 80,
        };
        Ok(Self { scheme, host, port })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use http::HeaderValue;

    fn base() -> BaseUrl {
        BaseUrl::parse("https://museum.example").unwrap()
    }

    fn headers(origin: Option<&str>, referer: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(origin) = origin {
            headers.insert(ORIGIN, HeaderValue::from_str(origin).unwrap());
        }
        if let Some(referer) = referer {
            headers.insert(REFERER, HeaderValue::from_str(referer).unwrap());
        }
        headers
    }

    #[test]
    fn an_origin_matching_the_base_url_is_accepted() {
        assert!(is_same_origin(
            &headers(Some("https://museum.example"), None),
            &base()
        ));
    }

    #[test]
    fn an_explicit_default_port_matches_its_absence() {
        assert!(is_same_origin(
            &headers(Some("https://museum.example:443"), None),
            &base()
        ));
        let plain = BaseUrl::parse("http://localhost:80").unwrap();
        assert!(is_same_origin(
            &headers(Some("http://localhost"), None),
            &plain
        ));
    }

    #[test]
    fn another_origin_is_refused() {
        assert!(!is_same_origin(
            &headers(Some("https://evil.example"), None),
            &base()
        ));
        assert!(!is_same_origin(
            &headers(Some("https://museum.example.evil.example"), None),
            &base()
        ));
        assert!(!is_same_origin(
            &headers(Some("http://museum.example"), None),
            &base()
        ));
        assert!(!is_same_origin(
            &headers(Some("https://museum.example:8443"), None),
            &base()
        ));
    }

    #[test]
    fn a_null_or_absent_origin_is_refused() {
        assert!(!is_same_origin(&headers(Some("null"), None), &base()));
        assert!(!is_same_origin(&headers(Some(""), None), &base()));
        assert!(!is_same_origin(&headers(None, None), &base()));
    }

    #[test]
    fn the_referer_is_the_fallback_when_no_origin_arrives() {
        assert!(is_same_origin(
            &headers(None, Some("https://museum.example/admin/#/users")),
            &base()
        ));
        assert!(!is_same_origin(
            &headers(None, Some("https://evil.example/admin")),
            &base()
        ));
        // An `Origin` that is present but wrong is not rescued by a referer.
        assert!(!is_same_origin(
            &headers(
                Some("https://evil.example"),
                Some("https://museum.example/x")
            ),
            &base()
        ));
    }

    #[test]
    fn a_base_url_carrying_a_path_still_matches_its_origin() {
        let nested = BaseUrl::parse("https://museum.example/omeka").unwrap();
        assert!(is_same_origin(
            &headers(Some("https://museum.example"), None),
            &nested
        ));
    }
}
