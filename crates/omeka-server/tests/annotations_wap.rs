//! The Web Annotation Protocol routes, read through HTTP as a client reads
//! them: a create, a replay, a conditional write, the container and its page.
//!
//! Every expected value comes from the design's wire documents (section 6),
//! not from the handlers.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{
    Seed, VISIBILITY_PASSWORD, append_key, get, header, login_cookie, multipart_asset_request,
    router, router_with_store, seeded_pool, send, visibility_pool,
};
use omeka_db::ids::{ItemId, MediaId};
use omeka_db::repos::{
    Cue, ExtentRow, MediaAnnotationRepo, MediaAnnotationWrite, MediaExtentRepo, MediaRepo,
};
use omeka_db::rows::{NewMedia, NewResource};
use omeka_media::store::FileStore;
use serde_json::{Value, json};

const ANNO: &str = "application/ld+json; profile=\"http://www.w3.org/ns/anno.jsonld\"";

fn canvas(item: u64, media: u64) -> String {
    format!("http://localhost/iiif/3/item/{item}/canvas/{media}")
}

fn container(media: u64) -> String {
    format!("/annotations/media/{media}/")
}

fn page(media: u64) -> String {
    format!("/annotations/media/{media}/page")
}

/// One annotation a client may POST: a whole-object note, or one with the
/// selector named.
fn note(canvas: &str, text: &str, selector: Option<Value>) -> String {
    let target = match selector {
        None => json!(canvas),
        Some(selector) => json!({
            "type": "SpecificResource",
            "source": { "id": canvas, "type": "Canvas" },
            "selector": selector,
        }),
    };
    json!({
        "@context": "http://www.w3.org/ns/anno.jsonld",
        "type": "Annotation",
        "motivation": "commenting",
        "body": {
            "type": "TextualBody",
            "value": text,
            "format": "text/plain",
            "language": "en",
        },
        "target": target,
    })
    .to_string()
}

fn box_selector(x: u32, y: u32, w: u32, h: u32) -> Value {
    json!({
        "type": "FragmentSelector",
        "conformsTo": "http://www.w3.org/TR/media-frags/",
        "value": format!("xywh=pixel:{x},{y},{w},{h}"),
    })
}

fn time_selector(start: &str, end: &str) -> Value {
    json!({
        "type": "FragmentSelector",
        "conformsTo": "http://www.w3.org/TR/media-frags/",
        "value": format!("t={start},{end}"),
    })
}

