//! The real HTTP client behind [`HttpFetch`].
//!
//! Upstream reaches the network through two Laminas clients: `Omeka\HttpClient`
//! (the `File/Downloader`) and the two ingesters that hold a `Laminas\Http\Client`
//! directly (`Media/Ingester/IIIF.php:44`, `IiifPresentation.php:44`). This
//! module is the port's single replacement for all of them, behind the
//! [`HttpFetch`] seam the ingesters already take.
//!
//! # The cuts
//!
//! - **Blocking, because the trait is.** [`HttpFetch`] is a synchronous method
//!   and every ingester is synchronous, so the client is blocking too. A caller
//!   inside a Tokio worker should run an ingest on `spawn_blocking`.
//! - **A timeout per operation, not a deadline for the whole body.** A large
//!   download on a slow link keeps its connection; only a stalled resolve,
//!   connect, send or read is cut. Upstream sets no explicit timeout, so
//!   Laminas' Socket adapter default applies (`application/config/module.config.php:212`
//!   configures only the adapter, the SSL paths and nothing else).
//! - **A response size cap.** Upstream has none: `File/Downloader.php` streams
//!   into a temporary file. The port hands the body back as one `Vec<u8>`, so it
//!   caps the body to bound memory. The cap is the port's addition.
//! - **Same-scheme redirects only.** The [`HttpFetch`] contract forbids
//!   following a redirect into another scheme. A same-scheme redirect is
//!   followed up to [`MAX_REDIRECTS`] times, matching Laminas' default
//!   `maxredirects` of 5. A cross-scheme redirect comes back as the 3xx
//!   response, and the caller's non-2xx handling reports it.
//! - **No retry.** Upstream retries a transport error three times
//!   (`File/Downloader.php:61`). The trait puts retries on the caller, and no
//!   caller retries yet.

use std::time::Duration;

use crate::http::{FetchError, FetchErrorKind, HttpFetch, HttpResponse};

/// The timeout applied to each network operation: resolve, connect, send,
/// receive headers, receive body.
///
/// Laminas' Socket adapter defaults to 10 seconds. The port picks 30 to leave
/// headroom for a large media file, and exposes [`HttpClient::with_timeout`] to
/// change it.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// The default cap on a response body: 64 MiB.
///
/// The port's addition; upstream streams to a file and caps nothing.
pub const DEFAULT_MAX_BYTES: usize = 64 * 1024 * 1024;

/// The largest number of redirects the client follows.
///
/// Laminas' `maxredirects` default, which the upstream config does not override.
pub const MAX_REDIRECTS: usize = 5;

/// The user agent, as `Laminas\Http\Client` sends its own.
const USER_AGENT: &str = concat!("omeka-s-port/", env!("CARGO_PKG_VERSION"));

/// A blocking HTTP/1.1 client over rustls, behind [`HttpFetch`].
///
/// Build one with [`HttpClient::new`] and hand it to an ingester through
/// [`crate::ingest::IngestEnv::http`]. A command that drives an ingester owns
/// the client, because it owns the process' network policy.
#[derive(Debug, Clone)]
pub struct HttpClient {
    timeout: Duration,
    max_bytes: usize,
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient {
    /// A client with [`DEFAULT_TIMEOUT`] and [`DEFAULT_MAX_BYTES`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }

    /// Replace the per-operation timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Replace the response body cap.
    #[must_use]
    pub fn with_max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = max_bytes;
        self
    }

    /// The per-operation timeout.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// The response body cap.
    #[must_use]
    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    /// Build an agent for one fetch.
    ///
    /// One agent per fetch costs a connection pool per call, which is nothing
    /// next to one ingest's one or two requests, and keeps the type free of a
    /// connection handle that would have to be rebuilt whenever the timeout
    /// changes.
    fn agent(&self) -> ureq::Agent {
        let config = ureq::config::Config::builder()
            // A 3xx must reach the caller as a response, not an error, so the
            // ingesters' `is_ok` check reports it the way the fake does.
            .http_status_as_error(false)
            // Redirects are followed by hand, to hold the scheme rule.
            .max_redirects(0)
            .user_agent(USER_AGENT)
            // Upstream disables compressed responses, which break alongside the
            // streaming download (`File/Downloader.php:57`).
            .accept_encoding("identity")
            .timeout_resolve(Some(self.timeout))
            .timeout_connect(Some(self.timeout))
            .timeout_send_request(Some(self.timeout))
            .timeout_recv_response(Some(self.timeout))
            .timeout_recv_body(Some(self.timeout))
            .build();
        ureq::Agent::new_with_config(config)
    }

