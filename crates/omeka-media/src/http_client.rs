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
//! - **Every hop passes the URL guard.** Upstream fetches any URL. The port
//!   admits each request and each redirect through [`crate::http_guard`] and
//!   connects only to the addresses the guard checked: ureq gets a resolver
//!   that answers those addresses and never looks the name up again, and no
//!   proxy, because a proxy would resolve the target itself.
//! - **Blocking, because the trait is.** [`HttpFetch`] is a synchronous method
//!   and every ingester is synchronous, so the client is blocking too. A caller
//!   inside a Tokio worker should run an ingest on `spawn_blocking`.
//! - **An idle timeout and an overall cap.** A fetch gives up after
//!   [`DEFAULT_IDLE_TIMEOUT`] (60 seconds) to connect, to answer, and between
//!   two chunks of the body, so a slow but live download of a large file
//!   finishes. [`DEFAULT_TIMEOUT`] (one hour) caps the whole fetch, redirects
//!   included. Upstream sets no explicit timeout, so Laminas' Socket adapter
//!   default applies. ureq's own timeouts are all totals, so the transport
//!   clamps every wait on the socket to the idle timeout: a stalled read fails
//!   on the caller's thread, and dropping the response closes the socket.
//! - **A response size cap, and a streamed body.** Upstream has none:
//!   `File/Downloader.php` streams into a temporary file.
//!   [`HttpClient::fetch_stream`] hands the body back as a reader, so a caller
//!   streams it into the store; [`HttpFetch::fetch`] reads it into one
//!   `Vec<u8>`. Both cap the body at [`DEFAULT_MAX_BYTES`], the contract's
//!   `import_upload_max_file`.
//! - **Same-scheme redirects only.** The [`HttpFetch`] contract forbids
//!   following a redirect into another scheme. A same-scheme redirect is
//!   followed up to [`MAX_REDIRECTS`] times, matching Laminas' default
//!   `maxredirects` of 5. A cross-scheme redirect comes back as the 3xx
//!   response, and the caller's non-2xx handling reports it.
//! - **No retry.** Upstream retries a transport error three times
//!   (`File/Downloader.php:61`). The trait puts retries on the caller, and no
//!   caller retries yet.

use std::io::{self, Read};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ureq::unversioned::transport::{
    Buffers, ConnectionDetails, Connector, DefaultConnector, NextTimeout, Transport,
};

use crate::http::{FetchError, FetchErrorKind, HttpFetch, HttpResponse};
use crate::http_guard::{AddressPolicy, Pinned, Resolve, UrlGuard};

/// How long a fetch waits for a connection, for the response, and between two
/// chunks of the body before it gives up.
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// The cap on one fetch, every redirect and the whole body included. A fetch
/// that keeps making progress still ends here.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60 * 60);

/// The default cap on a response body: 2 GiB, the default of
/// `import_upload_max_file` (docs/import-v2.md "Staging"). A caller that reads
/// the setting passes it to [`HttpClient::with_max_bytes`].
pub const DEFAULT_MAX_BYTES: usize = 2 * 1024 * 1024 * 1024;

/// The largest number of redirects the client follows.
///
/// Laminas' `maxredirects` default, which the upstream config does not override.
pub const MAX_REDIRECTS: usize = 5;

/// The user agent, as `Laminas\Http\Client` sends its own.
const USER_AGENT: &str = concat!("romeka-s/", env!("CARGO_PKG_VERSION"));

