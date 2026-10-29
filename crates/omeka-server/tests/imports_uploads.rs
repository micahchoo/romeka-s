//! The tus routes into an import's staging area (`docs/import-v2.md`,
//! "Arrival", "Staging"): discovery, create, resume after a lost `PATCH`,
//! the limits with uploads in progress, who may upload, the unzip job a
//! finished ZIP starts, and the scan of what arrived.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod imports_fixture;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{Answer, KEY_CREDENTIAL, KEY_IDENTITY, send};
use imports_fixture::{EDITOR, JPEG, SECOND_ADMIN, fixture};
use omeka_db::ids::ImportId;
use omeka_db::repos::{ImportRepo, JobRepo, SettingRepo};
use omeka_db::rows::ImportEnding;
use omeka_db::with_db;
use serde_json::{Value, json};

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0_u32, |n, (at, byte)| n | u32::from(*byte) << (16 - 8 * at));
        for at in 0..4 {
            if at <= chunk.len() {
                out.push(char::from(TABLE[(n >> (18 - 6 * at)) as usize & 63]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn keyed(uri: &str, (identity, credential): (&str, &str)) -> String {
    let separator = if uri.contains('?') { '&' } else { '?' };
    format!("{uri}{separator}key_identity={identity}&key_credential={credential}")
}

const ADMIN: (&str, &str) = (KEY_IDENTITY, KEY_CREDENTIAL);

struct Tus<'f> {
    fixture: &'f imports_fixture::Fixture,
    collection: String,
}

impl Tus<'_> {
    async fn send(
        &self,
        caller: (&str, &str),
        method: &str,
        uri: &str,
        headers: &[(&str, String)],
        body: Vec<u8>,
    ) -> Answer {
        let mut request = Request::builder()
            .method(method)
            .uri(keyed(uri, caller))
            .header("tus-resumable", "1.0.0");
        for (name, value) in headers {
            request = request.header(*name, value);
        }
        send(
            self.fixture.app.clone(),
            request.body(Body::from(body)).unwrap(),
        )
        .await
    }

    async fn create_as(&self, caller: (&str, &str), path: &str, length: u64) -> Answer {
        self.send(
            caller,
            "POST",
            &self.collection,
            &[
                ("upload-length", length.to_string()),
                (
                    "upload-metadata",
                    format!(
                        "filetype aW1hZ2UvanBlZw==,filename {}",
                        base64(path.as_bytes())
                    ),
                ),
            ],
            Vec::new(),
        )
        .await
    }

    async fn create(&self, path: &str, length: u64) -> String {
        let made = self.create_as(ADMIN, path, length).await;
        assert_eq!(made.status, StatusCode::CREATED, "{}", made.body);
        made.headers["location"].to_str().unwrap().to_string()
    }

    async fn patch(&self, location: &str, offset: u64, bytes: &[u8]) -> Answer {
        self.send(
            ADMIN,
            "PATCH",
            location,
            &[
                ("content-type", "application/offset+octet-stream".into()),
                ("upload-offset", offset.to_string()),
            ],
            bytes.to_vec(),
        )
        .await
    }

    async fn head(&self, location: &str) -> Answer {
        self.send(ADMIN, "HEAD", location, &[], Vec::new()).await
    }

    async fn whole(&self, path: &str, bytes: &[u8]) -> Answer {
        let location = self.create(path, bytes.len() as u64).await;
        let done = self.patch(&location, 0, bytes).await;
        assert_eq!(done.status, StatusCode::NO_CONTENT, "{}", done.body);
        done
    }
}

async fn staging_draft(fixture: &imports_fixture::Fixture) -> Value {
    let made = fixture
        .call(
            "POST",
            "/api/romeka/imports",
            Some(&json!({ "sources": [{ "kind": "staging", "id": "up" }] })),
        )
        .await;
    assert_eq!(made.status, StatusCode::CREATED, "{}", made.body);
    made.json()
}

fn header<'a>(answer: &'a Answer, name: &str) -> Option<&'a str> {
    answer
        .headers
        .get(name)
        .and_then(|value| value.to_str().ok())
}

