//! The file URLs the API and the public site render on an S3 store: an
//! original is always signed, a thumbnail is plain only when its media and its
//! item are public, and a private resource's URL reaches only a caller who may
//! see the resource.
//!
//! No request reaches the endpoint: every row says it has thumbnails, so the
//! documents never ask the store whether the files exist.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use omeka_api::jsonld::BaseUrl;
use omeka_db::ids::{ItemId, MediaId, UserId};
use omeka_db::repos::MediaRepo;
use omeka_db::rows::{NewMedia, NewResource};
use omeka_media::store::{S3Config, S3Store};
use serde_json::Value;

use common::{
    OWNER_KEY_CREDENTIAL, OWNER_KEY_IDENTITY, VISIBILITY_PASSWORD, get, get_cookie, get_keyed,
    login_cookie, send, visibility_pool,
};

const BUCKET_URL: &str = "http://127.0.0.1:9/romeka";

fn s3_store() -> Arc<S3Store> {
    Arc::new(
        S3Store::new(S3Config {
            endpoint: "http://127.0.0.1:9".parse().unwrap(),
            bucket: "romeka".to_string(),
            region: "us-east-1".to_string(),
            access_key: "AKIDEXAMPLE".to_string(),
            secret_key: "file-urls-secret".to_string(),
            public_base: None,
            url_ttl: Duration::from_secs(900),
        })
        .unwrap(),
    )
}

struct Scene {
    app: Router,
    owner_email: &'static str,
    item: ItemId,
    private_item: ItemId,
    media_public: MediaId,
    media_private: MediaId,
    /// A public media on the private item.
    media_in_private_item: MediaId,
}

async fn scene() -> Scene {
    let (pool, vis) = visibility_pool().await;
    let media_in_private_item = add_media(&pool, vis.private_item, vis.owner, "dddddddd").await;
    let app = omeka_server::build_router_with_store(
        pool,
        BaseUrl::parse("http://localhost").unwrap(),
        s3_store(),
    );
    Scene {
        app,
        owner_email: vis.owner_email,
        item: vis.item,
        private_item: vis.private_item,
        media_public: vis.media_public,
        media_private: vis.media_private,
        media_in_private_item,
    }
}

async fn add_media(pool: &sqlx::SqlitePool, item: ItemId, owner: UserId, storage: &str) -> MediaId {
    MediaRepo::new(pool)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: Some(owner),
                title: Some("Public media of a private item".to_string()),
                is_public: true,
                ..NewResource::default()
            },
            item_id: item,
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            media_type: Some("image/jpeg".to_string()),
            storage_id: Some(storage.to_string()),
            extension: Some("jpg".to_string()),
            has_original: true,
            has_thumbnails: true,
            ..NewMedia::default()
        })
        .await
        .unwrap()
        .id()
}

fn is_signed(url: &str, path: &str) -> bool {
    url.starts_with(&format!("{BUCKET_URL}/{path}?")) && url.contains("X-Amz-Signature=")
}

fn is_plain(url: &str, path: &str) -> bool {
    url == format!("{BUCKET_URL}/{path}")
}

fn json(body: &str) -> Value {
    serde_json::from_str(body).unwrap()
}

