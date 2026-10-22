//! The wire documents of one media's annotations: the section 6 page renders
//! byte for byte, another client's page keeps its extension keys, and every
//! selector form either canonicalises or is refused.
//!
//! Every test drives `Annotations::create` or `::replace`, the two entry points
//! unit 5's WAP routes call, and asserts the document the caller gets back.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use omeka_api::annotation::{
    Annotation, AnnotationError, Annotations, Created, ExtentSource, IdempotencyKey, IfMatch, Iris,
    MediaRef, Problem, Selector, render,
};
use omeka_api::auth::Actor;
use omeka_api::jsonld::BaseUrl;
use omeka_api::read::Api;
use omeka_db::DbPool;
use omeka_db::ids::{ItemId, MediaId, UserId};
use omeka_db::repos::{ItemRepo, MediaRepo, UserRepo};
use omeka_db::rows::{MediaRow, NewItem, NewMedia, NewResource, NewUser};
use omeka_db::{migrate, open_memory};
use omeka_media::kind::{Extent, MediaKind};
use serde_json::{Value, json};
use sqlx::SqlitePool;

/// The configured base every IRI in these tests starts from.
const BASE: &str = "https://archive.example";

struct Seed {
    db: DbPool,
    editor: UserId,
    image: MediaId,
    video: MediaId,
    pdf: MediaId,
}

struct FakeExtents {
    extents: HashMap<MediaId, Extent>,
}

impl FakeExtents {
    fn of(seed: &Seed) -> Self {
        let mut extents = HashMap::new();
        extents.insert(
            seed.image,
            Extent {
                size: Some((4000, 3000)),
                duration_ms: None,
            },
        );
        extents.insert(
            seed.video,
            Extent {
                size: Some((1920, 1080)),
                duration_ms: Some(90_000),
            },
        );
        Self { extents }
    }
}

impl ExtentSource for FakeExtents {
    fn extent<'a>(
        &'a self,
        media: &'a MediaRow,
    ) -> Pin<Box<dyn Future<Output = Extent> + Send + 'a>> {
        let extent = self.extents.get(&media.id()).copied().unwrap_or_default();
        Box::pin(async move { extent })
    }
}

async fn seed() -> Seed {
    let pool = open_memory().await.expect("in-memory database");
    migrate(&pool).await.expect("migrations");
    let editor = UserRepo::new(&pool)
        .create(&NewUser {
            email: "editor@example.com".to_string(),
            name: "Editor".to_string(),
            password_hash: None,
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .expect("editor row")
        .id;
    let item = ItemRepo::new(&pool)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(editor),
                title: Some("Folio 12r".to_string()),
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .expect("item row")
        .id();
    let image = media(&pool, item, editor, "page.jpg", "image/jpeg", true).await;
    let video = media(&pool, item, editor, "talk.mp4", "video/mp4", false).await;
    let pdf = media(&pool, item, editor, "talk.pdf", "application/pdf", false).await;
    Seed {
        db: DbPool::from(pool),
        editor,
        image,
        video,
        pdf,
    }
}

async fn media(
    pool: &SqlitePool,
    item: ItemId,
    owner: UserId,
    name: &str,
    media_type: &str,
    thumbnails: bool,
) -> MediaId {
    MediaRepo::new(pool)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: Some(owner),
                title: Some(name.to_string()),
                ..NewResource::default()
            },
            item_id: item,
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            storage_id: Some(format!("storage-{name}")),
            extension: Some(name.rsplit('.').next().unwrap_or("bin").to_string()),
            media_type: Some(media_type.to_string()),
            has_original: true,
            has_thumbnails: thumbnails,
            ..NewMedia::default()
        })
        .await
        .expect("media row")
        .id()
}

async fn api<'a>(seed: &'a Seed, user: UserId, role: &str) -> Api<'a> {
    Api::load(
        &seed.db,
        BaseUrl::parse(BASE).expect("base URL"),
        Actor::new(user, role),
    )
    .await
    .expect("read surface")
}

async fn canvas(notes: &Annotations<'_>, iris: &Iris, media: MediaId) -> String {
    let (media, _) = notes.rights(media).await.expect("rights");
    iris.canvas(&media)
}

fn body(text: &str) -> Value {
    json!({ "type": "TextualBody", "value": text, "format": "text/plain" })
}

