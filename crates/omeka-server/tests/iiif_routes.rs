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
use omeka_media::store::Store;
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
