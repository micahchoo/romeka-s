//! The IIIF routes: the viewer page, the item manifest, the item set
//! collection, and the item page links into them.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod iiif_fixture;

use axum::http::StatusCode;
use iiif_fixture::{
    CURATOR_EMAIL, CURATOR_PASSWORD, HEIGHT, LARGE, WIDTH, assert_presentation_3, get,
    get_with_cookie, iiif_seed, send_bytes,
};
use omeka_db::ids::MediaId;
use omeka_db::repos::{ExtentRow, MediaExtentRepo};
use omeka_media::store::{FileStore, Store};
use omeka_media::tool::Tools;
use serde_json::{Value, json};

/// The configuration a viewer page carries in its data attribute.
fn page_config(page: &str) -> Value {
    let start = page.find("data-mirador-config=\"").expect("the attribute") + 21;
    let end = start + page[start..].find('"').expect("the attribute end");
    let json = page[start..end]
        .replace("&quot;", "\"")
        .replace("&amp;", "&");
    serde_json::from_str(&json).expect("the configuration")
}

/// `IiifViewerController::indexAction`: a bare page that opens the `url` in
/// Mirador, loads it from this origin, and lets no inline script run.
#[tokio::test]
async fn the_viewer_page_opens_the_url_under_a_strict_policy() {
    let seed = iiif_seed().await;
    let answer = send_bytes(
        seed.app.clone(),
        get("/iiif-viewer?url=https%3A%2F%2Fexample.org%2Fmanifest.json"),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(
        answer.header("content-type"),
        Some("text/html; charset=utf-8")
    );
    let policy = answer.header("content-security-policy").expect("a policy");
    for directive in [
        "default-src 'none'",
        "script-src 'self';",
        "connect-src 'self' https:",
        "img-src 'self' https: data: blob:",
        "frame-ancestors 'self'",
    ] {
        assert!(
            policy.contains(directive),
            "{directive} missing from {policy}"
        );
    }
    let page = String::from_utf8(answer.body).unwrap();
    let config = page_config(&page);
    assert_eq!(
        config["windows"][0]["manifestId"],
        "https://example.org/manifest.json"
    );
    assert_eq!(config["window"]["sideBarOpen"], true);
    let mirador = page
        .split("<script src=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("the Mirador script");
    let library = send_bytes(seed.app, get(mirador)).await;
    assert_eq!(library.status, StatusCode::OK, "{mirador}");
    assert_eq!(
        library.header("content-type"),
        Some("application/javascript")
    );
    assert!(
        library.body.len() > 1_000_000,
        "the Mirador bundle is served whole"
    );
}

/// The allow-list: `window.sideBarOpen` and `selectedTheme` apply, and every
/// other key of `mirador_config` is dropped.
#[tokio::test]
async fn the_viewer_page_takes_only_the_allowed_keys() {
    let seed = iiif_seed().await;
    let user = json!({
        "window.sideBarOpen": false,
        "selectedTheme": "dark",
        "window": {"allowClose": true},
        "requests": {"preprocessors": ["steal"]},
        "id": "elsewhere",
    });
    let uri = format!(
        "/iiif-viewer?url=https%3A%2F%2Fexample.org%2Fm&mirador_config={}",
        urlencoding(&user.to_string())
    );
    let answer = send_bytes(seed.app, get(&uri)).await;
    let config = page_config(&String::from_utf8(answer.body).unwrap());
    assert_eq!(config["window"]["sideBarOpen"], false);
    assert_eq!(config["selectedTheme"], "dark");
    assert_eq!(config["window"]["allowClose"], false);
    assert_eq!(config["id"], "viewer");
    assert!(config.get("requests").is_none(), "{config}");
}

/// OpenSeadragon and its button images are served from the vendored files.
#[tokio::test]
async fn openseadragon_and_its_images_are_served() {
    let seed = iiif_seed().await;
    let script = send_bytes(
        seed.app.clone(),
        get("/assets/vendor/openseadragon/openseadragon.min.js"),
    )
    .await;
    assert_eq!(script.status, StatusCode::OK);
    assert_eq!(
        script.header("content-type"),
        Some("application/javascript")
    );
    let image = send_bytes(
        seed.app,
        get("/assets/vendor/openseadragon/images/zoomin_rest.png"),
    )
    .await;
    assert_eq!(image.status, StatusCode::OK);
    assert_eq!(image.header("content-type"), Some("image/png"));
    assert!(image.body.starts_with(b"\x89PNG"));
}

/// A public item's manifest: Presentation 3, one canvas per image at the
/// original's size painting the "large" derivative at its own size, the Image
/// API service, the original as `rendering`, and label, summary, rights and
/// metadata from the item's values.
#[tokio::test]
async fn a_public_item_has_a_presentation_3_manifest() {
    let seed = iiif_seed().await;
    let uri = format!("/iiif/3/item/{}/manifest", seed.item);
    let answer = send_bytes(seed.app, get(&uri)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(
        answer.header("content-type"),
        Some("application/ld+json;profile=\"http://iiif.io/api/presentation/3/context.json\"")
    );
    assert_eq!(answer.header("access-control-allow-origin"), Some("*"));
    let manifest = answer.json();
    assert_presentation_3(&manifest);
    assert_eq!(manifest["id"], format!("http://localhost{uri}"));
    assert_eq!(manifest["type"], "Manifest");
    assert_eq!(manifest["label"], json!({"none": ["Harbour chart"]}));
    assert_eq!(
        manifest["summary"],
        json!({"none": ["A chart of the harbour."]})
    );
    assert_eq!(
        manifest["rights"],
        "http://creativecommons.org/licenses/by/4.0/"
    );
    let labels: Vec<&Value> = manifest["metadata"]
        .as_array()
        .expect("metadata")
        .iter()
        .map(|entry| &entry["label"]["none"][0])
        .collect();
    assert_eq!(
        labels,
        [&json!("Title"), &json!("Description"), &json!("Rights")]
    );

    let canvases = manifest["items"].as_array().expect("canvases");
    assert_eq!(canvases.len(), 1);
    let canvas = &canvases[0];
    assert_eq!(canvas["label"], json!({"none": ["Harbour chart, recto"]}));
    assert_eq!(
        (canvas["width"].as_u64(), canvas["height"].as_u64()),
        (Some(u64::from(WIDTH)), Some(u64::from(HEIGHT)))
    );
    let annotation = &canvas["items"][0]["items"][0];
    assert_eq!(annotation["motivation"], "painting");
    assert_eq!(annotation["target"], canvas["id"]);
    let body = &annotation["body"];
    assert!(
        body["id"].as_str().unwrap().contains("/files/large/"),
        "{body}"
    );
    assert_eq!(
        (body["width"].as_u64(), body["height"].as_u64()),
        (Some(u64::from(LARGE.0)), Some(u64::from(LARGE.1)))
    );
    assert_eq!(
        body["service"][0],
        json!({"id": format!("http://localhost/iiif/image/3/{}", seed.media), "type": "ImageService3", "profile": "level1"})
    );
    assert!(
        canvas["rendering"][0]["id"]
            .as_str()
            .unwrap()
            .contains("/files/original/")
    );
    assert_eq!(canvas["rendering"][0]["format"], "image/png");
    assert!(
        manifest["thumbnail"][0]["id"]
            .as_str()
            .unwrap()
            .contains("/files/square/")
    );
}

/// The sizes are read once and remembered: with the files gone, a second
/// manifest still knows them.
#[tokio::test]
async fn image_sizes_are_read_once() {
    let seed = iiif_seed().await;
    let uri = format!("/iiif/3/item/{}/manifest", seed.item);
    let first = send_bytes(seed.app.clone(), get(&uri)).await.json();
    for prefix in ["original", "large"] {
        for file in seed.store.list(prefix).unwrap() {
            seed.store.delete(&file.storage_path).unwrap();
        }
    }
    let second = send_bytes(seed.app, get(&uri)).await.json();
    assert_eq!(second["items"][0]["width"], first["items"][0]["width"]);
    assert_eq!(second["items"][0]["items"], first["items"][0]["items"]);
}

/// A private item answers 404 to a visitor, as its `/s/` page does, and its
/// manifest to a signed-in curator. An item with no image has no canvas.
#[tokio::test]
async fn manifests_follow_the_public_page_visibility() {
    let seed = iiif_seed().await;
    let private = format!("/iiif/3/item/{}/manifest", seed.private_item);
    assert_eq!(
        send_bytes(seed.app.clone(), get(&private)).await.status,
        StatusCode::NOT_FOUND
    );
    let page = format!("/s/{}/item/{}", seed.site, seed.private_item);
    assert_eq!(
        send_bytes(seed.app.clone(), get(&page)).await.status,
        StatusCode::NOT_FOUND
    );
    let missing = send_bytes(seed.app.clone(), get("/iiif/3/item/9999/manifest")).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);

    let cookie = common::login_cookie(seed.app.clone(), CURATOR_EMAIL, CURATOR_PASSWORD).await;
    let signed_in = send_bytes(seed.app.clone(), get_with_cookie(&private, &cookie)).await;
    assert_eq!(signed_in.status, StatusCode::OK);
    assert_eq!(signed_in.json()["items"].as_array().map(Vec::len), Some(1));

    let bare = send_bytes(
        seed.app,
        get(&format!("/iiif/3/item/{}/manifest", seed.bare_item)),
    )
    .await;
    assert_eq!(bare.json()["items"], json!([]));
}

/// An item set's collection lists a manifest for each item the caller may
/// see, and leaves the private one out.
#[tokio::test]
async fn an_item_set_collection_lists_the_visible_items() {
    let seed = iiif_seed().await;
    let uri = format!("/iiif/3/item-set/{}/collection", seed.item_set);
    let collection = send_bytes(seed.app.clone(), get(&uri)).await.json();
    assert_presentation_3(&collection);
    assert_eq!(collection["type"], "Collection");
    assert_eq!(collection["label"], json!({"none": ["Harbour charts"]}));
    let members: Vec<&str> = collection["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|member| member["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        members,
        [
            format!("http://localhost/iiif/3/item/{}/manifest", seed.item),
            format!("http://localhost/iiif/3/item/{}/manifest", seed.bare_item),
        ]
    );
    assert!(
        collection["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|member| member["type"] == "Manifest")
    );
    let missing = send_bytes(seed.app, get("/iiif/3/item-set/9999/collection")).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}

/// The item page of an item with an image links its manifest and opens it in
/// the viewer; an item with no image offers neither.
#[tokio::test]
async fn the_item_page_links_the_viewer_and_the_manifest() {
    let seed = iiif_seed().await;
    let page = send_bytes(
        seed.app.clone(),
        get(&format!("/s/{}/item/{}", seed.site, seed.item)),
    )
    .await;
    assert_eq!(page.status, StatusCode::OK);
    let page = String::from_utf8(page.body).unwrap();
    let manifest = format!("http://localhost/iiif/3/item/{}/manifest", seed.item);
    assert!(
        page.contains(&format!("<a class=\"iiif-manifest\" href=\"{manifest}\">")),
        "{page}"
    );
    assert!(page.contains("IIIF manifest</a>"), "{page}");
    assert!(
        page.contains(&format!(
            "<a href=\"/iiif-viewer?url={}&amp;mirador_config=",
            urlencoding(&manifest)
        )),
        "{page}"
    );
    assert!(page.contains(">View in the IIIF viewer</a>"), "{page}");
    let bare = send_bytes(
        seed.app,
        get(&format!("/s/{}/item/{}", seed.site, seed.bare_item)),
    )
    .await;
    assert!(!String::from_utf8(bare.body).unwrap().contains("iiif-links"));
}

fn urlencoding(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

/// A router over a tempdir store, with the pool a test writes an extent row
/// through.
struct Stored {
    app: axum::Router,
    pool: sqlx::SqlitePool,
    store: FileStore,
    /// The store's root, kept alive with the router.
    _dir: tempfile::TempDir,
}

async fn stored() -> Stored {
    let (pool, _seed) = common::seeded_pool().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let app = common::router_with_store(pool.clone(), store.clone());
    Stored {
        app,
        pool,
        store,
        _dir: dir,
    }
}

/// One file of the media-kind corpus, the run's fixture for every kind.
fn corpus(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/media-kinds")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("{name}: {error}"))
}

/// Upload one corpus file as a public item's file media, and answer the item's
/// id and the media representation.
async fn upload(app: &axum::Router, name: &str) -> (u64, Value) {
    let data = json!({
        "o:is_public": true,
        "dcterms:title": [{"type": "literal", "@value": name}],
        "o:media": [{"o:ingester": "upload", "o:is_public": true, "file_index": 0}],
    })
    .to_string();
    let answer = send_bytes(
        app.clone(),
        common::multipart_asset_request(
            &common::append_key("/api/items"),
            name,
            &corpus(name),
            &data,
        ),
    )
    .await;
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&answer.body)
    );
    let created = answer.json();
    let item = created["o:id"].as_u64().expect("an item");
    let media = created["o:media"][0]["o:id"].as_u64().expect("a media");
    let doc = send_bytes(
        app.clone(),
        common::get(&format!("/api/media/{media}"), true),
    )
    .await
    .json();
    (item, doc)
}

async fn manifest(app: &axum::Router, item: u64) -> Value {
    let answer = send_bytes(app.clone(), get(&format!("/iiif/3/item/{item}/manifest"))).await;
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&answer.body)
    );
    answer.json()
}

