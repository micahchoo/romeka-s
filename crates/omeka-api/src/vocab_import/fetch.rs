//! Read a vocabulary from a URL without letting the URL reach inside the server's network.
//!
//! Upstream's `Graph::load` fetches any URL with Laminas' client. A server that fetches a URL a
//! user types can be pointed at its own private services (server-side request forgery), so this
//! client differs in four ways:
//!
//! - **http and https only.** Any other scheme is refused before a socket opens.
//! - **Public addresses only.** Every name resolves through [`PublicOnly`], which drops loopback,
//!   private, link-local, shared, multicast and reserved addresses, so the client never connects
//!   to one. The check runs on the resolved addresses, on every redirect hop, so neither a name
//!   that resolves to `127.0.0.1` nor a redirect to `http://10.0.0.1/` gets through. No proxy is
//!   read from the environment, because a proxy would make the connection on the client's behalf.
//! - **A size cap**, [`MAX_BYTES`]. Upstream reads any size into memory.
//! - **A deadline for the whole request**, [`TIMEOUT`], resolve to last byte.

use std::io;
use std::net::IpAddr;
use std::time::Duration;

use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::{DefaultConnector, NextTimeout};

use super::Format;

/// The largest vocabulary file a URL import reads: 16 MiB, several times the largest common
/// vocabulary (schema.org's full Turtle file is about 1.5 MB).
pub const MAX_BYTES: usize = 16 * 1024 * 1024;

/// The longest a URL import waits, from the name lookup to the last byte. It sits under the
/// server's 30-second request deadline, so a slow host reports as a slow host.
pub const TIMEOUT: Duration = Duration::from_secs(20);

/// `Graph::load` follows up to ten redirects; five is Laminas' and the media client's default.
const MAX_REDIRECTS: u32 = 5;

const USER_AGENT: &str = concat!("romeka-s/", env!("CARGO_PKG_VERSION"));

/// The marker [`PublicOnly`] puts in the error it raises, so [`FetchError`] can tell a refusal
/// from a lookup failure.
const REFUSED: &str = "refused: the address is not public";

/// A fetched vocabulary file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    pub bytes: Vec<u8>,
    /// The response's `Content-Type`, which a guessed format reads first.
    pub content_type: Option<String>,
}

/// Why a URL gave no file.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FetchError {
    #[error("only http and https URLs are fetched")]
    Scheme,
    #[error("the URL names a private or local address")]
    PrivateAddress,
    #[error("the file is larger than the cap")]
    TooLarge,
    #[error("the server did not answer in time")]
    Timeout,
    #[error("the server answered {0}")]
    Status(u16),
    /// Any other transport failure, in the client's words.
    #[error("{0}")]
    Transport(String),
}

/// Which addresses a fetch may connect to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// Public addresses only: every server path uses this.
    Public,
    /// Any address. Only a test reaches its own loopback server through this.
    Any,
}

/// Fetch a vocabulary URL. `format` sets the `Accept` header as `Graph::load` sets it.
///
/// This blocks; an async caller runs it on `spawn_blocking`.
///
/// # Errors
/// [`FetchError`] for a refused URL or a failed request.
pub fn fetch(url: &str, format: Format, reach: Reach) -> Result<Fetched, FetchError> {
    let scheme = url
        .split_once("://")
        .map(|(scheme, _)| scheme.to_ascii_lowercase());
    if !matches!(scheme.as_deref(), Some("http" | "https")) {
        return Err(FetchError::Scheme);
    }
    let config = ureq::Agent::config_builder()
        .proxy(None)
        .max_redirects(MAX_REDIRECTS)
        .timeout_global(Some(TIMEOUT))
        .user_agent(USER_AGENT)
        .build();
    let agent = ureq::Agent::with_parts(config, DefaultConnector::default(), PublicOnly { reach });
    let mut response = agent
        .get(url)
        .header("Accept", accept(format))
        .call()
        .map_err(map_error)?;
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_BYTES as u64 + 1)
        .read_to_vec()
        .map_err(map_error)?;
    if bytes.len() > MAX_BYTES {
        return Err(FetchError::TooLarge);
    }
    Ok(Fetched {
        bytes,
        content_type,
    })
}

/// `Graph::load`'s `Accept`: the chosen format's type, or every type it parses, weighted.
fn accept(format: Format) -> &'static str {
    match format {
        Format::Guess => {
            "application/ld+json,application/rdf+xml;q=0.9,text/turtle;q=0.8,application/n-triples;q=0.7"
        }
        Format::JsonLd => "application/ld+json",
        Format::NTriples => "application/n-triples",
        Format::RdfXml => "application/rdf+xml",
        Format::Turtle => "text/turtle",
    }
}