fn doc(motivation: &str, target: Value, body: Option<Value>) -> Vec<u8> {
    let mut document = json!({
        "@context": "http://www.w3.org/ns/anno.jsonld",
        "type": "Annotation",
        "motivation": motivation,
        "target": target,
    });
    if let Some(body) = body {
        document["body"] = body;
    }
    serde_json::to_vec(&document).expect("a JSON document")
}

fn whole(canvas: &str) -> Value {
    json!(canvas)
}

fn fragment(canvas: &str, value: &str) -> Value {
    json!({
        "type": "SpecificResource",
        "source": { "id": canvas, "type": "Canvas" },
        "selector": {
            "type": "FragmentSelector",
            "conformsTo": "http://www.w3.org/TR/media-frags/",
            "value": value,
        },
    })
}

fn svg(canvas: &str, value: &str) -> Value {
    json!({
        "type": "SpecificResource",
        "source": { "id": canvas, "type": "Canvas" },
        "selector": { "type": "SvgSelector", "value": value },
    })
}

async fn create_ok(
    notes: &Annotations<'_>,
    media: MediaId,
    document: Vec<u8>,
    iris: &Iris,
) -> Annotation {
    match notes
        .create(media, &document, iris, None)
        .await
        .expect("created")
    {
        Created::New(annotation) => annotation,
        Created::Replayed(_) => panic!("a create without a key is never a replay"),
    }
}

fn problem<T: std::fmt::Debug>(result: Result<T, AnnotationError>) -> Problem {
    match result {
        Err(AnnotationError::Invalid(problem)) => problem,
        other => panic!("expected Invalid, got {other:?}"),
    }
}

fn stamp(at: chrono::NaiveDateTime) -> String {
    at.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// The fixture with the server's own members filled in: the note's IRI, its
/// times, the seeded canvas and the item manifest. Everything else is compared
/// byte for byte, key order included.
fn canonical(fixture: &Value, annotation: &Annotation, iris: &Iris) -> Value {
    let mut expected = fixture.clone();
    expected["id"] = json!(iris.annotation(annotation.id));
    expected["created"] = json!(stamp(annotation.created));
    expected["modified"] = json!(stamp(annotation.modified));
    expected["target"]["source"]["id"] = json!(iris.canvas(&annotation.media));
    expected["target"]["source"]["partOf"][0]["id"] =
        json!(iris.item_manifest(annotation.media.item));
    expected
}

fn fixture(name: &str) -> Value {
    serde_json::from_str(match name {
        "page" => include_str!("fixtures/annotation/page.json"),
        "container" => include_str!("fixtures/annotation/container.json"),
        "foreign" => include_str!("fixtures/annotation/foreign-page.json"),
        other => panic!("no fixture {other}"),
    })
    .expect("a fixture document")
}

#[tokio::test]
async fn the_contract_page_round_trips_byte_for_byte() {
    let seed = seed().await;
    let api = api(&seed, seed.editor, "editor").await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.image).await;
    let page = fixture("page");
    let mut document = page["items"][0].clone();
    document["target"]["source"]["id"] = json!(target);
    let bytes = serde_json::to_vec(&document).expect("a JSON document");

    let annotation = create_ok(&notes, seed.image, bytes, &iris).await;
    assert_eq!(
        annotation.draft.selector,
        Selector::Rect {
            x: 1200,
            y: 340,
            w: 410,
            h: 388
        }
    );
    let rendered = render(&annotation, &iris);
    assert_eq!(
        rendered.to_string(),
        canonical(&page["items"][0], &annotation, &iris).to_string(),
        "the server writes the canonical document, key order included"
    );

    let put = serde_json::to_vec(&rendered).expect("a JSON document");
    let if_match = IfMatch::parse("*").expect("a strong wildcard");
    let replaced = notes
        .replace(annotation.id, &if_match, &put, &iris)
        .await
        .expect("an equal replace");
    assert_eq!(replaced.revision, annotation.revision, "no revision moves");
    assert_eq!(replaced.draft, annotation.draft, "the document parses back");
}