fn canvas(manifest: &Value, media: u64) -> &Value {
    manifest["items"]
        .as_array()
        .expect("canvases")
        .iter()
        .find(|canvas| {
            canvas["id"]
                .as_str()
                .is_some_and(|id| id.ends_with(&format!("/canvas/{media}")))
        })
        .unwrap_or_else(|| panic!("no canvas for media {media}: {manifest}"))
}

fn body(canvas: &Value) -> &Value {
    &canvas["items"][0]["items"][0]["body"]
}

/// The storage path of a media representation's original.
fn original(media: &Value) -> String {
    format!(
        "original/{}",
        media["o:filename"].as_str().expect("a filename")
    )
}

/// Record an extent the probe would have answered. A test seeds it so the
/// canvas is deterministic and no external command runs.
async fn record(
    stored: &Stored,
    media: u64,
    path: &str,
    duration_ms: u64,
    size: Option<(u32, u32)>,
) {
    let row = ExtentRow {
        duration_ms: Some(duration_ms),
        width: size.map(|size| size.0),
        height: size.map(|size| size.1),
    };
    MediaExtentRepo::new(&stored.pool)
        .put(MediaId(media), path, &row)
        .await
        .expect("record an extent");
}

/// An audio media's canvas is a `Sound` body with the duration the extent
/// table holds, on the strip a player lays out. The duration is not the file's
/// own, so the answer proves the row was read rather than probed.
#[tokio::test]
async fn an_audio_media_has_a_sound_canvas_with_the_duration_that_was_recorded() {
    let stored = stored().await;
    let (item, media) = upload(&stored.app, "audio.mp3").await;
    let media_id = media["o:id"].as_u64().expect("a media");
    record(&stored, media_id, &original(&media), 12_345, None).await;

    let manifest = manifest(&stored.app, item).await;
    assert_presentation_3(&manifest);
    let canvas = canvas(&manifest, media_id);
    assert_eq!(canvas["duration"], json!(12.345));
    assert_eq!(
        (canvas["width"].as_u64(), canvas["height"].as_u64()),
        (Some(600), Some(100))
    );
    assert!(canvas.get("thumbnail").is_none(), "{canvas}");
    let body = body(canvas);
    assert_eq!(body["type"], "Sound");
    assert_eq!(body["format"], "audio/mpeg");
    assert_eq!(body["duration"], json!(12.345));
    assert!(
        body["id"]
            .as_str()
            .expect("a body url")
            .contains("/files/original/"),
        "{body}"
    );
    assert!(canvas.get("rendering").is_none(), "{canvas}");
}