    /// One request with no redirect handling: the response, its body and the
    /// `Location` header when the response carries one.
    fn request(&self, url: &str) -> Result<(HttpResponse, Option<String>), FetchError> {
        let response = self
            .agent()
            .get(url)
            .call()
            .map_err(|error| map_error(error, self.max_bytes))?;
        let status = response.status().as_u16();
        let reason = response
            .status()
            .canonical_reason()
            .unwrap_or_default()
            .to_string();
        let location = response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        // Read one byte past the cap, so a body of exactly the cap passes and a
        // body one byte over fails as `TooLarge`.
        let body = response
            .into_body()
            .into_with_config()
            .limit(self.max_bytes as u64 + 1)
            .read_to_vec()
            .map_err(|error| map_error(error, self.max_bytes))?;
        if body.len() > self.max_bytes {
            return Err(too_large(self.max_bytes));
        }
        Ok((
            HttpResponse {
                status,
                reason,
                body,
            },
            location,
        ))
    }
}

impl HttpFetch for HttpClient {
    /// Fetch a URL, following same-scheme redirects up to [`MAX_REDIRECTS`].
    fn fetch(&self, url: &str) -> Result<HttpResponse, FetchError> {
        let mut current = parse_http_url(url)?;
        for _ in 0..=MAX_REDIRECTS {
            let (response, location) = self.request(current.as_str())?;
            if !(300..400).contains(&response.status) {
                return Ok(response);
            }
            let Some(location) = location else {
                return Ok(response);
            };
            let Ok(next) = current.join(&location) else {
                return Ok(response);
            };
            if next.scheme() != current.scheme() {
                return Ok(response);
            }
            current = next;
        }
        Err(FetchError::with_kind(
            FetchErrorKind::Transport,
            format!("too many redirects fetching {url}"),
        ))
    }
}

/// Parse a URL the client will fetch, rejecting any scheme but http(s).
fn parse_http_url(url: &str) -> Result<url::Url, FetchError> {
    let parsed = url::Url::parse(url).map_err(|error| {
        FetchError::with_kind(
            FetchErrorKind::InvalidUrl,
            format!("invalid URL {url}: {error}"),
        )
    })?;
    match parsed.scheme() {
        "http" | "https" => Ok(parsed),
        other => Err(FetchError::with_kind(
            FetchErrorKind::InvalidUrl,
            format!("unsupported scheme {other} in {url}"),
        )),
    }
}

fn too_large(max_bytes: usize) -> FetchError {
    FetchError::with_kind(
        FetchErrorKind::TooLarge,
        format!("response body exceeded the {max_bytes} byte cap"),
    )
}

/// Map a ureq failure onto the port's error kinds.
fn map_error(error: ureq::Error, max_bytes: usize) -> FetchError {
    if let ureq::Error::BodyExceedsLimit(_) = &error {
        return too_large(max_bytes);
    }
    let kind = match &error {
        ureq::Error::Timeout(_) => FetchErrorKind::Timeout,
        ureq::Error::Io(io) if io.kind() == std::io::ErrorKind::TimedOut => FetchErrorKind::Timeout,
        ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => FetchErrorKind::Connect,
        ureq::Error::Io(io) if is_connect_error(io) => FetchErrorKind::Connect,
        ureq::Error::BadUri(_) => FetchErrorKind::InvalidUrl,
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) => FetchErrorKind::Tls,
        _ => FetchErrorKind::Transport,
    };
    FetchError::with_kind(kind, format!("fetch failed: {error}"))
}

fn is_connect_error(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::NotConnected
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A non-http scheme and a relative string are `InvalidUrl`, caught before
    /// any socket opens.
    #[test]
    fn rejects_a_non_http_url() {
        let client = HttpClient::new();
        assert_eq!(
            client.fetch("ftp://example.org/a").map_err(|e| e.kind()),
            Err(FetchErrorKind::InvalidUrl)
        );
        assert_eq!(
            client.fetch("example.org/a").map_err(|e| e.kind()),
            Err(FetchErrorKind::InvalidUrl)
        );
    }

    /// The builder carries the timeout and the cap.
    #[test]
    fn builder_sets_the_limits() {
        let client = HttpClient::new()
            .with_timeout(Duration::from_millis(5))
            .with_max_bytes(16);
        assert_eq!(client.timeout(), Duration::from_millis(5));
        assert_eq!(client.max_bytes(), 16);
        assert_eq!(HttpClient::default().max_bytes(), DEFAULT_MAX_BYTES);
    }
}
