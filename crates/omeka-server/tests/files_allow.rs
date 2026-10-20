//! `GET /api/romeka/files/allow`: the answer Caddy's `forward_auth` asks for
//! before it serves a stored file. It reads the original URI from
//! `X-Forwarded-Uri` and answers 204 (serve), 403 (refuse) or 404 (no such
//! file), with `X-Romeka-Private: 1` on a 204 for a file that is not public.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use omeka_db::ids::{ItemId, UserId};
use omeka_db::repos::{ApiKeyRepo, AssetRepo, ItemRepo, MediaRepo, UserRepo};
use omeka_db::rows::{NewApiKey, NewAsset, NewItem, NewMedia, NewResource, NewUser};
use omeka_db::with_db;

use common::{KEY_CREDENTIAL, KEY_IDENTITY};

const ALLOW: &str = "/api/romeka/files/allow";
const PASSWORD: &str = "files-allow-pass-1";
const OWNER_EMAIL: &str = "files-owner@example.com";
const STRANGER_EMAIL: &str = "files-stranger@example.com";
const OWNER_KEY: &str = "cccccccccccccccccccccccccccccc00";
const OWNER_SECRET: &str = "cccccccccccccccccccccccccccccc01";

/// The private import of S14 plus one public media and one asset: a private
/// item owned by an author, its private media, a public item with a public
/// media, and a public media on that private item.
async fn files_router() -> Router {
    let (pool, _seed) = common::seeded_engine_pool().await;
    let owner = user(&pool, OWNER_EMAIL).await;
    user(&pool, STRANGER_EMAIL).await;
    with_db!(&pool, |db| ApiKeyRepo::new(db)
        .create(&NewApiKey {
            id: OWNER_KEY.to_string(),
            owner_id: owner,
            label: "Owner key".to_string(),
            credential_hash: bcrypt::hash(OWNER_SECRET, 4).unwrap(),
            last_ip: None,
            last_accessed: None,
            created: None,
        })
        .await)
    .unwrap();
    let private_item = item(&pool, owner, false).await;
    media(
        &pool,
        private_item,
        owner,
        false,
        "privatemedia",
        Some("jpg"),
    )
    .await;
    media(
        &pool,
        private_item,
        owner,
        true,
        "publiconprivate",
        Some("jpg"),
    )
    .await;
    let public_item = item(&pool, owner, true).await;
    media(&pool, public_item, owner, true, "publicmedia", Some("png")).await;
    media(&pool, public_item, owner, true, "bare", None).await;
    with_db!(&pool, |db| AssetRepo::new(db)
        .create(&NewAsset {
            owner_id: Some(owner),
            name: "logo.png".to_string(),
            media_type: "image/png".to_string(),
            storage_id: "siteasset".to_string(),
            extension: Some("png".to_string()),
            alt_text: None,
        })
        .await)
    .unwrap();
    common::router(pool)
}

async fn user(pool: &omeka_db::DbPool, email: &str) -> UserId {
    with_db!(pool, |db| UserRepo::new(db)
        .create(&NewUser {
            email: email.to_string(),
            name: email.to_string(),
            password_hash: Some(bcrypt::hash(PASSWORD, 4).unwrap()),
            role: "author".to_string(),
            is_active: true,
            created: None,
        })
        .await)
    .unwrap()
    .id
}

async fn item(pool: &omeka_db::DbPool, owner: UserId, is_public: bool) -> ItemId {
    with_db!(pool, |db| ItemRepo::new(db)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(owner),
                is_public,
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await)
    .unwrap()
    .id()
}

async fn media(
    pool: &omeka_db::DbPool,
    item: ItemId,
    owner: UserId,
    is_public: bool,
    storage_id: &str,
    extension: Option<&str>,
) {
    with_db!(pool, |db| MediaRepo::new(db)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: Some(owner),
                is_public,
                ..NewResource::default()
            },
            item_id: item,
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            storage_id: Some(storage_id.to_string()),
            extension: extension.map(str::to_string),
            has_original: true,
            has_thumbnails: true,
            ..NewMedia::default()
        })
        .await)
    .unwrap();
}

fn allow(forwarded_uri: &str, cookie: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method("GET")
        .uri(ALLOW)
        .header("x-forwarded-method", "GET")
        .header("x-forwarded-uri", forwarded_uri);
    if let Some(cookie) = cookie {
        builder = builder.header("cookie", cookie);
    }
    builder.body(Body::empty()).unwrap()
}

async fn status(app: &Router, forwarded_uri: &str, cookie: Option<&str>) -> StatusCode {
    common::send(app.clone(), allow(forwarded_uri, cookie))
        .await
        .status
}

const PRIVATE_FILES: [&str; 4] = [
    "/files/original/privatemedia.jpg",
    "/files/large/privatemedia.jpg",
    "/files/medium/privatemedia.jpg",
    "/files/square/privatemedia.jpg",
];