fn map_error(error: ureq::Error) -> FetchError {
    match error {
        ureq::Error::StatusCode(status) => FetchError::Status(status),
        ureq::Error::BodyExceedsLimit(_) => FetchError::TooLarge,
        ureq::Error::Timeout(_) => FetchError::Timeout,
        ureq::Error::Io(io) if io.kind() == io::ErrorKind::TimedOut => FetchError::Timeout,
        ureq::Error::Io(io) if io.to_string().contains(REFUSED) => FetchError::PrivateAddress,
        other => FetchError::Transport(other.to_string()),
    }
}

/// The system resolver, keeping only the addresses [`Reach`] allows.
#[derive(Debug)]
struct PublicOnly {
    reach: Reach,
}

impl Resolver for PublicOnly {
    fn resolve(
        &self,
        uri: &ureq::http::Uri,
        config: &ureq::config::Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let resolved = DefaultResolver::default().resolve(uri, config, timeout)?;
        if self.reach == Reach::Any {
            return Ok(resolved);
        }
        let mut kept = self.empty();
        for address in resolved.iter().filter(|address| is_public(address.ip())) {
            kept.push(*address);
        }
        if kept.is_empty() {
            return Err(ureq::Error::Io(io::Error::new(
                io::ErrorKind::PermissionDenied,
                REFUSED,
            )));
        }
        Ok(kept)
    }
}

/// Whether an address is on the public internet: not loopback, private (RFC 1918, RFC 4193),
/// link-local, shared (RFC 6598), multicast, documentation, benchmarking or reserved.
fn is_public(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_documentation()
                || a == 0
                || (a == 100 && (64..128).contains(&b))
                || (a == 198 && (b == 18 || b == 19))
                || a >= 240)
        }
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_public(IpAddr::V4(v4)),
            None => {
                !(v6.is_loopback()
                    || v6.is_unspecified()
                    || v6.is_multicast()
                    || v6.is_unique_local()
                    || v6.is_unicast_link_local()
                    || v6.segments()[0] == 0x2001 && v6.segments()[1] == 0x0db8)
            }
        },
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;

    /// Serve one HTTP answer on a loopback port and return the URL that reaches it.
    fn serve_once(answer: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0u8; 2048];
                let _ = stream.read(&mut request);
                let _ = stream.write_all(answer.as_bytes());
            }
        });
        format!("http://127.0.0.1:{port}/vocab.ttl")
    }

    #[test]
    fn a_loopback_server_is_refused_before_any_byte_is_sent() {
        let url = serve_once("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
        assert_eq!(
            fetch(&url, Format::Guess, Reach::Public),
            Err(FetchError::PrivateAddress)
        );
        for url in [
            "http://localhost/",
            "http://10.1.2.3/",
            "http://192.168.0.1/",
            "http://169.254.169.254/latest/meta-data/",
            "http://[::1]/",
            "http://[::ffff:127.0.0.1]/",
            "http://[fd00::1]/",
            "http://100.64.0.1/",
            "http://198.18.0.1/",
            "http://0.0.0.0/",
        ] {
            assert_eq!(
                fetch(url, Format::Guess, Reach::Public),
                Err(FetchError::PrivateAddress),
                "{url}"
            );
        }
    }

    #[test]
    fn only_http_and_https_are_fetched() {
        for url in [
            "file:///etc/passwd",
            "ftp://example.org/v.rdf",
            "example.org/v.rdf",
        ] {
            assert_eq!(
                fetch(url, Format::Guess, Reach::Public),
                Err(FetchError::Scheme),
                "{url}"
            );
        }
    }

    #[test]
    fn a_reachable_server_answers_its_body_and_content_type() {
        let url = serve_once(
            "HTTP/1.1 200 OK\r\nContent-Type: text/turtle\r\nContent-Length: 5\r\n\r\n<a>.\n",
        );
        let fetched = fetch(&url, Format::Guess, Reach::Any).unwrap();
        assert_eq!(fetched.bytes, b"<a>.\n");
        assert_eq!(fetched.content_type.as_deref(), Some("text/turtle"));
    }

    #[test]
    fn an_error_status_is_reported_with_its_code() {
        let url = serve_once("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
        assert_eq!(
            fetch(&url, Format::Guess, Reach::Any),
            Err(FetchError::Status(404))
        );
    }
}