#[tokio::test]
async fn the_container_and_the_page_documents_carry_the_same_note() {
    let seed = seed().await;
    let api = api(&seed, seed.editor, "editor").await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.image).await;

    let page = fixture("page");
    let container = fixture("container");
    let mut from_page = page["items"][0].clone();
    let mut from_container = container["first"]["items"][0].clone();
    from_page["target"]["source"]["id"] = json!(target);
    from_container["target"]["source"]["id"] = json!(target);

    let first = create_ok(
        &notes,
        seed.image,
        serde_json::to_vec(&from_page).expect("JSON"),
        &iris,
    )
    .await;
    let second = create_ok(
        &notes,
        seed.image,
        serde_json::to_vec(&from_container).expect("JSON"),
        &iris,
    )
    .await;
    assert_eq!(first.draft, second.draft);
}

#[tokio::test]
async fn another_clients_page_keeps_its_extension_keys_and_loses_its_script() {
    let seed = seed().await;
    let api = api(&seed, seed.editor, "editor").await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.image).await;
    let page = fixture("foreign");
    let mut document = page["items"][0].clone();
    document["target"]["source"]["id"] = json!(target);
    let bytes = serde_json::to_vec(&document).expect("a JSON document");

    let annotation = create_ok(&notes, seed.image, bytes, &iris).await;
    assert_eq!(
        annotation.draft.selector,
        Selector::Polygon(vec![(10, 20), (30, 40), (50, 60)])
    );
    let rendered = render(&annotation, &iris);
    assert_eq!(rendered["dcterms:source"], "Briquet 1442");
    assert_eq!(rendered["romeka:client"]["name"], "Another WAP client");
    assert_eq!(rendered["romeka:client"]["version"], 2);
    assert_eq!(rendered["example:flag"], true);
    assert!(
        rendered.get("creator").is_none(),
        "a known member the server owns is not an extension"
    );
    assert_eq!(
        rendered["target"]["selector"]["value"],
        "<svg xmlns=\"http://www.w3.org/2000/svg\"><polygon points=\"10,20 30,40 50,60\"/></svg>",
        "the client's own SVG text is never echoed"
    );
    let bodies = rendered["body"].as_array().expect("two bodies");
    let html = bodies[0]["value"].as_str().expect("a text body");
    assert!(html.contains("<b>seal</b>"), "{html}");
    assert!(
        !html.contains("script"),
        "the purifier dropped the script: {html}"
    );
    assert_eq!(bodies[0]["format"], "text/html");
    assert_eq!(bodies[0]["purpose"], "describing");
    assert_eq!(bodies[1]["id"], "https://example.org/seals/12");
    assert_eq!(bodies[1]["purpose"], "linking");
}

#[tokio::test]
async fn a_whole_note_renders_as_the_canvas_iri() {
    let seed = seed().await;
    let api = api(&seed, seed.editor, "editor").await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.pdf).await;
    let annotation = create_ok(
        &notes,
        seed.pdf,
        doc(
            "commenting",
            whole(&target),
            Some(body("A scan of the deed")),
        ),
        &iris,
    )
    .await;

    assert_eq!(annotation.draft.selector, Selector::Whole);
    let rendered = render(&annotation, &iris);
    assert_eq!(rendered["target"], json!(target));
}

#[tokio::test]
async fn a_time_note_keeps_millisecond_precision() {
    let seed = seed().await;
    let api = api(&seed, seed.editor, "editor").await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.video).await;

    let annotation = create_ok(
        &notes,
        seed.video,
        doc(
            "commenting",
            fragment(&target, "t=62,70.5"),
            Some(body("The speaker names the place")),
        ),
        &iris,
    )
    .await;
    assert_eq!(
        annotation.draft.selector,
        Selector::Time {
            start_ms: 62_000,
            end_ms: 70_500
        }
    );
    assert_eq!(
        render(&annotation, &iris)["target"]["selector"]["value"],
        "t=62,70.5"
    );

    let brief = create_ok(
        &notes,
        seed.video,
        doc(
            "commenting",
            fragment(&target, "t=0,0.5"),
            Some(body("Hello")),
        ),
        &iris,
    )
    .await;
    assert_eq!(
        render(&brief, &iris)["target"]["selector"]["value"],
        "t=0,0.5"
    );
}

