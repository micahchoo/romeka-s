//! The per-user staged uploads, the media create that names one, the limits
//! route and the link probe (`crates/omeka-server/README.md`, "Staged uploads").
//!
//! The tus routes are driven through the router, so a dropped `PATCH` is the
//! same sequence the admin runs: send, get 409, ask HEAD, send the rest. The
//! media create runs the import's streaming path, and the probe fetches a real
//! loopback server with the guarded client.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod imports_fixture;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{Answer, request, send};
use imports_fixture::{ADMIN, EDITOR, SECOND_ADMIN, fixture};
use omeka_api::settings::{
    DISABLE_FILE_VALIDATION, IMPORT_ALLOW_PRIVATE_URLS, IMPORT_UPLOAD_MAX_FILE,
};
use omeka_db::repos::SettingRepo;
use omeka_db::with_db;
use omeka_media::store::{FileStore, Store};
use serde_json::{Value, json};

const COLLECTION: &str = "/api/romeka/uploads";

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

fn header<'a>(answer: &'a Answer, name: &str) -> Option<&'a str> {
    answer
        .headers
        .get(name)
        .and_then(|value| value.to_str().ok())
}

fn metadata(pairs: &[(&str, &[u8])]) -> String {
    pairs
        .iter()
        .map(|(key, value)| format!("{key} {}", base64(value)))
        .collect::<Vec<_>>()
        .join(",")
}

struct Tus<'f> {
    fixture: &'f imports_fixture::Fixture,
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

    async fn create_as(
        &self,
        caller: (&str, &str),
        name: &str,
        length: u64,
        item: Option<u64>,
        media_type: Option<&str>,
    ) -> Answer {
        let mut pairs: Vec<(&str, &[u8])> = vec![("filename", name.as_bytes())];
        if let Some(media_type) = media_type {
            pairs.push(("filetype", media_type.as_bytes()));
        }
        let item_text = item.map(|item| item.to_string());
        if let Some(item) = &item_text {
            pairs.push(("item", item.as_bytes()));
        }
        self.send(
            caller,
            "POST",
            COLLECTION,
            &[
                ("upload-length", length.to_string()),
                ("upload-metadata", metadata(&pairs)),
            ],
            Vec::new(),
        )
        .await
    }

    async fn create(&self, name: &str, length: u64) -> String {
        let made = self.create_as(ADMIN, name, length, None, None).await;
        assert_eq!(made.status, StatusCode::CREATED, "{}", made.body);
        made.headers["location"].to_str().unwrap().to_string()
    }

    async fn patch_as(
        &self,
        caller: (&str, &str),
        location: &str,
        offset: u64,
        bytes: &[u8],
    ) -> Answer {
        self.send(
            caller,
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

    async fn patch(&self, location: &str, offset: u64, bytes: &[u8]) -> Answer {
        self.patch_as(ADMIN, location, offset, bytes).await
    }

    async fn head(&self, location: &str) -> Answer {
        self.send(ADMIN, "HEAD", location, &[], Vec::new()).await
    }
}

fn upload_id(location: &str) -> String {
    location.rsplit('/').next().unwrap().to_string()
}

/// The seeded item's id: the fixtures build the resource table in order, so it
/// is not 1.
async fn the_item(fixture: &imports_fixture::Fixture) -> u64 {
    let listed = fixture.call("GET", "/api/items", None).await;
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.body);
    listed.json()[0]["o:id"].as_u64().expect("the seeded item")
}

fn photo() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/media-kinds/image.jpg"),
    )
    .expect("the JPEG fixture")
}

