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

/// A public address standing in for a public host. The guard checks it
/// as public; [`UrlGuard::dial_as`] then dials it at a loopback server,
/// so no test connects to it.
const PUBLIC: &str = "8.8.4.4";

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
