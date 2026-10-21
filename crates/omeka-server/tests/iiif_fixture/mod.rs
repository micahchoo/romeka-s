//! The IIIF route fixture: a seeded database over a tempdir store, with items
//! whose image media were uploaded through the API, so their originals and
//! derivatives are real files of known sizes.

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::io::Write;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use omeka_db::repos::{PropertyRepo, UserRepo};
use omeka_db::rows::{NewProperty, NewUser};
use omeka_media::store::FileStore;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::common::{append_key, multipart_asset_request, router_with_store, seeded_pool, send};

/// The uploaded image's size. Its "large" derivative fits 800 pixels.
pub const WIDTH: u32 = 1200;
pub const HEIGHT: u32 = 900;
pub const LARGE: (u32, u32) = (800, 600);

pub const CURATOR_EMAIL: &str = "iiif-curator@example.com";
pub const CURATOR_PASSWORD: &str = "iiif-curator-pass-1";

/// The rows the IIIF tests read.
pub struct IiifSeed {
    pub app: Router,
    pub store: FileStore,
    /// The seeded database, for a test that writes a setting or another row.
    pub pool: omeka_db::DbPool,
    /// A public item with one public image, a description and a rights URI.
    pub item: u64,
    pub media: u64,
    /// A private item with one image, owned by the seeded admin.
    pub private_item: u64,
    pub private_media: u64,
    /// A public item with no media.
    pub bare_item: u64,
    /// A public item set holding the three items.
    pub item_set: u64,
    /// The seeded site's slug.
    pub site: String,
    /// The tempdir the store writes under; kept alive with the seed.
    pub dir: tempfile::TempDir,
}

/// An answer whose body may be binary.
pub struct Bytes {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Bytes {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }

    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).expect("a JSON body")
    }
}

pub async fn send_bytes(app: Router, request: Request<Body>) -> Bytes {
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    Bytes {
        status,
        headers,
        body: body.to_vec(),
    }
}

pub fn get(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

pub fn get_with_cookie(uri: &str, cookie: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header("cookie", cookie)
        .body(Body::empty())
        .unwrap()
}

/// The width and height an image's header names.
pub fn image_size(bytes: &[u8]) -> (u32, u32) {
    let tags = omeka_media::image_metadata(&mut std::io::Cursor::new(bytes));
    (
        tags["Width"].values[0].parse().expect("a width"),
        tags["Height"].values[0].parse().expect("a height"),
    )
}

/// A PNG of `width` by `height` in two colours, so a crop is visible.
pub fn png(width: u32, height: u32) -> Vec<u8> {
    let mut raw = Vec::with_capacity(((width * 3 + 1) * height) as usize);
    for y in 0..height {
        raw.push(0);
        for x in 0..width {
            let left = x < width / 2;
            raw.extend_from_slice(if left { &[200, 40, 40] } else { &[40, 40, 200] });
            let _ = y;
        }
    }
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(&raw).unwrap();
    let data = encoder.finish().unwrap();
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 2, 0, 0, 0]);
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    for (kind, body) in [
        (&b"IHDR"[..], header),
        (b"IDAT", data),
        (b"IEND", Vec::new()),
    ] {
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        let mut crc = flate2::Crc::new();
        crc.update(kind);
        crc.update(&body);
        out.extend_from_slice(kind);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc.sum().to_be_bytes());
    }
    out
}

