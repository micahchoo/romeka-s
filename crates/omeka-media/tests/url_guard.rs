//! The URL guard every fetch passes through (docs/import-v2.md "Arrival",
//! "URL sources").
//!
//! Each test injects a stub resolver into the real [`HttpClient`], so a host
//! name answers a chosen address and no test needs DNS. A refused fetch never
//! opens a socket, so these tests reach no network either.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{IpAddr, TcpListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use omeka_media::http::{FetchErrorKind, HttpFetch};
use omeka_media::http_client::HttpClient;
use omeka_media::http_guard::{AddressPolicy, RefusedRange, Resolve, refused_range};

/// A resolver that answers from a table and counts its lookups.
#[derive(Debug, Default)]
struct Stub {
    hosts: BTreeMap<String, Vec<IpAddr>>,
    lookups: AtomicUsize,
}

impl Stub {
    fn answering(host: &str, addresses: &[&str]) -> Arc<Self> {
        let mut hosts = BTreeMap::new();
        hosts.insert(
            host.to_string(),
            addresses.iter().map(|a| a.parse().unwrap()).collect(),
        );
        Arc::new(Self {
            hosts,
            lookups: AtomicUsize::new(0),
        })
    }

    fn lookups(&self) -> usize {
        self.lookups.load(Ordering::SeqCst)
    }
}

impl Resolve for Stub {
    fn resolve(&self, host: &str) -> std::io::Result<Vec<IpAddr>> {
        self.lookups.fetch_add(1, Ordering::SeqCst);
        Ok(self.hosts.get(host).cloned().unwrap_or_default())
    }
}

/// Assert that `address` falls in `range`, and that a fetch of a host that
/// resolves to it is refused before any connection.
fn assert_refused(address: &str, range: RefusedRange) {
    let ip: IpAddr = address.parse().unwrap();
    assert_eq!(refused_range(ip), Some(range), "{address}");
    let stub = Stub::answering("target.test", &[address]);
    let client = HttpClient::new().with_resolver(Arc::clone(&stub));
    let error = client
        .fetch("http://target.test/file.jpg")
        .expect_err(address);
    assert_eq!(error.kind(), FetchErrorKind::Refused, "{address}: {error}");
    assert_eq!(stub.lookups(), 1, "{address}: the host resolves once");
}

#[test]
fn loopback_is_refused() {
    assert_refused("127.0.0.1", RefusedRange::Loopback);
    assert_refused("127.8.9.10", RefusedRange::Loopback);
    assert_refused("::1", RefusedRange::Loopback);
}

#[test]
fn ipv4_link_local_and_the_metadata_address_are_refused() {
    assert_refused("169.254.169.254", RefusedRange::LinkLocal);
    assert_refused("169.254.0.1", RefusedRange::LinkLocal);
}

#[test]
fn rfc1918_private_ranges_are_refused() {
    assert_refused("10.0.0.1", RefusedRange::Private);
    assert_refused("172.16.0.1", RefusedRange::Private);
    assert_refused("172.31.255.255", RefusedRange::Private);
    assert_refused("192.168.1.1", RefusedRange::Private);
}

#[test]
fn cgnat_shared_space_is_refused() {
    assert_refused("100.64.0.1", RefusedRange::SharedAddressSpace);
    assert_refused("100.127.255.254", RefusedRange::SharedAddressSpace);
}

#[test]
fn ipv6_unique_local_is_refused() {
    assert_refused("fc00::1", RefusedRange::UniqueLocal);
    assert_refused("fd12:3456:789a::1", RefusedRange::UniqueLocal);
}

#[test]
fn ipv6_link_local_is_refused() {
    assert_refused("fe80::1", RefusedRange::LinkLocal);
    assert_refused("febf::1", RefusedRange::LinkLocal);
}

#[test]
fn ipv4_mapped_and_compatible_forms_are_refused_as_their_ipv4_range() {
    assert_refused("::ffff:127.0.0.1", RefusedRange::Loopback);
    assert_refused("::ffff:169.254.169.254", RefusedRange::LinkLocal);
    assert_refused("::ffff:10.0.0.1", RefusedRange::Private);
    assert_refused("::ffff:100.64.0.1", RefusedRange::SharedAddressSpace);
    assert_refused("::127.0.0.1", RefusedRange::Loopback);
    assert_refused("::192.168.0.1", RefusedRange::Private);
}

#[test]
fn unspecified_is_refused() {
    assert_refused("0.0.0.0", RefusedRange::Unspecified);
    assert_refused("0.1.2.3", RefusedRange::Unspecified);
    assert_refused("::", RefusedRange::Unspecified);
}