/// A request with the WAP content type, the seeded key pair, and any extra
/// headers the test needs.
fn wap_request(method: &str, path: &str, body: &str, extra: &[(&str, &str)]) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(append_key(path))
        .header("content-type", ANNO);
    for (name, value) in extra {
        builder = builder.header(*name, *value);
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

async fn create(app: &Router, media: u64, body: &str, key: Option<&str>) -> common::Answer {
    let path = container(media);
    let request = match key {
        Some(key) => wap_request("POST", &path, body, &[("idempotency-key", key)]),
        None => wap_request("POST", &path, body, &[]),
    };
    send(app.clone(), request).await
}

#[tokio::test]
async fn a_create_answers_201_with_its_location_and_first_revision() {
    let (app, seed) = common::seeded_router().await;
    let target = canvas(seed.item.0, seed.media.0);
    let answer = create(&app, seed.media.0, &note(&target, "Watermark", None), None).await;
    assert_eq!(answer.status, StatusCode::CREATED, "{}", answer.body);
    let id = answer.json()["id"].as_str().unwrap().to_string();
    assert!(
        id.starts_with("http://localhost/annotations/")
            && id.rsplit('/').next().unwrap().parse::<u64>().is_ok(),
        "{id}"
    );
    assert_eq!(
        header(&answer, "location"),
        Some(id.as_str()),
        "Location is the annotation's own IRI"
    );
    assert_eq!(header(&answer, "etag"), Some("\"r1\""));
    assert_eq!(header(&answer, "access-control-allow-origin"), Some("*"));
    assert!(
        header(&answer, "content-type")
            .unwrap()
            .starts_with("application/ld+json"),
        "{:?}",
        header(&answer, "content-type")
    );
    let document = answer.json();
    assert_eq!(document["@context"], "http://www.w3.org/ns/anno.jsonld");
    assert_eq!(document["type"], "Annotation");
    assert_eq!(document["motivation"], "commenting");
    assert_eq!(document["target"], target.as_str());
    assert_eq!(document["body"]["value"], "Watermark");

    let read = send(app, get(&id, true)).await;
    assert_eq!(read.status, StatusCode::OK);
    assert_eq!(header(&read, "etag"), Some("\"r1\""));
    assert_eq!(
        header(&read, "link"),
        Some("<http://www.w3.org/ns/oa#Annotation>; rel=\"type\"")
    );
    assert_eq!(read.json()["id"], id.as_str());
}

#[tokio::test]
async fn a_repeated_key_replays_its_note_and_a_changed_body_is_422() {
    let (app, seed) = common::seeded_router().await;
    let target = canvas(seed.item.0, seed.media.0);
    let first = create(
        &app,
        seed.media.0,
        &note(&target, "One note", None),
        Some("6f1c-key.5"),
    )
    .await;
    assert_eq!(first.status, StatusCode::CREATED);
    let id = first.json()["id"].as_str().unwrap().to_string();

    let replay = create(
        &app,
        seed.media.0,
        &note(&target, "One note", None),
        Some("6f1c-key.5"),
    )
    .await;
    assert_eq!(replay.status, StatusCode::OK, "{}", replay.body);
    assert_eq!(replay.json()["id"], id.as_str());
    assert_eq!(header(&replay, "location"), Some(id.as_str()));

    let reused = create(
        &app,
        seed.media.0,
        &note(&target, "A different note", None),
        Some("6f1c-key.5"),
    )
    .await;
    assert_eq!(reused.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        reused.json()["errors"]["error"],
        "the idempotency key was used with a different draft"
    );

    let bad_key = create(
        &app,
        seed.media.0,
        &note(&target, "One note", None),
        Some("no spaces allowed"),
    )
    .await;
    assert_eq!(bad_key.status, StatusCode::BAD_REQUEST);

    let listing = send(app, get(&container(seed.media.0), true)).await;
    assert_eq!(listing.json()["total"], 1, "{}", listing.body);
}

#[tokio::test]
async fn a_replace_needs_a_strong_tag_and_a_delete_is_a_one_way_street() {
    let (app, seed) = common::seeded_router().await;
    let target = canvas(seed.item.0, seed.media.0);
    let created = create(&app, seed.media.0, &note(&target, "First", None), None).await;
    let id = created.json()["id"].as_str().unwrap().to_string();

    let missing = send(
        app.clone(),
        wap_request("PUT", &id, &note(&target, "Second", None), &[]),
    )
    .await;
    assert_eq!(missing.status, StatusCode::PRECONDITION_REQUIRED);

    let weak = send(
        app.clone(),
        wap_request(
            "PUT",
            &id,
            &note(&target, "Second", None),
            &[("if-match", "W/\"r1\"")],
        ),
    )
    .await;
    assert_eq!(weak.status, StatusCode::PRECONDITION_FAILED);
    assert_eq!(header(&weak, "etag"), Some("\"r1\""));
    assert_eq!(
        weak.json()["body"]["value"],
        "First",
        "the current note is the body"
    );

    let changed = send(
        app.clone(),
        wap_request(
            "PUT",
            &id,
            &note(&target, "Second", None),
            &[("if-match", "\"r1\"")],
        ),
    )
    .await;
    assert_eq!(changed.status, StatusCode::OK, "{}", changed.body);
    assert_eq!(header(&changed, "etag"), Some("\"r2\""));
    assert_eq!(changed.json()["body"]["value"], "Second");

    let stale = send(
        app.clone(),
        wap_request(
            "PUT",
            &id,
            &note(&target, "Third", None),
            &[("if-match", "\"r1\"")],
        ),
    )
    .await;
    assert_eq!(stale.status, StatusCode::PRECONDITION_FAILED);
    assert_eq!(stale.json()["body"]["value"], "Second");

    let removed = send(
        app.clone(),
        wap_request("DELETE", &id, "", &[("if-match", "\"r2\"")]),
    )
    .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT);
    assert_eq!(removed.body, "");

    let gone = send(
        app,
        wap_request("DELETE", &id, "", &[("if-match", "\"r2\"")]),
    )
    .await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_container_states_the_surface_the_canvas_and_the_notes() {
    let (app, seed) = common::seeded_router().await;
    let target = canvas(seed.item.0, seed.media.0);
    create(&app, seed.media.0, &note(&target, "Whole", None), None).await;
    create(
        &app,
        seed.media.0,
        &note(&target, "Boxed", Some(box_selector(1, 2, 10, 20))),
        None,
    )
    .await;

    let answer = send(app.clone(), get(&container(seed.media.0), true)).await;
    assert_eq!(answer.status, StatusCode::OK);
    let document = answer.json();
    assert_eq!(
        document["type"],
        json!(["BasicContainer", "AnnotationCollection"])
    );
    assert_eq!(document["label"], "Notes on A photo");
    assert_eq!(document["total"], 2);
    assert_eq!(document["first"]["type"], "AnnotationPage");
    assert_eq!(document["first"]["startIndex"], 0);
    assert_eq!(document["first"]["items"].as_array().unwrap().len(), 2);
    assert_eq!(
        header(&answer, "romeka-canvas"),
        Some(target.as_str()),
        "a create on an empty container needs the canvas"
    );
    let surface = header(&answer, "romeka-surface").unwrap();
    assert!(surface.starts_with("image;"), "{surface}");
    assert_eq!(header(&answer, "cache-control"), Some("no-cache"));
    let vary = header(&answer, "vary").unwrap();
    assert!(
        vary.contains("Accept, Cookie, Prefer") && vary.contains("accept-encoding"),
        "the route's Vary and the compression dimension both survive: {vary}"
    );
    assert!(
        surface.contains(&format!(
            "service=\"http://localhost/iiif/image/3/{}\"",
            seed.media.0
        )),
        "{surface}"
    );
    assert!(
        header(&answer, "allow").unwrap().contains("POST"),
        "the media's updater may create"
    );
    assert!(
        header(&answer, "accept-post")
            .unwrap()
            .starts_with("application/ld+json")
    );

    let tag = header(&answer, "etag").unwrap().to_string();
    assert!(tag.starts_with("\"p") && tag.len() == 19, "{tag}");
    let not_modified = send(
        app.clone(),
        Request::builder()
            .uri(append_key(&container(seed.media.0)))
            .header("if-none-match", &tag)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(not_modified.status, StatusCode::NOT_MODIFIED);
    assert_eq!(not_modified.body, "");
    assert_eq!(header(&not_modified, "etag"), Some(tag.as_str()));

    let head = send(
        app.clone(),
        Request::builder()
            .method("HEAD")
            .uri(append_key(&container(seed.media.0)))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(head.status, StatusCode::OK);
    assert_eq!(head.body, "");

    let minimal = send(
        app,
        Request::builder()
            .uri(append_key(&container(seed.media.0)))
            .header(
                "prefer",
                "return=representation;include=\"http://www.w3.org/ns/ldp#PreferMinimalContainer\"",
            )
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert!(
        minimal.json()["first"].is_string(),
        "a minimal container names its page by IRI"
    );
}

#[tokio::test]
async fn the_page_is_the_canvas_annotation_page_in_both_media_types() {
    let (app, seed) = common::seeded_router().await;
    let target = canvas(seed.item.0, seed.media.0);
    create(&app, seed.media.0, &note(&target, "A note", None), None).await;

    let wap = send(app.clone(), get(&page(seed.media.0), true)).await;
    assert_eq!(wap.status, StatusCode::OK);
    let document = wap.json();
    let types = document["@context"].as_array().unwrap();
    assert_eq!(types[0], "http://www.w3.org/ns/anno.jsonld");
    assert_eq!(
        types[1], "http://iiif.io/api/presentation/3/context.json",
        "the IIIF context is last"
    );
    assert_eq!(document["type"], "AnnotationPage");
    assert_eq!(
        document["partOf"]["id"],
        container(seed.media.0).replace("/annotations", "http://localhost/annotations")
    );
    assert_eq!(document["items"][0]["target"], target.as_str());
    assert_eq!(document["next"], Value::Null);
    assert!(
        header(&wap, "content-type")
            .unwrap()
            .contains("anno.jsonld")
    );
    assert_eq!(header(&wap, "romeka-canvas"), Some(target.as_str()));

    let iiif = send(
        app,
        Request::builder()
            .uri(append_key(&page(seed.media.0)))
            .header(
                "accept",
                "application/ld+json; profile=\"http://iiif.io/api/presentation/3/context.json\"",
            )
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(iiif.status, StatusCode::OK);
    assert!(
        header(&iiif, "content-type")
            .unwrap()
            .contains("presentation/3/context.json")
    );
    assert_eq!(
        iiif.json()["items"][0]["id"],
        wap.json()["items"][0]["id"],
        "both forms answer the same page"
    );
    assert_ne!(
        header(&iiif, "etag"),
        header(&wap, "etag"),
        "the two forms are different documents"
    );
}

#[tokio::test]
async fn page_two_holds_the_ids_after_page_one() {
    let (pool, seed) = common::seeded_pool().await;
    let app = router(pool.clone());
    seed_notes(&pool, &seed, 1001).await;

    let first = send(app.clone(), get(&page(seed.media.0), true)).await;
    assert_eq!(first.status, StatusCode::OK);
    let body = first.json();
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1000);
    let last = items.last().unwrap()["id"]
        .as_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    let next = body["next"].as_str().unwrap().to_string();
    assert!(next.ends_with(&format!("?after={last}")), "{next}");

    let second = send(app, get(next.trim_start_matches("http://localhost"), true)).await;
    assert_eq!(second.status, StatusCode::OK);
    let body = second.json();
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    let id = items[0]["id"]
        .as_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    assert!(id > last, "{id} follows {last}");
    assert_eq!(body["next"], Value::Null);
    assert!(
        body.get("startIndex").is_none(),
        "a later page cannot compute startIndex"
    );
}

#[tokio::test]
async fn a_caption_row_is_named_by_the_track_header() {
    let (pool, seed) = common::seeded_pool().await;
    let app = router(pool.clone());
    let cues = [
        Cue {
            start_ms: 0,
            end_ms: 500,
            body_json: r#"[{"type":"TextualBody","value":"Hello","format":"text/plain"}]"#,
            body_text: "Hello",
        },
        Cue {
            start_ms: 500,
            end_ms: 1000,
            body_json: r#"[{"type":"TextualBody","value":"World","format":"text/plain"}]"#,
            body_text: "World",
        },
    ];
    MediaAnnotationRepo::new(&pool)
        .replace_track(seed.media, Some("en"), &cues)
        .await
        .expect("two caption rows");

    let notes = send(app.clone(), get(&container(seed.media.0), true)).await;
    assert_eq!(notes.status, StatusCode::OK);
    assert_eq!(
        notes.json()["total"],
        0,
        "the notes container leaves caption cues out"
    );

    let answer = send(
        app,
        get(&format!("{}?origin=all", container(seed.media.0)), true),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    let ids = header(&answer, "romeka-track-ids").unwrap();
    let ids: Vec<u64> = ids.split(' ').map(|id| id.parse().unwrap()).collect();
    assert_eq!(ids.len(), 2);
    let document = answer.json();
    assert_eq!(document["total"], 2);
    for item in document["first"]["items"].as_array().unwrap() {
        let id = item["id"].as_str().unwrap();
        assert!(ids.iter().any(|ours| id.ends_with(&format!("/{ours}"))));
        assert_eq!(item["motivation"], "supplementing");
    }
}

#[tokio::test]
async fn the_kind_decides_which_selector_the_server_takes() {
    let (pool, seed) = common::seeded_pool().await;
    let app = router(pool.clone());
    let target = canvas(seed.item.0, seed.media.0);
    let boxed = send(
        app.clone(),
        wap_request(
            "POST",
            &container(seed.media.0),
            &note(&target, "Region", Some(box_selector(0, 0, 10, 10))),
            &[],
        ),
    )
    .await;
    assert_eq!(boxed.status, StatusCode::CREATED, "an image takes a box");

    let (pdf, audio) = extra_media(&pool, &seed).await;
    let pdf_canvas = canvas(seed.item.0, pdf.0);
    let empty = send(app.clone(), get(&container(pdf.0), true)).await;
    assert_eq!(empty.json()["total"], 0);
    assert_eq!(
        header(&empty, "romeka-canvas"),
        Some(pdf_canvas.as_str()),
        "an empty container still names the canvas its first note targets"
    );
    let refused = send(
        app.clone(),
        wap_request(
            "POST",
            &container(pdf.0),
            &note(&pdf_canvas, "Boxed", Some(box_selector(0, 0, 10, 10))),
            &[],
        ),
    )
    .await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        refused.json()["errors"]["error"],
        "This media takes notes about the whole file only."
    );
    assert_eq!(
        header(
            &send(app.clone(), get(&container(pdf.0), true)).await,
            "romeka-surface"
        ),
        Some("whole")
    );

    let audio_canvas = canvas(seed.item.0, audio.0);
    let timed = send(
        app.clone(),
        wap_request(
            "POST",
            &container(audio.0),
            &note(&audio_canvas, "A phrase", Some(time_selector("0", "0.5"))),
            &[],
        ),
    )
    .await;
    assert_eq!(timed.status, StatusCode::CREATED, "{}", timed.body);
    let container_answer = send(app.clone(), get(&container(audio.0), true)).await;
    assert!(
        header(&container_answer, "romeka-surface")
            .unwrap()
            .starts_with("time"),
        "{:?}",
        header(&container_answer, "romeka-surface")
    );

    let box_on_audio = send(
        app.clone(),
        wap_request(
            "POST",
            &container(audio.0),
            &note(&audio_canvas, "Boxed", Some(box_selector(0, 0, 10, 10))),
            &[],
        ),
    )
    .await;
    assert_eq!(box_on_audio.status, StatusCode::BAD_REQUEST);

    let wrong_type = send(
        app,
        Request::builder()
            .method("POST")
            .uri(append_key(&container(seed.media.0)))
            .header("content-type", "text/plain")
            .body(Body::from("not json"))
            .unwrap(),
    )
    .await;
    assert_eq!(wrong_type.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn a_note_over_the_document_limit_is_a_413() {
    let (app, seed) = common::seeded_router().await;
    let target = canvas(seed.item.0, seed.media.0);
    let huge = note(&target, &"x".repeat(70 * 1024), None);
    let answer = create(&app, seed.media.0, &huge, None).await;
    assert_eq!(answer.status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn a_visitor_cannot_read_a_private_media_and_a_researcher_cannot_write() {
    let (pool, seed) = visibility_pool().await;
    let app = router(pool);

    let private = send(app.clone(), get(&container(seed.media_private.0), false)).await;
    assert_eq!(private.status, StatusCode::NOT_FOUND);
    let private_page = send(app.clone(), get(&page(seed.media_private.0), false)).await;
    assert_eq!(private_page.status, StatusCode::NOT_FOUND);

    let viewer = send(
        app.clone(),
        common::get_keyed(
            &container(seed.media_public.0),
            common::VIEWER_KEY_IDENTITY,
            common::VIEWER_KEY_CREDENTIAL,
        ),
    )
    .await;
    assert_eq!(viewer.status, StatusCode::OK);
    assert!(
        !header(&viewer, "allow").unwrap().contains("POST"),
        "a researcher reads but does not write"
    );
    let target = canvas(seed.item.0, seed.media_public.0);
    let body = note(&target, "A researcher's note", None);
    let refused = send(
        app,
        Request::builder()
            .method("POST")
            .uri(format!(
                "{}?key_identity={}&key_credential={}",
                container(seed.media_public.0),
                common::VIEWER_KEY_IDENTITY,
                common::VIEWER_KEY_CREDENTIAL
            ))
            .header("content-type", ANNO)
            .body(Body::from(body))
            .unwrap(),
    )
    .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_cross_origin_cookie_write_is_refused_and_a_key_pair_write_is_not() {
    let (pool, seed) = visibility_pool().await;
    let app = router(pool);
    let cookie = login_cookie(app.clone(), seed.owner_email, VISIBILITY_PASSWORD).await;
    let target = canvas(seed.item.0, seed.media_public.0);
    let body = note(&target, "From the editor", None);

    let refused = send(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(container(seed.media_public.0))
            .header("content-type", ANNO)
            .header("cookie", &cookie)
            .header("origin", "https://evil.example")
            .body(Body::from(body.clone()))
            .unwrap(),
    )
    .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);

    let same_origin = send(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(container(seed.media_public.0))
            .header("content-type", ANNO)
            .header("cookie", &cookie)
            .header("origin", "http://localhost")
            .body(Body::from(body.clone()))
            .unwrap(),
    )
    .await;
    assert_eq!(
        same_origin.status,
        StatusCode::CREATED,
        "{}",
        same_origin.body
    );

    let keyed = send(
        app,
        Request::builder()
            .method("POST")
            .uri(append_key(&container(seed.media_public.0)))
            .header("content-type", ANNO)
            .header("origin", "https://elsewhere.example")
            .body(Body::from(note(&target, "From a WAP client", None)))
            .unwrap(),
    )
    .await;
    assert_eq!(keyed.status, StatusCode::CREATED, "{}", keyed.body);
}

#[tokio::test]
async fn a_preflight_states_the_headers_a_wap_client_sends() {
    let (app, seed) = common::seeded_router().await;
    let answer = send(
        app,
        Request::builder()
            .method("OPTIONS")
            .uri(container(seed.media.0))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(answer.status, StatusCode::NO_CONTENT);
    assert_eq!(header(&answer, "access-control-allow-origin"), Some("*"));
    let methods = header(&answer, "access-control-allow-methods").unwrap();
    for method in ["GET", "HEAD", "POST", "PUT", "DELETE", "OPTIONS"] {
        assert!(methods.contains(method), "{methods}");
    }
    let allowed = header(&answer, "access-control-allow-headers").unwrap();
    for name in ["Content-Type", "If-Match", "Idempotency-Key", "Prefer"] {
        assert!(allowed.contains(name), "{allowed}");
    }
    let exposed = header(&answer, "access-control-expose-headers").unwrap();
    for name in [
        "ETag",
        "Location",
        "Romeka-Surface",
        "Romeka-Canvas",
        "Romeka-Track-Ids",
    ] {
        assert!(exposed.contains(name), "{exposed}");
    }
}

/// An image the storage really holds: its header gives the canvas size, so the
/// container's surface names the pixels the editor draws in.
#[tokio::test]
async fn an_images_surface_names_its_upright_size_and_image_service() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let app = router_with_store(pool.clone(), store.clone());
    let media = upload_image(&app, &seed).await;
    // The create deferred the derivatives; this test reads one, so make it.
    common::settle_derivatives(pool.clone(), std::sync::Arc::new(store)).await;

    let answer = send(app, get(&container(media), true)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let surface = header(&answer, "romeka-surface").unwrap();
    assert_eq!(
        surface,
        format!("image; width=80; height=120; service=\"http://localhost/iiif/image/3/{media}\""),
        "image.jpg is landscape pixels with EXIF Orientation 6"
    );
}

/// `image.jpg` is 120 by 80 with EXIF Orientation 6; the canvas is the upright
/// 80 by 120.
async fn upload_image(app: &Router, seed: &Seed) -> u64 {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/media-kinds/image.jpg");
    let bytes = std::fs::read(path).expect("image.jpg");
    let data = json!({
        "o:is_public": true,
        "dcterms:title": [{"type": "literal", "@value": "Oriented"}],
        "o:media": [{
            "o:ingester": "upload",
            "o:is_public": true,
            "file_index": 0,
            "o:item": {"o:id": seed.item.0},
        }],
    })
    .to_string();
    let answer = send(
        app.clone(),
        multipart_asset_request(&append_key("/api/items"), "image.jpg", &bytes, &data),
    )
    .await;
    assert!(answer.status.is_success(), "{}", answer.body);
    answer.json()["o:media"][0]["o:id"]
        .as_u64()
        .expect("media id")
}

/// Insert 1,001 whole notes directly, faster than 1,001 HTTP creates.
async fn seed_notes(pool: &sqlx::SqlitePool, seed: &Seed, count: u32) {
    let write = MediaAnnotationWrite {
        motivation: "commenting",
        selector_kind: "whole",
        selector: None,
        time_ms: None,
        body_json: r#"[{"type":"TextualBody","value":"n","format":"text/plain"}]"#,
        body_text: "n",
        lang: None,
        extensions: None,
    };
    for _ in 0..count {
        MediaAnnotationRepo::new(pool)
            .insert(seed.media, Some(seed.user), None, &write)
            .await
            .expect("insert a note");
    }
}

/// A video's frame is a drawing surface over time: its surface names the frame,
/// a box with a time range is a note on it, and a box with no range is refused
/// with the sentence that says what it needs.
#[tokio::test]
async fn a_video_takes_a_box_over_a_time_range() {
    let (pool, seed) = common::seeded_pool().await;
    let app = router(pool.clone());
    let video = add_media(&pool, &seed, "A talk", "talk.mp4", "mp4", "video/mp4", true).await;
    MediaExtentRepo::new(&pool)
        .put(
            video,
            "original/mp4-missing.mp4",
            &ExtentRow {
                duration_ms: Some(90_000),
                width: Some(640),
                height: Some(360),
            },
        )
        .await
        .expect("the recorded extent");
    let target = canvas(seed.item.0, video.0);

    let empty = send(app.clone(), get(&container(video.0), true)).await;
    assert_eq!(
        header(&empty, "romeka-surface"),
        Some("time; duration=90; width=640; height=360")
    );

    let boxed = send(
        app.clone(),
        wap_request(
            "POST",
            &container(video.0),
            &note(
                &target,
                "The speaker's hands",
                Some(json!({
                    "type": "FragmentSelector",
                    "conformsTo": "http://www.w3.org/TR/media-frags/",
                    "value": "xywh=pixel:10,20,300,150&t=1.5,4",
                })),
            ),
            &[],
        ),
    )
    .await;
    assert_eq!(boxed.status, StatusCode::CREATED, "{}", boxed.body);
    let page = send(app.clone(), get(&container(video.0), true))
        .await
        .json();
    assert_eq!(
        page["first"]["items"][0]["target"]["selector"]["value"],
        "xywh=pixel:10,20,300,150&t=1.5,4"
    );

    let bare = send(
        app,
        wap_request(
            "POST",
            &container(video.0),
            &note(&target, "Boxed", Some(box_selector(0, 0, 10, 10))),
            &[],
        ),
    )
    .await;
    assert_eq!(bare.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        bare.json()["errors"]["error"],
        "A shape on a video frame is a box with a time range."
    );
}

/// A PDF and an audio row on the seeded item: the first has no drawing surface,
/// the second takes a time range and no box.
async fn extra_media(pool: &sqlx::SqlitePool, seed: &Seed) -> (MediaId, MediaId) {
    let pdf = add_media(
        pool,
        seed,
        "A document",
        "application/pdf",
        "pdf",
        "application/pdf",
        false,
    )
    .await;
    let audio = add_media(
        pool,
        seed,
        "A recording",
        "audio/mpeg",
        "mp3",
        "audio/mpeg",
        false,
    )
    .await;
    (pdf, audio)
}

async fn add_media(
    pool: &sqlx::SqlitePool,
    seed: &Seed,
    title: &str,
    source: &str,
    extension: &str,
    media_type: &str,
    has_thumbnails: bool,
) -> MediaId {
    MediaRepo::new(pool)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: Some(seed.user),
                title: Some(title.to_string()),
                is_public: true,
                ..NewResource::default()
            },
            item_id: ItemId(seed.item.0),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            data: None,
            source: Some(source.to_string()),
            media_type: Some(media_type.to_string()),
            storage_id: Some(format!("{}-missing", extension)),
            extension: Some(extension.to_string()),
            sha256: Some("missing".to_string()),
            size: Some(1),
            has_original: true,
            has_thumbnails,
            position: None,
            lang: None,
            alt_text: None,
        })
        .await
        .expect("create a media row")
        .id()
}
