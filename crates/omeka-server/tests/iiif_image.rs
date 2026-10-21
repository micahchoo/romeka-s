//! The IIIF Image API routes at level 1 over a stored image: `info.json`, the
//! sizes and regions, the refusals, the answer cache, and visibility.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod iiif_fixture;

use axum::http::StatusCode;
use iiif_fixture::{HEIGHT, WIDTH, get, iiif_seed, image_size, send_bytes};
use omeka_media::store::Store;

/// `info.json` names a level 1 `ImageService3` at its own URL, the original's
/// size and the limits.
#[tokio::test]
async fn info_json_describes_the_original() {
    let seed = iiif_seed().await;
    let uri = format!("/iiif/image/3/{}/info.json", seed.media);
    let answer = send_bytes(seed.app, get(&uri)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(
        answer.header("content-type"),
        Some("application/ld+json;profile=\"http://iiif.io/api/image/3/context.json\"")
    );
    assert_eq!(answer.header("access-control-allow-origin"), Some("*"));
    let info = answer.json();
    assert_eq!(info["@context"], "http://iiif.io/api/image/3/context.json");
    assert_eq!(
        info["id"],
        format!("http://localhost/iiif/image/3/{}", seed.media)
    );
    assert_eq!(info["type"], "ImageService3");
    assert_eq!(info["protocol"], "http://iiif.io/api/image");
    assert_eq!(info["profile"], "level1");
    assert_eq!(
        (info["width"].as_u64(), info["height"].as_u64()),
        (Some(u64::from(WIDTH)), Some(u64::from(HEIGHT)))
    );
}

/// Each region and size form answers an image of the size the specification
/// gives, in the requested format.
#[tokio::test]
async fn image_requests_answer_the_requested_size() {
    let seed = iiif_seed().await;
    for (path, media_type, size) in [
        ("full/max/0/default.jpg", "image/jpeg", (1200, 900)),
        ("full/600,/0/default.jpg", "image/jpeg", (600, 450)),
        ("full/,300/0/default.jpg", "image/jpeg", (400, 300)),
        ("full/300,100/0/default.jpg", "image/jpeg", (300, 100)),
        ("full/!300,300/0/default.jpg", "image/jpeg", (300, 225)),
        ("full/pct:25/0/default.jpg", "image/jpeg", (300, 225)),
        ("square/200,/0/default.jpg", "image/jpeg", (200, 200)),
        ("0,0,600,450/300,/0/default.png", "image/png", (300, 225)),
        (
            "1000,800,512,512/max/0/default.jpg",
            "image/jpeg",
            (200, 100),
        ),
    ] {
        let uri = format!("/iiif/image/3/{}/{path}", seed.media);
        let answer = send_bytes(seed.app.clone(), get(&uri)).await;
        assert_eq!(answer.status, StatusCode::OK, "{path}");
        assert_eq!(answer.header("content-type"), Some(media_type), "{path}");
        assert_eq!(image_size(&answer.body), size, "{path}");
    }
}

/// A request outside level 1, past the image or larger than the region is a
/// 400, and a media the caller may not see, or that is not a stored image, is
/// a 404.
#[tokio::test]
async fn image_requests_outside_the_service_are_refused() {
    let seed = iiif_seed().await;
    for path in [
        "full/1201,/0/default.jpg",
        "full/max/90/default.jpg",
        "full/max/0/gray.jpg",
        "full/max/0/default.gif",
        "1200,0,10,10/max/0/default.jpg",
        "pct:10,10,10,10/max/0/default.jpg",
    ] {
        let uri = format!("/iiif/image/3/{}/{path}", seed.media);
        assert_eq!(
            send_bytes(seed.app.clone(), get(&uri)).await.status,
            StatusCode::BAD_REQUEST,
            "{path}"
        );
    }
    for uri in [
        format!("/iiif/image/3/{}/info.json", seed.private_media),
        format!(
            "/iiif/image/3/{}/full/max/0/default.jpg",
            seed.private_media
        ),
        "/iiif/image/3/9999/info.json".to_string(),
    ] {
        assert_eq!(
            send_bytes(seed.app.clone(), get(&uri)).await.status,
            StatusCode::NOT_FOUND,
            "{uri}"
        );
    }
}

/// An answer is kept in the store under its request path, below the cache's
/// version segment, and a second request is served from there: with the
/// original gone, it still answers the same bytes.
#[tokio::test]
async fn a_second_request_is_served_from_the_cache() {
    let seed = iiif_seed().await;
    let uri = format!("/iiif/image/3/{}/full/600,/0/default.jpg", seed.media);
    let first = send_bytes(seed.app.clone(), get(&uri)).await;
    assert_eq!(first.status, StatusCode::OK);
    let media = common::send(
        seed.app.clone(),
        common::get(&format!("/api/media/{}", seed.media), true),
    )
    .await
    .json();
    let filename = media["o:filename"].as_str().expect("a stored file");
    let storage_id = filename.trim_end_matches(".png");
    assert!(
        seed.store
            .exists(&format!("iiif/v2/{storage_id}/full/600,/0/default.jpg"))
            .unwrap(),
        "the answer is kept under its request path"
    );
    seed.store.delete(&format!("original/{filename}")).unwrap();
    let second = send_bytes(seed.app, get(&uri)).await;
    assert_eq!(second.status, StatusCode::OK);
    assert_eq!(second.body, first.body);
}
