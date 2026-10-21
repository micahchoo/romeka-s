//! The media-kind matrix: one row per file in `fixtures/media-kinds/`, one
//! column per part of Romeka, each cell read from what the server answered.
//!
//! The test uploads every corpus file through the public API as an item's file
//! media, then asks the API, the public site and the IIIF manifest what they
//! made of it. `KNOWN_RED` is the set of cells the current server fails. The
//! test fails when a cell is red and not listed, and when a listed cell passes,
//! so the list only shrinks as units fix the pipeline.
//!
//! `MEDIA_MATRIX_PRINT=1` prints the table and rewrites
//! `fixtures/media-kinds/BASELINE.md`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeSet;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{
    Answer, Seed, append_key, get, header, json_request, multipart_asset_request,
    router_with_store, seeded_pool, send,
};
use omeka_db::DbPool;
use omeka_db::repos::{ExtentRow, MediaExtentRepo};
use omeka_db::with_db;
use omeka_media::image_metadata::image_metadata;
use omeka_media::store::{FileStore, Store};
use omeka_media::thumbnailer::ImageThumbnailer;
use serde_json::{Value, json};

/// The seeded site every corpus item is attached to, so its card reaches the
/// public item browse.
const SITE: &str = "/s/museum";

/// The cells the current server fails, as `(corpus file, cell)`. Remove a pair
/// when a unit turns that cell green.
const KNOWN_RED: &[(&str, &str)] = &[
    ("audio.flac", "thumbnail"),
    ("audio.m4a", "thumbnail"),
    ("audio.mp3", "thumbnail"),
    ("audio.ogg", "thumbnail"),
    ("audio.opus", "thumbnail"),
    ("audio.wav", "thumbnail"),
    ("captions.srt", "iiif"),
    ("captions.srt", "thumbnail"),
    ("captions.vtt", "iiif"),
    ("captions.vtt", "thumbnail"),
    ("doc.docx", "iiif"),
    ("doc.docx", "thumbnail"),
    ("image.avif", "iiif"),
    ("image.heic", "iiif"),
    ("model.glb", "iiif"),
    ("model.glb", "thumbnail"),
    ("model.gltf", "iiif"),
    ("model.gltf", "thumbnail"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Image,
    Svg,
    Audio,
    Video,
    Pdf,
    Document,
    Model,
    Captions,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cell {
    Ingest,
    Annotate,
    Thumbnail,
    Api,
    SiteMediaPage,
    SiteItemCard,
    Iiif,
    IiifNotes,
    Safe,
    Captions,
}

const CELLS: [Cell; 10] = [
    Cell::Ingest,
    Cell::Annotate,
    Cell::Thumbnail,
    Cell::Api,
    Cell::SiteMediaPage,
    Cell::SiteItemCard,
    Cell::Iiif,
    Cell::IiifNotes,
    Cell::Safe,
    Cell::Captions,
];

impl Cell {
    const fn label(self) -> &'static str {
        match self {
            Self::Ingest => "ingest",
            Self::Annotate => "annotate",
            Self::Thumbnail => "thumbnail",
            Self::Api => "api",
            Self::SiteMediaPage => "site-media-page",
            Self::SiteItemCard => "site-item-card",
            Self::Iiif => "iiif",
            Self::IiifNotes => "iiif-notes",
            Self::Safe => "safe",
            Self::Captions => "captions",
        }
    }
}

/// One corpus file and what its media is. `media_type` is the type a correct
/// server answers for the bytes, which is not always the current answer.
struct Fixture {
    file: &'static str,
    media_type: &'static str,
    kind: Kind,
    /// The video.mp4 item also carries captions.vtt, as a curator would attach
    /// a caption file to the video it transcribes.
    captions: bool,
    /// The large derivative must be portrait: the original is landscape pixels
    /// with EXIF Orientation 6, so an upright derivative is 90 degrees turned.
    upright: bool,
}

const FIXTURES: &[Fixture] = &[
    Fixture {
        file: "image.jpg",
        media_type: "image/jpeg",
        kind: Kind::Image,
        captions: false,
        upright: true,
    },
    Fixture {
        file: "image.png",
        media_type: "image/png",
        kind: Kind::Image,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "image.gif",
        media_type: "image/gif",
        kind: Kind::Image,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "image.webp",
        media_type: "image/webp",
        kind: Kind::Image,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "image.tif",
        media_type: "image/tiff",
        kind: Kind::Image,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "image.nef",
        media_type: "image/x-nikon-nef",
        kind: Kind::Image,
        captions: false,
        upright: true,
    },
    Fixture {
        file: "image.avif",
        media_type: "image/avif",
        kind: Kind::Image,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "image.heic",
        media_type: "image/heic",
        kind: Kind::Image,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "image.svg",
        media_type: "image/svg+xml",
        kind: Kind::Svg,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "hostile.svg",
        media_type: "image/svg+xml",
        kind: Kind::Svg,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "audio.mp3",
        media_type: "audio/mpeg",
        kind: Kind::Audio,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "audio.m4a",
        media_type: "audio/mp4",
        kind: Kind::Audio,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "audio.ogg",
        media_type: "audio/ogg",
        kind: Kind::Audio,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "audio.opus",
        media_type: "audio/ogg",
        kind: Kind::Audio,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "audio.flac",
        media_type: "audio/flac",
        kind: Kind::Audio,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "audio.wav",
        media_type: "audio/x-wav",
        kind: Kind::Audio,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "video.mp4",
        media_type: "video/mp4",
        kind: Kind::Video,
        captions: true,
        upright: false,
    },
    Fixture {
        file: "video.webm",
        media_type: "video/webm",
        kind: Kind::Video,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "video.mov",
        media_type: "video/quicktime",
        kind: Kind::Video,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "video.mkv",
        media_type: "video/x-matroska",
        kind: Kind::Video,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "video.ogv",
        media_type: "video/ogg",
        kind: Kind::Video,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "doc.pdf",
        media_type: "application/pdf",
        kind: Kind::Pdf,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "doc.docx",
        media_type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        kind: Kind::Document,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "model.glb",
        media_type: "model/gltf-binary",
        kind: Kind::Model,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "model.gltf",
        media_type: "model/gltf+json",
        kind: Kind::Model,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "captions.vtt",
        media_type: "text/vtt",
        kind: Kind::Captions,
        captions: false,
        upright: false,
    },
    Fixture {
        file: "captions.srt",
        media_type: "application/x-subrip",
        kind: Kind::Captions,
        captions: false,
        upright: false,
    },
];

/// What one corpus file's server answers looked like.
struct Observed {
    status: StatusCode,
    item: Option<u64>,
    media: Option<u64>,
    media_doc: Option<Value>,
    /// What the Web Annotation Protocol answered for this media.
    notes: Option<Notes>,
    page: Option<String>,
    item_page: Option<String>,
    manifest: Option<Value>,
    /// The page the media's canvas references, read as a visitor after the
    /// manifest named it.
    notes_page: Option<Value>,
    original: Option<Vec<u8>>,
    bytes: Vec<u8>,
    /// The answer to each `<track src>` the media page holds.
    tracks: Vec<Answer>,
}

struct Row {
    file: &'static str,
    cells: Vec<Option<bool>>,
}

/// What the two annotation writes and the container answered.
struct Notes {
    /// The server's own answer to what a selector may address.
    surface: Option<String>,
    whole: StatusCode,
    selector: StatusCode,
    accepted: u32,
    total: Option<u64>,
}

#[tokio::test]
async fn the_media_kind_matrix_matches_the_known_red_list() {
    // A poster needs ffmpeg, a PDF page needs pdftoppm, and a RAW preview
    // needs exiftool. The runtime image installs all three
    // (`packaging/Dockerfile`); a host without them cannot say whether these
    // cells pass, so it stops before judging any of them.
    let tools = omeka_media::tool::Tools::on_path();
    for (tool, path) in [
        ("ffmpeg", tools.ffmpeg()),
        ("pdftoppm", tools.pdftoppm()),
        ("exiftool", tools.exiftool()),
    ] {
        assert!(
            path.is_some(),
            "the matrix needs {tool} on PATH; install ffmpeg, poppler-utils and exiftool"
        );
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let app = router_with_store(pool, store.clone());

    let mut observed = Vec::new();
    for fixture in FIXTURES {
        observed.push((fixture, upload(&app, &store, &seed, fixture).await));
    }
    // Every item was attached as it was created, so one browse page carries
    // every card.
    let browse = send(
        app.clone(),
        get(&format!("{SITE}/item?per_page=200"), false),
    )
    .await
    .body;
    let css = stylesheet(&app, &browse).await;

    let frames = avif_and_heic_decode();
    let mut rows = Vec::new();
    for (fixture, seen) in &observed {
        let cells = CELLS
            .iter()
            .map(|cell| judge(fixture, *cell, seen, &browse, &css, &store, frames))
            .collect();
        rows.push(Row {
            file: fixture.file,
            cells,
        });
    }

    let table = render_table(&rows);
    let actual: BTreeSet<(&str, &str)> = rows
        .iter()
        .flat_map(|row| {
            row.cells.iter().enumerate().filter_map(|(index, cell)| {
                (*cell == Some(false)).then_some((row.file, CELLS[index].label()))
            })
        })
        .collect();
    let expected: BTreeSet<(&str, &str)> = KNOWN_RED.iter().copied().collect();

    if std::env::var("MEDIA_MATRIX_PRINT").as_deref() == Ok("1") {
        println!("{table}");
        let baseline = baseline_document(&rows, &actual);
        std::fs::write(corpus_dir().join("BASELINE.md"), baseline).expect("write BASELINE.md");
    }

    if actual != expected {
        println!("{table}");
        let unlisted: Vec<_> = actual.difference(&expected).collect();
        let stale: Vec<_> = expected.difference(&actual).collect();
        panic!(
            "the matrix changed.\nunlisted red cells: {unlisted:?}\nlisted cells that pass: {stale:?}"
        );
    }
}

/// An item with no file says "No file" where the kind would stand, and an
/// audio item's card names its kind in words and carries the server's kind.
#[tokio::test]
async fn a_card_names_its_kind_and_an_item_with_no_file_says_so() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let app = router_with_store(pool, store.clone());
    let audio = FIXTURES
        .iter()
        .find(|fixture| fixture.file == "audio.mp3")
        .expect("audio.mp3 in the corpus");
    let seen = upload(&app, &store, &seed, audio).await;
    let audio_item = seen.item.expect("the audio item");
    let data = json!({
        "o:is_public": true,
        "dcterms:title": [{"type": "literal", "@value": "No file"}],
    })
    .to_string();
    let answer = send(
        app.clone(),
        json_request("POST", &append_key("/api/items"), &data),
    )
    .await;
    assert!(answer.status.is_success(), "{}", answer.body);
    let bare = answer.json()["o:id"].as_u64().expect("the bare item's id");
    attach(&app, &seed, bare).await;
    let browse = send(
        app.clone(),
        get(&format!("{SITE}/item?per_page=200"), false),
    )
    .await
    .body;

    let audio_card = card_for(&browse, audio_item).expect("the audio item's card");
    assert_eq!(card_media_kind(audio_card), Some("audio"), "{audio_card}");
    assert!(
        audio_card.contains("<span class=\"resource-kind\">Audio</span>"),
        "{audio_card}"
    );

    let bare_card = card_for(&browse, bare).expect("the bare item's card");
    assert_eq!(card_media_kind(bare_card), None, "{bare_card}");
    assert!(
        bare_card.contains("<span class=\"resource-kind\">No file</span>"),
        "{bare_card}"
    );
}

/// A card shows a recording's run time only once `romeka_media_extent` holds a
/// row for the original. The extent unit fills that table; this unit only
/// reads it, and a row seeded here proves the badge waits for one.
#[tokio::test]
async fn a_card_shows_a_run_time_only_when_the_extent_row_exists() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = seeded_pool().await;
    let app = router_with_store(pool.clone(), store.clone());
    let audio = FIXTURES
        .iter()
        .find(|fixture| fixture.file == "audio.mp3")
        .expect("audio.mp3 in the corpus");
    let seen = upload(&app, &store, &seed, audio).await;
    let item = seen.item.expect("the audio item");
    let media = seen.media.expect("the audio media");
    let filename = seen
        .media_doc
        .as_ref()
        .and_then(|doc| doc["o:filename"].as_str())
        .expect("the stored file's name")
        .to_string();

    // `upload` reads the manifest, which probes the file and stores its extent; remove it so the card
    // starts with no run time to show.
    sqlx::query("DELETE FROM romeka_media_extent")
        .execute(&pool)
        .await
        .expect("clear the probed extent");
    let before = send(
        app.clone(),
        get(&format!("{SITE}/item?per_page=200"), false),
    )
    .await
    .body;
    let card = card_for(&before, item).expect("the audio item's card");
    assert!(!card.contains("resource-duration"), "{card}");

    let pool = DbPool::from(pool);
    with_db!(&pool, |db| MediaExtentRepo::new(db)
        .put(
            omeka_db::ids::MediaId::from(media),
            &format!("original/{filename}"),
            &ExtentRow {
                duration_ms: Some(222_000),
                width: None,
                height: None,
            },
        )
        .await)
    .expect("seed the extent row");

    let after = send(
        app.clone(),
        get(&format!("{SITE}/item?per_page=200"), false),
    )
    .await
    .body;
    let card = card_for(&after, item).expect("the audio item's card");
    assert!(
        card.contains("<span class=\"resource-duration\">"),
        "the card shows the run time: {card}"
    );
    assert!(card.contains("▶</span> 3:42"), "{card}");
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/media-kinds")
}