/// A video media's canvas is a `Video` body at the frame size the extent
/// holds, with the poster when one was made, and its duration.
#[tokio::test]
async fn a_video_media_has_a_video_canvas_at_its_frame_size() {
    let stored = stored().await;
    let (item, media) = upload(&stored.app, "video.mp4").await;
    let media_id = media["o:id"].as_u64().expect("a media");
    record(&stored, media_id, &original(&media), 7_500, Some((96, 64))).await;

    let manifest = manifest(&stored.app, item).await;
    assert_presentation_3(&manifest);
    let canvas = canvas(&manifest, media_id);
    assert_eq!(canvas["duration"], json!(7.5));
    assert_eq!(
        (canvas["width"].as_u64(), canvas["height"].as_u64()),
        (Some(96), Some(64))
    );
    let body = body(canvas);
    assert_eq!(body["type"], "Video");
    assert_eq!(body["format"], "video/mp4");
    assert_eq!(body["duration"], json!(7.5));
    assert_eq!(
        (body["width"].as_u64(), body["height"].as_u64()),
        (Some(96), Some(64))
    );
    assert!(
        canvas["thumbnail"][0]["id"]
            .as_str()
            .expect("a poster")
            .contains("/files/large/"),
        "{canvas}"
    );
}

/// An extent row for another path is stale: the manifest probes the original
/// it has instead of painting the old answer, and records the new one.
#[tokio::test]
async fn an_extent_row_for_another_path_is_probed_again() {
    let stored = stored().await;
    let (item, media) = upload(&stored.app, "audio.mp3").await;
    let media_id = media["o:id"].as_u64().expect("a media");
    record(&stored, media_id, "original/gone.mp3", 999_000, None).await;

    let manifest = manifest(&stored.app, item).await;
    let painted = manifest["items"]
        .as_array()
        .expect("canvases")
        .iter()
        .find(|canvas| {
            canvas["id"]
                .as_str()
                .is_some_and(|id| id.ends_with(&format!("/canvas/{media_id}")))
        })
        .and_then(|canvas| canvas["duration"].as_f64());
    assert_ne!(
        painted,
        Some(999.0),
        "a stale row is not painted: {manifest}"
    );
    if Tools::on_path().ffprobe().is_some() {
        assert_eq!(painted, Some(0.35), "{manifest}");
        let recorded = MediaExtentRepo::new(&stored.pool)
            .get(MediaId(media_id), &original(&media))
            .await
            .expect("the extent read")
            .expect("the probe was recorded");
        assert_eq!(recorded.duration_ms, Some(350));
    } else {
        assert_eq!(painted, None, "without ffprobe there is no duration");
    }
}