#[tokio::test]
async fn a_box_on_a_video_frame_keeps_its_time_range() {
    let seed = seed().await;
    let api = api(&seed, seed.editor, "editor").await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.video).await;

    let annotation = create_ok(
        &notes,
        seed.video,
        doc(
            "commenting",
            fragment(&target, "xywh=pixel:100,200,300,150&t=62,70.5"),
            Some(body("The speaker's hands")),
        ),
        &iris,
    )
    .await;
    assert_eq!(
        annotation.draft.selector,
        Selector::BoxDuring {
            x: 100,
            y: 200,
            w: 300,
            h: 150,
            start_ms: 62_000,
            end_ms: 70_500,
        }
    );
    assert_eq!(
        render(&annotation, &iris)["target"]["selector"]["value"],
        "xywh=pixel:100,200,300,150&t=62,70.5"
    );

    let reversed = create_ok(
        &notes,
        seed.video,
        doc(
            "commenting",
            fragment(&target, "t=1,2&xywh=percent:50,50,10,10"),
            Some(body("Either order, either unit")),
        ),
        &iris,
    )
    .await;
    assert_eq!(
        render(&reversed, &iris)["target"]["selector"]["value"],
        "xywh=pixel:960,540,192,108&t=1,2"
    );
}

#[tokio::test]
async fn a_percent_box_becomes_canvas_pixels() {
    let seed = seed().await;
    let api = api(&seed, seed.editor, "editor").await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.image).await;

    let annotation = create_ok(
        &notes,
        seed.image,
        doc(
            "commenting",
            fragment(&target, "xywh=percent:25,25,50,50"),
            Some(body("The middle")),
        ),
        &iris,
    )
    .await;
    assert_eq!(
        annotation.draft.selector,
        Selector::Rect {
            x: 1000,
            y: 750,
            w: 2000,
            h: 1500
        }
    );
    assert_eq!(
        render(&annotation, &iris)["target"]["selector"]["value"],
        "xywh=pixel:1000,750,2000,1500"
    );
}

#[tokio::test]
async fn a_selector_the_media_cannot_take_is_refused() {
    let seed = seed().await;
    let api = api(&seed, seed.editor, "editor").await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&api, &extents);
    let iris = Iris::new(BASE);
    let image = canvas(&notes, &iris, seed.image).await;
    let video = canvas(&notes, &iris, seed.video).await;
    let pdf = canvas(&notes, &iris, seed.pdf).await;
    let text = Some(body("A note"));

    let cases = [
        (
            seed.image,
            fragment(&image, "xywh=pixel:3900,2900,200,200"),
            Problem::BoxOutsideCanvas,
        ),
        (seed.image, fragment(&image, "t=0,1"), Problem::TimeOnImage),
        (
            seed.video,
            fragment(&video, "xywh=pixel:0,0,10,10"),
            Problem::VideoShapeNeedsTime,
        ),
        (
            seed.video,
            svg(&video, "<svg><polygon points=\"0,0 10,0 10,10\"/></svg>"),
            Problem::VideoShapeNeedsTime,
        ),
        (
            seed.video,
            fragment(&video, "xywh=pixel:1900,0,40,40&t=0,1"),
            Problem::BoxOutsideCanvas,
        ),
        (
            seed.video,
            fragment(&video, "xywh=pixel:0,0,40,40&t=0,900"),
            Problem::PastDuration,
        ),
        (
            seed.image,
            fragment(&image, "xywh=pixel:0,0,40,40&t=0,1"),
            Problem::TimeOnImage,
        ),
        (
            seed.video,
            fragment(&video, "t=0,900"),
            Problem::PastDuration,
        ),
        (
            seed.video,
            fragment(&video, "t=2,2"),
            Problem::EmptyTimeRange,
        ),
        (
            seed.pdf,
            fragment(&pdf, "xywh=pixel:0,0,10,10"),
            Problem::WholeFileOnly,
        ),
    ];
    for (media, target, expected) in cases {
        let document = doc("commenting", target, text.clone());
        assert_eq!(
            problem(notes.create(media, &document, &iris, None).await),
            expected
        );
    }
}

