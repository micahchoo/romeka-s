//! Write-endpoint tests for `assets` and `jobs`, and the two creates the port
//! leaves unbuilt.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{
    append_key, get, json_request, multipart_asset_request, multipart_data_request,
    multipart_file_request, request, router_with_store, seed_system, seeded_pool, send,
};
use omeka_media::store::{FileStore, Store, StorePrefix, storage_path};

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
    let path = storage_path(StorePrefix::Asset, ASSET_STORAGE_ID, Some("png"));
    store
        .put(b"png bytes", &path, Some("image/png"))
        .expect("put asset");

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
    let asset = deleted.json();
    assert!(
        asset["o:id"].is_null(),
        "the detached asset kept its id: {}",
        deleted.body
    );
    assert_eq!(asset["@id"], "http://localhost/api/assets");
    assert_eq!(asset["o:name"], before.json()["o:name"]);
    assert!(
        !store.exists(&path).unwrap(),
        "the asset file survived at {path}"
    );
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

/// A job's required `class` comes from the dispatcher, not a body, so
/// `POST /api/jobs` stays unbuilt.
#[tokio::test]
async fn the_unbuilt_job_create_stays_501() {
    let (pool, seed) = seeded_pool().await;
    seed_system(&pool, &seed).await;
    let app = common::router(pool);
    for uri in ["/api/jobs", "/api/jobs/1"] {
        let answer = send(app.clone(), json_request("POST", &keyed(uri), "{}")).await;
        assert_eq!(answer.status, StatusCode::NOT_IMPLEMENTED, "{uri}");
        assert_eq!(
            answer.body,
            r#"{"errors":{"error":"The jobs adapter does not implement the create operation."}}"#,
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

/// A PNG signature, the magic bytes `finfo` and `image::guess_format` both read
/// as `image/png`.
const PNG_BYTES: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01";

/// `AssetAdapter::hydrate` on create stores the uploaded file under the `asset`
/// prefix, names the row from `o:name`, and answers the stored representation.
#[tokio::test]
async fn create_an_asset_from_a_multipart_upload() {
    let (pool, seed) = seeded_pool().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), omeka_server::FILES_URI);
    let app = router_with_store(pool, store.clone());

    let answer = send(
        app.clone(),
        multipart_asset_request(
            &keyed("/api/assets"),
            "logo.png",
            PNG_BYTES,
            r#"{"o:name":"The logo","o:alt_text":"A round logo"}"#,
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let document = answer.json();
    assert_eq!(document["o:name"], "The logo");
    assert_eq!(document["o:alt_text"], "A round logo");
    assert_eq!(document["o:media_type"], "image/png");
    assert_eq!(document["o:owner"]["o:id"], seed.user.0);
    let filename = document["o:filename"].as_str().expect("filename");
    assert!(filename.ends_with(".png"), "{filename}");
    assert_eq!(
        store.read(&format!("asset/{filename}")).expect("stored"),
        PNG_BYTES,
        "the upload did not land at asset/{filename}"
    );

    let id = document["o:id"].as_u64().expect("id");
    let read = send(app, get(&format!("/api/assets/{id}"), false)).await;
    assert_eq!(read.body, answer.body, "create and read differ");
}

/// `o:name` absent falls back to the client file name; `o:alt_text` absent is
/// null. Both are `AssetAdapter::hydrate` on create.
#[tokio::test]
async fn create_names_an_asset_from_the_client_file() {
    let (pool, _seed) = seeded_pool().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), omeka_server::FILES_URI);
    let app = router_with_store(pool, store);
    let answer = send(
        app,
        multipart_asset_request(&keyed("/api/assets"), "photo.png", PNG_BYTES, "{}"),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let document = answer.json();
    assert_eq!(document["o:name"], "photo.png");
    assert!(document["o:alt_text"].is_null(), "{}", answer.body);
}

/// A create with no `file` field answers the upstream 422, whether the body is
/// JSON or a multipart body whose only field is `data`.
#[tokio::test]
async fn create_an_asset_without_a_file_is_422() {
    let (pool, _seed) = seeded_pool().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), omeka_server::FILES_URI);
    let app = router_with_store(pool, store);

    let json = send(
        app.clone(),
        json_request("POST", &keyed("/api/assets"), "{}"),
    )
    .await;
    assert_eq!(json.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json.body, r#"{"errors":{"file":["No file was uploaded"]}}"#);

    let multipart = send(app, multipart_data_request(&keyed("/api/assets"), "{}")).await;
    assert_eq!(multipart.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        multipart.body,
        r#"{"errors":{"file":["No file was uploaded"]}}"#
    );
}

/// A multipart body with no `data` field decodes as invalid JSON, the upstream
/// 400 (`jsonDecode(null)`).
#[tokio::test]
async fn create_without_a_data_field_is_400() {
    let (pool, _seed) = seeded_pool().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), omeka_server::FILES_URI);
    let app = router_with_store(pool, store);
    let answer = send(
        app,
        multipart_file_request(&keyed("/api/assets"), "logo.png", PNG_BYTES),
    )
    .await;
    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert_eq!(answer.body, r#"{"errors":{"error":"JSON: Syntax error"}}"#);
}

/// `Omeka\File\Validator` reports the media type then the resolved extension,
/// both under `file`, for a file outside both whitelists.
#[tokio::test]
async fn create_rejects_a_non_whitelisted_upload() {
    let (pool, _seed) = seeded_pool().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), omeka_server::FILES_URI);
    let app = router_with_store(pool, store);
    let answer = send(
        app,
        multipart_asset_request(&keyed("/api/assets"), "note.txt", b"hello text\n", "{}"),
    )
    .await;
    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        answer.body,
        r#"{"errors":{"file":["Error validating \"note.txt\". Cannot store files with the media type \"text/plain\".","Error validating \"note.txt\". Cannot store files with the resolved extension \"txt\"."]}}"#
    );
}

/// `AssetAdapter::validateEntity` refuses a create whose `o:name` is empty.
#[tokio::test]
async fn create_with_an_empty_name_is_422() {
    let (pool, _seed) = seeded_pool().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), omeka_server::FILES_URI);
    let app = router_with_store(pool, store);
    let answer = send(
        app,
        multipart_asset_request(
            &keyed("/api/assets"),
            "logo.png",
            PNG_BYTES,
            r#"{"o:name":""}"#,
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        answer.body,
        r#"{"errors":{"o:name":["An asset must have a name."]}}"#
    );
}
