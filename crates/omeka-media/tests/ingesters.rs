//! The three registries end to end, over upstream-shaped fixtures.
//!
//! Every payload under `tests/fixtures/` mirrors a request body or a remote
//! response upstream would see. The network is the `FakeHttp` test double, so no
//! test touches the wire.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use omeka_media::data::MediaData;
use omeka_media::file_render::FileRendererRegistry;
use omeka_media::ingest::{ErrorStore, IngestRequest, IngesterRegistry, MediaDraft, UploadedFile};
use omeka_media::render::{MediaView, RenderOptions, RendererRegistry, View};
use omeka_media::store::Store;
use omeka_media::testing::{Harness, media_row};
use serde_json::Value;

fn fixture(json: &str) -> Value {
    serde_json::from_str(json).expect("fixture is valid JSON")
}

fn request(json: &str) -> IngestRequest {
    IngestRequest::from_value(fixture(json))
}

/// `Ingester/Upload.php` over the fixture request: the file is stored under
/// `original/<storage id>.jpg`.
#[test]
fn upload_ingester_registry_path() {
    let harness = Harness::new("fixture-upload");
    let registry = IngesterRegistry::with_defaults();
    let env = harness.env();
    let request = request(include_str!("fixtures/ingest/upload_request.json"))
        .with_files(vec![UploadedFile::new("Photo.JPG", b"jpegbytes")]);
    let mut draft = MediaDraft::new(media_row(1, "upload", "file"));
    let mut errors = ErrorStore::default();

    registry
        .get("upload")
        .ingest(&mut draft, &request, &env, &mut errors);

    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(draft.row.source.as_deref(), Some("Photo.JPG"));
    assert_eq!(draft.row.extension.as_deref(), Some("jpg"));
    assert!(draft.row.has_original);
    let storage_id = draft.row.storage_id.clone().expect("storage id");
    assert!(
        harness
            .store
            .exists(&format!("original/{storage_id}.jpg"))
            .unwrap()
    );
}

/// `Ingester/Url.php` over the fixture request: the download becomes the
/// original and the source. The body is the recorded response fixture.
#[test]
fn url_ingester_registry_path() {
    let body = include_bytes!("fixtures/ingest/url_response.png");
    let harness = Harness::new("fixture-url")
        .with_body("https://example.org/files/logo-iiif-34x30.png", &body[..]);
    let registry = IngesterRegistry::with_defaults();
    let env = harness.env();
    let request = request(include_str!("fixtures/ingest/url_request.json"));
    let mut draft = MediaDraft::new(media_row(1, "url", "file"));
    let mut errors = ErrorStore::default();

    registry
        .get("url")
        .ingest(&mut draft, &request, &env, &mut errors);

    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        draft.row.source.as_deref(),
        Some("https://example.org/files/logo-iiif-34x30.png")
    );
    assert_eq!(draft.row.extension.as_deref(), Some("png"));
    assert!(draft.row.has_original);
    let storage_id = draft.row.storage_id.clone().expect("storage id");
    let path = format!("original/{storage_id}.png");
    assert_eq!(harness.store.read(&path).expect("stored original"), body);
}

/// `Ingester/Html.php` over the fixture request: the fragment is stored as the
/// payload.
#[test]
fn html_ingester_registry_path() {
    let harness = Harness::new("fixture-html");
    let registry = IngesterRegistry::with_defaults();
    let env = harness.env();
    let request = request(include_str!("fixtures/ingest/html_request.json"));
    let mut draft = MediaDraft::new(media_row(1, "html", "html"));
    let mut errors = ErrorStore::default();

    registry
        .get("html")
        .ingest(&mut draft, &request, &env, &mut errors);

    let expected = fixture(include_str!("fixtures/ingest/html_request.json"));
    // Upstream stores the whole request content with `html` replaced
    // (`Ingester/Html.php:67`); the port stores the typed `html` member only,
    // because `MediaData::Html` in DESIGN.md carries no other field.
    assert_eq!(
        draft.data.to_value(),
        serde_json::json!({ "html": expected["html"] })
    );
}

/// `Ingester/Youtube.php` over the fixture request: the id and times are stored
/// and the requested poster frame is served from the recorded fixture.
#[test]
fn youtube_ingester_registry_path() {
    let poster = include_bytes!("fixtures/ingest/youtube_poster.jpg");
    let harness = Harness::new("fixture-youtube")
        .with_body("http://img.youtube.com/vi/dQw4w9WgXcQ/0.jpg", &poster[..]);
    let registry = IngesterRegistry::with_defaults();
    let env = harness.env();
    let request = request(include_str!("fixtures/ingest/youtube_request.json"));
    let mut draft = MediaDraft::new(media_row(1, "youtube", "youtube"));
    let mut errors = ErrorStore::default();

    registry
        .get("youtube")
        .ingest(&mut draft, &request, &env, &mut errors);

    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        draft.data.to_value(),
        serde_json::json!({"id": "dQw4w9WgXcQ", "start": "30", "end": "90"})
    );
    assert!(draft.row.has_thumbnails);
    assert_eq!(
        harness.http.calls(),
        ["http://img.youtube.com/vi/dQw4w9WgXcQ/0.jpg"]
    );
}