#[tokio::test]
async fn a_polygon_outside_the_canvas_or_without_enough_points_is_refused() {
    let seed = seed().await;
    let api = api(&seed, seed.editor, "editor").await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&api, &extents);
    let iris = Iris::new(BASE);
    let image = canvas(&notes, &iris, seed.image).await;
    let text = Some(body("A shape"));

    let two_points = doc(
        "commenting",
        svg(
            &image,
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><polygon points=\"1,2 3,4\"/></svg>",
        ),
        text.clone(),
    );
    assert_eq!(
        problem(notes.create(seed.image, &two_points, &iris, None).await),
        Problem::BadSelector
    );

    let outside = doc(
        "commenting",
        svg(
            &image,
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><polygon points=\"10,10 20,20 5000,30\"/></svg>",
        ),
        text.clone(),
    );
    assert_eq!(
        problem(notes.create(seed.image, &outside, &iris, None).await),
        Problem::BoxOutsideCanvas
    );

    let no_polygon = doc(
        "commenting",
        svg(
            &image,
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><circle r=\"4\"/></svg>",
        ),
        text,
    );
    assert_eq!(
        problem(notes.create(seed.image, &no_polygon, &iris, None).await),
        Problem::BadSelector
    );
}

#[tokio::test]
async fn a_document_that_is_not_an_annotation_is_refused() {
    let seed = seed().await;
    let api = api(&seed, seed.editor, "editor").await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&api, &extents);
    let iris = Iris::new(BASE);

    let page = serde_json::to_vec(&json!({ "type": "AnnotationPage", "items": [] })).expect("JSON");
    assert_eq!(
        problem(notes.create(seed.image, &page, &iris, None).await),
        Problem::NotAnnotation
    );
    assert_eq!(
        problem(notes.create(seed.image, b"not json", &iris, None).await),
        Problem::NotAnnotation
    );
    assert_eq!(
        problem(notes.create(seed.image, b"[]", &iris, None).await),
        Problem::NotAnnotation
    );

    let foreign = doc(
        "commenting",
        whole("https://elsewhere.example/iiif/3/item/1/canvas/2"),
        Some(body("Elsewhere")),
    );
    assert_eq!(
        problem(notes.create(seed.image, &foreign, &iris, None).await),
        Problem::ForeignTarget
    );
}

#[tokio::test]
async fn a_note_over_64_kib_is_too_large() {
    let seed = seed().await;
    let api = api(&seed, seed.editor, "editor").await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.pdf).await;
    let document = doc(
        "commenting",
        whole(&target),
        Some(body(&"a".repeat(70_000))),
    );
    assert!(matches!(
        notes.create(seed.pdf, &document, &iris, None).await,
        Err(AnnotationError::TooLarge)
    ));
}

#[tokio::test]
async fn a_body_outside_the_shapes_the_server_stores_is_refused() {
    let seed = seed().await;
    let api = api(&seed, seed.editor, "editor").await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.pdf).await;

    let twenty_one = Value::Array(
        (0..21)
            .map(|index| body(&format!("Body {index}")))
            .collect(),
    );
    assert_eq!(
        problem(
            notes
                .create(
                    seed.pdf,
                    &doc("commenting", whole(&target), Some(twenty_one)),
                    &iris,
                    None,
                )
                .await
        ),
        Problem::TooManyBodies
    );

    let long = doc(
        "commenting",
        whole(&target),
        Some(body(&"b".repeat(16_001))),
    );
    assert_eq!(
        problem(notes.create(seed.pdf, &long, &iris, None).await),
        Problem::TextTooLong
    );

    let unknown_format = doc(
        "commenting",
        whole(&target),
        Some(json!({ "type": "TextualBody", "value": "x", "format": "text/turtle" })),
    );
    assert_eq!(
        problem(notes.create(seed.pdf, &unknown_format, &iris, None).await),
        Problem::BadBody
    );

    let script_link = doc(
        "linking",
        whole(&target),
        Some(json!({ "id": "javascript:alert(1)" })),
    );
    assert_eq!(
        problem(notes.create(seed.pdf, &script_link, &iris, None).await),
        Problem::BadBody
    );

    let no_text = doc("tagging", whole(&target), None);
    assert_eq!(
        problem(notes.create(seed.pdf, &no_text, &iris, None).await),
        Problem::NoTextBody
    );

    let empty_body = doc("tagging", whole(&target), Some(body("   ")));
    assert_eq!(
        problem(notes.create(seed.pdf, &empty_body, &iris, None).await),
        Problem::NoTextBody
    );
}