/// A blocking HTTP/1.1 client over rustls, behind [`HttpFetch`].
///
/// Build one with [`HttpClient::new`] and hand it to an ingester through
/// [`crate::ingest::IngestEnv::http`]. A command that drives an ingester owns
/// the client, because it owns the process' network policy.
#[derive(Debug, Clone)]
pub struct HttpClient {
    timeout: Duration,
    idle_timeout: Duration,
    max_bytes: usize,
    accept: Option<String>,
    guard: UrlGuard,
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient {
    /// A client with [`DEFAULT_IDLE_TIMEOUT`], [`DEFAULT_TIMEOUT`],
    /// [`DEFAULT_MAX_BYTES`], the system resolver and
    /// [`AddressPolicy::PublicOnly`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            max_bytes: DEFAULT_MAX_BYTES,
            accept: None,
            guard: UrlGuard::default(),
        }
    }

    /// Resolve host names through `resolver` instead of the system's.
    #[must_use]
    pub fn with_resolver<R: Resolve>(mut self, resolver: Arc<R>) -> Self {
        self.guard = self.guard.with_resolver(resolver);
        self
    }

    /// Apply `policy` to the resolved addresses: the operator's
    /// `import_allow_private_urls`.
    #[must_use]
    pub fn with_address_policy(mut self, policy: AddressPolicy) -> Self {
        self.guard = self.guard.with_policy(policy);
        self
    }

    /// Replace the cap on one fetch, redirects and body included.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Replace the time a fetch waits without progress.
    #[must_use]
    pub fn with_idle_timeout(mut self, idle_timeout: Duration) -> Self {
        self.idle_timeout = idle_timeout;
        self
    }

    /// Send `Accept: accept` on every request.
    #[must_use]
    pub fn with_accept(mut self, accept: impl Into<String>) -> Self {
        self.accept = Some(accept.into());
        self
    }

    /// Replace the response body cap.
    #[must_use]
    pub fn with_max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = max_bytes;
        self
    }

    /// The cap on one fetch.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// The time a fetch waits without progress.
    #[must_use]
    pub fn idle_timeout(&self) -> Duration {
        self.idle_timeout
    }

    /// The response body cap.
    #[must_use]
    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    /// Build an agent for one hop: it connects only to `addresses`, waits at
    /// most `idle` to connect and for the response head, and gives up at
    /// `remaining`.
    ///
    /// One agent per hop costs a connection pool per call, which is nothing
    /// next to one ingest's one or two requests, and pins each agent to the
    /// addresses its own hop was admitted with.
    fn agent(addresses: Vec<SocketAddr>, remaining: Duration, idle: Duration) -> ureq::Agent {
        let config = ureq::config::Config::builder()
            // A 3xx must reach the caller as a response, not an error, so the
            // ingesters' `is_ok` check reports it the way the fake does.
            .http_status_as_error(false)
            // Redirects are followed by hand, so each hop passes the guard.
            .max_redirects(0)
            // A proxy from the environment would resolve the target itself,
            // past the guard.
            .proxy(None)
            .user_agent(USER_AGENT)
            // Upstream disables compressed responses, which break alongside the
            // streaming download (`File/Downloader.php:57`).
            .accept_encoding("identity")
            .timeout_global(Some(remaining))
            .timeout_connect(Some(idle))
            .timeout_recv_response(Some(idle))
            .build();
        let connector = IdleConnector {
            inner: DefaultConnector::new(),
            idle,
        };
        ureq::Agent::with_parts(config, connector, Pinned(addresses))
    }

    /// One request with no redirect handling: the response, with its body
    /// unread, and the `Location` header when the response carries one.
    fn request(
        &self,
        url: &url::Url,
        deadline: Instant,
    ) -> Result<(StreamedResponse, Option<String>), FetchError> {
        let addresses = self
            .guard
            .admit(url, remaining(deadline)?.min(self.idle_timeout))?;
        let agent = Self::agent(addresses, remaining(deadline)?, self.idle_timeout);
        let mut request = agent.get(url.as_str());
        if let Some(accept) = &self.accept {
            request = request.header("Accept", accept);
        }
        let response = request
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
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let content_length = response
            .headers()
            .get("content-length")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse().ok());
        Ok((
            StreamedResponse {
                status,
                reason,
                content_type,
                content_length,
                body: Capped {
                    inner: Box::new(response.into_body().into_reader()),
                    left: self.max_bytes,
                    max_bytes: self.max_bytes,
                },
            },
            location,
        ))
    }

    /// Fetch a URL as [`HttpFetch::fetch`] does, with the body unread: the
    /// caller streams it, so a large file never sits in memory. Reading past
    /// the size cap fails with an error that [`FetchError::from_body`] reads
    /// as [`FetchErrorKind::TooLarge`]; a read that waits longer than the idle
    /// timeout fails as [`FetchErrorKind::Timeout`], and dropping the response
    /// closes its socket.
    ///
    /// # Errors
    /// A [`FetchError`] when no response arrived.
    pub fn fetch_stream(&self, url: &str) -> Result<StreamedResponse, FetchError> {
        let deadline = Instant::now() + self.timeout;
        let mut current = parse_http_url(url)?;
        for _ in 0..=MAX_REDIRECTS {
            let (response, location) = self.request(&current, deadline)?;
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

/// A response whose body has not been read yet.
pub struct StreamedResponse {
    pub status: u16,
    pub reason: String,
    pub content_type: Option<String>,
    /// The `Content-Length` header, when the response carried one.
    pub content_length: Option<u64>,
    body: Capped,
}

impl std::fmt::Debug for StreamedResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamedResponse")
            .field("status", &self.status)
            .field("content_length", &self.content_length)
            .finish_non_exhaustive()
    }
}