#[tokio::test]
async fn s14_a_private_items_files_are_refused_signed_out_and_served_to_its_owner() {
    let app = files_router().await;
    let owner = common::login_cookie(app.clone(), OWNER_EMAIL, PASSWORD).await;
    let stranger = common::login_cookie(app.clone(), STRANGER_EMAIL, PASSWORD).await;
    for path in PRIVATE_FILES {
        assert_eq!(
            status(&app, path, None).await,
            StatusCode::FORBIDDEN,
            "{path} signed out"
        );
        assert_eq!(
            status(&app, path, Some(&stranger)).await,
            StatusCode::FORBIDDEN,
            "{path} for an author who does not own it"
        );

        let answer = common::send(app.clone(), allow(path, Some(&owner))).await;
        assert_eq!(
            answer.status,
            StatusCode::NO_CONTENT,
            "{path} for its owner"
        );
        assert_eq!(
            common::header(&answer, "x-romeka-private"),
            Some("1"),
            "{path}"
        );

        let keyed = format!("{path}?key_identity={OWNER_KEY}&key_credential={OWNER_SECRET}");
        let answer = common::send(app.clone(), allow(&keyed, None)).await;
        assert_eq!(
            answer.status,
            StatusCode::NO_CONTENT,
            "{path} by the owner's key"
        );
        assert_eq!(
            common::header(&answer, "x-romeka-private"),
            Some("1"),
            "{path}"
        );

        let admin = format!("{path}?key_identity={KEY_IDENTITY}&key_credential={KEY_CREDENTIAL}");
        assert_eq!(
            status(&app, &admin, None).await,
            StatusCode::NO_CONTENT,
            "{path} by a global admin's key"
        );
    }
}

#[tokio::test]
async fn a_public_media_on_a_private_item_is_private() {
    let app = files_router().await;
    let path = "/files/original/publiconprivate.jpg";
    assert_eq!(status(&app, path, None).await, StatusCode::FORBIDDEN);
    let owner = common::login_cookie(app.clone(), OWNER_EMAIL, PASSWORD).await;
    let answer = common::send(app.clone(), allow(path, Some(&owner))).await;
    assert_eq!(answer.status, StatusCode::NO_CONTENT);
    assert_eq!(common::header(&answer, "x-romeka-private"), Some("1"));
}

#[tokio::test]
async fn a_public_file_is_served_to_anyone_and_not_marked_private() {
    let app = files_router().await;
    for path in [
        "/files/original/publicmedia.png",
        "/files/large/publicmedia.jpg",
        "/files/square/publicmedia.jpg",
        "/files/original/bare",
        "/files/asset/siteasset.png",
        "/files/original/publicmedia.png?v=2",
        "/files/original/public%6dedia.png",
    ] {
        let answer = common::send(app.clone(), allow(path, None)).await;
        assert_eq!(answer.status, StatusCode::NO_CONTENT, "{path}");
        assert_eq!(common::header(&answer, "x-romeka-private"), None, "{path}");
    }
}

#[tokio::test]
async fn a_file_no_row_owns_is_not_found() {
    let app = files_router().await;
    for path in [
        "/files/original/missing.jpg",
        "/files/original/publicmedia.gif",
        "/files/original/publicmedia",
        "/files/original/bare.jpg",
        "/files/large/publicmedia.png",
        "/files/large/missing.jpg",
        "/files/asset/missing.png",
        "/files/asset/siteasset.jpg",
    ] {
        assert_eq!(
            status(&app, path, None).await,
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
}

#[tokio::test]
async fn a_path_outside_the_store_layout_is_refused() {
    let app = files_router().await;
    for path in [
        "/files/../deploy/.env",
        "/files/original/../asset/siteasset.png",
        "/files/original/./publicmedia.png",
        "/files//original/publicmedia.png",
        "/files/original//publicmedia.png",
        "/files/original/publicmedia.png/",
        "/files/%2e%2e/original/publicmedia.png",
        "/files/original/%2e%2e%2fasset/siteasset.png",
        "/files/original%2fpublicmedia.png",
        "/files/original%2Fpublicmedia.png",
        "/files/original/%252e%252e",
        "/files/original/publicmedia.png%00",
        "/files/original/..%5casset%5csiteasset.png",
        "/files/original\\publicmedia.png",
        "/files/original/%ff.png",
        "/files/thumbnails/publicmedia.jpg",
        "/files/original",
        "/files/original/",
        "/files/",
        "/files",
        "/original/publicmedia.png",
        "/FILES/original/publicmedia.png",
        "original/publicmedia.png",
        "",
    ] {
        assert_eq!(
            status(&app, path, None).await,
            StatusCode::FORBIDDEN,
            "{path:?}"
        );
    }
}

#[tokio::test]
async fn a_request_without_the_forwarded_uri_is_refused() {
    let app = files_router().await;
    let request = Request::builder()
        .method("GET")
        .uri(ALLOW)
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        common::send(app, request).await.status,
        StatusCode::FORBIDDEN
    );
}
