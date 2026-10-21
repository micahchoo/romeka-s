//! IIIF and the notes of a media: the canvas's `annotations` reference, the
//! media manifest, and the Content Search 2.0 service.
//!
//! Every expected value is read from the documents the routes answer, over a
//! seeded store whose image is a real file. A note is written through the WAP,
//! as an editor's browser writes it, so the reference a manifest publishes is
//! the one a viewer then dereferences.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod iiif_fixture;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{append_key, login_cookie};
use iiif_fixture::{CURATOR_EMAIL, CURATOR_PASSWORD, iiif_seed, send_bytes};
use omeka_api::settings::ANNOTATIONS_PUBLIC;
use omeka_db::DbPool;
use omeka_db::ids::{ItemId, MediaId};
use omeka_db::repos::{MediaRepo, SettingRepo};
use omeka_db::rows::{NewMedia, NewResource};
use omeka_db::with_db;
use serde_json::{Value, json};

const ANNO: &str = "application/ld+json; profile=\"http://www.w3.org/ns/anno.jsonld\"";

fn get(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

fn canvas_iri(item: u64, media: u64) -> String {
    format!("http://localhost/iiif/3/item/{item}/canvas/{media}")
}

fn page_iri(media: u64) -> String {
    format!("http://localhost/annotations/media/{media}/page")
}

/// A request with the WAP content type and the seeded admin's key pair.
fn wap_request(method: &str, path: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(append_key(path))
        .header("content-type", ANNO)
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// One whole-object note on `canvas`.
fn whole_note(canvas: &str, text: &str) -> String {
    json!({
        "@context": "http://www.w3.org/ns/anno.jsonld",
        "type": "Annotation",
        "motivation": "commenting",
        "body": {"type": "TextualBody", "value": text, "format": "text/plain"},
        "target": canvas,
    })
    .to_string()
}

/// One note on the canvas's box `x,y,w,h`.
fn box_note(canvas: &str, text: &str, boxed: &str) -> String {
    json!({
        "@context": "http://www.w3.org/ns/anno.jsonld",
        "type": "Annotation",
        "motivation": "commenting",
        "body": {"type": "TextualBody", "value": text, "format": "text/plain"},
        "target": {
            "type": "SpecificResource",
            "source": {"id": canvas, "type": "Canvas"},
            "selector": {
                "type": "FragmentSelector",
                "conformsTo": "http://www.w3.org/TR/media-frags/",
                "value": format!("xywh=pixel:{boxed}"),
            },
        },
    })
    .to_string()
}

async fn post_note(app: &Router, media: u64, body: &str) -> StatusCode {
    send_bytes(
        app.clone(),
        wap_request("POST", &format!("/annotations/media/{media}/"), body),
    )
    .await
    .status
}

async fn manifest(app: &Router, item: u64) -> Value {
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

/// Whether a W3C target names the canvas: a bare IRI for a whole-object note,
/// or a `SpecificResource` whose source is the canvas.
fn targets_canvas(target: &Value, canvas: &str) -> bool {
    target.as_str() == Some(canvas) || target["source"]["id"].as_str() == Some(canvas)
}

/// Add a public or private media row to an item, with no file. The kind follows
/// from the type and the derivative flag, which is all a manifest asks.
async fn add_media(pool: &DbPool, item: u64, media_type: &str, public: bool) -> MediaId {
    with_db!(pool, |db| MediaRepo::new(db)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: None,
                resource_class_id: None,
                resource_template_id: None,
                thumbnail_id: None,
                title: Some(format!("Added {media_type}")),
                is_public: public,
                created: None,
            },
            item_id: ItemId(item),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            data: None,
            source: Some(format!("added.{media_type}")),
            media_type: Some(media_type.to_string()),
            storage_id: Some(format!("added-{media_type}-{public}")),
            extension: None,
            sha256: None,
            size: Some(10),
            has_original: true,
            has_thumbnails: false,
            position: Some(2),
            lang: None,
            alt_text: None,
        })
        .await)
    .expect("the added media row")
    .id()
}