#[test]
fn multicast_is_refused() {
    assert_refused("224.0.0.1", RefusedRange::Multicast);
    assert_refused("239.255.255.250", RefusedRange::Multicast);
    assert_refused("ff02::1", RefusedRange::Multicast);
}

#[test]
fn broadcast_is_refused() {
    assert_refused("255.255.255.255", RefusedRange::Broadcast);
}

#[test]
fn nat64_is_refused_when_the_embedded_ipv4_is() {
    // RFC 6052 well-known prefix: the IPv4 address sits in the last 32 bits.
    assert_refused("64:ff9b::7f00:1", RefusedRange::Loopback);
    assert_refused("64:ff9b::a9fe:a9fe", RefusedRange::LinkLocal);
    assert_refused("64:ff9b::a00:1", RefusedRange::Private);
    // RFC 8215 local-use prefix: an operator picks the layout, so either
    // reading of the address (last 32 bits, or the RFC 6052 /48 layout that
    // skips bits 64..72) that lands in a refused range refuses the whole.
    assert_refused("64:ff9b:1:808:8:800:a00:1", RefusedRange::Private);
    assert_refused("64:ff9b:1:a00:0:100:808:808", RefusedRange::Private);
    assert_refused("64:ff9b:1:7f00:0:100:808:808", RefusedRange::Loopback);
}

#[test]
fn six_to_four_is_refused_when_the_embedded_ipv4_is() {
    assert_refused("2002:7f00:1::1", RefusedRange::Loopback);
    assert_refused("2002:a9fe:a9fe::", RefusedRange::LinkLocal);
    assert_refused("2002:c0a8:101::5", RefusedRange::Private);
    assert_refused("2002:6440:1::", RefusedRange::SharedAddressSpace);
}

#[test]
fn ipv6_site_local_is_refused() {
    assert_refused("fec0::1", RefusedRange::SiteLocal);
    assert_refused("feff::1", RefusedRange::SiteLocal);
}

#[test]
fn ietf_protocol_assignments_are_refused() {
    assert_refused("192.0.0.1", RefusedRange::ProtocolAssignment);
    assert_refused("192.0.0.255", RefusedRange::ProtocolAssignment);
}

#[test]
fn benchmarking_range_is_refused() {
    assert_refused("198.18.0.1", RefusedRange::Benchmarking);
    assert_refused("198.19.255.255", RefusedRange::Benchmarking);
}

#[test]
fn reserved_class_e_is_refused() {
    assert_refused("240.0.0.1", RefusedRange::Reserved);
    assert_refused("254.1.2.3", RefusedRange::Reserved);
    assert_refused("255.255.255.254", RefusedRange::Reserved);
}

/// The documentation ranges (RFC 5737, RFC 3849) never hold a real host, so
/// a name that resolves into one is a misconfiguration or a probe.
#[test]
fn documentation_ranges_are_refused() {
    assert_refused("192.0.2.1", RefusedRange::Documentation);
    assert_refused("198.51.100.7", RefusedRange::Documentation);
    assert_refused("203.0.113.255", RefusedRange::Documentation);
    assert_refused("2001:db8::1", RefusedRange::Documentation);
    assert_refused("2001:db8:ffff::1", RefusedRange::Documentation);
    assert_refused("::ffff:203.0.113.9", RefusedRange::Documentation);
}

/// The neighbours of each refused range stay fetchable.
#[test]
fn public_addresses_fall_in_no_refused_range() {
    for address in [
        "8.8.8.8",
        "100.63.255.255",
        "100.128.0.1",
        "172.15.255.255",
        "172.32.0.1",
        "169.253.255.255",
        "192.169.0.1",
        "::ffff:8.8.8.8",
        "2606:4700:4700::1111",
        "1.0.0.0",
        "192.0.1.1",
        "192.0.3.1",
        "198.51.99.255",
        "198.51.101.0",
        "203.0.112.255",
        "203.0.114.0",
        "2001:db9::1",
        "2001:db7:ffff::1",
        "198.17.255.255",
        "198.20.0.1",
        "223.255.255.255",
        "fe7f::1",
        "2002:808:808::1",
        "2003::1",
        "64:ff9b::808:808",
        "64:ff9b:1:808:8:800:808:808",
        "64:ff9b:2::a00:1",
    ] {
        assert_eq!(refused_range(address.parse().unwrap()), None, "{address}");
    }
}

