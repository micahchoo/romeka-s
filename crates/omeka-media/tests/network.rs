//! The real client and the network ingesters over loopback.
//!
//! `HttpClient` is the one production [`HttpFetch`]. Its tests need a real
//! socket, so they start a throwaway HTTP/1.1 server on `127.0.0.1:0` (an
//! ephemeral port, AGENTS.md "Tests use ephemeral ports") and speak to it.
//!
//! The ingester tests then drive the real client the same way: a loopback
//! document whose `@id` or thumbnail URL is itself loopback, so no request
//! leaves the machine. The recorded fixtures drive the fake-based tests in
//! `ingesters.rs`.
//!
//! `Youtube` is not covered here. Its poster host is a fixed `img.youtube.com`
//! (`Ingester/Youtube.php:79`), which a loopback server cannot serve without a
//! DNS override. It runs against `omeka_media::testing::FakeHttp` instead.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use omeka_media::http::{FetchErrorKind, HttpFetch};
use omeka_media::http_client::HttpClient;
use omeka_media::ingest::{ErrorStore, IngestEnv, IngestRequest, IngesterRegistry, MediaDraft};
use omeka_media::oembed::{AllowRule, Oembed, OembedAllowList};
use omeka_media::store::Store;
use omeka_media::testing::{FakeThumbnailer, Harness, PassthroughPurifier, media_row};

/// What the loopback server answers for one path.
#[derive(Clone)]
enum Route {
    /// A complete response.
    Reply {
        status: u16,
        reason: &'static str,
        headers: Vec<(&'static str, String)>,
        body: Vec<u8>,
    },
    /// Accept the connection, then send nothing for longer than a test cares.
    Stall,
}

impl Route {
    /// A `200 OK` response.
    fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self::status(200, "OK", body)
    }

    /// A response with an explicit status.
    fn status(status: u16, reason: &'static str, body: impl Into<Vec<u8>>) -> Self {
        Route::Reply {
            status,
            reason,
            headers: Vec::new(),
            body: body.into(),
        }
    }

    /// A `302 Found` to `location`.
    fn redirect(location: impl Into<String>) -> Self {
        Route::Reply {
            status: 302,
            reason: "Found",
            headers: vec![("Location", location.into())],
            body: Vec::new(),
        }
    }
}

/// A loopback HTTP/1.1 server for the duration of one test.
struct Server {
    addr: SocketAddr,
    routes: Arc<Mutex<BTreeMap<String, Route>>>,
    stop: Arc<AtomicBool>,
    accept: Option<thread::JoinHandle<()>>,
}

impl Server {
    /// Start a server with no routes; add them with [`Server::with_route`].
    ///
    /// Routes are added after the port is known, because a test document has to
    /// name the server's own URL.
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let addr = listener.local_addr().expect("local addr");
        let routes: Arc<Mutex<BTreeMap<String, Route>>> = Arc::new(Mutex::new(BTreeMap::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let stop_accept = Arc::clone(&stop);
        let routes_accept = Arc::clone(&routes);
        let accept = thread::spawn(move || {
            for stream in listener.incoming() {
                if stop_accept.load(Ordering::Relaxed) {
                    break;
                }
                if let Ok(stream) = stream {
                    let routes = Arc::clone(&routes_accept);
                    thread::spawn(move || serve(stream, &routes));
                }
            }
        });
        Self {
            addr,
            routes,
            stop,
            accept: Some(accept),
        }
    }

    /// Serve `route` at `path`.
    fn route(&mut self, path: &str, route: Route) {
        self.routes
            .lock()
            .expect("routes lock")
            .insert(path.to_string(), route);
    }

    /// An absolute `http://` URL for a path on this server.
    fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Wake the accept loop so it observes the stop flag.
        let _ = TcpStream::connect(self.addr);
        if let Some(accept) = self.accept.take() {
            let _ = accept.join();
        }
    }
}

/// Answer one connection: read the request line and headers, write the route.
fn serve(mut stream: TcpStream, routes: &Arc<Mutex<BTreeMap<String, Route>>>) {
    let Ok(mut reader) = stream.try_clone().map(BufReader::new) else {
        return;
    };
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    loop {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) | Err(_) => break,
            Ok(_) if header == "\r\n" => break,
            Ok(_) => {}
        }
    }
    let path = request_line.split_whitespace().nth(1).unwrap_or("/");
    let route = routes.lock().expect("routes lock").get(path).cloned();
    let (head, body) = match route {
        Some(Route::Stall) => {
            thread::sleep(Duration::from_secs(3));
            (
                "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
                Vec::new(),
            )
        }
        Some(Route::Reply {
            status,
            reason,
            headers,
            body,
        }) => {
            let mut head = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\n",
                body.len()
            );
            for (name, value) in headers {
                head.push_str(&format!("{name}: {value}\r\n"));
            }
            head.push_str("Connection: close\r\n\r\n");
            (head, body)
        }
        None => (
            "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: close\r\n\r\nnot found"
                .to_string(),
            Vec::new(),
        ),
    };
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
    let _ = stream.flush();
}