impl StreamedResponse {
    /// `Laminas\Http\Response::isOk`: status in the 2xx range.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Read the whole body.
    ///
    /// # Errors
    /// [`FetchErrorKind::TooLarge`] past the cap, [`FetchErrorKind::Timeout`]
    /// for a stall, or the transport's failure.
    pub fn into_response(mut self) -> Result<HttpResponse, FetchError> {
        let mut body = Vec::new();
        self.body
            .read_to_end(&mut body)
            .map_err(|error| FetchError::from_body(&error))?;
        Ok(HttpResponse {
            status: self.status,
            reason: self.reason,
            body,
            content_type: self.content_type,
        })
    }
}

impl Read for StreamedResponse {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.body.read(buffer)
    }
}

/// A body reader that fails one byte past the cap.
struct Capped {
    inner: Box<dyn Read + Send + Sync>,
    left: usize,
    max_bytes: usize,
}

impl Read for Capped {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        // One byte more than the cap is asked for, so a body of exactly the
        // cap passes and one byte over fails.
        let want = buffer.len().min(self.left.saturating_add(1));
        let read = self.inner.read(&mut buffer[..want]).map_err(|error| {
            io::Error::other(map_error(ureq::Error::from(error), self.max_bytes))
        })?;
        if read > self.left {
            return Err(io::Error::other(too_large(self.max_bytes)));
        }
        self.left -= read;
        Ok(read)
    }
}

impl FetchError {
    /// The fetch error a body read failed with: the one [`Capped`] carries,
    /// or a transport failure.
    #[must_use]
    pub fn from_body(error: &io::Error) -> Self {
        match error
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<Self>())
        {
            Some(fetch) => Self::with_kind(fetch.kind, fetch.message.clone()),
            None => Self::with_kind(FetchErrorKind::Body, format!("fetch failed: {error}")),
        }
    }
}

/// The connector of every agent: ureq's own, with each socket wait clamped
/// to the idle timeout.
#[derive(Debug)]
struct IdleConnector {
    inner: DefaultConnector,
    idle: Duration,
}

impl Connector<()> for IdleConnector {
    type Out = IdleTransport;

    fn connect(
        &self,
        details: &ConnectionDetails,
        chained: Option<()>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        Ok(self
            .inner
            .connect(details, chained)?
            .map(|inner| IdleTransport {
                inner,
                idle: self.idle,
            }))
    }
}

/// A transport whose every wait for the socket ends at the idle timeout, so
/// a stalled read fails where it was made and the socket goes with the
/// response that owns it.
#[derive(Debug)]
struct IdleTransport {
    inner: Box<dyn Transport>,
    idle: Duration,
}

impl IdleTransport {
    fn clamp(&self, timeout: NextTimeout) -> NextTimeout {
        if *timeout.after > self.idle {
            NextTimeout {
                after: self.idle.into(),
                reason: timeout.reason,
            }
        } else {
            timeout
        }
    }
}

impl Transport for IdleTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        self.inner.buffers()
    }

    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), ureq::Error> {
        let timeout = self.clamp(timeout);
        self.inner.transmit_output(amount, timeout)
    }

    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, ureq::Error> {
        let timeout = self.clamp(timeout);
        self.inner.await_input(timeout)
    }

    fn is_open(&mut self) -> bool {
        self.inner.is_open()
    }

    fn is_tls(&self) -> bool {
        self.inner.is_tls()
    }
}

/// The time left before `deadline`, or a timeout once it has passed.
fn remaining(deadline: Instant) -> Result<Duration, FetchError> {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        return Err(FetchError::with_kind(
            FetchErrorKind::Timeout,
            "the fetch passed its deadline",
        ));
    }
    Ok(left)
}

impl HttpFetch for HttpClient {
    /// Fetch a URL, following same-scheme redirects up to [`MAX_REDIRECTS`].
    fn fetch(&self, url: &str) -> Result<HttpResponse, FetchError> {
        self.fetch_stream(url)?.into_response()
    }
}

/// Parse a URL the client will fetch. The guard judges its scheme.
fn parse_http_url(url: &str) -> Result<url::Url, FetchError> {
    url::Url::parse(url).map_err(|error| {
        FetchError::with_kind(
            FetchErrorKind::InvalidUrl,
            format!("invalid URL {url}: {error}"),
        )
    })
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
mod tests;
