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
//!   default applies. ureq's own timeouts are all totals, so the body is read
//!   on a thread and the idle clock runs on the caller's side of a channel.
//! - **A response size cap.** Upstream has none: `File/Downloader.php` streams
//!   into a temporary file. The port hands the body back as one `Vec<u8>`, so it
//!   caps the body at [`DEFAULT_MAX_BYTES`], the contract's
//!   `import_upload_max_file`.
//! - **Same-scheme redirects only.** The [`HttpFetch`] contract forbids
//!   following a redirect into another scheme. A same-scheme redirect is
//!   followed up to [`MAX_REDIRECTS`] times, matching Laminas' default
//!   `maxredirects` of 5. A cross-scheme redirect comes back as the 3xx
//!   response, and the caller's non-2xx handling reports it.
//! - **No retry.** Upstream retries a transport error three times
//!   (`File/Downloader.php:61`). The trait puts retries on the caller, and no
//!   caller retries yet.

use std::io::Read;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use ureq::unversioned::transport::DefaultConnector;

use crate::http::{FetchError, FetchErrorKind, HttpFetch, HttpResponse};
use crate::http_guard::{AddressPolicy, Pinned, Resolve, UrlGuard};

/// How long a fetch waits for a connection, for the response, and between two
/// chunks of the body before it gives up.
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// The cap on one fetch, every redirect and the whole body included. A fetch
/// that keeps making progress still ends here.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60 * 60);

/// The size of one read from the response body.
const CHUNK_BYTES: usize = 64 * 1024;

/// The default cap on a response body: 2 GiB, the default of
/// `import_upload_max_file` (docs/import-v2.md "Staging"). The setting does
/// not exist yet, so the cap is this constant; a caller that reads the setting
/// passes it to [`HttpClient::with_max_bytes`].
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
        ureq::Agent::with_parts(config, DefaultConnector::new(), Pinned(addresses))
    }

    /// One request with no redirect handling: the response, its body and the
    /// `Location` header when the response carries one.
    fn request(
        &self,
        url: &url::Url,
        deadline: Instant,
    ) -> Result<(HttpResponse, Option<String>), FetchError> {
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
        let body = self.read_body(response.into_body())?;
        Ok((
            HttpResponse {
                status,
                reason,
                body,
                content_type,
            },
            location,
        ))
    }
}

impl HttpClient {
    /// Read a body, giving up when no chunk arrives for the idle timeout.
    ///
    /// The read runs on a thread because ureq's timeouts are totals. If the
    /// idle clock fires, the thread is left to end when the agent's overall
    /// deadline closes its socket; it holds nothing the caller needs.
    fn read_body(&self, body: ureq::Body) -> Result<Vec<u8>, FetchError> {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = body.into_reader();
            loop {
                let mut chunk = vec![0u8; CHUNK_BYTES];
                match reader.read(&mut chunk) {
                    Ok(0) => return,
                    Ok(read) => {
                        chunk.truncate(read);
                        if sender.send(Ok(chunk)).is_err() {
                            return;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(Err(error));
                        return;
                    }
                }
            }
        });
        let mut body = Vec::new();
        loop {
            match receiver.recv_timeout(self.idle_timeout) {
                Ok(Ok(chunk)) => {
                    // A body of exactly the cap passes; one byte over fails.
                    if body.len() + chunk.len() > self.max_bytes {
                        return Err(too_large(self.max_bytes));
                    }
                    body.extend_from_slice(&chunk);
                }
                Ok(Err(error)) => {
                    return Err(map_error(ureq::Error::from(error), self.max_bytes));
                }
                Err(RecvTimeoutError::Timeout) => {
                    return Err(FetchError::with_kind(
                        FetchErrorKind::Timeout,
                        format!(
                            "fetch failed: no data for {} seconds",
                            self.idle_timeout.as_secs_f64()
                        ),
                    ));
                }
                Err(RecvTimeoutError::Disconnected) => return Ok(body),
            }
        }
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

    use std::collections::VecDeque;
    use std::io::{Read, Write};
    use std::net::{IpAddr, TcpListener};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// An address no test machine owns, standing in for a public host. The
    /// guard checks it as public; [`UrlGuard::dial_as`] then dials it at a
    /// loopback server.
    const PUBLIC: &str = "198.51.100.7";

    /// A resolver that answers each lookup with the next queued answer, and
    /// repeats the last one.
    #[derive(Debug)]
    struct Answers {
        queue: Mutex<VecDeque<Vec<IpAddr>>>,
        lookups: AtomicUsize,
    }

    impl Answers {
        fn new(answers: &[&str]) -> Arc<Self> {
            Arc::new(Self {
                queue: Mutex::new(
                    answers
                        .iter()
                        .map(|answer| vec![answer.parse().unwrap()])
                        .collect(),
                ),
                lookups: AtomicUsize::new(0),
            })
        }
    }

    impl Resolve for Answers {
        fn resolve(&self, _host: &str) -> std::io::Result<Vec<IpAddr>> {
            self.lookups.fetch_add(1, Ordering::SeqCst);
            let mut queue = self.queue.lock().unwrap();
            let answer = if queue.len() > 1 {
                queue.pop_front().unwrap_or_default()
            } else {
                queue.front().cloned().unwrap_or_default()
            };
            Ok(answer)
        }
    }