/// A media whose probe answers nothing gets no canvas, and the manifest still
/// answers: the same path a host without `ffprobe` takes.
#[tokio::test]
async fn a_media_with_no_probe_answer_gets_no_canvas() {
    let stored = stored().await;
    let (item, media) = upload(&stored.app, "audio.mp3").await;
    stored
        .store
        .delete(&original(&media))
        .expect("remove the original");

    let manifest = manifest(&stored.app, item).await;
    assert_eq!(manifest["items"], json!([]), "{manifest}");
    assert_presentation_3(&manifest);
}

/// image.jpg is Orientation 6: the canvas is the upright 80 by 120, and the
/// Image API's full image is the same portrait pixels.
#[tokio::test]
async fn an_exif_oriented_image_paints_upright_pixels() {
    let stored = stored().await;
    let (item, media) = upload(&stored.app, "image.jpg").await;
    let media_id = media["o:id"].as_u64().expect("a media");

    let manifest = manifest(&stored.app, item).await;
    let canvas = canvas(&manifest, media_id);
    assert_eq!(
        (canvas["width"].as_u64(), canvas["height"].as_u64()),
        (Some(80), Some(120))
    );

    let info = send_bytes(
        stored.app.clone(),
        get(&format!("/iiif/image/3/{media_id}/info.json")),
    )
    .await
    .json();
    assert_eq!(
        (info["width"].as_u64(), info["height"].as_u64()),
        (Some(80), Some(120))
    );

    let image = send_bytes(
        stored.app.clone(),
        get(&format!("/iiif/image/3/{media_id}/full/max/0/default.jpg")),
    )
    .await;
    assert_eq!(image.status, StatusCode::OK);
    assert_eq!(iiif_fixture::image_size(&image.body), (80, 120));
}

