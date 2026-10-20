//! Micro-benches for the ingest parse, one case per ingester.
//!
//! Covers `crates/omeka-media/src/ingest/`: each ingester over the request and
//! remote-response fixtures the behavioural tests use, plus the two boundaries
//! around it, `IngestRequest::from_value` and `MediaData::parse`.
//!
//! Every case runs against the crate's test doubles: the fake network serves the
//! recorded fixture and the fake thumbnailer returns fixed bytes, so no case
//! touches the wire. The store is a fresh scratch directory per iteration, under
//! the cargo target directory, so the timed region excludes directory creation
//! and never accumulates files.
//!
//! Run with `cargo bench -p omeka-media --bench ingest_parse`. The numbers, their
//! limiters and the checks that did not apply are in `crates/omeka-media/README.md`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::hint::black_box;
use std::time::Duration;

use criterion::measurement::WallTime;
use criterion::{BatchSize, BenchmarkGroup, Criterion, criterion_group, criterion_main};
use omeka_media::data::MediaData;
use omeka_media::ingest::{
    ErrorStore, IngestEnv, IngestRequest, IngesterRegistry, MediaDraft, UploadedFile,
};
use omeka_media::oembed::{Oembed, OembedAllowList};
use omeka_media::store::FileStore;
use omeka_media::testing::{FakeHttp, FakeThumbnailer, PassthroughPurifier, media_row};
use omeka_media::validation::FileValidation;

/// Everything an ingester reads, over a scratch store the case drops when it ends.
///
/// This is `testing::Harness` with the scratch tree moved under the cargo target
/// directory (`benches/support/mod.rs`); the fakes themselves come from the crate.
struct Fixture {
    store: FileStore,
    http: FakeHttp,
    oembed: Oembed,
    purifier: PassthroughPurifier,
    thumbnailer: FakeThumbnailer,
    // The upload body is arbitrary bytes, which upstream's default lists
    // refuse; the bench times the store, so validation is off.
    validation: FileValidation,
    _scratch: tempfile::TempDir,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let scratch = tempfile::Builder::new()
            .prefix(tag)
            .tempdir_in(support::scratch_dir())
            .unwrap();
        Self {
            store: FileStore::new(scratch.path(), "/files"),
            http: FakeHttp::new(),
            oembed: Oembed::new(OembedAllowList::shipped()),
            purifier: PassthroughPurifier,
            thumbnailer: FakeThumbnailer::default(),
            validation: FileValidation::Disabled,
            _scratch: scratch,
        }
    }

    fn with_json(mut self, url: &str, value: serde_json::Value) -> Self {
        let http = std::mem::take(&mut self.http);
        self.http = http.with_json(url, value);
        self
    }

    fn with_body(mut self, url: &str, body: impl Into<Vec<u8>>) -> Self {
        let http = std::mem::take(&mut self.http);
        self.http = http.with_body(url, body);
        self
    }

    fn env(&self) -> IngestEnv<'_> {
        IngestEnv {
            store: &self.store,
            http: &self.http,
            purifier: &self.purifier,
            thumbnailer: &self.thumbnailer,
            oembed: &self.oembed,
            validation: &self.validation,
        }
    }
}

/// One setup: a fresh fixture with its routes, the request, and the draft.
type Case = (Fixture, IngestRequest, MediaDraft, ErrorStore);

/// An 8 KiB upload body.
fn upload_bytes() -> Vec<u8> {
    (0..8 * 1024).map(|index| (index % 251) as u8).collect()
}

fn fixture(json: &str) -> serde_json::Value {
    serde_json::from_str(json).unwrap()
}

fn upload() -> Case {
    let request = IngestRequest::from_value(fixture(include_str!(
        "../tests/fixtures/ingest/upload_request.json"
    )))
    .with_files(vec![UploadedFile::new("Photo.JPG", upload_bytes())]);
    (
        Fixture::new("upload-"),
        request,
        MediaDraft::new(media_row(1, "upload", "file")),
        ErrorStore::default(),
    )
}

fn url() -> Case {
    let state = Fixture::new("url-").with_body(
        "https://example.org/files/logo-iiif-34x30.png",
        &include_bytes!("../tests/fixtures/ingest/url_response.png")[..],
    );
    let request = IngestRequest::from_value(fixture(include_str!(
        "../tests/fixtures/ingest/url_request.json"
    )));
    (
        state,
        request,
        MediaDraft::new(media_row(1, "url", "file")),
        ErrorStore::default(),
    )
}