/// Whether this host's `ffmpeg` reads the AVIF and HEIC corpus files as stills.
///
/// Where it does not, those two files' derivative cells do not apply rather
/// than failing: the shipped Debian 12 image carries ffmpeg 5.1, whose probe
/// reads no HEIC still (docs/compat.md), so the answer is the tool's, not the
/// server's, and the cell says so.
fn avif_and_heic_decode() -> bool {
    let tools = omeka_media::tool::Tools::on_path();
    ["image.avif", "image.heic"].iter().all(|name| {
        omeka_media::tool::Still::Frame
            .make(tools, &corpus_dir().join(name))
            .is_ok()
    })
}

/// An SVG the filter cannot parse is refused with the 422 document, under
/// `file`, and no item is created. A filtered SVG is stored rewritten, so an
/// unreadable one is never stored as it arrived.
#[tokio::test]
async fn an_svg_that_is_not_xml_is_refused_at_ingest() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, _seed) = seeded_pool().await;
    let app = router_with_store(pool, store);
    let before = send(app.clone(), get("/api/items", true)).await.json();
    let data = json!({
        "o:is_public": true,
        "dcterms:title": [{"type": "literal", "@value": "Broken"}],
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
            "broken.svg",
            b"<svg xmlns=\"http://www.w3.org/2000/svg\"><rect",
            &data,
        ),
    )
    .await;
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
    let errors = answer.json();
    let message = errors["errors"]["o:media"][0]["file"][0]
        .as_str()
        .unwrap_or_else(|| panic!("{}", errors));
    assert!(message.contains("did not parse"), "{message}");
    let after = send(app, get("/api/items", true)).await.json();
    assert_eq!(
        after["o:total_results"], before["o:total_results"],
        "{after}"
    );
}