/// An SVG gets a canvas at the size the thumbnailer draws it, and its Image
/// API service answers that same size.
#[tokio::test]
async fn an_svg_gets_a_canvas_at_its_drawn_size() {
    let stored = stored().await;
    let (item, media) = upload(&stored.app, "image.svg").await;
    let media_id = media["o:id"].as_u64().expect("a media");

    let manifest = manifest(&stored.app, item).await;
    let canvas = canvas(&manifest, media_id);
    assert_eq!(
        (canvas["width"].as_u64(), canvas["height"].as_u64()),
        (Some(120), Some(80))
    );
    let body = body(canvas);
    assert_eq!(body["type"], "Image");
    assert_eq!(
        body["service"][0]["id"],
        format!("http://localhost/iiif/image/3/{media_id}")
    );

    let image = send_bytes(
        stored.app.clone(),
        get(&format!("/iiif/image/3/{media_id}/full/max/0/default.jpg")),
    )
    .await;
    assert_eq!(image.status, StatusCode::OK);
    assert_eq!(iiif_fixture::image_size(&image.body), (120, 80));
}

/// A PDF with a first-page derivative paints that page, at the derivative's
/// own size, and links the PDF itself.
#[tokio::test]
async fn a_pdf_paints_its_first_page() {
    // The first page is pdftoppm's, and a host without it makes no such
    // derivative, so there is no canvas to read.
    if Tools::on_path().pdftoppm().is_none() {
        return;
    }
    let stored = stored().await;
    let (item, media) = upload(&stored.app, "doc.pdf").await;
    let media_id = media["o:id"].as_u64().expect("a media");
    let page = media["o:thumbnail_urls"]["large"]
        .as_str()
        .expect("a first page")
        .split("/files/")
        .nth(1)
        .expect("a stored path")
        .to_string();
    let page_size = iiif_fixture::image_size(&stored.store.read(&page).expect("the page"));

    let manifest = manifest(&stored.app, item).await;
    let canvas = canvas(&manifest, media_id);
    assert_eq!(
        (
            canvas["width"].as_u64().map(|w| w as u32),
            canvas["height"].as_u64().map(|h| h as u32)
        ),
        (Some(page_size.0), Some(page_size.1))
    );
    let body = body(canvas);
    assert_eq!(body["type"], "Image");
    assert!(
        body["id"]
            .as_str()
            .expect("a body url")
            .contains("/files/large/"),
        "{body}"
    );
    assert!(
        canvas["rendering"][0]["id"]
            .as_str()
            .expect("the pdf")
            .contains("/files/original/"),
        "{canvas}"
    );
}