/// A lost `PATCH` reply: the client sends the chunk again from its old offset,
/// gets 409 with the stored offset, asks HEAD, and goes on. The list route then
/// answers the upload, filtered by the item it was started for.
#[tokio::test]
async fn an_upload_resumes_after_a_lost_patch_and_lists_by_item() {
    let fixture = fixture().await;
    let tus = Tus { fixture: &fixture };
    let item = the_item(&fixture).await;
    let bytes = photo();
    let made = tus
        .create_as(
            EDITOR,
            "photo.jpg",
            bytes.len() as u64,
            Some(item),
            Some("image/jpeg"),
        )
        .await;
    assert_eq!(made.status, StatusCode::CREATED, "{}", made.body);
    let location = made.headers["location"].to_str().unwrap().to_string();
    assert!(location.starts_with(COLLECTION), "{location}");
    assert_eq!(made.json()["itemId"], item);
    let other = tus.create_as(EDITOR, "other.jpg", 3, None, None).await;
    assert_eq!(other.status, StatusCode::CREATED, "{}", other.body);
    let other = other.headers["location"].to_str().unwrap().to_string();

    let on_the_way = tus.patch_as(EDITOR, &location, 0, &bytes[..6]).await;
    assert_eq!(
        on_the_way.status,
        StatusCode::NO_CONTENT,
        "{}",
        on_the_way.body
    );
    assert_eq!(header(&on_the_way, "upload-offset"), Some("6"));

    let again = tus.patch_as(EDITOR, &location, 0, &bytes[..6]).await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(again.json()["code"], "offsetConflict");
    assert_eq!(header(&again, "upload-offset"), Some("6"));

    let head = tus.send(EDITOR, "HEAD", &location, &[], Vec::new()).await;
    assert_eq!(head.status, StatusCode::OK);
    assert_eq!(header(&head, "upload-offset"), Some("6"));
    assert_eq!(
        header(&head, "upload-length"),
        Some(bytes.len().to_string().as_str())
    );

    let rest = tus.patch_as(EDITOR, &location, 6, &bytes[6..]).await;
    assert_eq!(rest.status, StatusCode::NO_CONTENT, "{}", rest.body);
    assert_eq!(
        header(&rest, "upload-offset"),
        Some(bytes.len().to_string().as_str())
    );

    let listed = fixture
        .call_as(EDITOR, "GET", &format!("{COLLECTION}?item={item}"), None)
        .await;
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.body);
    let uploads = listed.json()["uploads"].as_array().unwrap().clone();
    assert_eq!(uploads.len(), 1, "the item filter leaves the other one out");
    assert_eq!(uploads[0]["path"], "photo.jpg");
    assert_eq!(uploads[0]["mediaType"], "image/jpeg");
    assert_eq!(uploads[0]["received"], bytes.len());
    assert!(uploads[0]["expires"].is_string(), "{}", listed.body);

    let all = fixture.call_as(EDITOR, "GET", COLLECTION, None).await;
    assert_eq!(all.json()["uploads"].as_array().unwrap().len(), 2);
    assert_ne!(upload_id(&location), upload_id(&other));
}

/// An upload belongs to the person who made it: another account, an
/// administrator included, answers 404 on every route.
#[tokio::test]
async fn another_account_gets_404_and_an_admin_has_no_shortcut() {
    let fixture = fixture().await;
    let tus = Tus { fixture: &fixture };
    let item = the_item(&fixture).await;
    let made = tus.create_as(EDITOR, "mine.jpg", 4, None, None).await;
    assert_eq!(made.status, StatusCode::CREATED, "{}", made.body);
    let location = made.headers["location"].to_str().unwrap().to_string();
    let edited = tus.patch_as(EDITOR, &location, 0, b"mine").await;
    assert_eq!(edited.status, StatusCode::NO_CONTENT, "{}", edited.body);

    let head = tus
        .send(SECOND_ADMIN, "HEAD", &location, &[], Vec::new())
        .await;
    assert_eq!(head.status, StatusCode::NOT_FOUND);
    let patched = tus.patch_as(SECOND_ADMIN, &location, 4, b"x").await;
    assert_eq!(patched.status, StatusCode::NOT_FOUND);
    let removed = tus
        .send(SECOND_ADMIN, "DELETE", &location, &[], Vec::new())
        .await;
    assert_eq!(removed.status, StatusCode::NOT_FOUND);

    let listed = fixture.call_as(SECOND_ADMIN, "GET", COLLECTION, None).await;
    assert_eq!(listed.json()["uploads"].as_array().unwrap().len(), 0);

    // A create that names someone else's upload is a 404 too, and the upload
    // survives for its owner.
    let create = fixture
        .call_as(
            SECOND_ADMIN,
            "POST",
            "/api/media",
            Some(&json!({
                "o:ingester": "upload",
                "o:item": {"o:id": item},
                "o-module-romeka:upload": upload_id(&location),
            })),
        )
        .await;
    assert_eq!(create.status, StatusCode::NOT_FOUND, "{}", create.body);
    assert_eq!(
        tus.send(EDITOR, "HEAD", &location, &[], Vec::new())
            .await
            .status,
        StatusCode::OK
    );
}