/// The registry above is the corpus: a file added without a row, or a row
/// without a file, fails here before the matrix runs.
#[test]
fn every_corpus_file_has_a_row_and_every_row_has_a_file() {
    let mut files: Vec<String> = std::fs::read_dir(corpus_dir())
        .expect("fixtures/media-kinds")
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            (!matches!(name.as_str(), "make.sh" | "README.md" | "BASELINE.md")
                && !name.starts_with('.'))
            .then_some(name)
        })
        .collect();
    files.sort();
    let mut registered: Vec<&str> = FIXTURES.iter().map(|fixture| fixture.file).collect();
    registered.sort_unstable();
    assert_eq!(files, registered);
}

fn read_fixture(name: &str) -> Vec<u8> {
    std::fs::read(corpus_dir().join(name)).unwrap_or_else(|error| panic!("{name}: {error}"))
}

/// Upload one fixture as the file media of a new item, attach the item to the
/// seeded site, and read back what the API, the site and IIIF answer.
async fn upload(app: &Router, store: &FileStore, seed: &Seed, fixture: &Fixture) -> Observed {
    let bytes = read_fixture(fixture.file);
    let data = json!({
        "o:is_public": true,
        "dcterms:title": [{"type": "literal", "@value": fixture.file}],
        "o:media": [{
            "o:ingester": "upload",
            "o:is_public": true,
            "file_index": 0,
            "o:alt_text": format!("The {} corpus file", fixture.file),
        }],
    })
    .to_string();
    let answer = send(
        app.clone(),
        multipart_asset_request(&append_key("/api/items"), fixture.file, &bytes, &data),
    )
    .await;
    let mut observed = Observed {
        status: answer.status,
        item: None,
        media: None,
        media_doc: None,
        notes: None,
        page: None,
        item_page: None,
        manifest: None,
        notes_page: None,
        original: None,
        bytes,
        tracks: Vec::new(),
    };
    if !answer.status.is_success() {
        return observed;
    }
    let created = answer.json();
    let item = created["o:id"].as_u64();
    let media = created["o:media"][0]["o:id"].as_u64();
    observed.item = item;
    observed.media = media;
    let Some(item_id) = item else {
        return observed;
    };
    attach(app, seed, item_id).await;
    observed.item_page = Some(
        send(app.clone(), get(&format!("{SITE}/item/{item_id}"), false))
            .await
            .body,
    );
    if fixture.captions
        && let Some(video) = media
    {
        attach_captions(app, item_id, video).await;
    }
    if let Some(media_id) = media {
        // Before the manifest and the page are read, so a later cell sees the
        // notes this cell wrote.
        observed.notes = Some(add_notes(app, item_id, media_id, fixture).await);
    }
    if let Some(media_id) = media {
        observed.media_doc = Some(
            send(app.clone(), get(&format!("/api/media/{media_id}"), true))
                .await
                .json(),
        );
        let page = send(app.clone(), get(&format!("{SITE}/media/{media_id}"), false))
            .await
            .body;
        if fixture.captions {
            observed.tracks = track_answers(app, &page).await;
        }
        observed.page = Some(page);
    }
    observed.manifest = Some(
        send(
            app.clone(),
            get(&format!("/iiif/3/item/{item_id}/manifest"), false),
        )
        .await
        .json(),
    );
    if let (Some(manifest), Some(media_id)) = (&observed.manifest, media) {
        observed.notes_page = notes_page(app, manifest, media_id).await;
    }
    observed.original = observed
        .media_doc
        .as_ref()
        .and_then(|doc| doc["o:filename"].as_str())
        .and_then(|name| store.read(&format!("original/{name}")).ok());
    observed
}