#[tokio::test]
async fn options_names_the_protocol_and_the_largest_file() {
    let fixture = fixture().await;
    let draft = staging_draft(&fixture).await;
    let tus = Tus {
        fixture: &fixture,
        collection: format!("/api/romeka/imports/{}/uploads", draft["id"]),
    };
    let answer = tus
        .send(ADMIN, "OPTIONS", &tus.collection, &[], Vec::new())
        .await;
    assert_eq!(answer.status, StatusCode::NO_CONTENT);
    assert_eq!(header(&answer, "tus-resumable"), Some("1.0.0"));
    assert_eq!(header(&answer, "tus-version"), Some("1.0.0"));
    assert_eq!(header(&answer, "tus-max-size"), Some("2147483648"));
    assert_eq!(
        header(&answer, "tus-extension"),
        Some("creation,termination")
    );
}

/// A lost `PATCH` reply: the client sends the chunk again from its old
/// offset, gets 409 with the stored offset, asks HEAD, and goes on.
#[tokio::test]
async fn an_upload_resumes_after_a_lost_patch() {
    let fixture = fixture().await;
    let draft = staging_draft(&fixture).await;
    let tus = Tus {
        fixture: &fixture,
        collection: format!("/api/romeka/imports/{}/uploads", draft["id"]),
    };
    let location = tus.create("Box 1/0001.jpg", JPEG.len() as u64).await;
    assert!(
        location.starts_with(&format!("/api/romeka/imports/{}/uploads/", draft["id"])),
        "{location}"
    );
    let first = tus.patch(&location, 0, &JPEG[..6]).await;
    assert_eq!(first.status, StatusCode::NO_CONTENT, "{}", first.body);
    assert_eq!(header(&first, "upload-offset"), Some("6"));

    let again = tus.patch(&location, 0, &JPEG[..6]).await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(again.json()["code"], "offsetConflict");
    assert_eq!(header(&again, "upload-offset"), Some("6"));

    let head = tus.head(&location).await;
    assert_eq!(head.status, StatusCode::OK);
    assert_eq!(header(&head, "upload-offset"), Some("6"));
    assert_eq!(
        header(&head, "upload-length"),
        Some(JPEG.len().to_string().as_str())
    );
    assert_eq!(header(&head, "cache-control"), Some("no-store"));

    let rest = tus.patch(&location, 6, &JPEG[6..]).await;
    assert_eq!(rest.status, StatusCode::NO_CONTENT, "{}", rest.body);
    assert_eq!(
        header(&rest, "upload-offset"),
        Some(JPEG.len().to_string().as_str())
    );
    assert_eq!(header(&rest, "romeka-unzip-job"), None, "not a ZIP");

    let listed = fixture.call("GET", &tus.collection, None).await;
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.body);
    let listed = listed.json();
    assert_eq!(listed["uploads"][0]["path"], "Box 1/0001.jpg");
    assert_eq!(listed["uploads"][0]["received"], JPEG.len());
    assert_eq!(listed["used"], JPEG.len());
    assert_eq!(listed["limit"]["file"], 2_147_483_648_u64);
    assert!(listed["keepUntil"].is_string(), "{listed}");

    let scanned = fixture
        .call(
            "POST",
            &format!("/api/romeka/imports/{}/scan", draft["id"]),
            None,
        )
        .await;
    assert_eq!(scanned.status, StatusCode::OK, "{}", scanned.body);
    assert_eq!(scanned.json()["sources"][0]["files"], 1);
}

const MIB: usize = 1024 * 1024;