/// `OPTIONS` names the protocol, its extensions and the largest file.
#[tokio::test]
async fn options_names_the_protocol_and_the_limit() {
    let fixture = fixture().await;
    let tus = Tus { fixture: &fixture };
    let answer = tus
        .send(EDITOR, "OPTIONS", COLLECTION, &[], Vec::new())
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

/// The file limit is the import's `import_upload_max_file`.
#[tokio::test]
async fn the_upload_limit_is_the_import_limit() {
    let fixture = fixture().await;
    with_db!(&fixture.pool, |db| SettingRepo::new(db)
        .set(IMPORT_UPLOAD_MAX_FILE, &json!(8))
        .await)
    .unwrap();
    let tus = Tus { fixture: &fixture };
    let fits = tus.create_as(EDITOR, "a.jpg", 8, None, None).await;
    assert_eq!(fits.status, StatusCode::CREATED, "{}", fits.body);
    let refused = tus.create_as(EDITOR, "b.jpg", 9, None, None).await;
    assert_eq!(refused.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(refused.json()["code"], "fileTooLarge");
    assert_eq!(refused.json()["limit"], 8);
}

/// A media create naming a completed upload ingests the bytes, removes the
/// upload, and places the new media after the item's last one.
#[tokio::test]
async fn a_media_create_moves_a_staged_file_and_appends_it() {
    let fixture = fixture().await;
    let tus = Tus { fixture: &fixture };
    let item = the_item(&fixture).await;
    let bytes = photo();
    let location = tus.create("photo.jpg", bytes.len() as u64).await;
    let done = tus.patch(&location, 0, &bytes).await;
    assert_eq!(done.status, StatusCode::NO_CONTENT, "{}", done.body);
    let before = fixture
        .call("GET", &format!("/api/items/{item}"), None)
        .await
        .json();
    let before_ids: Vec<u64> = before["o:media"]
        .as_array()
        .unwrap()
        .iter()
        .map(|media| media["o:id"].as_u64().unwrap())
        .collect();

    let created = fixture
        .call(
            "POST",
            "/api/media",
            Some(&json!({
                "o:ingester": "upload",
                "o:item": {"o:id": item},
                "o-module-romeka:upload": upload_id(&location),
            })),
        )
        .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let media = created.json();
    assert_eq!(media["o:ingester"], "upload");
    assert_eq!(media["o:renderer"], "file");
    assert_eq!(media["o:source"], "photo.jpg");
    assert_eq!(media["o:media_type"], "image/jpeg");
    assert_eq!(media["o:size"], bytes.len());
    assert_eq!(media["o:sha256"], omeka_media::store::sha256_hex(&bytes));
    assert_eq!(media["o:item"]["o:id"], item);
    let new_id = media["o:id"].as_u64().unwrap();

    assert_eq!(
        tus.head(&location).await.status,
        StatusCode::NOT_FOUND,
        "the upload is gone after the create"
    );
    assert!(
        fixture
            .files
            .path()
            .join("original")
            .read_dir()
            .unwrap()
            .any(|entry| entry.unwrap().path().is_file()),
        "the original is in the store"
    );

    let after = fixture
        .call("GET", &format!("/api/items/{item}"), None)
        .await
        .json();
    let after_ids: Vec<u64> = after["o:media"]
        .as_array()
        .unwrap()
        .iter()
        .map(|media| media["o:id"].as_u64().unwrap())
        .collect();
    assert_eq!(after_ids.len(), before_ids.len() + 1);
    assert_eq!(after_ids.last(), Some(&new_id), "the new media comes last");

    // A second one goes after the first, not beside it.
    let location = tus.create("second.jpg", bytes.len() as u64).await;
    tus.patch(&location, 0, &bytes).await;
    let second = fixture
        .call(
            "POST",
            "/api/media",
            Some(&json!({
                "o:ingester": "upload",
                "o:item": {"o:id": item},
                "o-module-romeka:upload": upload_id(&location),
            })),
        )
        .await;
    assert_eq!(second.status, StatusCode::OK, "{}", second.body);
    let after = fixture
        .call("GET", &format!("/api/items/{item}"), None)
        .await
        .json();
    let ids: Vec<u64> = after["o:media"]
        .as_array()
        .unwrap()
        .iter()
        .map(|media| media["o:id"].as_u64().unwrap())
        .collect();
    assert_eq!(ids.last(), Some(&second.json()["o:id"].as_u64().unwrap()));
}

/// Two creates naming one upload run at once. The claim's one UPDATE lets one
/// through; the other answers 409 and writes no media.
#[tokio::test]
async fn two_creates_for_one_upload_make_one_media() {
    const SIZE: usize = 32 * 1024 * 1024;
    let fixture = fixture().await;
    let tus = Tus { fixture: &fixture };
    let item = the_item(&fixture).await;
    let bytes = vec![b'x'; SIZE];
    let location = tus
        .create_as(ADMIN, "race.txt", SIZE as u64, None, Some("text/plain"))
        .await
        .headers["location"]
        .to_str()
        .unwrap()
        .to_string();
    let put = tus.patch(&location, 0, &bytes).await;
    assert_eq!(put.status, StatusCode::NO_CONTENT, "{}", put.body);
    let before: Vec<u64> = fixture
        .call("GET", &format!("/api/items/{item}"), None)
        .await
        .json()["o:media"]
        .as_array()
        .unwrap()
        .iter()
        .map(|media| media["o:id"].as_u64().unwrap())
        .collect();

    let body = json!({
        "o:ingester": "upload",
        "o:item": {"o:id": item},
        "o-module-romeka:upload": upload_id(&location),
    });
    let (first, second) = tokio::join!(
        fixture.call("POST", "/api/media", Some(&body)),
        fixture.call("POST", "/api/media", Some(&body)),
    );
    let mut statuses = [first.status, second.status];
    statuses.sort();
    assert_eq!(
        statuses,
        [StatusCode::OK, StatusCode::CONFLICT],
        "first: {} second: {}",
        first.body,
        second.body
    );
    let refused = if first.status == StatusCode::CONFLICT {
        &first
    } else {
        &second
    };
    assert!(
        refused.json()["errors"]["error"]
            .as_str()
            .is_some_and(|message| message.contains("being added")),
        "{}",
        refused.body
    );

    let after = fixture
        .call("GET", &format!("/api/items/{item}"), None)
        .await
        .json();
    let media = after["o:media"].as_array().unwrap();
    assert_eq!(media.len(), before.len() + 1, "exactly one media row");
    let added: Vec<u64> = media
        .iter()
        .map(|media| media["o:id"].as_u64().unwrap())
        .filter(|id| !before.contains(id))
        .collect();
    assert_eq!(added.len(), 1, "exactly one new media row");
    let document = fixture
        .call("GET", &format!("/api/media/{}", added[0]), None)
        .await
        .json();
    assert_eq!(document["o:source"], "race.txt");
    assert_eq!(
        tus.head(&location).await.status,
        StatusCode::NOT_FOUND,
        "the winner removed the upload"
    );
}

/// A claim is exclusive while it is fresh, reclaimable after the stale bound,
/// and released by a create that fails.
#[tokio::test]
async fn a_claim_is_exclusive_then_reclaimable_and_releasable() {
    let fixture = fixture().await;
    let tus = Tus { fixture: &fixture };
    let item = the_item(&fixture).await;
    let owner = fixture
        .call("GET", &format!("/api/items/{item}"), None)
        .await
        .json()["o:owner"]["o:id"]
        .as_u64()
        .expect("the owner");
    let location = tus.create("claim.txt", 4).await;
    tus.patch(&location, 0, b"data").await;
    let upload = upload_id(&location);
    let user = omeka_db::ids::UserId(owner);
    let now = chrono::Utc::now().naive_utc();

    let claim = with_db!(&fixture.pool, |db| omeka_api::uploads::claim(
        db, user, &upload, now
    )
    .await)
    .expect("the first claim");
    let busy = with_db!(&fixture.pool, |db| omeka_api::uploads::claim(
        db, user, &upload, now
    )
    .await);
    assert!(matches!(busy, Err(omeka_api::uploads::ClaimError::Busy)));
    let later = now + omeka_api::uploads::CLAIM_STALE + chrono::TimeDelta::seconds(1);
    let reclaimed = with_db!(&fixture.pool, |db| omeka_api::uploads::claim(
        db, user, &upload, later
    )
    .await)
    .expect("a stale claim is reclaimable");
    with_db!(&fixture.pool, |db| omeka_api::uploads::release(db, &claim)
        .await)
    .unwrap();
    with_db!(&fixture.pool, |db| omeka_api::uploads::release(
        db, &reclaimed
    )
    .await)
    .unwrap();
    let free = with_db!(&fixture.pool, |db| omeka_api::uploads::claim(
        db, user, &upload, later
    )
    .await);
    assert!(free.is_ok(), "the release made it claimable again");
}

/// A create the ingester refuses keeps the upload for a retry and leaves no
/// new file in the store.
#[tokio::test]
async fn a_refused_media_create_keeps_the_upload_for_a_retry() {
    let fixture = fixture().await;
    let tus = Tus { fixture: &fixture };
    let item = the_item(&fixture).await;
    let bytes = b"MZ not a storeable type";
    let location = tus.create("notes.exe", bytes.len() as u64).await;
    tus.patch(&location, 0, bytes).await;
    let before = fixture
        .call("GET", &format!("/api/items/{item}"), None)
        .await
        .json();
    let before_count = before["o:media"].as_array().unwrap().len();

    let refused = fixture
        .call(
            "POST",
            "/api/media",
            Some(&json!({
                "o:ingester": "upload",
                "o:item": {"o:id": item},
                "o-module-romeka:upload": upload_id(&location),
            })),
        )
        .await;
    assert_eq!(
        refused.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        refused.body
    );
    assert!(refused.json()["errors"]["o-module-romeka:upload"].is_array());
    assert_eq!(
        tus.head(&location).await.status,
        StatusCode::OK,
        "kept for a retry"
    );
    let originals = fixture.files.path().join("original");
    let stored = std::fs::read_dir(&originals)
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(stored, 0, "the refused file left nothing in the store");
    let after = fixture
        .call("GET", &format!("/api/items/{item}"), None)
        .await
        .json();
    assert_eq!(after["o:media"].as_array().unwrap().len(), before_count);

    // The operator turns validation off; the same upload now creates a media.
    with_db!(&fixture.pool, |db| SettingRepo::new(db)
        .set(DISABLE_FILE_VALIDATION, &json!(true))
        .await)
    .unwrap();
    let created = fixture
        .call(
            "POST",
            "/api/media",
            Some(&json!({
                "o:ingester": "upload",
                "o:item": {"o:id": item},
                "o-module-romeka:upload": upload_id(&location),
            })),
        )
        .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    assert_eq!(tus.head(&location).await.status, StatusCode::NOT_FOUND);
}

/// An inline `o:media` entry cannot name a staged upload. The create that claims
/// it is the media route; an item create would make a media without a claim.
#[tokio::test]
async fn an_inline_media_entry_cannot_name_a_staged_upload() {
    let fixture = fixture().await;
    let tus = Tus { fixture: &fixture };
    let location = tus.create("inline.jpg", 4).await;
    tus.patch(&location, 0, b"data").await;
    let items = fixture.call("GET", "/api/items", None).await.json();
    let count = items.as_array().unwrap().len();

    let refused = fixture
        .call(
            "POST",
            "/api/items",
            Some(&json!({
                "o:media": [{
                    "o:ingester": "upload",
                    "o-module-romeka:upload": upload_id(&location),
                }],
            })),
        )
        .await;
    assert_eq!(
        refused.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        refused.body
    );
    let messages = refused.json()["errors"]["o:media"]
        .as_array()
        .unwrap()
        .clone();
    assert!(
        messages.iter().any(|message| message
            .as_str()
            .is_some_and(|text| text.contains("its own media create"))),
        "{}",
        refused.body
    );
    let after = fixture.call("GET", "/api/items", None).await.json();
    assert_eq!(
        after.as_array().unwrap().len(),
        count,
        "no item was written"
    );
    assert_eq!(tus.head(&location).await.status, StatusCode::OK);
}

/// The limits answer a contributor reads, from the install's settings.
#[tokio::test]
async fn a_contributor_reads_the_limits_and_an_anonymous_caller_does_not() {
    let fixture = fixture().await;
    let answer = fixture
        .call_as(EDITOR, "GET", "/api/romeka/upload-limits", None)
        .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let limits = answer.json();
    assert_eq!(limits["maxFileBytes"], 2_147_483_648_u64);
    assert_eq!(limits["validationOff"], false);
    let kinds: Vec<&str> = limits["kinds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|kind| kind.as_str().unwrap())
        .collect();
    assert_eq!(kinds.len(), omeka_media::kind::MediaKind::ALL.len());
    assert!(kinds.contains(&"video"), "{kinds:?}");
    assert!(kinds.contains(&"document"), "{kinds:?}");
    assert!(
        limits["extensions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|extension| extension == "jpg")
    );

    with_db!(&fixture.pool, |db| SettingRepo::new(db)
        .set(IMPORT_UPLOAD_MAX_FILE, &json!(1234))
        .await)
    .unwrap();
    with_db!(&fixture.pool, |db| SettingRepo::new(db)
        .set(DISABLE_FILE_VALIDATION, &json!(true))
        .await)
    .unwrap();
    let changed = fixture
        .call_as(EDITOR, "GET", "/api/romeka/upload-limits", None)
        .await
        .json();
    assert_eq!(changed["maxFileBytes"], 1234);
    assert_eq!(changed["validationOff"], true);
    assert_eq!(changed["extensions"], json!([]));

    let anonymous = send(
        fixture.app.clone(),
        request("GET", "/api/romeka/upload-limits"),
    )
    .await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
}

/// A loopback HTTP/1.1 server that answers one request with `body`.
fn serve_once(body: &'static [u8], content_type: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("addr");
    thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut request = [0u8; 2048];
        let _ = stream.read(&mut request);
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(body);
        let _ = stream.flush();
    });
    format!("http://{address}/image.png")
}

/// The probe names an image on a real server: its type, its size, its pixels,
/// and the one reader that fits.
#[tokio::test]
async fn a_probe_names_an_image_on_a_test_server() {
    const PNG: &[u8] = include_bytes!("../../../fixtures/media-kinds/image.png");
    let fixture = fixture().await;
    with_db!(&fixture.pool, |db| SettingRepo::new(db)
        .set(IMPORT_ALLOW_PRIVATE_URLS, &json!(true))
        .await)
    .unwrap();
    let url = serve_once(PNG, "image/png");
    let before = fixture.call("GET", "/api/media", None).await.json();
    let before_count = before.as_array().unwrap().len();
    let answer = fixture
        .call_as(
            EDITOR,
            "POST",
            "/api/romeka/media/probe",
            Some(&json!({"url": url})),
        )
        .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let probe = answer.json();
    assert_eq!(probe["readers"], json!(["url"]));
    assert_eq!(probe["suggested"], "url");
    assert_eq!(probe["mediaType"], "image/png");
    assert_eq!(probe["size"], PNG.len());
    assert_eq!(probe["image"], json!({"w": 120, "h": 80}));
    assert_eq!(probe["canvases"], Value::Null);
    let after = fixture.call("GET", "/api/media", None).await.json();
    assert_eq!(
        after.as_array().unwrap().len(),
        before_count,
        "the probe writes no row"
    );
}

/// The same probe without the operator's switch is refused by the address
/// guard, and writes nothing.
#[tokio::test]
async fn a_probe_refuses_a_private_address() {
    const PNG: &[u8] = include_bytes!("../../../fixtures/media-kinds/image.png");
    let fixture = fixture().await;
    let url = serve_once(PNG, "image/png");
    let before = fixture.call("GET", "/api/media", None).await.json();
    let before_count = before.as_array().unwrap().len();
    let answer = fixture
        .call_as(
            EDITOR,
            "POST",
            "/api/romeka/media/probe",
            Some(&json!({"url": url})),
        )
        .await;
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
    assert!(
        answer.json()["errors"]["url"]
            .as_array()
            .is_some_and(|messages| !messages.is_empty()),
        "{}",
        answer.body
    );
    let after = fixture.call("GET", "/api/media", None).await.json();
    assert_eq!(after.as_array().unwrap().len(), before_count);
}

/// The probe refuses a URL that is not an http address, and a caller without
/// the media-create grant.
#[tokio::test]
async fn a_probe_needs_the_grant_and_an_address() {
    let fixture = fixture().await;
    let answer = fixture
        .call_as(
            EDITOR,
            "POST",
            "/api/romeka/media/probe",
            Some(&json!({"url": "not a url"})),
        )
        .await;
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
    let anonymous = send(
        fixture.app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/romeka/media/probe")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"url":"https://example.org/a.jpg"}"#))
            .unwrap(),
    )
    .await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
}

/// The sweep tick removes an upload 24 hours after its last byte, with its
/// row, its file and its parts.
#[tokio::test]
async fn the_sweep_removes_an_expired_upload() {
    let fixture = fixture().await;
    let tus = Tus { fixture: &fixture };
    let location = tus.create("old.jpg", 4).await;
    tus.patch(&location, 0, b"old!").await;
    let store: Arc<dyn Store> = Arc::new(FileStore::new(fixture.files.path(), "/files"));
    let now = chrono::Utc::now().naive_utc();

    let early = with_db!(&fixture.pool, |db| omeka_api::uploads::sweep(
        db,
        &store,
        now + chrono::TimeDelta::hours(23),
    )
    .await)
    .unwrap();
    assert_eq!(early, 0, "kept inside its 24 hours");
    assert_eq!(tus.head(&location).await.status, StatusCode::OK);

    let later = with_db!(&fixture.pool, |db| omeka_api::uploads::sweep(
        db,
        &store,
        now + chrono::TimeDelta::hours(25),
    )
    .await)
    .unwrap();
    assert_eq!(later, 1);
    assert_eq!(tus.head(&location).await.status, StatusCode::NOT_FOUND);
    assert!(
        std::fs::read_dir(fixture.files.path().join("uploads"))
            .map(|entries| entries
                .flat_map(|entry| std::fs::read_dir(entry.unwrap().path().join("files")).unwrap())
                .count())
            .unwrap_or(0)
            == 0,
        "the file went with the row"
    );
}