/// A canvas with no visible note carries no `annotations` key, so an unchanged
/// manifest stays byte for byte as it was. Once a box note is stored, the
/// canvas references the WAP page, and every item of that page targets the same
/// canvas. The item manifest names its Content Search 2.0 service.
#[tokio::test]
async fn a_canvas_references_the_page_that_holds_its_visible_notes() {
    let seed = iiif_seed().await;
    let before = manifest(&seed.app, seed.item).await;
    assert!(
        canvas(&before, seed.media).get("annotations").is_none(),
        "{before}"
    );
    assert_eq!(
        before["service"],
        json!([{
            "id": format!("http://localhost/iiif/search/2/item/{}", seed.item),
            "type": "SearchService2",
        }]),
        "the manifest advertises the service its notes are searchable through"
    );

    let target = canvas_iri(seed.item, seed.media);
    assert_eq!(
        post_note(
            &seed.app,
            seed.media,
            &box_note(&target, "Watermark: crowned shield", "10,20,30,40")
        )
        .await,
        StatusCode::CREATED
    );

    let after = manifest(&seed.app, seed.item).await;
    let reference = canvas(&after, seed.media)["annotations"]
        .as_array()
        .unwrap_or_else(|| panic!("an annotations array: {after}"));
    assert_eq!(reference.len(), 1, "{after}");
    assert_eq!(reference[0]["type"], "AnnotationPage");
    let reference_iri = reference[0]["id"].as_str().expect("a page IRI");
    assert_eq!(reference_iri, page_iri(seed.media));

    let page = send_bytes(
        seed.app.clone(),
        get(reference_iri
            .strip_prefix("http://localhost")
            .expect("a local IRI")),
    )
    .await;
    assert_eq!(page.status, StatusCode::OK, "{:?}", page.body);
    let page = page.json();
    assert_eq!(page["type"], "AnnotationPage");
    assert_eq!(page["id"], reference_iri);
    let items = page["items"].as_array().expect("items");
    assert_eq!(items.len(), 1, "{page}");
    assert_eq!(items[0]["body"]["value"], "Watermark: crowned shield");
    assert!(targets_canvas(&items[0]["target"], &target), "{page}");
}

