//! Read a vocabulary from a URL without letting the URL reach inside the server's network.
//!
//! Upstream's `Graph::load` fetches any URL with Laminas' client. A server that fetches a URL a
//! user types can be pointed at its own private services (server-side request forgery), so this
//! fetch goes through `omeka_media`'s [`HttpClient`], the one guard every outbound fetch passes:
//!
//! - **http and https only**, and no credentials in the URL. Any other scheme is refused before a
//!   socket opens.
//! - **Public addresses only**, unless the operator stored `import_allow_private_urls`
//!   ([`AddressPolicy`]). A name that resolves to a refused address is refused, not skipped, and
//!   so is a redirect to one. No proxy is read from the environment.
//! - **A size cap**, [`MAX_BYTES`]. Upstream reads any size into memory.
//! - **A cap on the whole request**, [`TIMEOUT`], resolve to last byte.

use std::time::Duration;

use omeka_media::http::{FetchError as MediaError, FetchErrorKind, HttpFetch};
use omeka_media::http_client::HttpClient;
use omeka_media::http_guard::AddressPolicy;

use super::Format;

/// The largest vocabulary file a URL import reads: 16 MiB, several times the largest common
/// vocabulary (schema.org's full Turtle file is about 1.5 MB).
pub const MAX_BYTES: usize = 16 * 1024 * 1024;

/// The longest a URL import waits, from the name lookup to the last byte. It sits under the
/// server's 30-second request deadline, so a slow host reports as a slow host.
pub const TIMEOUT: Duration = Duration::from_secs(20);

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

/// Fetch a vocabulary URL. `format` sets the `Accept` header as `Graph::load` sets it, and
/// `policy` is the operator's address setting (`settings::address_policy`).
///
/// This blocks; an async caller runs it on `spawn_blocking`.
///
/// # Errors
/// [`FetchError`] for a refused URL or a failed request.
pub fn fetch(url: &str, format: Format, policy: AddressPolicy) -> Result<Fetched, FetchError> {
    let scheme = url
        .split_once("://")
        .map(|(scheme, _)| scheme.to_ascii_lowercase());
    if !matches!(scheme.as_deref(), Some("http" | "https")) {
        return Err(FetchError::Scheme);
    }
    let response = HttpClient::new()
        .with_address_policy(policy)
        .with_max_bytes(MAX_BYTES)
        .with_timeout(TIMEOUT)
        .with_accept(accept(format))
        .fetch(url)
        .map_err(map_error)?;
    if !response.is_ok() {
        return Err(FetchError::Status(response.status));
    }
    Ok(Fetched {
        bytes: response.body,
        content_type: response.content_type,
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

fn map_error(error: MediaError) -> FetchError {
    match error.kind() {
        FetchErrorKind::TooLarge => FetchError::TooLarge,
        FetchErrorKind::Timeout => FetchError::Timeout,
        // The guard also refuses a URL that carries credentials; that is not an address.
        FetchErrorKind::Refused if !error.message.contains("credentials") => {
            FetchError::PrivateAddress
        }
        _ => FetchError::Transport(error.message),
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
            fetch(&url, Format::Guess, AddressPolicy::PublicOnly),
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
                fetch(url, Format::Guess, AddressPolicy::PublicOnly),
                Err(FetchError::PrivateAddress),
                "{url}"
            );
        }
    }

    #[test]
    fn a_name_that_resolves_to_loopback_is_refused_not_skipped() {
        assert_eq!(
            fetch(
                "http://localhost/",
                Format::Guess,
                AddressPolicy::PublicOnly
            ),
            Err(FetchError::PrivateAddress)
        );
    }

    #[test]
    fn newly_refused_ranges_are_refused_here_too() {
        for url in [
            "http://[64:ff9b::7f00:1]/",
            "http://[2002:a00:1::]/",
            "http://[fec0::1]/",
            "http://192.0.0.8/",
            "http://240.0.0.1/",
        ] {
            assert_eq!(
                fetch(url, Format::Guess, AddressPolicy::PublicOnly),
                Err(FetchError::PrivateAddress),
                "{url}"
            );
        }
    }

    #[test]
    fn a_url_with_credentials_is_refused_with_its_own_reason() {
        let error = fetch(
            "http://user:secret@example.org/v.ttl",
            Format::Guess,
            AddressPolicy::PublicOnly,
        )
        .unwrap_err();
        assert!(
            matches!(&error, FetchError::Transport(message) if message.contains("credentials")),
            "{error:?}"
        );
    }

    #[test]
    fn only_http_and_https_are_fetched() {
        for url in [
            "file:///etc/passwd",
            "ftp://example.org/v.rdf",
            "example.org/v.rdf",
        ] {
            assert_eq!(
                fetch(url, Format::Guess, AddressPolicy::PublicOnly),
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
        let fetched = fetch(&url, Format::Guess, AddressPolicy::AllowPrivate).unwrap();
        assert_eq!(fetched.bytes, b"<a>.\n");
        assert_eq!(fetched.content_type.as_deref(), Some("text/turtle"));
    }

    #[test]
    fn an_error_status_is_reported_with_its_code() {
        let url = serve_once("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
        assert_eq!(
            fetch(&url, Format::Guess, AddressPolicy::AllowPrivate),
            Err(FetchError::Status(404))
        );
    }
}