/// A temp store and the test doubles an ingester needs, with a real client.
struct NetHarness {
    harness: Harness,
    client: HttpClient,
    purifier: PassthroughPurifier,
    thumbnailer: FakeThumbnailer,
    oembed: Oembed,
}

impl NetHarness {
    fn new(tag: &str) -> Self {
        Self {
            harness: Harness::new(tag),
            client: HttpClient::new(),
            purifier: PassthroughPurifier,
            thumbnailer: FakeThumbnailer::default(),
            oembed: Oembed::new(OembedAllowList::shipped()),
        }
    }

    fn with_oembed(mut self, oembed: Oembed) -> Self {
        self.oembed = oembed;
        self
    }

    fn env(&self) -> IngestEnv<'_> {
        IngestEnv {
            store: &self.harness.store,
            http: &self.client,
            purifier: &self.purifier,
            thumbnailer: &self.thumbnailer,
            oembed: &self.oembed,
            validation: &self.harness.validation,
        }
    }

    /// Run one ingester from the default registry.
    fn run(&self, name: &str, content: serde_json::Value) -> (MediaDraft, ErrorStore) {
        let env = self.env();
        let request = IngestRequest::from_value(content);
        let mut draft = MediaDraft::new(media_row(1, name, name));
        let mut errors = ErrorStore::default();
        IngesterRegistry::with_defaults()
            .get(name)
            .ingest(&mut draft, &request, &env, &mut errors);
        (draft, errors)
    }
}

#[test]
fn client_fetches_status_reason_and_body() {
    let mut server = Server::start();
    server.route("/file", Route::ok("hello"));
    let response = HttpClient::new()
        .fetch(&server.url("/file"))
        .expect("fetch");
    assert_eq!(response.status, 200);
    assert_eq!(response.reason, "OK");
    assert_eq!(response.body, b"hello");
    assert!(response.is_ok());
}

#[test]
fn client_returns_a_non_2xx_response_without_an_error() {
    let mut server = Server::start();
    server.route("/missing", Route::status(404, "Not Found", ""));
    let response = HttpClient::new()
        .fetch(&server.url("/missing"))
        .expect("a 404 is a response, not a transport error");
    assert_eq!(response.status, 404);
    assert_eq!(response.reason, "Not Found");
    assert!(!response.is_ok());
}

#[test]
fn client_times_out_instead_of_hanging() {
    let mut server = Server::start();
    server.route("/slow", Route::Stall);
    let client = HttpClient::new().with_timeout(Duration::from_millis(200));
    let started = Instant::now();
    let error = client.fetch(&server.url("/slow")).expect_err("timeout");
    assert_eq!(error.kind(), FetchErrorKind::Timeout, "{error}");
    assert!(started.elapsed() < Duration::from_secs(2), "returned late");
}

#[test]
fn client_fails_an_oversized_response() {
    let mut server = Server::start();
    server.route("/big", Route::ok(vec![b'x'; 4096]));
    let client = HttpClient::new().with_max_bytes(1024);
    let error = client.fetch(&server.url("/big")).expect_err("too large");
    assert_eq!(error.kind(), FetchErrorKind::TooLarge, "{error}");
}

#[test]
fn client_follows_a_same_scheme_redirect() {
    let mut server = Server::start();
    server.route("/from", Route::redirect("/to"));
    server.route("/to", Route::ok("final"));
    let response = HttpClient::new()
        .fetch(&server.url("/from"))
        .expect("fetch");
    assert_eq!(response.status, 200);
    assert_eq!(response.body, b"final");
}

#[test]
fn client_does_not_follow_a_cross_scheme_redirect() {
    let mut server = Server::start();
    server.route("/from", Route::redirect("https://example.org/to"));
    let response = HttpClient::new()
        .fetch(&server.url("/from"))
        .expect("fetch");
    assert_eq!(
        response.status, 302,
        "a cross-scheme redirect is not followed"
    );
}