/// `Ingester/OEmbed.php` over the fixture request and the recorded response:
/// the response is stored whole, and its thumbnail is downloaded.
#[test]
fn oembed_ingester_registry_path() {
    let response = fixture(include_str!("fixtures/ingest/oembed_response.json"));
    let thumbnail = include_bytes!("fixtures/ingest/oembed_thumbnail.jpg");
    let harness = Harness::new("fixture-oembed")
        .with_json(
            "https://www.youtube.com/oembed?format=json&url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3DdQw4w9WgXcQ",
            response.clone(),
        )
        .with_body("https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg", &thumbnail[..]);
    let registry = IngesterRegistry::with_defaults();
    let env = harness.env();
    let request = request(include_str!("fixtures/ingest/oembed_request.json"));
    let mut draft = MediaDraft::new(media_row(1, "oembed", "oembed"));
    let mut errors = ErrorStore::default();

    registry
        .get("oembed")
        .ingest(&mut draft, &request, &env, &mut errors);

    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(draft.data.to_value(), response);
    assert_eq!(
        draft.row.source.as_deref(),
        Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ")
    );
    assert!(draft.row.has_thumbnails);
}

/// `Ingester/IIIF.php` over the recorded information document: the document is
/// stored and a size large enough for thumbnailing is requested from its `@id`.
#[test]
fn iiif_ingester_registry_path() {
    let document = fixture(include_str!("fixtures/ingest/iiif_info_v2.json"));
    let id = document["@id"].as_str().expect("@id");
    let source = format!("{id}/info.json");
    // Every recorded size is under the 800 constraint, so the request falls
    // back to the version's default size, `full` for API 2.
    let thumbnail = format!("{id}/full/full/0/default.jpg");
    let harness = Harness::new("fixture-iiif")
        .with_json(&source, document.clone())
        .with_body(&thumbnail, b"jpeg");
    let registry = IngesterRegistry::with_defaults();
    let env = harness.env();
    let request = IngestRequest::from_value(serde_json::json!({"o:source": source}));
    let mut draft = MediaDraft::new(media_row(1, "iiif", "iiif"));
    let mut errors = ErrorStore::default();

    registry
        .get("iiif")
        .ingest(&mut draft, &request, &env, &mut errors);

    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(draft.data.to_value(), document);
    assert_eq!(harness.http.calls(), [source, thumbnail]);
}

/// `Ingester/IiifPresentation.php` over the recorded manifest: the manifest is
/// stored and its v2 thumbnail is downloaded.
#[test]
fn iiif_presentation_ingester_registry_path() {
    let manifest = fixture(include_str!("fixtures/ingest/iiif_presentation_v2.json"));
    let source = manifest["@id"].as_str().expect("@id");
    let thumbnail = manifest["thumbnail"]["@id"]
        .as_str()
        .expect("thumbnail @id");
    let harness = Harness::new("fixture-iiif-presentation")
        .with_json(source, manifest.clone())
        .with_body(thumbnail, b"jpeg");
    let registry = IngesterRegistry::with_defaults();
    let env = harness.env();
    let request = IngestRequest::from_value(serde_json::json!({"o:source": source}));
    let mut draft = MediaDraft::new(media_row(1, "iiif_presentation", "iiif_presentation"));
    let mut errors = ErrorStore::default();

    registry
        .get("iiif_presentation")
        .ingest(&mut draft, &request, &env, &mut errors);

    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(draft.data.to_value(), manifest);
    assert!(draft.row.has_thumbnails);
    assert_eq!(harness.http.calls(), [source, thumbnail]);
}

/// A fixed media row rendered through the renderer registry and the file
/// renderer registry, the two registries `Renderer/File.php` joins.
#[test]
fn renderer_registry_renders_a_fixed_media_row() {
    let renderers = RendererRegistry::with_defaults();
    let file_renderers = FileRendererRegistry::with_defaults();
    let view = View {
        file_renderers: &file_renderers,
    };

    let mut row = media_row(7, "upload", "file");
    row.storage_id = Some("abc123".into());
    row.extension = Some("mp4".into());
    row.media_type = Some("video/mp4".into());
    row.has_original = true;
    row.source = Some("clip.mp4".into());
    let media = MediaView::new(
        row,
        MediaData::None,
        omeka_media::render::RenderEnv::default(),
        "/item/7",
        "/media/7",
    );

    assert_eq!(
        renderers.render("file", &view, &media, &RenderOptions::default()),
        "<video src=\"/files/original/abc123.mp4\" controls><a href=\"/files/original/abc123.mp4\">abc123.mp4</a></video>"
    );

    let youtube = MediaView::new(
        media_row(8, "youtube", "youtube"),
        MediaData::parse(
            "youtube",
            Some(&serde_json::json!({"id": "dQw4w9WgXcQ", "start": "30"})),
        )
        .expect("youtube data"),
        omeka_media::render::RenderEnv::default(),
        "/item/8",
        "/media/8",
    );
    assert_eq!(
        renderers.render("youtube", &view, &youtube, &RenderOptions::default()),
        "<iframe width=\"420\" height=\"315\" src=\"https://www.youtube.com/embed/dQw4w9WgXcQ?start=30\" title=\"[Untitled]\" frameborder=\"0\" allowfullscreen></iframe>"
    );
}