/// The switch off: a caller who may not update the media sees no reference and
/// finds no note, while an editor still sees the same notes on the manifest.
/// The note stays, which the editor's own read proves.
#[tokio::test]
async fn a_manifest_shows_no_note_reference_when_the_switch_hides_them() {
    let seed = iiif_seed().await;
    let target = canvas_iri(seed.item, seed.media);
    assert_eq!(
        post_note(
            &seed.app,
            seed.media,
            &whole_note(&target, "A visitor note")
        )
        .await,
        StatusCode::CREATED
    );
    with_db!(&seed.pool, |db| SettingRepo::new(db)
        .set(ANNOTATIONS_PUBLIC, &json!(false))
        .await)
    .expect("turn the switch off");

    let anonymous = manifest(&seed.app, seed.item).await;
    assert!(
        canvas(&anonymous, seed.media).get("annotations").is_none(),
        "{anonymous}"
    );
    let hits = send_bytes(
        seed.app.clone(),
        get(&format!("/iiif/search/2/item/{}?q=visitor", seed.item)),
    )
    .await
    .json();
    assert_eq!(hits["items"], json!([]), "{hits}");

    let cookie = login_cookie(seed.app.clone(), CURATOR_EMAIL, CURATOR_PASSWORD).await;
    let editor = send_bytes(
        seed.app.clone(),
        Request::builder()
            .uri(format!("/iiif/3/item/{}/manifest", seed.item))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .json();
    assert!(
        canvas(&editor, seed.media)["annotations"].is_array(),
        "an editor still sees the notes: {editor}"
    );
}

/// The media manifest is the item manifest's canvas for one media, alone: same
/// canvas IRI, the item's label, and `partOf` the item manifest.
#[tokio::test]
async fn the_media_manifest_is_the_items_canvas_alone() {
    let seed = iiif_seed().await;
    let item_manifest = manifest(&seed.app, seed.item).await;
    let item_canvas = canvas(&item_manifest, seed.media)["id"]
        .as_str()
        .expect("the canvas id")
        .to_string();
    let answer = send_bytes(
        seed.app.clone(),
        get(&format!("/iiif/3/media/{}/manifest", seed.media)),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.body);
    let media_manifest = answer.json();
    assert_eq!(
        media_manifest["@context"],
        "http://iiif.io/api/presentation/3/context.json"
    );
    let canvases = media_manifest["items"].as_array().expect("canvases");
    assert_eq!(canvases.len(), 1, "{media_manifest}");
    assert_eq!(canvases[0]["id"], item_canvas);
    assert_eq!(media_manifest["label"], item_manifest["label"]);
    assert_eq!(
        media_manifest["partOf"],
        json!([{
            "id": format!("http://localhost/iiif/3/item/{}/manifest", seed.item),
            "type": "Manifest",
        }])
    );
    assert_eq!(
        media_manifest["id"],
        format!("http://localhost/iiif/3/media/{}/manifest", seed.media)
    );
}

/// A media with no canvas has no media manifest: a PDF whose first page was
/// never rendered, and a caption file. The old reader gets the same 404 as a
/// media the caller may not see.
#[tokio::test]
async fn a_media_without_a_canvas_has_no_media_manifest() {
    let seed = iiif_seed().await;
    let pdf = add_media(&seed.pool, seed.item, "application/pdf", true).await;
    let captions = add_media(&seed.pool, seed.item, "text/vtt", true).await;
    for media in [pdf, captions] {
        let answer = send_bytes(
            seed.app.clone(),
            get(&format!("/iiif/3/media/{}/manifest", media.0)),
        )
        .await;
        assert_eq!(
            answer.status,
            StatusCode::NOT_FOUND,
            "{}",
            String::from_utf8_lossy(&answer.body)
        );
    }
}

/// Search reads the notes of the media a visitor may see. A private sibling of
/// the same item contributes no hit, and the answer names the parameters it did
/// not apply.
#[tokio::test]
async fn search_finds_a_public_medias_note_and_not_a_private_siblings() {
    let seed = iiif_seed().await;
    let public = canvas_iri(seed.item, seed.media);
    assert_eq!(
        post_note(
            &seed.app,
            seed.media,
            &whole_note(&public, "A public shield")
        )
        .await,
        StatusCode::CREATED
    );
    let private = add_media(&seed.pool, seed.item, "image/png", false).await;
    let private_canvas = canvas_iri(seed.item, private.0);
    assert_eq!(
        post_note(
            &seed.app,
            private.0,
            &whole_note(&private_canvas, "A private shield")
        )
        .await,
        StatusCode::CREATED
    );

    let answer = send_bytes(
        seed.app.clone(),
        get(&format!(
            "/iiif/search/2/item/{}?q=shield&motivation=commenting",
            seed.item
        )),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.body);
    assert_eq!(
        answer.header("content-type"),
        Some("application/ld+json;profile=\"http://iiif.io/api/search/2/context.json\"")
    );
    assert_eq!(answer.header("access-control-allow-origin"), Some("*"));
    let document = answer.json();
    assert_eq!(
        document["@context"],
        "http://iiif.io/api/search/2/context.json"
    );
    assert_eq!(document["type"], "AnnotationPage");
    let items = document["items"].as_array().expect("items");
    assert_eq!(items.len(), 1, "{document}");
    assert!(targets_canvas(&items[0]["target"], &public), "{document}");
    assert_eq!(items[0]["body"]["value"], "A public shield");
    assert_eq!(document["ignored"], json!(["motivation"]));
    assert_eq!(
        document["id"],
        format!("http://localhost/iiif/search/2/item/{}?q=shield", seed.item)
    );
}

/// A private item's search is a 404 to a visitor, as its manifest is.
#[tokio::test]
async fn search_of_a_private_item_is_not_found_to_a_visitor() {
    let seed = iiif_seed().await;
    let answer = send_bytes(
        seed.app.clone(),
        get(&format!(
            "/iiif/search/2/item/{}?q=chart",
            seed.private_item
        )),
    )
    .await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
}

/// The answer holds 100 hits and names its next page by the last id it showed,
/// so a client pages forward without an offset.
#[tokio::test]
async fn search_pages_by_id() {
    let seed = iiif_seed().await;
    let target = canvas_iri(seed.item, seed.media);
    for number in 0..101 {
        assert_eq!(
            post_note(
                &seed.app,
                seed.media,
                &whole_note(&target, &format!("shield {number}"))
            )
            .await,
            StatusCode::CREATED,
            "note {number}"
        );
    }
    let first = send_bytes(
        seed.app.clone(),
        get(&format!("/iiif/search/2/item/{}?q=shield", seed.item)),
    )
    .await
    .json();
    let items = first["items"].as_array().expect("items");
    assert_eq!(items.len(), 100, "a full answer");
    let last = items
        .last()
        .and_then(|item| item["id"].as_str())
        .and_then(|id| id.rsplit('/').next())
        .and_then(|id| id.parse::<i64>().ok())
        .expect("the last id");
    assert_eq!(
        first["next"],
        format!(
            "http://localhost/iiif/search/2/item/{}?q=shield&after={last}",
            seed.item
        )
    );

    let second = send_bytes(
        seed.app.clone(),
        get(&format!(
            "/iiif/search/2/item/{}?q=shield&after={last}",
            seed.item
        )),
    )
    .await
    .json();
    let rest = second["items"].as_array().expect("items");
    assert_eq!(rest.len(), 1, "{second}");
    assert!(second.get("next").is_none(), "{second}");
    let rest_id = rest[0]["id"]
        .as_str()
        .and_then(|id| id.rsplit('/').next())
        .and_then(|id| id.parse::<i64>().ok())
        .expect("the second page's id");
    assert!(rest_id > last, "{rest_id} is after {last}");
}

/// A search with no words matches nothing and still answers one document.
#[tokio::test]
async fn search_without_words_answers_an_empty_page() {
    let seed = iiif_seed().await;
    let answer = send_bytes(
        seed.app.clone(),
        get(&format!("/iiif/search/2/item/{}?q=", seed.item)),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    let document = answer.json();
    assert_eq!(document["items"], json!([]));
    assert!(document.get("ignored").is_none(), "{document}");
}