#[tokio::test]
async fn a_caption_is_a_supplementing_time_range() {
    let seed = seed().await;
    let api = api(&seed, seed.editor, "editor").await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.video).await;

    let cue = create_ok(
        &notes,
        seed.video,
        doc(
            "supplementing",
            fragment(&target, "t=1,2.5"),
            Some(json!({
                "type": "TextualBody",
                "value": "A cue",
                "format": "text/plain",
                "language": "en",
            })),
        ),
        &iris,
    )
    .await;
    assert_eq!(
        cue.draft.motivation,
        omeka_api::annotation::Motivation::Supplementing
    );
    let rendered = render(&cue, &iris);
    assert_eq!(rendered["motivation"], "supplementing");
    assert_eq!(rendered["body"]["language"], "en");

    let whole_note = doc(
        "supplementing",
        whole(&target),
        Some(body("A cue without a time")),
    );
    assert_eq!(
        problem(notes.create(seed.video, &whole_note, &iris, None).await),
        Problem::CaptionNeedsTime
    );
}

#[tokio::test]
async fn a_missing_motivation_is_a_comment() {
    let seed = seed().await;
    let api = api(&seed, seed.editor, "editor").await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.pdf).await;
    let document = json!({
        "@context": "http://www.w3.org/ns/anno.jsonld",
        "type": "Annotation",
        "body": body("No motivation given"),
        "target": target,
    });
    let annotation = create_ok(
        &notes,
        seed.pdf,
        serde_json::to_vec(&document).expect("JSON"),
        &iris,
    )
    .await;
    assert_eq!(render(&annotation, &iris)["motivation"], "commenting");
}

#[test]
fn the_iris_scheme_lives_in_one_place() {
    let iris = Iris::new("https://archive.example/");
    let media = MediaRef {
        id: MediaId(4127),
        item: ItemId(812),
        kind: MediaKind::LocalImage,
        extent: Extent::default(),
    };
    assert_eq!(
        iris.canvas(&media),
        "https://archive.example/iiif/3/item/812/canvas/4127"
    );
    assert_eq!(
        iris.item_manifest(media.item),
        "https://archive.example/iiif/3/item/812/manifest"
    );
    assert_eq!(
        iris.media_manifest(media.id),
        "https://archive.example/iiif/3/media/4127/manifest"
    );
    assert_eq!(
        iris.container(media.id),
        "https://archive.example/annotations/media/4127/"
    );
    assert_eq!(
        iris.page(media.id, None),
        "https://archive.example/annotations/media/4127/page"
    );
    assert_eq!(
        iris.page(media.id, Some(omeka_api::annotation::AnnotationId(90311))),
        "https://archive.example/annotations/media/4127/page?after=90311"
    );
    assert_eq!(
        iris.annotation(omeka_api::annotation::AnnotationId(90311)),
        "https://archive.example/annotations/90311"
    );
    assert_eq!(
        iris.captions(media.id, Some("en")),
        "https://archive.example/annotations/media/4127/captions.vtt?lang=en"
    );
    assert_eq!(
        iris.captions(media.id, None),
        "https://archive.example/annotations/media/4127/captions.vtt"
    );
    assert_eq!(
        iris.search(media.item),
        "https://archive.example/iiif/search/2/item/812"
    );
}

#[test]
fn if_match_compares_strongly() {
    assert_eq!(IfMatch::parse("*"), Some(IfMatch::Any));
    assert!(matches!(
        IfMatch::parse("\"r1\""),
        Some(IfMatch::Tags(tags)) if tags.len() == 1
    ));
    assert!(matches!(
        IfMatch::parse("\"r1\", \"r2\""),
        Some(IfMatch::Tags(tags)) if tags.len() == 2
    ));
    assert!(
        matches!(IfMatch::parse("W/\"r1\""), Some(IfMatch::Tags(tags)) if tags.is_empty()),
        "a weak tag parses but matches no revision"
    );
    assert_eq!(IfMatch::parse(""), None);
    assert_eq!(IfMatch::parse("\"r1"), None);
    assert_eq!(IfMatch::parse("\"x1\""), None);
}

#[test]
fn an_idempotency_key_is_a_narrow_token() {
    assert!(IdempotencyKey::parse("6f1c-2c4e9a0b:migrate_0001.x").is_some());
    assert_eq!(IdempotencyKey::parse(""), None);
    assert_eq!(IdempotencyKey::parse("has space"), None);
    assert_eq!(IdempotencyKey::parse(&"k".repeat(65)), None);
}
