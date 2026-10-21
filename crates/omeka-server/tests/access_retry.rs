//! `POST /api/romeka/media/{id}/derivatives/retry`: the button a curator
//! presses on a media whose playable copy failed. The route queues that one
//! row, starts the install's derivative worker, and answers the media document
//! with the copy `pending` again.
//!
//! The server under test is the real router with the seeded admin key and a
//! real file store. The worker is not started: the route composes the job row
//! through the same dispatcher a create uses, and the assertion is on the row
//! the retry queued.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{append_key, multipart_asset_request, router_with_store, seeded_pool, send};
use omeka_api::password;
use omeka_db::ids::MediaId;
use omeka_db::repos::{MediaDerivativeRepo, UserRepo};
use omeka_db::rows::{DerivativeKind, DerivativeState, NewUser};
use omeka_db::with_db;
use omeka_media::store::FileStore;
use serde_json::{Value, json};
use sqlx::SqlitePool;

/// The seeded site's admin password is not known to this test, so the author
/// who may not retry the admin's media signs in with this one.
const PASSWORD: &str = "a long test password";

fn corpus(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/media-kinds")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// Create one public AVI media as the seeded admin, and answer its id.
async fn create_media(app: &Router) -> u64 {
    create_media_public(app, true).await
}

async fn create_media_public(app: &Router, public: bool) -> u64 {
    let data = json!({
        "o:is_public": public,
        "dcterms:title": [{"type": "literal", "@value": "Reel"}],
        "o:media": [{
            "o:ingester": "upload",
            "o:is_public": public,
            "file_index": 0,
        }],
    })
    .to_string();
    let answer = send(
        app.clone(),
        multipart_asset_request(
            &append_key("/api/items"),
            "reel.avi",
            &corpus("mjpeg.avi"),
            &data,
        ),
    )
    .await;
    assert!(answer.status.is_success(), "{}", answer.body);
    let created: Value = answer.json();
    created["o:media"][0]["o:id"]
        .as_u64()
        .expect("the media id")
}

/// The access row a retry works, after the worker's first attempt failed.
async fn failed_access(pool: &SqlitePool, media: u64) -> omeka_db::rows::DerivativeRow {
    let repo = MediaDerivativeRepo::new(pool);
    let commands = omeka_media::tool::Tools::on_path().derivative_commands();
    let claimed = loop {
        let row = repo
            .claim_next(&commands)
            .await
            .expect("a claim")
            .expect("a row the create queued");
        if row.kind == DerivativeKind::AccessCopy {
            break row;
        }
        // The poster is not the row under test: put it back for the worker.
        repo.requeue(row.media_id, row.kind)
            .await
            .expect("the poster goes back to the queue");
    };
    assert_eq!(claimed.media_id, MediaId(media));
    repo.finish(
        claimed.media_id,
        claimed.kind,
        DerivativeState::Failed,
        Some("the ffmpeg command failed: exit status 1".to_string()),
        Some("ffmpeg".to_string()),
    )
    .await
    .expect("the failure");
    repo.get(MediaId(media), DerivativeKind::AccessCopy)
        .await
        .expect("the row")
        .expect("the access row")
}

/// A retry request carrying the seeded admin key.
fn retry_request(media: u64) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(append_key(&format!(
            "/api/romeka/media/{media}/derivatives/retry"
        )))
        .body(Body::empty())
        .expect("the request")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_copy_is_queued_again_and_the_document_says_pending() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, _seed) = seeded_pool().await;
    let db = omeka_db::DbPool::from(pool.clone());
    let app = router_with_store(pool.clone(), store);
    let media = create_media(&app).await;
    let failed = failed_access(&pool, media).await;
    assert_eq!(failed.state, DerivativeState::Failed, "{failed:?}");

    let answer = send(app.clone(), retry_request(media)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let document: Value = answer.json();
    assert_eq!(
        document["o-module-romeka:access"]["state"], "pending",
        "{document}"
    );
    assert_eq!(document["o:id"], media, "{document}");

    let queued = with_db!(&db, |db| MediaDerivativeRepo::new(db)
        .get(MediaId(media), DerivativeKind::AccessCopy)
        .await)
    .expect("the row")
    .expect("the access row");
    assert_eq!(queued.state, DerivativeState::Pending, "{queued:?}");
    assert_eq!(queued.tool.as_deref(), Some("ffmpeg"), "{queued:?}");
    assert!(queued.reason.is_none(), "{queued:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_media_with_nothing_to_retry_is_a_conflict() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, _seed) = seeded_pool().await;
    let app = router_with_store(pool.clone(), store);
    let media = create_media(&app).await;
    // The row is queued, not failed: the worker may already be making it.
    let answer = send(app.clone(), retry_request(media)).await;
    assert_eq!(answer.status, StatusCode::CONFLICT, "{}", answer.body);
    let document: Value = answer.json();
    assert!(
        document["errors"]["error"]
            .as_str()
            .is_some_and(|text| text.contains("no failed copy")),
        "{document}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_id_that_names_no_media_is_not_found() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let app = router_with_store(pool, store);
    let answer = send(app.clone(), retry_request(seed.media.0 + 1000)).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND, "{}", answer.body);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_private_media_is_not_found_for_a_reader_who_may_not_see_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, _seed) = seeded_pool().await;
    let app = router_with_store(pool.clone(), store);
    let media = create_media_public(&app, false).await;
    failed_access(&pool, media).await;

    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/romeka/media/{media}/derivatives/retry"))
        .header("origin", "http://localhost")
        .body(Body::empty())
        .expect("the request");
    let answer = send(app.clone(), request).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND, "{}", answer.body);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_author_who_may_not_edit_the_media_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, _seed) = seeded_pool().await;
    let db = omeka_db::DbPool::from(pool.clone());
    let app = router_with_store(pool.clone(), store);
    let media = create_media(&app).await;
    failed_access(&pool, media).await;
    let sam = with_db!(&db, |db| UserRepo::new(db)
        .create(&NewUser {
            email: "sam@example.org".to_string(),
            name: "Sam".to_string(),
            password_hash: Some(password::hash(PASSWORD).expect("hash")),
            role: "author".to_string(),
            is_active: true,
            created: None,
        })
        .await)
    .expect("the author");
    let cookie = common::login_cookie(app.clone(), &sam.email, PASSWORD).await;

    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/romeka/media/{media}/derivatives/retry"))
        .header("cookie", cookie)
        .header("origin", "http://localhost")
        .body(Body::empty())
        .expect("the request");
    let answer = send(app.clone(), request).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);

    let untouched = with_db!(&db, |db| MediaDerivativeRepo::new(db)
        .get(MediaId(media), DerivativeKind::AccessCopy)
        .await)
    .expect("the row")
    .expect("the access row");
    assert_eq!(
        untouched.state,
        DerivativeState::Failed,
        "a refused retry changes nothing: {untouched:?}"
    );
}