    /// A loopback server that answers every connection with `head` and
    /// `body`, and counts the connections.
    fn server(head: String, body: &'static str) -> (SocketAddr, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&hits);
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                counted.fetch_add(1, Ordering::SeqCst);
                let mut request = [0u8; 2048];
                let _ = stream.read(&mut request);
                let _ = write!(
                    stream,
                    "{head}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        (addr, hits)
    }

    fn ok_server(body: &'static str) -> (SocketAddr, Arc<AtomicUsize>) {
        server("HTTP/1.1 200 OK\r\n".to_string(), body)
    }

    fn redirect_server(location: &str) -> (SocketAddr, Arc<AtomicUsize>) {
        server(
            format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\n"),
            "",
        )
    }

    /// A client whose resolver is `answers`, and which dials each public
    /// address `PUBLIC:<port>` at the loopback server on that port.
    fn client(answers: &Arc<Answers>, locals: &[SocketAddr]) -> HttpClient {
        let routes: Vec<(u16, SocketAddr)> =
            locals.iter().map(|local| (local.port(), *local)).collect();
        routed(answers, &routes)
    }

    /// A client that dials `PUBLIC:<port>` at the server each route names.
    fn routed(answers: &Arc<Answers>, routes: &[(u16, SocketAddr)]) -> HttpClient {
        let mut client = HttpClient::new().with_resolver(Arc::clone(answers));
        let public: IpAddr = PUBLIC.parse().unwrap();
        client.guard.dial_as = routes
            .iter()
            .map(|(port, local)| (SocketAddr::new(public, *port), *local))
            .collect();
        client
    }

    /// A public host that redirects to a loopback address is refused at the
    /// redirect, and the loopback server sees no connection.
    #[test]
    fn a_redirect_to_loopback_is_refused() {
        let (inside, inside_hits) = ok_server("secret");
        let (outside, outside_hits) = redirect_server(&format!("http://{inside}/admin"));
        let answers = Answers::new(&[PUBLIC]);
        let error = client(&answers, &[outside])
            .fetch(&format!("http://public.test:{}/start", outside.port()))
            .expect_err("refused at the redirect");
        assert_eq!(error.kind(), FetchErrorKind::Refused, "{error}");
        assert_eq!(outside_hits.load(Ordering::SeqCst), 1, "the first hop ran");
        assert_eq!(
            inside_hits.load(Ordering::SeqCst),
            0,
            "no loopback connection"
        );
    }

    /// The same redirect shape is followed when the target is public, so the
    /// refusal above is the guard's and not the redirect handling's.
    #[test]
    fn a_redirect_between_public_hosts_is_followed() {
        let (target, target_hits) = ok_server("final");
        let (outside, _) = redirect_server(&format!("http://other.test:{}/to", target.port()));
        let answers = Answers::new(&[PUBLIC]);
        let response = client(&answers, &[outside, target])
            .fetch(&format!("http://public.test:{}/from", outside.port()))
            .expect("fetch");
        assert_eq!(response.body, b"final");
        assert_eq!(target_hits.load(Ordering::SeqCst), 1);
        assert_eq!(
            answers.lookups.load(Ordering::SeqCst),
            2,
            "one lookup per hop"
        );
    }

    /// DNS rebinding: the resolver answers public, then loopback. The fetch
    /// looks the host up once and connects to the first answer, and a
    /// redirect back to the same host is looked up again and refused.
    #[test]
    fn a_rebinding_host_is_connected_to_its_checked_address() {
        let (inside, inside_hits) = ok_server("inside");
        let (outside, outside_hits) = ok_server("outside");
        let answers = Answers::new(&[PUBLIC, "127.0.0.1"]);
        // The URL names the inside server's port, so a connection to the
        // second answer would reach it.
        let response = routed(&answers, &[(inside.port(), outside)])
            .fetch(&format!("http://rebind.test:{}/", inside.port()))
            .expect("fetch");
        assert_eq!(response.body, b"outside");
        assert_eq!(answers.lookups.load(Ordering::SeqCst), 1);
        assert_eq!(outside_hits.load(Ordering::SeqCst), 1);
        assert_eq!(inside_hits.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_redirect_to_a_rebound_host_is_checked_again() {
        let (inside, inside_hits) = ok_server("inside");
        let location = format!("http://rebind.test:{}/again", inside.port());
        let (outside, outside_hits) = redirect_server(&location);
        let answers = Answers::new(&[PUBLIC, "127.0.0.1"]);
        let error = routed(&answers, &[(inside.port(), outside)])
            .fetch(&format!("http://rebind.test:{}/", inside.port()))
            .expect_err("the second answer is refused");
        assert_eq!(error.kind(), FetchErrorKind::Refused, "{error}");
        assert_eq!(
            answers.lookups.load(Ordering::SeqCst),
            2,
            "one lookup per hop"
        );
        assert_eq!(outside_hits.load(Ordering::SeqCst), 1);
        assert_eq!(inside_hits.load(Ordering::SeqCst), 0);
    }

    /// A redirect that adds credentials is refused like a first URL with them.
    #[test]
    fn a_redirect_with_credentials_is_refused() {
        let (outside, _) = redirect_server(&format!("http://user:pw@{PUBLIC}/x"));
        let answers = Answers::new(&[PUBLIC]);
        let error = client(&answers, &[outside])
            .fetch(&format!("http://public.test:{}/", outside.port()))
            .expect_err("refused");
        assert_eq!(error.kind(), FetchErrorKind::Refused, "{error}");
    }
}