fn html() -> Case {
    let request = IngestRequest::from_value(fixture(include_str!(
        "../tests/fixtures/ingest/html_request.json"
    )));
    (
        Fixture::new("html-"),
        request,
        MediaDraft::new(media_row(1, "html", "html")),
        ErrorStore::default(),
    )
}

fn youtube() -> Case {
    let state = Fixture::new("youtube-").with_body(
        "http://img.youtube.com/vi/dQw4w9WgXcQ/0.jpg",
        &include_bytes!("../tests/fixtures/ingest/youtube_poster.jpg")[..],
    );
    let request = IngestRequest::from_value(fixture(include_str!(
        "../tests/fixtures/ingest/youtube_request.json"
    )));
    (
        state,
        request,
        MediaDraft::new(media_row(1, "youtube", "youtube")),
        ErrorStore::default(),
    )
}

fn oembed() -> Case {
    let response = fixture(include_str!(
        "../tests/fixtures/ingest/oembed_response.json"
    ));
    let state = Fixture::new("oembed-")
        .with_json(
            "https://www.youtube.com/oembed?format=json&url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3DdQw4w9WgXcQ",
            response,
        )
        .with_body(
            "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg",
            &include_bytes!("../tests/fixtures/ingest/oembed_thumbnail.jpg")[..],
        );
    let request = IngestRequest::from_value(fixture(include_str!(
        "../tests/fixtures/ingest/oembed_request.json"
    )));
    (
        state,
        request,
        MediaDraft::new(media_row(1, "oembed", "oembed")),
        ErrorStore::default(),
    )
}

fn iiif() -> Case {
    let document = fixture(include_str!("../tests/fixtures/ingest/iiif_info_v2.json"));
    let id = document["@id"].as_str().unwrap();
    let source = format!("{id}/info.json");
    let thumbnail = format!("{id}/full/full/0/default.jpg");
    let state = Fixture::new("iiif-")
        .with_json(&source, document)
        .with_body(&thumbnail, b"jpeg");
    let request = IngestRequest::from_value(serde_json::json!({ "o:source": source }));
    (
        state,
        request,
        MediaDraft::new(media_row(1, "iiif", "iiif")),
        ErrorStore::default(),
    )
}

fn iiif_presentation() -> Case {
    let manifest = fixture(include_str!(
        "../tests/fixtures/ingest/iiif_presentation_v2.json"
    ));
    let source = manifest["@id"].as_str().unwrap().to_string();
    let thumbnail = manifest["thumbnail"]["@id"].as_str().unwrap().to_string();
    let state = Fixture::new("iiif-presentation-")
        .with_json(&source, manifest)
        .with_body(&thumbnail, b"jpeg");
    let request = IngestRequest::from_value(serde_json::json!({ "o:source": source }));
    (
        state,
        request,
        MediaDraft::new(media_row(1, "iiif_presentation", "iiif_presentation")),
        ErrorStore::default(),
    )
}

/// Time one ingester end to end: request in, draft and error store out.
///
/// `check` runs once on a fresh case, outside the timed region, and asserts the
/// work happened. Without it a fixture that stopped fetching or storing would
/// leave the bench timing a no-op, and the number would look excellent.
fn ingest_case(
    group: &mut BenchmarkGroup<'_, WallTime>,
    registry: &IngesterRegistry,
    name: &'static str,
    ingester: &'static str,
    mut setup: impl FnMut() -> Case + 'static,
    check: impl Fn(&Fixture, &MediaDraft, &ErrorStore),
) {
    let (state, request, mut draft, mut errors) = setup();
    registry
        .get(ingester)
        .ingest(&mut draft, &request, &state.env(), &mut errors);
    check(&state, &draft, &errors);

    group.bench_function(name, |b| {
        b.iter_batched(
            &mut setup,
            |(state, request, mut draft, mut errors)| {
                registry
                    .get(ingester)
                    .ingest(&mut draft, &request, &state.env(), &mut errors);
                black_box((draft, errors))
            },
            BatchSize::SmallInput,
        );
    });
}