#[test]
fn client_reports_a_refused_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().expect("local addr");
    drop(listener);
    let client = HttpClient::new().with_timeout(Duration::from_millis(500));
    let error = client
        .fetch(&format!("http://{addr}/x"))
        .expect_err("connection refused");
    assert_eq!(error.kind(), FetchErrorKind::Connect, "{error}");
}

/// `Ingester/Url.php`: the real client downloads the loopback file and it
/// becomes the stored original.
#[test]
fn url_ingester_over_the_real_client() {
    let body = include_bytes!("fixtures/ingest/url_response.png");
    let mut server = Server::start();
    server.route("/files/logo.png", Route::ok(&body[..]));
    let source = server.url("/files/logo.png");
    let net = NetHarness::new("real-url");
    let (draft, errors) = net.run("url", serde_json::json!({"ingest_url": source}));

    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(draft.row.source.as_deref(), Some(source.as_str()));
    assert_eq!(draft.row.extension.as_deref(), Some("png"));
    let storage_id = draft.row.storage_id.clone().expect("storage id");
    assert_eq!(
        net.harness
            .store
            .read(&format!("original/{storage_id}.png"))
            .expect("stored original"),
        body
    );
}

/// `Ingester/IIIF.php`: the real client fetches the info document and the
/// pre-computed size the document points to, both loopback.
#[test]
fn iiif_ingester_over_the_real_client() {
    let mut server = Server::start();
    let id = server.url("/iiif/1");
    let document = serde_json::json!({
        "@context": "http://iiif.io/api/image/2/context.json",
        "@id": id,
        "protocol": "http://iiif.io/api/image",
        "width": 4000,
        "height": 3000,
        "sizes": [{"width": 900, "height": 800}],
    });
    let thumbnail_path = "/iiif/1/full/900,800/0/default.jpg";
    server.route("/iiif/1/info.json", Route::ok(document.to_string()));
    server.route(thumbnail_path, Route::ok("jpeg"));
    let source = server.url("/iiif/1/info.json");
    let net = NetHarness::new("real-iiif");
    let (draft, errors) = net.run("iiif", serde_json::json!({"o:source": source}));

    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(draft.data.to_value(), document);
    assert!(draft.row.has_thumbnails);
    assert_eq!(
        draft.row.source, None,
        "IIIF stores the document, not a source"
    );
}

/// `Ingester/IiifPresentation.php`: the real client fetches the manifest and its
/// thumbnail, both loopback.
#[test]
fn iiif_presentation_ingester_over_the_real_client() {
    let mut server = Server::start();
    let thumbnail = server.url("/thumb.jpg");
    let manifest = serde_json::json!({
        "@context": "http://iiif.io/api/presentation/2/context.json",
        "@id": server.url("/manifest"),
        "@type": "sc:Manifest",
        "thumbnail": {"@id": thumbnail},
    });
    let source = server.url("/manifest");
    server.route("/manifest", Route::ok(manifest.to_string()));
    server.route("/thumb.jpg", Route::ok("jpeg"));
    let net = NetHarness::new("real-iiif-presentation");
    let (draft, errors) = net.run("iiif_presentation", serde_json::json!({"o:source": source}));

    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(draft.data.to_value(), manifest);
    assert!(draft.row.has_thumbnails);
}

/// `Ingester/OEmbed.php`: the real client discovers the endpoint from the page,
/// then fetches the response and the thumbnail, all loopback.
#[test]
fn oembed_ingester_over_the_real_client() {
    let mut server = Server::start();
    let endpoint = server.url("/oembed");
    let thumb = server.url("/thumb.jpg");
    let page = format!(
        "<html><head><link rel=\"alternate\" type=\"application/json+oembed\" href=\"{endpoint}\"></head></html>"
    );
    let response = serde_json::json!({"type": "video", "title": "T", "thumbnail_url": thumb});
    let source = server.url("/page");
    server.route("/page", Route::ok(page));
    server.route("/oembed", Route::ok(response.to_string()));
    server.route("/thumb.jpg", Route::ok("jpeg"));

    let allow = OembedAllowList::new(vec![AllowRule {
        hosts: vec!["127.0.0.1"],
        path_prefixes: Vec::new(),
        endpoint: None,
    }]);
    let net = NetHarness::new("real-oembed").with_oembed(Oembed::new(allow));
    let (draft, errors) = net.run("oembed", serde_json::json!({"o:source": source}));

    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(draft.data.to_value()["title"], "T");
    assert_eq!(draft.row.source.as_deref(), Some(source.as_str()));
    assert!(draft.row.has_thumbnails);
}
