//! Write-endpoint tests for `assets` and `jobs`, and the two creates the port
//! leaves unbuilt.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{
    append_key, get, json_request, request, router_with_store, seed_system, seeded_pool, send,
};
use omeka_media::store::{FileStore, StorePrefix};

/// The storage id `common::seed_system` gives the asset.
const ASSET_STORAGE_ID: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678";

fn keyed(uri: &str) -> String {
    append_key(uri)
}

/// `AssetAdapter::hydrate` on an update writes `o:name` and `o:alt_text` only,
/// and `validateEntity` refuses an empty name.
#[tokio::test]
async fn update_an_asset() {
    let (pool, seed) = seeded_pool().await;
    let system = seed_system(&pool, &seed).await;
    let app = common::router(pool);

    let read = send(
        app.clone(),
        get(&format!("/api/assets/{}", system.asset), false),
    )
    .await;
    assert_eq!(read.status, StatusCode::OK, "{}", read.body);

    let update = r#"{"o:name":"Renamed logo","o:alt_text":"The Romeka S logo."}"#;
    let updated = send(
        app.clone(),
        json_request(
            "PUT",
            &keyed(&format!("/api/assets/{}", system.asset)),
            update,
        ),
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    let document = updated.json();
    assert_eq!(document["o:name"], "Renamed logo");
    assert_eq!(document["o:alt_text"], "The Romeka S logo.");
    assert_eq!(document["o:filename"], format!("{ASSET_STORAGE_ID}.png"));
    assert_eq!(document["o:owner"]["o:id"], seed.user.0);

    let stored = send(app, get(&format!("/api/assets/{}", system.asset), false)).await;
    assert_eq!(stored.body, updated.body, "update and read differ");
}

/// A full update that omits `o:name` hydrates it to null, which
/// `AssetAdapter::validateEntity` refuses.
#[tokio::test]
async fn a_full_asset_update_without_a_name_is_422() {
    let (pool, seed) = seeded_pool().await;
    let system = seed_system(&pool, &seed).await;
    let app = common::router(pool);
    let answer = send(
        app,
        json_request(
            "PUT",
            &keyed(&format!("/api/assets/{}", system.asset)),
            r#"{"o:alt_text":"No name."}"#,
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        answer.body,
        r#"{"errors":{"o:name":["An asset must have a name."]}}"#
    );
}

/// `Module::deleteAssetFile` removes `asset/<filename>` when the row goes.
#[tokio::test]
async fn deleting_an_asset_removes_the_stored_file() {
    let (pool, seed) = seeded_pool().await;
    let system = seed_system(&pool, &seed).await;
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), omeka_server::FILES_URI);
    let path = FileStore::storage_path(StorePrefix::Asset, ASSET_STORAGE_ID, Some("png"));
    store.put(b"png bytes", &path).expect("put asset");

    let app = router_with_store(pool, store.clone());
    let before = send(
        app.clone(),
        get(&format!("/api/assets/{}", system.asset), false),
    )
    .await;
    let deleted = send(
        app,
        request("DELETE", &keyed(&format!("/api/assets/{}", system.asset))),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.body);
    assert_eq!(deleted.body, before.body, "delete and read differ");
    assert!(!store.exists(&path), "the asset file survived at {path}");
}

/// `JobAdapter::hydrate` is empty, so an update writes nothing and answers the
/// stored representation unchanged.
#[tokio::test]
async fn update_a_job_changes_nothing() {
    let (pool, seed) = seeded_pool().await;
    let system = seed_system(&pool, &seed).await;
    let app = common::router(pool);

    let before = send(
        app.clone(),
        get(&keyed(&format!("/api/jobs/{}", system.job)), false),
    )
    .await;
    assert_eq!(before.status, StatusCode::OK, "{}", before.body);

    let updated = send(
        app.clone(),
        json_request(
            "PUT",
            &keyed(&format!("/api/jobs/{}", system.job)),
            r#"{"o:status":"stopped"}"#,
        ),
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    assert_eq!(
        updated.body, before.body,
        "the empty hydrate wrote a column"
    );

    let read = send(
        app,
        get(&keyed(&format!("/api/jobs/{}", system.job)), false),
    )
    .await;
    assert_eq!(read.body, before.body);
}

#[tokio::test]
async fn delete_a_job_then_read_404() {
    let (pool, seed) = seeded_pool().await;
    let system = seed_system(&pool, &seed).await;
    let app = common::router(pool);
    let deleted = send(
        app.clone(),
        request("DELETE", &keyed(&format!("/api/jobs/{}", system.job))),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.body);
    assert_eq!(deleted.json()["o:status"], "completed");

    let read = send(
        app,
        get(&keyed(&format!("/api/jobs/{}", system.job)), false),
    )
    .await;
    assert_eq!(read.status, StatusCode::NOT_FOUND);
    assert_eq!(
        read.body,
        format!(
            r#"{{"errors":{{"error":"Omeka\\Entity\\Job entity with criteria {{\"id\":\"{}\"}} not found"}}}}"#,
            system.job
        )
    );
}

/// The two creates the port leaves unbuilt: an asset needs a multipart file the
/// boundary cannot carry, and a job's required `class` comes from the
/// dispatcher, not a body.
#[tokio::test]
async fn the_unbuilt_creates_stay_501() {
    let (pool, seed) = seeded_pool().await;
    seed_system(&pool, &seed).await;
    let app = common::router(pool);
    for (uri, adapter) in [
        ("/api/assets", "assets"),
        ("/api/assets/1", "assets"),
        ("/api/jobs", "jobs"),
        ("/api/jobs/1", "jobs"),
    ] {
        let answer = send(app.clone(), json_request("POST", &keyed(uri), "{}")).await;
        assert_eq!(answer.status, StatusCode::NOT_IMPLEMENTED, "{uri}");
        assert_eq!(
            answer.body,
            format!(
                r#"{{"errors":{{"error":"The {adapter} adapter does not implement the create operation."}}}}"#
            ),
            "{uri}"
        );
    }
}

/// An anonymous update and delete reach the adapter grant and are refused:
/// `AclFactory` gives no role below the admin roles a write on either adapter.
#[tokio::test]
async fn an_anonymous_asset_write_is_403() {
    let (pool, seed) = seeded_pool().await;
    let system = seed_system(&pool, &seed).await;
    let app = common::router(pool);
    let update = send(
        app.clone(),
        json_request(
            "PUT",
            &format!("/api/assets/{}", system.asset),
            r#"{"o:name":"x"}"#,
        ),
    )
    .await;
    assert_eq!(update.status, StatusCode::FORBIDDEN);
    assert_eq!(
        update.body,
        r#"{"errors":{"error":"Permission denied for the current user to update the Omeka\\Api\\Adapter\\AssetAdapter resource."}}"#
    );
    let delete = send(
        app,
        request("DELETE", &format!("/api/assets/{}", system.asset)),
    )
    .await;
    assert_eq!(delete.status, StatusCode::FORBIDDEN);
    assert_eq!(
        delete.body,
        r#"{"errors":{"error":"Permission denied for the current user to delete the Omeka\\Api\\Adapter\\AssetAdapter resource."}}"#
    );
}