/// Seed the IIIF scenario.
pub async fn iiif_seed() -> IiifSeed {
    let (pool, seed) = seeded_pool().await;
    let property = |local_name: &str, label: &str| NewProperty {
        owner_id: None,
        vocabulary_id: seed.vocabulary,
        local_name: local_name.to_string(),
        label: label.to_string(),
        comment: None,
    };
    let description = PropertyRepo::new(&pool)
        .create(&property("description", "Description"))
        .await
        .unwrap();
    let rights = PropertyRepo::new(&pool)
        .create(&property("rights", "Rights"))
        .await
        .unwrap();
    UserRepo::new(&pool)
        .create(&NewUser {
            email: CURATOR_EMAIL.to_string(),
            name: "IIIF Curator".to_string(),
            password_hash: Some(bcrypt::hash(CURATOR_PASSWORD, 4).unwrap()),
            role: "global_admin".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let store = FileStore::new(dir.path(), "/files");
    let pool = omeka_db::DbPool::from(pool);
    let app = router_with_store(pool.clone(), store.clone());
    let title_id = send(
        app.clone(),
        crate::common::get("/api/properties?term=dcterms:title", false),
    )
    .await
    .json()[0]["o:id"]
        .as_u64()
        .expect("the seeded title property");

    let item_set = send(
        app.clone(),
        crate::common::json_request(
            "POST",
            &append_key("/api/item_sets"),
            &json!({
                "o:is_public": true,
                "dcterms:title": [{"type": "literal", "property_id": title_id, "@value": "Harbour charts"}],
            })
            .to_string(),
        ),
    )
    .await
    .json()["o:id"]
        .as_u64()
        .expect("an item set");

    let upload = |public: bool, title: &str, extra: Value| {
        let mut data = json!({
            "o:is_public": public,
            "o:item_set": [{"o:id": item_set}],
            "dcterms:title": [{"type": "literal", "property_id": title_id, "@value": title}],
            "o:media": [{
                "o:ingester": "upload",
                "file_index": 0,
                "o:is_public": public,
                "dcterms:title": [{"type": "literal", "property_id": title_id, "@value": format!("{title}, recto")}],
            }],
        });
        if let (Some(target), Some(extra)) = (data.as_object_mut(), extra.as_object()) {
            target.extend(extra.clone());
        }
        multipart_asset_request(
            &append_key("/api/items"),
            "chart.png",
            &png(WIDTH, HEIGHT),
            &data.to_string(),
        )
    };
    let public = send(
        app.clone(),
        upload(
            true,
            "Harbour chart",
            json!({
                "dcterms:description": [{"type": "literal", "property_id": description.id.0, "@value": "A chart of the harbour."}],
                "dcterms:rights": [{"type": "uri", "property_id": rights.id.0, "@id": "http://creativecommons.org/licenses/by/4.0/"}],
            }),
        ),
    )
    .await;
    assert_eq!(public.status, StatusCode::OK, "{}", public.body);
    let private = send(app.clone(), upload(false, "Draft chart", json!({}))).await;
    assert_eq!(private.status, StatusCode::OK, "{}", private.body);
    // The creates deferred every derivative and the IIIF routes read them, so
    // settle the queue as the worker would before the fixture is read.
    crate::common::settle_derivatives(pool.clone(), std::sync::Arc::new(store.clone())).await;
    let bare = send(
        app.clone(),
        crate::common::json_request(
            "POST",
            &append_key("/api/items"),
            &json!({
                "o:is_public": true,
                "o:item_set": [{"o:id": item_set}],
                "dcterms:title": [{"type": "literal", "property_id": title_id, "@value": "Chart notes"}],
            })
            .to_string(),
        ),
    )
    .await;
    let media_of = |answer: &crate::common::Answer| {
        answer.json()["o:media"][0]["o:id"]
            .as_u64()
            .expect("a media")
    };
    IiifSeed {
        item: public.json()["o:id"].as_u64().unwrap(),
        media: media_of(&public),
        private_item: private.json()["o:id"].as_u64().unwrap(),
        private_media: media_of(&private),
        bare_item: bare.json()["o:id"].as_u64().unwrap(),
        item_set,
        site: "museum".to_string(),
        app,
        store,
        pool,
        dir,
    }
}

/// Every check the Presentation 3 shape asks of the documents this server
/// writes: each resource has its type's required properties, every `id` is an
/// absolute URL, and labels are language maps.
pub fn assert_presentation_3(document: &Value) {
    assert_eq!(
        document["@context"], "http://iiif.io/api/presentation/3/context.json",
        "{document}"
    );
    walk(document);
}

fn walk(node: &Value) {
    match node {
        Value::Array(items) => items.iter().for_each(walk),
        Value::Object(object) => {
            if let Some(kind) = object.get("type").and_then(Value::as_str) {
                let required: &[&str] = match kind {
                    "Manifest" | "Collection" => &["id", "label"],
                    "Canvas" => &["id", "width", "height", "items"],
                    "AnnotationPage" => &["id", "items"],
                    "Annotation" => &["id", "motivation", "body", "target"],
                    "Image" | "ImageService3" | "Dataset" | "Sound" | "Video" | "Text"
                    | "SearchService2" => &["id"],
                    other => panic!("an unknown type {other}: {node}"),
                };
                for property in required {
                    assert!(
                        object.contains_key(*property),
                        "{kind} without {property}: {node}"
                    );
                }
                let id = object["id"].as_str().expect("an id string");
                assert!(
                    id.starts_with("http://") || id.starts_with("https://"),
                    "{id} is not absolute"
                );
                if kind == "Canvas" {
                    assert!(object["width"].as_u64().is_some_and(|w| w > 0), "{node}");
                    assert!(object["height"].as_u64().is_some_and(|h| h > 0), "{node}");
                }
            }
            for key in ["label", "summary"] {
                if let Some(label) = object.get(key) {
                    let map = label.as_object().expect("a language map");
                    assert!(
                        map.values().all(Value::is_array),
                        "{key} is not a language map: {label}"
                    );
                }
            }
            object.values().for_each(walk);
        }
        _ => {}
    }
}