/// Put the new item on the seeded site so its card reaches the browse.
async fn attach(app: &Router, seed: &Seed, item: u64) {
    let body = json!({"action": "add", "items": [item]}).to_string();
    let answer = send(
        app.clone(),
        json_request(
            "POST",
            &append_key(&format!("/api/sites/{}/items", seed.site)),
            &body,
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
}

/// Attach `captions.vtt` to the item as a second media, then post both caption
/// files to the video's tracks route: the curator's two steps.
async fn attach_captions(app: &Router, item: u64, video: u64) {
    let captions = read_fixture("captions.vtt");
    let data = json!({
        "o:ingester": "upload",
        "o:is_public": true,
        "file_index": 0,
        "o:item": {"o:id": item},
        // The video is position 0 (its index in `o:media`). A media created
        // without a position sorts before it, so the item's card would read as
        // captions; naming the next position keeps the video the first file.
        "position": 1,
    })
    .to_string();
    let _ = send(
        app.clone(),
        multipart_asset_request(&append_key("/api/media"), "captions.vtt", &captions, &data),
    )
    .await;
    for (file, lang, content_type) in [
        ("captions.vtt", "en", "text/vtt"),
        ("captions.srt", "fr", "application/x-subrip"),
    ] {
        let answer = send(
            app.clone(),
            caption_request(video, lang, &read_fixture(file), content_type),
        )
        .await;
        assert_eq!(
            answer.status,
            StatusCode::OK,
            "{file} for {lang}: {}",
            answer.body
        );
    }
}

/// One caption file posted to a media's tracks route, with the seeded key pair.
fn caption_request(media: u64, lang: &str, body: &[u8], content_type: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(append_key(&format!(
            "/annotations/media/{media}/tracks?lang={lang}"
        )))
        .header("content-type", content_type)
        .body(Body::from(body.to_vec()))
        .unwrap()
}

/// GET every `<track src>` the media page holds, under this run's origin.
async fn track_answers(app: &Router, page: &str) -> Vec<Answer> {
    let mut answers = Vec::new();
    for part in page.split("<track").skip(1) {
        let Some(src) = part
            .split("src=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
        else {
            continue;
        };
        let path = src.strip_prefix("http://localhost").unwrap_or(src);
        answers.push(send(app.clone(), get(path, false)).await);
    }
    answers
}

/// The video page offers both languages as WebVTT the browser can read.
fn captions(seen: &Observed) -> bool {
    let Some(page) = &seen.page else {
        return false;
    };
    let sources: Vec<&str> = page
        .split("<track")
        .skip(1)
        .filter_map(|part| part.split("src=\"").nth(1))
        .filter_map(|rest| rest.split('"').next())
        .collect();
    sources.len() == 2
        && sources.iter().all(|src| src.contains(".vtt"))
        && seen.tracks.len() == 2
        && seen.tracks.iter().all(|answer| {
            answer.status == StatusCode::OK
                && header(answer, "content-type").is_some_and(|kind| kind.starts_with("text/vtt"))
                && answer.body.starts_with("WEBVTT")
                && answer.body.contains("A red square, then a blue bar.")
        })
}

/// The theme's own sheet, read from the href the page links.
async fn stylesheet(app: &Router, page: &str) -> String {
    let Some(path) = page.split("href=\"").skip(1).find_map(|part| {
        let url = part.split('"').next()?;
        url.contains("css/style.css").then_some(url)
    }) else {
        return String::new();
    };
    send(app.clone(), get(path, false)).await.body
}

/// One cell, or `None` when the cell does not apply to this kind.
fn judge(
    fixture: &Fixture,
    cell: Cell,
    seen: &Observed,
    browse: &str,
    css: &str,
    store: &FileStore,
    frames: bool,
) -> Option<bool> {
    if !frames
        && matches!(fixture.file, "image.avif" | "image.heic")
        && matches!(cell, Cell::Thumbnail | Cell::SiteMediaPage)
    {
        return None;
    }
    match cell {
        Cell::Ingest => Some(
            seen.status.is_success()
                && seen
                    .media_doc
                    .as_ref()
                    .and_then(|doc| doc["o:media_type"].as_str())
                    == Some(fixture.media_type),
        ),
        Cell::Annotate => Some(annotate(fixture, seen)),
        Cell::Thumbnail => Some(thumbnail(fixture, seen, store)),
        Cell::Api => Some(api_cell(seen)),
        Cell::SiteMediaPage => Some(site_media_page(fixture, seen, css)),
        Cell::SiteItemCard => Some(site_item_card(seen, browse)),
        Cell::Iiif => Some(iiif(fixture, seen)),
        Cell::IiifNotes => iiif_notes(seen),
        Cell::Safe => (fixture.file == "hostile.svg").then(|| safe(seen)),
        Cell::Captions => (fixture.file == "video.mp4").then(|| captions(seen)),
    }
}

/// A whole note is accepted on every kind; then the container's `Romeka-Surface`
/// decides the second write: a pixel box and a time range are accepted on the
/// surface that takes them, and a kind that only takes whole-file notes answers
/// 400. The cell reads the server's own surface, so a kind whose derivative is
/// not made yet (an AVIF with no thumbnail, a PDF with no first page) is judged
/// by what the server says it accepts, not by its file extension. The
/// container's total is the number of writes the caller saw accepted.
fn annotate(_fixture: &Fixture, seen: &Observed) -> bool {
    let Some(notes) = &seen.notes else {
        return false;
    };
    let Some(surface) = notes.surface.as_deref() else {
        return false;
    };
    if notes.whole != StatusCode::CREATED {
        return false;
    }
    let second = if surface.starts_with("image") || surface.starts_with("time") {
        StatusCode::CREATED
    } else if surface == "whole" {
        StatusCode::BAD_REQUEST
    } else {
        return false;
    };
    notes.selector == second && notes.total == Some(u64::from(notes.accepted))
}

/// Write one whole note and one selector note through the WAP, then read the
/// container's total.
async fn add_notes(app: &Router, item: u64, media: u64, fixture: &Fixture) -> Notes {
    let canvas = format!("http://localhost/iiif/3/item/{item}/canvas/{media}");
    let whole = note(&canvas, "The whole file", None);
    let whole = post_note(app, media, &whole).await.status;
    let selector = match fixture.kind {
        Kind::Image | Kind::Svg => json!({
            "type": "FragmentSelector",
            "conformsTo": "http://www.w3.org/TR/media-frags/",
            "value": "xywh=pixel:1,2,10,20",
        }),
        Kind::Audio | Kind::Video => json!({
            "type": "FragmentSelector",
            "conformsTo": "http://www.w3.org/TR/media-frags/",
            "value": "t=0,0.2",
        }),
        Kind::Pdf | Kind::Document | Kind::Model | Kind::Captions => json!({
            "type": "FragmentSelector",
            "conformsTo": "http://www.w3.org/TR/media-frags/",
            "value": "xywh=pixel:1,2,10,20",
        }),
    };
    let selector = post_note(app, media, &note(&canvas, "A second note", Some(selector)))
        .await
        .status;
    let accepted =
        u32::from(whole == StatusCode::CREATED) + u32::from(selector == StatusCode::CREATED);
    let container = send(
        app.clone(),
        get(&format!("/annotations/media/{media}/"), true),
    )
    .await;
    let ok = container.status == StatusCode::OK;
    let surface = ok
        .then(|| common::header(&container, "romeka-surface").map(str::to_string))
        .flatten();
    let total = ok.then(|| container.json()["total"].as_u64()).flatten();
    Notes {
        surface,
        whole,
        selector,
        accepted,
        total,
    }
}

/// One WAP create as the seeded administrator, with the media canvas as target.
async fn post_note(app: &Router, media: u64, body: &str) -> Answer {
    send(
        app.clone(),
        json_request(
            "POST",
            &append_key(&format!("/annotations/media/{media}/")),
            body,
        ),
    )
    .await
}

/// One annotation document: a whole-object note, or one with a selector.
fn note(canvas: &str, text: &str, selector: Option<Value>) -> String {
    let target = match selector {
        None => json!(canvas),
        Some(selector) => json!({
            "type": "SpecificResource",
            "source": { "id": canvas },
            "selector": selector,
        }),
    };
    json!({
        "@context": "http://www.w3.org/ns/anno.jsonld",
        "type": "Annotation",
        "motivation": "commenting",
        "body": { "type": "TextualBody", "value": text, "format": "text/plain" },
        "target": target,
    })
    .to_string()
}

/// `o:thumbnail_urls.large` is a stored derivative that decodes. `image.jpg`
/// must decode upright, because its EXIF Orientation is 6.
fn thumbnail(fixture: &Fixture, seen: &Observed, store: &FileStore) -> bool {
    let Some(doc) = &seen.media_doc else {
        return false;
    };
    let Some(url) = doc["o:thumbnail_urls"]["large"].as_str() else {
        return false;
    };
    let Some(path) = url.split("/files/").nth(1) else {
        return false;
    };
    let Ok(bytes) = store.read(path) else {
        return false;
    };
    if ImageThumbnailer::new()
        .derivative_bytes(&bytes, Some("image/jpeg"))
        .is_err()
    {
        return false;
    }
    if fixture.upright {
        let Some((width, height)) = dimensions(&bytes) else {
            return false;
        };
        return height > width;
    }
    true
}

fn dimensions(bytes: &[u8]) -> Option<(u64, u64)> {
    let tags = image_metadata(&mut Cursor::new(bytes));
    let width = tags.get("Width")?.values.first()?.parse().ok()?;
    let height = tags.get("Height")?.values.first()?.parse().ok()?;
    Some((width, height))
}

/// The API carries the type and the byte size the ingester recorded. A
/// rewritten original (an SVG is filtered before it is stored) is measured as
/// stored, not as uploaded.
fn api_cell(seen: &Observed) -> bool {
    let Some(doc) = &seen.media_doc else {
        return false;
    };
    let size = seen.original.as_ref().map_or(seen.bytes.len(), Vec::len);
    doc["o:media_type"].is_string() && doc["o:size"].as_u64() == Some(size as u64)
}

/// The public media page renders the element the kind needs, and an image is
/// not cover-cropped by the theme's sheet.
fn site_media_page(fixture: &Fixture, seen: &Observed, css: &str) -> bool {
    let Some(page) = &seen.page else {
        return false;
    };
    let Some(doc) = &seen.media_doc else {
        return false;
    };
    let element = match fixture.kind {
        Kind::Image | Kind::Svg => has_media_image(page, doc),
        Kind::Audio => has_tag(page, "<audio", &["controls", "src=\""]),
        Kind::Video => {
            let video = has_tag(page, "<video", &["controls", "poster=\""]);
            let track = !fixture.captions
                || page
                    .split("<track")
                    .skip(1)
                    .any(|tag| tag.contains("src=\"") && tag.contains(".vtt"));
            video && track
        }
        Kind::Pdf => {
            // The file's own page draws the browser's reader and the card; an
            // item page draws one card per file and no reader at all.
            has_pdf_object(page)
                && page.contains("class=\"pdf-card\"")
                && seen
                    .item_page
                    .as_deref()
                    .is_some_and(|item| !has_pdf_object(item))
        }
        Kind::Document | Kind::Captions => has_file_link(page, doc),
        Kind::Model => has_model_reader(page),
    };
    let crop = matches!(fixture.kind, Kind::Image | Kind::Svg) && cover_crop(css);
    // Only a page that shows a 3D file carries the viewer's facade, so a page
    // of images, recordings or documents never names it.
    let model_only = matches!(fixture.kind, Kind::Model)
        || !(page.contains("<model-viewer") || page.contains("/assets/js/model.js"));
    element && !crop && model_only
}

/// The model page offers the vendored viewer behind a button that states the
/// download, and the element's source waits for that press. The facade is the
/// theme's script; the library is named on the button's root, not loaded.
fn has_model_reader(page: &str) -> bool {
    page.contains("<script src=\"/assets/js/model.js?v=")
        && page.contains("<model-viewer")
        && page.contains("data-library=\"/assets/vendor/model-viewer/model-viewer-umd.min.js?v=")
        && page.contains("data-src=\"")
        && page.contains("/files/original/")
        && page.contains("class=\"model-turn\"")
        && !page.contains("<script src=\"/assets/vendor/model-viewer/")
}

/// An `<img>` of the media itself, with a non-empty alt.
fn has_media_image(page: &str, doc: &Value) -> bool {
    let Some(filename) = doc["o:filename"].as_str() else {
        return false;
    };
    let stem = filename.rsplit_once('.').map_or(filename, |(stem, _)| stem);
    page.split("<img")
        .skip(1)
        .map(|tag| tag.split('>').next().unwrap_or(tag))
        .any(|tag| {
            let own = tag.split("src=\"").nth(1).is_some_and(|src| {
                let src = src.split('"').next().unwrap_or_default();
                src.contains("/files/large/")
                    || (src.contains("/files/original/") && src.contains(stem))
            });
            let alt = tag
                .split("alt=\"")
                .nth(1)
                .is_some_and(|alt| !alt.split('"').next().unwrap_or_default().is_empty());
            own && alt
        })
}

fn has_tag(page: &str, open: &str, attributes: &[&str]) -> bool {
    page.split(open)
        .skip(1)
        .map(|tag| tag.split('>').next().unwrap_or(tag))
        .any(|tag| attributes.iter().all(|attribute| tag.contains(attribute)))
}

/// A PDF is shown by the browser's own viewer: an object, embed or iframe
/// whose source is the file.
fn has_pdf_object(page: &str) -> bool {
    ["<object", "<embed", "<iframe"].iter().any(|open| {
        page.split(open)
            .skip(1)
            .map(|tag| tag.split('>').next().unwrap_or(tag))
            .any(|tag| tag.to_ascii_lowercase().contains("pdf"))
    })
}

/// A file that is not drawn but read: a link to the stored original.
fn has_file_link(page: &str, doc: &Value) -> bool {
    let Some(filename) = doc["o:filename"].as_str() else {
        return false;
    };
    page.split("<a")
        .skip(1)
        .map(|tag| tag.split('>').next().unwrap_or(tag))
        .any(|tag| {
            tag.contains("href=\"") && tag.contains("/files/original/") && tag.contains(filename)
        })
}

/// The item's card in the public browse. The card carries the server's kind
/// for its first file in `data-media-kind`, names that kind in words (in the
/// empty slot, or on the corner badge of a picture that is not an image), and
/// never draws a non-image original as an `<img>`. An item with no file says
/// "No file" and carries no kind.
fn site_item_card(seen: &Observed, browse: &str) -> bool {
    let Some(item) = seen.item else {
        return false;
    };
    let Some(card) = card_for(browse, item) else {
        return false;
    };
    let kind = seen
        .media_doc
        .as_ref()
        .and_then(|doc| doc["o:kind"].as_str())
        .unwrap_or_default();
    if kind.is_empty() || card_media_kind(card) != Some(kind) {
        return false;
    }
    let word = kind_word(kind);
    let images: Vec<&str> = card
        .split("<img")
        .skip(1)
        .filter_map(|tag| tag.split('>').next())
        .filter_map(|tag| tag.split("src=\"").nth(1))
        .filter_map(|src| src.split('"').next())
        .collect();
    let picture_ok = if images.is_empty() {
        card.contains("resource-thumb-empty")
    } else {
        images.iter().all(|src| image_url(src))
    };
    let named = names_the_kind(card, kind, word, images.is_empty());
    picture_ok && named
}

/// The `<li>` that holds one item's card in a browse page.
fn card_for(browse: &str, item: u64) -> Option<&str> {
    let needle = format!("{SITE}/item/{item}\"");
    let at = browse.find(&needle)?;
    let start = browse[..at].rfind("<li ")?;
    let end = browse[at..].find("</li>").map(|end| at + end + 5)?;
    Some(&browse[start..end])
}

/// The `data-media-kind` a card carries, if any.
fn card_media_kind(card: &str) -> Option<&str> {
    card.split("data-media-kind=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
}

/// The kind in the site's English words, as the card must say it.
fn kind_word(kind: &str) -> &'static str {
    match kind {
        "local-image" | "iiif-image" => "Image",
        "audio" => "Audio",
        "video" => "Video",
        "pdf" => "PDF",
        "model-3d" => "3D model",
        "captions" => "Captions",
        "embed" | "iiif-presentation" => "Embed",
        "other-file" => "File",
        other => panic!("the card has no word for kind {other}"),
    }
}

/// Whether the card names the kind in words where a reader can see it: the
/// empty slot's label, or the badge on a picture of a non-image file. An
/// image's own picture is its cue, so an image card needs no badge.
fn names_the_kind(card: &str, kind: &str, word: &str, placeholder: bool) -> bool {
    if placeholder {
        return card.contains(&format!("<span class=\"resource-kind\">{word}</span>"));
    }
    if matches!(kind, "local-image" | "iiif-image") {
        return true;
    }
    card.contains("class=\"resource-badge\"") && card.contains(word)
}

/// A URL that serves an image: a derivative, the theme's placeholder, or an
/// original with an image extension.
fn image_url(url: &str) -> bool {
    if url.contains("/application/asset/thumbnails/") {
        return true;
    }
    if url.contains("/files/") && !url.contains("/files/original/") {
        return true;
    }
    let extension = url
        .split('?')
        .next()
        .and_then(|url| url.rsplit_once('.'))
        .map(|(_, extension)| extension.to_ascii_lowercase());
    matches!(
        extension.as_deref(),
        Some("jpg" | "jpeg" | "png" | "gif" | "webp" | "avif" | "tif" | "tiff" | "svg")
    )
}

/// The manifest gives the media a canvas that paints what the kind is: an
/// image, or a Sound/Video with a duration. A still canvas for a file that is
/// not a still image does not count.
fn iiif(fixture: &Fixture, seen: &Observed) -> bool {
    let (Some(media), Some(manifest)) = (seen.media, &seen.manifest) else {
        return false;
    };
    manifest["items"].as_array().is_some_and(|items| {
        items.iter().any(|canvas| {
            let ours = canvas["id"]
                .as_str()
                .is_some_and(|id| id.ends_with(&format!("/canvas/{media}")));
            let body = &canvas["items"][0]["items"][0]["body"];
            let timed = || {
                body["duration"]
                    .as_f64()
                    .is_some_and(|seconds| seconds > 0.0)
            };
            ours && match fixture.kind {
                Kind::Image | Kind::Svg => body["type"] == "Image",
                Kind::Audio => body["type"] == "Sound" && timed(),
                Kind::Video => body["type"] == "Video" && timed(),
                // A PDF's pages are canvases too, but only once a unit
                // renders them; a model and a caption paint nothing.
                Kind::Pdf | Kind::Document | Kind::Model | Kind::Captions => {
                    body["type"] == "Image"
                }
            }
        })
    })
}

/// The canvas references the page its notes live on: `annotations[0].id`
/// dereferences to an AnnotationPage, and every item of that page targets the
/// canvas. A kind with no canvas has no cell, and a server that wrote a canvas
/// for a media with a note but no reference is red.
fn iiif_notes(seen: &Observed) -> Option<bool> {
    let Some(manifest) = &seen.manifest else {
        return None;
    };
    let media = seen.media?;
    let canvas = manifest["items"].as_array()?.iter().find(|canvas| {
        canvas["id"]
            .as_str()
            .is_some_and(|id| id.ends_with(&format!("/canvas/{media}")))
    })?;
    let canvas_id = canvas["id"].as_str().unwrap_or_default();
    let Some(page) = &seen.notes_page else {
        return Some(false);
    };
    let items = page["items"].as_array();
    Some(
        page["type"] == "AnnotationPage"
            && page["id"] == canvas["annotations"][0]["id"]
            && items.is_some_and(|items| {
                !items.is_empty()
                    && items
                        .iter()
                        .all(|item| target_names(&item["target"], canvas_id))
            }),
    )
}

/// Whether a W3C target names `canvas`: a bare IRI for a whole-object note, or
/// a `SpecificResource` whose source is the canvas.
fn target_names(target: &Value, canvas: &str) -> bool {
    target.as_str() == Some(canvas) || target["source"]["id"].as_str() == Some(canvas)
}

/// The page `manifest` references for `media`, read as a visitor.
async fn notes_page(app: &Router, manifest: &Value, media: u64) -> Option<Value> {
    let canvas = manifest["items"].as_array()?.iter().find(|canvas| {
        canvas["id"]
            .as_str()
            .is_some_and(|id| id.ends_with(&format!("/canvas/{media}")))
    })?;
    let page = canvas["annotations"][0]["id"].as_str()?;
    let answer = send(app.clone(), get(page, false)).await;
    (answer.status == StatusCode::OK).then(|| answer.json())
}

/// The hostile SVG is never served with a script reachable. A file the server
/// refused proves nothing about what it would serve, so the cell stays red
/// until the file is accepted and its stored original can be read.
fn safe(seen: &Observed) -> bool {
    let Some(original) = &seen.original else {
        return false;
    };
    let text = String::from_utf8_lossy(original).to_ascii_lowercase();
    if ["<script", "onload=", "javascript:", "<foreignobject"]
        .iter()
        .any(|marker| text.contains(marker))
    {
        return false;
    }
    let page = seen
        .page
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    ![
        "alert('script')",
        "alert('onload')",
        "javascript:",
        "<foreignobject",
    ]
    .iter()
    .any(|marker| page.contains(marker))
}

/// Whether a rule a media page's main image matches crops it with
/// `object-fit: cover`.
fn cover_crop(css: &str) -> bool {
    css.split('}')
        .filter_map(|block| block.split_once('{'))
        .any(|(selectors, body)| {
            let crops = body.split(';').any(|declaration| {
                declaration
                    .trim()
                    .replace(' ', "")
                    .starts_with("object-fit:cover")
            });
            crops
                && selectors.split(',').any(|selector| {
                    let selector = selector.trim();
                    selector.starts_with(".media-page") && selector.contains("img")
                })
        })
}

fn render_table(rows: &[Row]) -> String {
    let mut out = String::from("| kind |");
    for cell in CELLS {
        out.push_str(&format!(" {} |", cell.label()));
    }
    out.push_str("\n| --- |");
    for _ in CELLS {
        out.push_str(" --- |");
    }
    out.push('\n');
    for row in rows {
        out.push_str(&format!("| {} |", row.file));
        for cell in &row.cells {
            out.push_str(match cell {
                Some(true) => " pass |",
                Some(false) => " **fail** |",
                None => " n/a |",
            });
        }
        out.push('\n');
    }
    out
}

fn baseline_document(rows: &[Row], red: &BTreeSet<(&str, &str)>) -> String {
    let mut out = String::from(
        "# Media matrix baseline\n\n\
         One row per file in this directory, one column per part of Romeka. Each cell is what\n\
         the server answered for that file through the public API, the public site and the IIIF\n\
         manifest. A `**fail**` is a red cell.\n\n\
         Generated by the matrix test with `MEDIA_MATRIX_PRINT=1`:\n\n\
         ```\nMEDIA_MATRIX_PRINT=1 cargo test -p omeka-server --test media_matrix -- --nocapture\n```\n\n",
    );
    out.push_str(&render_table(rows));
    out.push_str("\n## Known red\n\n");
    if red.is_empty() {
        out.push_str("None.\n");
    } else {
        for (file, cell) in red {
            out.push_str(&format!("- `{file}` / {cell}\n"));
        }
    }
    out
}