#[tokio::test]
async fn the_api_signs_originals_and_private_thumbnails_and_leaves_public_thumbnails_plain() {
    let scene = scene().await;

    let answer = send(
        scene.app.clone(),
        get(&format!("/api/media/{}", scene.media_public.0), false),
    )
    .await;
    assert_eq!(answer.status, 200, "{}", answer.body);
    let media = json(&answer.body);
    let original = media["o:original_url"].as_str().unwrap();
    assert!(is_signed(original, "original/eeeeeeee.jpg"), "{original}");
    assert!(original.contains("X-Amz-Expires=900"), "{original}");
    assert!(
        original.contains("response-content-type=image%2Fjpeg"),
        "the bucket answers with the stored type: {original}"
    );
    assert!(
        original.contains("response-content-disposition=inline%3B%20filename%3D%22eeeeeeee.jpg%22"),
        "and with the source's name: {original}"
    );
    for kind in ["large", "medium", "square"] {
        let path = format!("{kind}/eeeeeeee.jpg");
        let url = media["o:thumbnail_urls"][kind].as_str().unwrap();
        assert!(is_plain(url, &path), "{url}");
        let display = media["thumbnail_display_urls"][kind].as_str().unwrap();
        assert!(is_plain(display, &path), "{display}");
    }
    assert!(!answer.body.contains("file-urls-secret"));

    let hidden = send(
        scene.app.clone(),
        get(&format!("/api/media/{}", scene.media_private.0), false),
    )
    .await;
    assert_eq!(hidden.status, 404);
    assert!(!hidden.body.contains("ffffffff"), "{}", hidden.body);

    let owned = send(
        scene.app.clone(),
        get_keyed(
            &format!("/api/media/{}", scene.media_private.0),
            OWNER_KEY_IDENTITY,
            OWNER_KEY_CREDENTIAL,
        ),
    )
    .await;
    assert_eq!(owned.status, 200, "{}", owned.body);
    let square = json(&owned.body)["o:thumbnail_urls"]["square"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(is_signed(&square, "square/ffffffff.jpg"), "{square}");

    let in_private_item = send(
        scene.app.clone(),
        get_keyed(
            &format!("/api/media/{}", scene.media_in_private_item.0),
            OWNER_KEY_IDENTITY,
            OWNER_KEY_CREDENTIAL,
        ),
    )
    .await;
    assert_eq!(in_private_item.status, 200, "{}", in_private_item.body);
    let square = json(&in_private_item.body)["o:thumbnail_urls"]["square"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        is_signed(&square, "square/dddddddd.jpg"),
        "a public media of a private item is private: {square}"
    );

    let item = send(
        scene.app.clone(),
        get(&format!("/api/items/{}", scene.item.0), false),
    )
    .await;
    assert_eq!(item.status, 200, "{}", item.body);
    let square = json(&item.body)["thumbnail_display_urls"]["square"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(is_plain(&square, "square/eeeeeeee.jpg"), "{square}");
    assert!(!item.body.contains("ffffffff"), "{}", item.body);
}

#[tokio::test]
async fn the_site_renders_signed_urls_only_to_a_viewer_who_may_see_the_resource() {
    let scene = scene().await;

    // The item page shows its first visible file whole: the public media's large
    // thumbnail is a plain URL, the private media's does not reach the page, and
    // the file itself opens its media page rather than the signed original.
    let page = send(
        scene.app.clone(),
        get(&format!("/s/vis-pub/item/{}", scene.item.0), false),
    )
    .await;
    assert_eq!(page.status, 200);
    assert!(
        page.body
            .contains(&format!("src=\"{BUCKET_URL}/large/eeeeeeee.jpg\"")),
        "a public thumbnail is plain"
    );
    assert!(
        page.body
            .contains(&format!("/s/vis-pub/media/{}", scene.media_public.0)),
        "the file opens its media page"
    );
    assert!(!page.body.contains("ffffffff"), "a private media stays out");

    // The media page links the original, which is always signed.
    let media_page = send(
        scene.app.clone(),
        get(&format!("/s/vis-pub/media/{}", scene.media_public.0), false),
    )
    .await;
    assert_eq!(media_page.status, 200);
    assert!(
        media_page
            .body
            .contains(&format!("{BUCKET_URL}/original/eeeeeeee.jpg?X-Amz-")),
        "a public original is signed"
    );

    let path = format!("/s/vis-pub/item/{}", scene.private_item.0);
    let anonymous = send(scene.app.clone(), get(&path, false)).await;
    assert_eq!(anonymous.status, 404);
    assert!(!anonymous.body.contains("dddddddd"));

    let cookie = login_cookie(scene.app.clone(), scene.owner_email, VISIBILITY_PASSWORD).await;
    let owned = send(scene.app.clone(), get_cookie(&path, &cookie)).await;
    assert_eq!(owned.status, 200, "{}", owned.body);
    assert!(
        owned
            .body
            .contains(&format!("{BUCKET_URL}/large/dddddddd.jpg?X-Amz-")),
        "a private item's thumbnail is signed for its owner"
    );
    assert!(
        !owned
            .body
            .contains(&format!("{BUCKET_URL}/large/dddddddd.jpg\"")),
        "and never plain"
    );
}