/// A `PATCH` with a chunked body of 1 MiB blocks over a real socket, until
/// the server answers or `blocks` are sent. The body is never in memory: one
/// block is written again and again. Answers the status line and how many
/// blocks left before the server answered.
async fn patch_streamed(
    fixture: &imports_fixture::Fixture,
    location: &str,
    blocks: usize,
) -> (String, usize) {
    use std::io::{Read, Write};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = fixture.app.clone();
    let server = tokio::spawn(async move { axum::serve(listener, app).await });
    let head = format!(
        "PATCH {} HTTP/1.1\r\nhost: localhost\r\ntus-resumable: 1.0.0\r\n\
         content-type: application/offset+octet-stream\r\nupload-offset: 0\r\n\
         transfer-encoding: chunked\r\nconnection: close\r\n\r\n",
        keyed(location, ADMIN)
    );
    let outcome = tokio::task::spawn_blocking(move || {
        let mut socket = std::net::TcpStream::connect(address).unwrap();
        let mut reader = socket.try_clone().unwrap();
        let answered = Arc::new(AtomicBool::new(false));
        let status = Arc::new(std::sync::Mutex::new(String::new()));
        let listening = {
            let (answered, status) = (Arc::clone(&answered), Arc::clone(&status));
            std::thread::spawn(move || {
                let mut text = Vec::new();
                let mut buffer = [0_u8; 4096];
                while let Ok(read) = reader.read(&mut buffer) {
                    if read == 0 {
                        break;
                    }
                    text.extend_from_slice(&buffer[..read]);
                    if text.contains(&b'\n') {
                        *status.lock().unwrap() = String::from_utf8_lossy(&text)
                            .lines()
                            .next()
                            .unwrap()
                            .to_string();
                        answered.store(true, Ordering::SeqCst);
                        break;
                    }
                }
            })
        };
        socket.write_all(head.as_bytes()).unwrap();
        let block = vec![b'x'; MIB];
        let sent = AtomicUsize::new(0);
        for _ in 0..blocks {
            if answered.load(Ordering::SeqCst) {
                break;
            }
            let framed = socket
                .write_all(format!("{MIB:x}\r\n").as_bytes())
                .and_then(|()| socket.write_all(&block))
                .and_then(|()| socket.write_all(b"\r\n"));
            if framed.is_err() {
                break;
            }
            sent.fetch_add(1, Ordering::SeqCst);
        }
        let _ = socket.write_all(b"0\r\n\r\n");
        listening.join().unwrap();
        let line = status.lock().unwrap().clone();
        (line, sent.load(Ordering::SeqCst))
    })
    .await
    .unwrap();
    server.abort();
    outcome
}