/// Every resolved address is checked: one private answer among public ones
/// refuses the fetch.
#[test]
fn one_private_answer_among_several_refuses_the_fetch() {
    let stub = Stub::answering("mixed.test", &["8.8.8.8", "10.1.2.3"]);
    let error = HttpClient::new()
        .with_resolver(stub)
        .fetch("http://mixed.test/a")
        .expect_err("refused");
    assert_eq!(error.kind(), FetchErrorKind::Refused, "{error}");
}

/// An address literal in the URL is checked without a lookup, in every
/// spelling the URL parser accepts.
#[test]
fn an_address_literal_is_checked_without_a_lookup() {
    for url in [
        "http://127.0.0.1/a",
        "http://[::1]/a",
        "http://0x7f000001/a",
        "http://2130706433/a",
        "https://169.254.169.254/latest/meta-data/",
        "http://[::ffff:10.0.0.1]/a",
    ] {
        let stub = Stub::answering("unused.test", &["8.8.8.8"]);
        let error = HttpClient::new()
            .with_resolver(Arc::clone(&stub))
            .fetch(url)
            .expect_err(url);
        assert_eq!(error.kind(), FetchErrorKind::Refused, "{url}: {error}");
        assert_eq!(stub.lookups(), 0, "{url}: a literal needs no lookup");
    }
}

/// Only http and https pass, before any lookup.
#[test]
fn a_scheme_other_than_http_is_refused_before_a_lookup() {
    for url in [
        "ftp://target.test/a",
        "file:///etc/passwd",
        "gopher://target.test/",
    ] {
        let stub = Stub::answering("target.test", &["8.8.8.8"]);
        let error = HttpClient::new()
            .with_resolver(Arc::clone(&stub))
            .fetch(url)
            .expect_err(url);
        assert_eq!(error.kind(), FetchErrorKind::InvalidUrl, "{url}: {error}");
        assert_eq!(stub.lookups(), 0, "{url}");
    }
}

/// Credentials in the URL are refused, before any lookup, and the operator
/// setting does not lift that rule.
#[test]
fn credentials_in_the_url_are_refused() {
    for policy in [AddressPolicy::PublicOnly, AddressPolicy::AllowPrivate] {
        for url in [
            "http://user:secret@target.test/a",
            "http://user@target.test/a",
            "https://:secret@target.test/a",
        ] {
            let stub = Stub::answering("target.test", &["8.8.8.8"]);
            let error = HttpClient::new()
                .with_resolver(Arc::clone(&stub))
                .with_address_policy(policy)
                .fetch(url)
                .expect_err(url);
            assert_eq!(error.kind(), FetchErrorKind::Refused, "{url}: {error}");
            assert_eq!(stub.lookups(), 0, "{url}");
        }
    }
}

/// A host that resolves to nothing is a connect failure, not a refusal.
#[test]
fn a_host_with_no_address_is_a_connect_failure() {
    let stub = Stub::answering("other.test", &["8.8.8.8"]);
    let error = HttpClient::new()
        .with_resolver(stub)
        .fetch("http://nowhere.test/a")
        .expect_err("no address");
    assert_eq!(error.kind(), FetchErrorKind::Connect, "{error}");
}

/// A loopback server that counts the connections it accepts and answers each
/// with `body`.
fn counting_server(body: &'static str) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&hits);
    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            counted.fetch_add(1, Ordering::SeqCst);
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request);
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (format!("http://localhost.test:{}/", addr.port()), hits)
}

/// `import_allow_private_urls` lifts the address check and nothing else: the
/// same loopback server is refused by default and fetched when allowed.
#[test]
fn the_allow_private_policy_lifts_the_address_check() {
    let (url, hits) = counting_server("inside");
    let stub = Stub::answering("localhost.test", &["127.0.0.1"]);

    let refused = HttpClient::new()
        .with_resolver(Arc::clone(&stub))
        .fetch(&url)
        .expect_err("refused by default");
    assert_eq!(refused.kind(), FetchErrorKind::Refused, "{refused}");
    assert_eq!(hits.load(Ordering::SeqCst), 0, "no connection was made");

    let response = HttpClient::new()
        .with_resolver(Arc::clone(&stub))
        .with_address_policy(AddressPolicy::AllowPrivate)
        .fetch(&url)
        .expect("allowed");
    assert_eq!(response.body, b"inside");
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

/// The client connects to the address the guard checked: the stub's answer is
/// the only route to the server, because `localhost.test` has no DNS entry.
#[test]
fn the_connection_goes_to_the_resolved_address() {
    let (url, hits) = counting_server("pinned");
    let stub = Stub::answering("localhost.test", &["127.0.0.1"]);
    let response = HttpClient::new()
        .with_resolver(Arc::clone(&stub))
        .with_address_policy(AddressPolicy::AllowPrivate)
        .fetch(&url)
        .expect("fetch");
    assert_eq!(response.body, b"pinned");
    assert_eq!(
        stub.lookups(),
        1,
        "one lookup, and ureq made none of its own"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

/// A loopback server that runs `script` on the first connection it accepts,
/// after reading the request.
fn scripted_server(script: fn(std::net::TcpStream)) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request);
            script(stream);
        }
    });
    format!("http://localhost.test:{port}/")
}

