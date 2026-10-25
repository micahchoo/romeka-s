//! A bulk delete on an S3 store: the files move under `trash/` in the bucket, an undo moves them
//! back byte for byte, and emptying the trash removes them.
//!
//! Gated on `ROMEKA_S3_TEST_ENDPOINT`, as `import_s3.rs` is; `ROMEKA_S3_TEST_ACCESS_KEY` and
//! `ROMEKA_S3_TEST_SECRET_KEY` default to `minioadmin`.

// The shared `bulk_site` re-exports `site`, which this test replaces with an S3 `site_with`.
#![allow(clippy::unwrap_used, clippy::expect_used, unused_imports)]

mod bulk_site;
mod import_site;

use std::sync::Arc;
use std::time::Duration;

use bulk_site::*;
use import_site::{PNG, site_with};
use omeka_db::ids::ItemId;
use omeka_db::repos::MediaRepo;
use omeka_db::rows::{NewMedia, NewResource};
use omeka_media::store::{S3Config, S3Store, Store};
use serde_json::json;

fn s3_store(test: &str) -> Option<Arc<S3Store>> {
    let Ok(endpoint) = std::env::var("ROMEKA_S3_TEST_ENDPOINT") else {
        eprintln!("SKIP: ROMEKA_S3_TEST_ENDPOINT is not set, so {test} did not run");
        return None;
    };
    let key = |name: &str| std::env::var(name).unwrap_or_else(|_| "minioadmin".to_string());
    let bucket = format!(
        "romeka-trash-{}",
        &omeka_media::store::new_storage_id().unwrap()[..16]
    );
    let store = S3Store::new(S3Config {
        endpoint: endpoint.parse().unwrap(),
        bucket,
        region: "us-east-1".to_string(),
        access_key: key("ROMEKA_S3_TEST_ACCESS_KEY"),
        secret_key: key("ROMEKA_S3_TEST_SECRET_KEY"),
        public_base: None,
        url_ttl: Duration::from_secs(900),
    })
    .unwrap();
    store.create_bucket().unwrap();
    Some(Arc::new(store))
}

#[tokio::test(flavor = "multi_thread")]
async fn on_s3_the_trash_moves_the_objects_and_undo_and_the_purge_follow() {
    let Some(s3) = s3_store("on_s3_the_trash_moves_the_objects_and_undo_and_the_purge_follow")
    else {
        return;
    };
    let store: Arc<dyn Store> = s3.clone();
    let site = site_with(|_| store).await;
    let anna = item(&site, "Letter to Anna", &[]).await;
    s3.put(PNG, "original/annas3.png", Some("image/png"))
        .unwrap();
    s3.put(b"thumbnail", "square/annas3.jpg", Some("image/jpeg"))
        .unwrap();
    MediaRepo::new(&site.pool)
        .create(&NewMedia {
            resource: NewResource::default(),
            item_id: ItemId(anna),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            media_type: Some("image/png".to_string()),
            storage_id: Some("annas3".to_string()),
            extension: Some("png".to_string()),
            size: Some(PNG.len() as i64),
            has_original: true,
            has_thumbnails: true,
            ..NewMedia::default()
        })
        .await
        .unwrap();

    let id = preview(&site, json!({ "id": [anna] }), json!({ "kind": "delete" })).await["id"]
        .as_u64()
        .unwrap();
    assert_eq!(run(&site, id).await["status"], "done");
    assert!(!s3.exists("original/annas3.png").unwrap());
    assert_eq!(s3.read("trash/original/annas3.png").unwrap(), PNG);
    assert!(s3.exists("trash/square/annas3.jpg").unwrap());

    assert_eq!(undo(&site, id, false).await["status"], "undone");
    assert_eq!(s3.read("original/annas3.png").unwrap(), PNG);
    assert!(!s3.exists("trash/original/annas3.png").unwrap());

    let again = preview(&site, json!({ "id": [anna] }), json!({ "kind": "delete" })).await["id"]
        .as_u64()
        .unwrap();
    assert_eq!(run(&site, again).await["status"], "done");
    let (status, purged) = admin(&site, "POST", "/api/romeka/bulk/trash/purge", None).await;
    assert_eq!(status, 200, "{purged}");
    for path in [
        "trash/original/annas3.png",
        "trash/square/annas3.jpg",
        "original/annas3.png",
    ] {
        assert!(!s3.exists(path).unwrap(), "{path} is gone");
    }
}
