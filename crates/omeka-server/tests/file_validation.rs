//! The file validation settings end to end: `GET` and `PATCH /api/settings`
//! carry them, and a media upload is checked against them.
//!
//! Upstream's `Omeka\File\Validator` reads `media_type_whitelist`,
//! `extension_whitelist` and `disable_file_validation`; a fresh install stores
//! `SettingForm`'s two default lists.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::StatusCode;
use omeka_media::store::FileStore;
use omeka_media::validation::{DEFAULT_EXTENSIONS, DEFAULT_MEDIA_TYPES};
use serde_json::{Value, json};

use common::{
    Answer, Seed, append_key, get, json_request, multipart_asset_request, router_with_store,
    seeded_pool, send,
};

async fn app(dir: &std::path::Path) -> (Router, Seed) {
    let (pool, seed) = seeded_pool().await;
    (router_with_store(pool, FileStore::new(dir, "/files")), seed)
}

async fn patch_settings(app: &Router, body: &Value) -> Answer {
    send(
        app.clone(),
        json_request("PATCH", &append_key("/api/settings"), &body.to_string()),
    )
    .await
}

async fn upload(app: &Router, seed: &Seed, name: &str, bytes: &[u8]) -> Answer {
    let data = json!({"o:ingester": "upload", "file_index": 0, "o:item": {"o:id": seed.item.0}});
    send(
        app.clone(),
        multipart_asset_request(&append_key("/api/media"), name, bytes, &data.to_string()),
    )
    .await
}

/// With no rows stored, the settings answer upstream's defaults and
/// validation on.
#[tokio::test]
async fn the_settings_answer_upstream_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let (app, _seed) = app(dir.path()).await;
    let answer = send(app, get("/api/settings", true)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let settings = answer.json();
    assert_eq!(settings["media_type_whitelist"], json!(DEFAULT_MEDIA_TYPES));
    assert_eq!(settings["extension_whitelist"], json!(DEFAULT_EXTENSIONS));
    assert_eq!(settings["disable_file_validation"], json!(false));
}

/// `Validator::validate` adds its message under `file`: a `Thumbs.db` is
/// refused by its extension, and an executable by its media type.
#[tokio::test]
async fn an_upload_outside_the_lists_is_refused_under_file() {
    let dir = tempfile::tempdir().unwrap();
    let (app, seed) = app(dir.path()).await;

    let thumbs = upload(&app, &seed, "Thumbs.db", b"thumbnail cache").await;
    assert_eq!(
        thumbs.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        thumbs.body
    );
    assert_eq!(
        thumbs.json(),
        json!({"errors": {"file": [
            "Error validating \"Thumbs.db\". Cannot store files with the resolved extension \"db\"."
        ]}})
    );

    let mut exe = vec![0; 0x80];
    exe[..2].copy_from_slice(b"MZ");
    exe[0x3C] = 0x40;
    exe[0x40..0x46].copy_from_slice(b"PE\x00\x00\x4c\x01");
    let setup = upload(&app, &seed, "setup.exe", &exe).await;
    assert_eq!(
        setup.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        setup.body
    );
    assert_eq!(
        setup.json(),
        json!({"errors": {"file": [
            "Error validating \"setup.exe\". Cannot store files with the media type \"application/x-dosexec\"."
        ]}})
    );

    let pdf = upload(&app, &seed, "report.pdf", b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n").await;
    assert_eq!(pdf.status, StatusCode::OK, "{}", pdf.body);
    assert_eq!(pdf.json()["o:media_type"], json!("application/pdf"));
}

/// An edited list applies to the next upload, `disable_file_validation`
/// lets anything through, and `null` restores the default list.
#[tokio::test]
async fn the_settings_govern_the_next_upload() {
    let dir = tempfile::tempdir().unwrap();
    let (app, seed) = app(dir.path()).await;

    let narrowed =
        patch_settings(&app, &json!({"extension_whitelist": [" pdf ", "pdf", ""]})).await;
    assert_eq!(narrowed.status, StatusCode::OK, "{}", narrowed.body);
    assert_eq!(narrowed.json()["extension_whitelist"], json!(["pdf"]));
    let text = upload(&app, &seed, "notes.txt", b"plain notes\n").await;
    assert_eq!(
        text.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        text.body
    );

    let disabled = patch_settings(&app, &json!({"disable_file_validation": true})).await;
    assert_eq!(disabled.json()["disable_file_validation"], json!(true));
    let thumbs = upload(&app, &seed, "Thumbs.db", b"thumbnail cache").await;
    assert_eq!(thumbs.status, StatusCode::OK, "{}", thumbs.body);

    let restored = patch_settings(
        &app,
        &json!({"extension_whitelist": null, "disable_file_validation": false}),
    )
    .await;
    assert_eq!(
        restored.json()["extension_whitelist"],
        json!(DEFAULT_EXTENSIONS)
    );
    let text = upload(&app, &seed, "notes.txt", b"plain notes\n").await;
    assert_eq!(text.status, StatusCode::OK, "{}", text.body);
}
