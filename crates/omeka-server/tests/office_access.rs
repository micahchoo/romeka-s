//! An office document's PDF access copy end to end: the derivative worker makes
//! it through LibreOffice, the media page's reader opens it, its first page is
//! the thumbnail and the IIIF canvas's still, and the `.docx` stays the
//! original that downloads.
//!
//! The server under test is the real router over a seeded site and a real file
//! store, and the copy is made by the worker's own queue, so nothing here stands
//! in for the conversion. Where LibreOffice or `pdftoppm` is not installed the
//! test states what it skipped: the copy is optional at runtime.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;

use common::{
    Seed, append_key, get, json_request, multipart_asset_request, router_with_store, seeded_pool,
    send, settle_derivatives,
};
use http_body_util::BodyExt;
use omeka_db::repos::MediaDerivativeRepo;
use omeka_db::rows::{DerivativeKind, DerivativeState};
use omeka_db::with_db;
use omeka_media::store::FileStore;
use omeka_media::tool::Tools;
use serde_json::{Value, json};
use tower::ServiceExt;

/// The seeded site the fixture item is attached to.
const SITE: &str = "/s/museum";

/// The document type the corpus `.docx` stores.
const DOCX: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

fn corpus(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/media-kinds")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

#[tokio::test(flavor = "multi_thread")]
async fn an_office_document_reads_its_pdf_copy_and_downloads_the_original() {
    if Tools::on_path().soffice().is_none() {
        eprintln!("skipping: no soffice on PATH");
        return;
    }
    if Tools::on_path().pdftoppm().is_none() {
        eprintln!("skipping: no pdftoppm on PATH, so the copy has no first page");
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let app = router_with_store(pool.clone(), store.clone());
    let (item, media) = upload_document(&app, &seed).await;
    let pool = omeka_db::DbPool::from(pool);

    // The create planned the copy and named the command it waits for.
    let planned = with_db!(&pool, |db| MediaDerivativeRepo::new(db)
        .get(omeka_db::ids::MediaId(media), DerivativeKind::AccessCopy)
        .await)
    .expect("the row")
    .expect("the access row");
    assert_eq!(planned.state, DerivativeState::Pending, "{planned:?}");
    assert_eq!(planned.tool.as_deref(), Some("soffice"));
    let pending = send(app.clone(), get(&format!("/api/media/{media}"), true))
        .await
        .json();
    assert_eq!(pending["o-module-romeka:access"]["state"], "pending");

    // The worker's queue makes the copy, and its first page is the thumbnail.
    settle_derivatives(pool.clone(), std::sync::Arc::new(store.clone())).await;
    let ready = with_db!(&pool, |db| MediaDerivativeRepo::new(db)
        .get(omeka_db::ids::MediaId(media), DerivativeKind::AccessCopy)
        .await)
    .expect("the row")
    .expect("the access row");
    assert_eq!(ready.state, DerivativeState::Ready, "{ready:?}");
    assert_eq!(ready.tool.as_deref(), Some("soffice"));
    let document = send(app.clone(), get(&format!("/api/media/{media}"), true))
        .await
        .json();
    let storage_id = stored_stem(&document);
    let copy = format!("access/{storage_id}.pdf");
    assert!(
        std::fs::read(dir.path().join(&copy)).is_ok_and(|bytes| bytes.starts_with(b"%PDF-")),
        "the copy is a PDF in the store"
    );
    assert_eq!(
        std::fs::read(dir.path().join(format!("original/{storage_id}.docx"))).expect("original"),
        corpus("doc.docx"),
        "the original is never changed"
    );
    for kind in omeka_media::file_render::ThumbnailType::ALL {
        let thumb = omeka_media::thumbnailer::thumbnail_path(&storage_id, kind);
        assert!(dir.path().join(&thumb).is_file(), "{thumb} was not written");
    }

    // The media document names the copy and keeps the document.
    let access = &document["o-module-romeka:access"];
    assert_eq!(access["state"], "ready", "{document}");
    assert_eq!(access["media_type"], "application/pdf", "{document}");
    let access_url = access["url"].as_str().expect("the copy's URL");
    assert!(access_url.ends_with(&copy), "{access_url}");
    let original_url = document["o:original_url"]
        .as_str()
        .expect("the original's URL");
    assert!(
        original_url.ends_with(&format!("original/{storage_id}.docx")),
        "{original_url}"
    );
    assert_eq!(document["o:kind"], "document", "{document}");

    // The file's own page opens the reader on the copy, and both cards link the
    // document the reader downloads.
    let page = send(app.clone(), get(&format!("{SITE}/media/{media}"), false))
        .await
        .body;
    assert!(
        page.contains(&format!(
            "<object class=\"pdf-object\" data=\"{access_url}\" type=\"application/pdf\""
        )),
        "the reader opens the copy: {page}"
    );
    assert!(
        page.contains(&format!("<a class=\"pdf-card\" href=\"{original_url}\">")),
        "the card links the document: {page}"
    );
    assert!(
        page.contains(&format!("href=\"{SITE}/media/{media}/download\"")),
        "the page still downloads the original: {page}"
    );

    // The download is the `.docx`, byte for byte, under its own name.
    let (status, headers, bytes) = download_bytes(
        app.clone(),
        get(&format!("{SITE}/media/{media}/download"), false),
    )
    .await;
    assert!(status.is_success(), "the download answers: {status}");
    assert_eq!(bytes, corpus("doc.docx"), "the download is the document");
    assert_eq!(
        headers
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some(DOCX),
        "the download keeps the document's type"
    );
    assert!(
        headers
            .get("content-disposition")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("field-notes.docx")),
        "the download keeps the document's name"
    );

    // The canvas paints the copy's first page and links the document through
    // `rendering`, so a viewer and a reader tell the same story.
    let manifest = send(
        app.clone(),
        get(&format!("/iiif/3/item/{item}/manifest"), false),
    )
    .await
    .json();
    let canvas = manifest["items"]
        .as_array()
        .and_then(|items| {
            items.iter().find(|canvas| {
                canvas["id"]
                    .as_str()
                    .is_some_and(|id| id.ends_with(&format!("/canvas/{media}")))
            })
        })
        .expect("the media's canvas");
    let body = &canvas["items"][0]["items"][0]["body"];
    assert_eq!(body["type"], "Image", "{canvas}");
    assert!(
        body["id"]
            .as_str()
            .is_some_and(|id| id.ends_with(&format!("large/{storage_id}.jpg"))),
        "the canvas paints the first page: {canvas}"
    );
    assert!(
        canvas["width"].as_u64().is_some_and(|width| width > 0),
        "{canvas}"
    );
    assert_eq!(
        canvas["rendering"][0]["id"].as_str(),
        Some(original_url),
        "the document is a rendering link: {canvas}"
    );
    assert_eq!(canvas["rendering"][0]["format"], DOCX, "{canvas}");
}

/// The stored name's stem, which every derivative is named after.
fn stored_stem(document: &Value) -> String {
    let filename = document["o:filename"]
        .as_str()
        .unwrap_or_else(|| panic!("{document}"));
    filename
        .rsplit_once('.')
        .map_or_else(|| filename.to_string(), |(stem, _)| stem.to_string())
}

/// The answer to a request, read as status, headers and bytes: a download's body
/// is the stored file, which a text read would not hold.
async fn download_bytes(
    app: axum::Router,
    request: axum::http::Request<axum::body::Body>,
) -> (axum::http::StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = app.oneshot(request).await.expect("the download answers");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("the download body")
        .to_bytes()
        .to_vec();
    (status, headers, bytes)
}

/// Upload the corpus `.docx` as an item's only media and attach the item to the
/// seeded site. Answers the item and the media.
async fn upload_document(app: &axum::Router, seed: &Seed) -> (u64, u64) {
    let data = json!({
        "o:is_public": true,
        "dcterms:title": [{"type": "literal", "@value": "Field notes"}],
        "o:media": [{
            "o:ingester": "upload",
            "o:is_public": true,
            "file_index": 0,
        }],
    })
    .to_string();
    let answer = send(
        app.clone(),
        multipart_asset_request(
            &append_key("/api/items"),
            "field-notes.docx",
            &corpus("doc.docx"),
            &data,
        ),
    )
    .await;
    assert!(answer.status.is_success(), "{}", answer.body);
    let created: Value = answer.json();
    let item = created["o:id"].as_u64().expect("the item");
    let media = created["o:media"][0]["o:id"].as_u64().expect("the media");
    let add = json!({"action": "add", "items": [item]}).to_string();
    let attached = send(
        app.clone(),
        json_request(
            "POST",
            &append_key(&format!("/api/sites/{}/items", seed.site)),
            &add,
        ),
    )
    .await;
    assert!(attached.status.is_success(), "{}", attached.body);
    (item, media)
}