fn ingest_parse(c: &mut Criterion) {
    let registry = IngesterRegistry::with_defaults();

    let mut group = c.benchmark_group("media/ingest_parse");
    group
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(50);

    ingest_case(
        &mut group,
        &registry,
        "upload",
        "upload",
        upload,
        |state, draft, errors| {
            assert!(errors.is_empty(), "{errors:?}");
            let storage_id = draft.row.storage_id.as_deref().unwrap();
            assert!(draft.row.has_original);
            assert!(state.store.exists(&format!("original/{storage_id}.jpg")));
        },
    );
    ingest_case(
        &mut group,
        &registry,
        "url",
        "url",
        url,
        |state, draft, errors| {
            assert!(errors.is_empty(), "{errors:?}");
            assert!(draft.row.has_original);
            let storage_id = draft.row.storage_id.as_deref().unwrap();
            assert!(state.store.exists(&format!("original/{storage_id}.png")));
        },
    );
    ingest_case(
        &mut group,
        &registry,
        "html",
        "html",
        html,
        |_, draft, _| {
            assert!(matches!(draft.data, MediaData::Html(_)));
        },
    );
    ingest_case(
        &mut group,
        &registry,
        "youtube",
        "youtube",
        youtube,
        |state, draft, errors| {
            assert!(errors.is_empty(), "{errors:?}");
            assert!(draft.row.has_thumbnails);
            assert_eq!(state.http.calls().len(), 1);
        },
    );
    ingest_case(
        &mut group,
        &registry,
        "oembed",
        "oembed",
        oembed,
        |state, draft, errors| {
            assert!(errors.is_empty(), "{errors:?}");
            assert!(draft.row.has_thumbnails);
            assert_eq!(state.http.calls().len(), 2);
        },
    );
    ingest_case(
        &mut group,
        &registry,
        "iiif",
        "iiif",
        iiif,
        |state, draft, errors| {
            assert!(errors.is_empty(), "{errors:?}");
            assert!(matches!(draft.data, MediaData::Iiif(_)));
            assert_eq!(state.http.calls().len(), 2);
        },
    );
    ingest_case(
        &mut group,
        &registry,
        "iiif_presentation",
        "iiif_presentation",
        iiif_presentation,
        |state, draft, errors| {
            assert!(errors.is_empty(), "{errors:?}");
            assert!(draft.row.has_thumbnails);
            assert!(matches!(draft.data, MediaData::IiifPresentation(_)));
            assert_eq!(state.http.calls().len(), 2);
        },
    );

    group.finish();

    let mut group = c.benchmark_group("media/ingest_parse/boundary");
    group
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(50);

    let html_request = fixture(include_str!("../tests/fixtures/ingest/html_request.json"));
    group.bench_function("ingest_request_from_value/html", |b| {
        b.iter(|| IngestRequest::from_value(black_box(html_request.clone())))
    });
    let upload_request = fixture(include_str!("../tests/fixtures/ingest/upload_request.json"));
    group.bench_function("ingest_request_from_value/upload", |b| {
        b.iter(|| IngestRequest::from_value(black_box(upload_request.clone())))
    });

    let html_data = serde_json::json!({
        "html": "<p>Hello <strong>world</strong></p><script>alert(1)</script>"
    });
    let oembed_data = fixture(include_str!(
        "../tests/fixtures/ingest/oembed_response.json"
    ));
    let presentation_data = fixture(include_str!(
        "../tests/fixtures/ingest/iiif_presentation_v2.json"
    ));
    for (name, ingester, data) in [
        ("html", "html", &html_data),
        ("oembed", "oembed", &oembed_data),
        ("iiif_presentation", "iiif_presentation", &presentation_data),
    ] {
        group.bench_function(format!("media_data_parse/{name}"), |b| {
            b.iter(|| MediaData::parse(black_box(ingester), Some(black_box(data))))
        });
    }
    group.bench_function("media_data_parse/none", |b| {
        b.iter(|| MediaData::parse(black_box("upload"), None))
    });
    group.bench_function("media_data_parse/iiif_json_parse_99KiB", |b| {
        let bytes = &include_bytes!("../tests/fixtures/ingest/iiif_presentation_v2.json")[..];
        b.iter(|| serde_json::from_slice::<serde_json::Value>(black_box(bytes)))
    });
    group.bench_function("media_data_parse/iiif_json_clone_99KiB", |b| {
        b.iter(|| black_box(&presentation_data).clone())
    });

    group.finish();
}

criterion_group!(benches, ingest_parse);
criterion_main!(benches);