fn loopback_client() -> HttpClient {
    HttpClient::new()
        .with_resolver(Stub::answering("localhost.test", &["127.0.0.1"]))
        .with_address_policy(AddressPolicy::AllowPrivate)
}

/// The deadline is idle time, not total time: a body that keeps arriving
/// outlasts the idle timeout.
#[test]
fn a_slow_body_that_keeps_arriving_is_not_timed_out() {
    let url = scripted_server(|mut stream| {
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\n");
        for byte in b"slowbody" {
            let _ = stream.write_all(&[*byte]);
            let _ = stream.flush();
            thread::sleep(std::time::Duration::from_millis(150));
        }
    });
    let response = loopback_client()
        .with_idle_timeout(std::time::Duration::from_millis(500))
        .fetch(&url)
        .expect("progress keeps the fetch alive");
    assert_eq!(response.body, b"slowbody");
}

/// A body that stalls halfway ends the fetch at the idle timeout, long before
/// the overall cap.
#[test]
fn a_body_that_stalls_times_out_at_the_idle_timeout() {
    let url = scripted_server(|mut stream| {
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nhalf");
        let _ = stream.flush();
        thread::sleep(std::time::Duration::from_secs(10));
    });
    let started = std::time::Instant::now();
    let error = loopback_client()
        .with_idle_timeout(std::time::Duration::from_millis(300))
        .fetch(&url)
        .expect_err("stalled");
    assert_eq!(error.kind(), FetchErrorKind::Timeout, "{error}");
    assert!(started.elapsed() < std::time::Duration::from_secs(3));
}

/// A server that accepts and never answers is the same stall, before the
/// first header byte.
#[test]
fn a_server_that_never_answers_times_out_at_the_idle_timeout() {
    let url = scripted_server(|_stream| thread::sleep(std::time::Duration::from_secs(10)));
    let started = std::time::Instant::now();
    let error = loopback_client()
        .with_idle_timeout(std::time::Duration::from_millis(300))
        .fetch(&url)
        .expect_err("stalled");
    assert_eq!(error.kind(), FetchErrorKind::Timeout, "{error}");
    assert!(started.elapsed() < std::time::Duration::from_secs(3));
}

/// The overall cap ends a fetch that keeps making progress.
#[test]
fn the_overall_cap_ends_a_fetch_that_keeps_arriving() {
    let url = scripted_server(|mut stream| {
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 40\r\n\r\n");
        for _ in 0..40 {
            if stream.write_all(b"x").is_err() {
                return;
            }
            let _ = stream.flush();
            thread::sleep(std::time::Duration::from_millis(100));
        }
    });
    let error = loopback_client()
        .with_idle_timeout(std::time::Duration::from_millis(500))
        .with_timeout(std::time::Duration::from_millis(600))
        .fetch(&url)
        .expect_err("over the cap");
    assert_eq!(error.kind(), FetchErrorKind::Timeout, "{error}");
}

/// A fetch that gives up at the idle timeout closes its socket then, not at
/// the overall cap: the server sees the connection end.
#[test]
fn a_stalled_fetch_closes_its_socket_at_the_idle_timeout() {
    let (closed_after, receiver) = std::sync::mpsc::channel();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nhalf");
            let _ = stream.flush();
            let stalled = std::time::Instant::now();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(20)))
                .unwrap();
            let mut rest = [0u8; 16];
            // A closed peer reads as the end of the stream (or a reset).
            let _ = stream.read(&mut rest);
            let _ = closed_after.send(stalled.elapsed());
        }
    });
    let error = loopback_client()
        .with_idle_timeout(std::time::Duration::from_millis(300))
        .fetch(&format!("http://localhost.test:{port}/"))
        .expect_err("stalled");
    assert_eq!(error.kind(), FetchErrorKind::Timeout, "{error}");
    let closed = receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the server saw the socket close within 5 s");
    assert!(
        closed < std::time::Duration::from_secs(3),
        "closed after {closed:?}"
    );
}
