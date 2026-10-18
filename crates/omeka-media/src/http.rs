//! The one network seam for the media subsystem.
//!
//! Four upstream ingesters reach the network: `Url` and `Youtube` through
//! `Omeka\File\Downloader`, `IIIF` and `IiifPresentation` through
//! `Laminas\Http\Client`, and `OEmbed` through `Omeka\Stdlib\Oembed`. The port
//! routes all of them through [`HttpFetch`], so a test supplies a fake and no
//! test touches the network (DESIGN.md "Exclusions: live external services").

use thiserror::Error;

/// A fetched HTTP response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// The status code.
    pub status: u16,
    /// The reason phrase, e.g. `OK`.
    pub reason: String,
    /// The response body.
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// A `200 OK` response with the given body.
    #[must_use]
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self {
            status: 200,
            reason: "OK".to_string(),
            body: body.into(),
        }
    }

    /// A response with an explicit status and reason.
    #[must_use]
    pub fn new(status: u16, reason: impl Into<String>, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            reason: reason.into(),
            body: body.into(),
        }
    }

    /// `Laminas\Http\Response::isOk`: status in the 2xx range.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// `Laminas\Http\Response::isSuccess`: alias of [`HttpResponse::is_ok`]
    /// used by `Stdlib/Oembed.php:151`.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.is_ok()
    }

    /// The body decoded as UTF-8, replacing invalid sequences.
    #[must_use]
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// Why a fetch produced no response.
///
/// A test asserts on the kind, because the message is prose. The fake in
/// [`crate::testing`] and a real client both report a kind, so a caller can
/// treat a timeout differently from a refused connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FetchErrorKind {
    /// A transport failure with no more specific kind.
    #[default]
    Transport,
    /// The URL is not an absolute http(s) URL.
    InvalidUrl,
    /// The host did not resolve or the connection was refused.
    Connect,
    /// The request or the response timed out.
    Timeout,
    /// The response body passed the client's size cap.
    TooLarge,
    /// The TLS handshake failed.
    Tls,
    /// The response could not be read.
    Body,
}

/// A fetch that did not produce a response at all.
#[derive(Debug, Error)]
#[error("fetch failed: {message}")]
pub struct FetchError {
    /// Why the fetch failed.
    pub kind: FetchErrorKind,
    /// The transport error message, as the ingesters report it.
    pub message: String,
}

impl FetchError {
    /// Build a fetch error from any message, with the default kind.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            kind: FetchErrorKind::Transport,
            message: message.into(),
        }
    }

    /// Build a fetch error with an explicit kind.
    #[must_use]
    pub fn with_kind(kind: FetchErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// Why the fetch failed.
    #[must_use]
    pub fn kind(&self) -> FetchErrorKind {
        self.kind
    }
}

/// The network seam: one method, one fake in tests.
pub trait HttpFetch {
    /// Fetch a URL. The implementation must not follow a redirect into a
    /// different scheme and must not retry; retries belong to the caller.
    fn fetch(&self, url: &str) -> Result<HttpResponse, FetchError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `isOk` is the 2xx range and `isSuccess` is the same predicate.
    #[test]
    fn status_predicates_match_laminas_response() {
        assert!(HttpResponse::ok(b"x").is_ok());
        assert!(HttpResponse::ok(b"x").is_success());
        assert!(!HttpResponse::new(404, "Not Found", b"").is_ok());
        assert!(!HttpResponse::new(500, "Error", b"").is_ok());
    }

    /// The body is readable as text with replacement for invalid bytes.
    #[test]
    fn text_replaces_invalid_utf8() {
        let response = HttpResponse::ok(vec![0x68, 0x69, 0xff]);
        assert_eq!(response.text(), "hi\u{fffd}");
    }

    /// A plain message carries the default kind, and the kind is readable back.
    #[test]
    fn fetch_error_carries_a_kind() {
        assert_eq!(FetchError::new("boom").kind(), FetchErrorKind::Transport);
        let error = FetchError::with_kind(FetchErrorKind::Timeout, "slow");
        assert_eq!(error.kind(), FetchErrorKind::Timeout);
        assert_eq!(error.message, "slow");
    }
}