/// One `PATCH` takes at most 64 MiB. The cap is read as the body streams, so
/// a body that is far larger is not read to its end.
#[tokio::test]
async fn a_patch_over_64_mib_is_refused_with_413_and_keeps_nothing() {
    let fixture = fixture().await;
    let draft = staging_draft(&fixture).await;
    let tus = Tus {
        fixture: &fixture,
        collection: format!("/api/romeka/imports/{}/uploads", draft["id"]),
    };
    let location = tus.create("big.tif", (600 * MIB) as u64).await;

    let declared = tus
        .send(
            ADMIN,
            "PATCH",
            &location,
            &[
                ("content-type", "application/offset+octet-stream".into()),
                ("upload-offset", "0".into()),
                ("content-length", (65 * MIB).to_string()),
            ],
            b"x".to_vec(),
        )
        .await;
    assert_eq!(declared.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(declared.json()["code"], "chunkTooLarge");

    let (status, sent) = patch_streamed(&fixture, &location, 512).await;
    assert!(status.contains("413"), "{status}");
    assert!(sent < 200, "the server read {sent} MiB of 512 MiB");

    let head = tus.head(&location).await;
    assert_eq!(header(&head, "upload-offset"), Some("0"), "no byte kept");

    let fits = tus.patch(&location, 0, &vec![b'y'; MIB]).await;
    assert_eq!(fits.status, StatusCode::NO_CONTENT, "{}", fits.body);
}

#[tokio::test]
async fn the_import_limit_counts_uploads_in_progress() {
    let fixture = fixture().await;
    with_db!(&fixture.pool, |db| SettingRepo::new(db)
        .set("import_upload_max_total", &json!(100))
        .await)
    .unwrap();
    with_db!(&fixture.pool, |db| SettingRepo::new(db)
        .set("import_upload_max_file", &json!(80))
        .await)
    .unwrap();
    let draft = staging_draft(&fixture).await;
    let tus = Tus {
        fixture: &fixture,
        collection: format!("/api/romeka/imports/{}/uploads", draft["id"]),
    };
    tus.create("a.jpg", 60).await;
    let full = tus.create_as(ADMIN, "b.jpg", 41).await;
    assert_eq!(full.status, StatusCode::PAYLOAD_TOO_LARGE, "{}", full.body);
    assert_eq!(full.json()["code"], "importFull");
    assert_eq!(full.json()["used"], 60);
    assert_eq!(full.json()["limit"], 100);
    let large = tus.create_as(ADMIN, "c.jpg", 81).await;
    assert_eq!(large.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(large.json()["code"], "fileTooLarge");
    tus.create("b.jpg", 40).await;
}

#[tokio::test]
async fn a_refused_name_a_missing_header_and_the_wrong_protocol_are_refused() {
    let fixture = fixture().await;
    let draft = staging_draft(&fixture).await;
    let tus = Tus {
        fixture: &fixture,
        collection: format!("/api/romeka/imports/{}/uploads", draft["id"]),
    };
    let escaping = tus.create_as(ADMIN, "../x.jpg", 3).await;
    assert_eq!(escaping.status, StatusCode::BAD_REQUEST);
    assert_eq!(escaping.json()["code"], "uploadPath");
    let no_length = tus
        .send(
            ADMIN,
            "POST",
            &tus.collection,
            &[("upload-metadata", format!("filename {}", base64(b"a.jpg")))],
            Vec::new(),
        )
        .await;
    assert_eq!(no_length.json()["code"], "uploadLength");
    let no_name = tus
        .send(
            ADMIN,
            "POST",
            &tus.collection,
            &[("upload-length", "3".into())],
            Vec::new(),
        )
        .await;
    assert_eq!(no_name.json()["code"], "uploadMetadata");
    let unversioned = send(
        fixture.app.clone(),
        Request::builder()
            .method("POST")
            .uri(keyed(&tus.collection, ADMIN))
            .header("upload-length", "3")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(unversioned.status, StatusCode::PRECONDITION_FAILED);
    assert_eq!(header(&unversioned, "tus-version"), Some("1.0.0"));
    let location = tus.create("a.jpg", 3).await;
    let wrong_type = tus
        .send(
            ADMIN,
            "PATCH",
            &location,
            &[
                ("content-type", "application/octet-stream".into()),
                ("upload-offset", "0".into()),
            ],
            b"abc".to_vec(),
        )
        .await;
    assert_eq!(wrong_type.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let past = tus.patch(&location, 0, b"abcd").await;
    assert_eq!(past.status, StatusCode::BAD_REQUEST);
    assert_eq!(past.json()["code"], "pastLength");
}

#[tokio::test]
async fn only_the_owner_and_global_admins_upload_and_only_into_a_draft() {
    let fixture = fixture().await;
    let draft = staging_draft(&fixture).await;
    let tus = Tus {
        fixture: &fixture,
        collection: format!("/api/romeka/imports/{}/uploads", draft["id"]),
    };
    let editor = tus.create_as(EDITOR, "a.jpg", 1).await;
    assert_eq!(editor.status, StatusCode::FORBIDDEN);
    let global = tus.create_as(SECOND_ADMIN, "a.jpg", 1).await;
    assert_eq!(global.status, StatusCode::CREATED, "a global admin may");
    with_db!(&fixture.pool, |db| sqlx::query(
        "UPDATE romeka_import SET status = 'done' WHERE id = ?"
    )
    .bind(draft["id"].as_i64().unwrap())
    .execute(db)
    .await
    .map(|_| ()))
    .unwrap();
    let ran = tus.create_as(ADMIN, "b.jpg", 1).await;
    assert_eq!(ran.status, StatusCode::CONFLICT);
    assert_eq!(ran.json()["code"], "notDraft");
}

#[tokio::test]
async fn a_finished_zip_starts_the_unzip_job() {
    let fixture = fixture().await;
    let draft = staging_draft(&fixture).await;
    let tus = Tus {
        fixture: &fixture,
        collection: format!("/api/romeka/imports/{}/uploads", draft["id"]),
    };
    let done = tus.whole("Scans/batch.ZIP", b"PK\x05\x06 not really").await;
    let job: u64 = header(&done, "romeka-unzip-job")
        .expect("the job id")
        .parse()
        .unwrap();
    let row = with_db!(&fixture.pool, |db| JobRepo::new(db)
        .get(omeka_db::ids::JobId::from(job))
        .await)
    .unwrap()
    .unwrap();
    assert_eq!(row.class, "Romeka\\Job\\ImportUnzip");
    let args = row.args.unwrap();
    assert_eq!(args["importId"], draft["id"]);
    assert!(args["upload"].is_string());
    assert!(
        fixture
            .jobs
            .started
            .lock()
            .unwrap()
            .contains(&omeka_db::ids::JobId::from(job))
    );
}

#[tokio::test]
async fn delete_removes_an_upload_and_deleting_the_draft_removes_its_files() {
    let fixture = fixture().await;
    let draft = staging_draft(&fixture).await;
    let tus = Tus {
        fixture: &fixture,
        collection: format!("/api/romeka/imports/{}/uploads", draft["id"]),
    };
    let location = tus.create("a.jpg", 4).await;
    let removed = tus.send(ADMIN, "DELETE", &location, &[], Vec::new()).await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT);
    assert_eq!(tus.head(&location).await.status, StatusCode::NOT_FOUND);
    tus.whole("b.jpg", JPEG).await;
    let staged = fixture
        .files
        .path()
        .join(format!("staging/{}/files", draft["id"]));
    assert_eq!(std::fs::read_dir(&staged).unwrap().count(), 1);
    let gone = fixture
        .call(
            "DELETE",
            &format!("/api/romeka/imports/{}", draft["id"]),
            None,
        )
        .await;
    assert_eq!(gone.status, StatusCode::NO_CONTENT, "{}", gone.body);
    assert_eq!(std::fs::read_dir(&staged).unwrap().count(), 0);
}

/// Discard is for an import that added nothing. Once a run has added, the import
/// keeps its row and its staged files, and Undo is the way back.
#[tokio::test]
async fn an_import_that_added_refuses_discard_and_keeps_its_files() {
    let fixture = fixture().await;
    let draft = staging_draft(&fixture).await;
    let tus = Tus {
        fixture: &fixture,
        collection: format!("/api/romeka/imports/{}/uploads", draft["id"]),
    };
    tus.whole("b.jpg", JPEG).await;
    let id = ImportId(draft["id"].as_u64().unwrap());
    with_db!(&fixture.pool, |db| ImportRepo::new(db)
        .start_run(id, 0)
        .await)
    .unwrap();
    assert!(
        with_db!(&fixture.pool, |db| ImportRepo::new(db)
            .finish(id, ImportEnding::Done)
            .await)
        .unwrap()
    );
    let uri = format!("/api/romeka/imports/{}", id.0);

    let refused = fixture.call("DELETE", &uri, None).await;

    assert_eq!(refused.status, StatusCode::CONFLICT, "{}", refused.body);
    assert_eq!(
        refused.json()["errors"]["error"],
        "Only an import that is not added yet can be deleted. An import that has run stays in Past imports."
    );
    let staged = fixture.files.path().join(format!("staging/{}/files", id.0));
    assert_eq!(std::fs::read_dir(&staged).unwrap().count(), 1);
    let kept = fixture.call("GET", &uri, None).await;
    assert_eq!(kept.status, StatusCode::OK);
    assert_eq!(kept.json()["status"], "done");
}
