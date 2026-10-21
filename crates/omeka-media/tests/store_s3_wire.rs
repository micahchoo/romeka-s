//! What the S3 store puts on the wire: the `Content-Type` of a whole object and
//! of a multipart create, and the response overrides a signed download carries.
//!
//! The endpoint is a loopback server this test starts, so no MinIO and no
//! network are needed. The signed URL is inspected as a string; the two writes
//! are sent to the server, which records each request's head.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use omeka_media::store::{Access, Download, FileUrls, S3Config, S3Store, Store};

/// One recorded request: the request line and the headers, lowercased.
#[derive(Debug, Clone)]
struct Recorded {
    head: String,
}

impl Recorded {
    fn header(&self, name: &str) -> Option<&str> {
        let prefix = format!("{name}:");
        self.head
            .lines()
            .find_map(|line| line.strip_prefix(&prefix).map(str::trim))
    }
}

/// A server that answers `answers` requests, then stops.
struct Wire {
    port: u16,
    recorded: Arc<Mutex<Vec<Recorded>>>,
}

impl Wire {
    fn start(answers: Vec<(u16, &'static str, &'static str)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let recorded = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&recorded);
        thread::spawn(move || {
            for (status, extra, body) in answers {
                let Ok((stream, _)) = listener.accept() else {
                    return;
                };
                let head = read_head(&stream);
                seen.lock().unwrap().push(Recorded { head });
                write_answer(stream, status, extra, body);
            }
        });
        Self { port, recorded }
    }

    fn config(&self) -> S3Config {
        S3Config {
            endpoint: format!("http://127.0.0.1:{}", self.port).parse().unwrap(),
            bucket: "romeka".to_string(),
            region: "us-east-1".to_string(),
            access_key: "AKIDEXAMPLE".to_string(),
            secret_key: "wire-secret".to_string(),
            public_base: None,
            url_ttl: Duration::from_secs(900),
        }
    }

    fn store(&self) -> S3Store {
        S3Store::new(self.config()).unwrap()
    }

    fn requests(&self) -> Vec<Recorded> {
        self.recorded.lock().unwrap().clone()
    }
}

/// Read the request line and headers, then drain the body.
fn read_head(stream: &TcpStream) -> String {
    let mut reader = stream.try_clone().unwrap();
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if reader.read(&mut byte).unwrap_or(0) == 0 {
            break;
        }
        head.push(byte[0]);
    }
    let text = String::from_utf8_lossy(&head).to_lowercase();
    let length = text
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    let mut read = 0;
    while read < length {
        match reader.read(&mut body[read..]) {
            Ok(0) | Err(_) => break,
            Ok(count) => read += count,
        }
    }
    text
}

fn write_answer(mut stream: TcpStream, status: u16, extra: &str, body: &str) {
    let answer = format!(
        "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(answer.as_bytes());
}

const MULTIPART_CREATED: &str = "<InitiateMultipartUploadResult \
xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\"><UploadId>upload-1</UploadId>\
</InitiateMultipartUploadResult>";
const MULTIPART_DONE: &str = "<CompleteMultipartUploadResult><Location>http://example/</Location>\
<Bucket>romeka</Bucket><Key>original/large.webm</Key><ETag>\"done\"</ETag>\
</CompleteMultipartUploadResult>";

#[test]
fn put_object_sends_the_media_type() {
    let wire = Wire::start(vec![(200, "", "")]);
    wire.store()
        .put(b"webm bytes", "original/a.webm", Some("video/webm"))
        .unwrap();
    let requests = wire.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].header("content-type"), Some("video/webm"));
}

#[test]
fn a_put_without_a_type_sends_no_content_type() {
    let wire = Wire::start(vec![(200, "", "")]);
    wire.store().put(b"x", "original/a.bin", None).unwrap();
    assert_eq!(wire.requests()[0].header("content-type"), None);
}

#[test]
fn a_multipart_create_sends_the_media_type_and_the_parts_do_not() {
    let wire = Wire::start(vec![
        (200, "", MULTIPART_CREATED),
        (200, "ETag: \"part-1\"\r\n", ""),
        (200, "", MULTIPART_DONE),
    ]);
    let bytes = vec![7u8; 5 * 1024 * 1024];
    let mut source: &[u8] = &bytes;
    let written = wire
        .store()
        .put_stream(
            &mut source,
            Some(bytes.len() as u64),
            "original/large.webm",
            Some("video/webm"),
        )
        .unwrap();
    assert_eq!(written, bytes.len() as u64);
    let requests = wire.requests();
    assert_eq!(requests.len(), 3, "create, one part, complete");
    assert_eq!(requests[0].header("content-type"), Some("video/webm"));
    assert_eq!(requests[1].header("content-type"), None);
    assert_eq!(requests[2].header("content-type"), None);
}

#[test]
fn a_signed_download_asks_the_bucket_to_answer_with_the_type_and_name() {
    let wire = Wire::start(Vec::new());
    let store = wire.store();
    let download = Download::of(Some("video/webm"), Some("A Talk.webm"), "abc.webm");
    let url = store.download_url(
        "original/abc.webm",
        Access::Signed {
            ttl: Duration::from_secs(3600),
        },
        &download,
    );
    assert!(url.contains("response-content-type=video%2Fwebm"), "{url}");
    assert!(
        url.contains("response-content-disposition=inline%3B%20filename%3D%22A%20Talk.webm%22"),
        "{url}"
    );
    assert!(url.contains("X-Amz-Expires=3600"), "{url}");
    assert!(!url.contains("wire-secret"), "{url}");

    let urls = FileUrls::new(Arc::new(store), "https://example.org");
    let url = urls.original("original/abc.webm", &download);
    assert!(url.contains("response-content-type=video%2Fwebm"), "{url}");
}

#[test]
fn a_signed_attachment_asks_the_bucket_to_save_the_file() {
    let wire = Wire::start(Vec::new());
    let store = wire.store();
    let download = Download::attachment(
        Some("application/pdf"),
        Some("C:\\Users\\a\\scan 1.pdf"),
        "abc.pdf",
    );
    let url = store.download_url(
        "original/abc.pdf",
        Access::Signed {
            ttl: Duration::from_secs(3600),
        },
        &download,
    );
    // The query is URL-encoded once more than the header, so read the value
    // the bucket would decode.
    let parsed = url::Url::parse(&url).expect("a URL");
    let disposition = parsed
        .query_pairs()
        .find(|(name, _)| name == "response-content-disposition")
        .map(|(_, value)| value.into_owned())
        .expect("the signed disposition");
    assert_eq!(disposition, download.disposition());
    assert!(
        url.contains("response-content-type=application%2Fpdf"),
        "{url}"
    );
    assert!(url.contains("X-Amz-Signature="), "{url}");
}
