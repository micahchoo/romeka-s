//! Micro-benches for the HTML renderers and the text helpers they build markup with.
//!
//! Covers `crates/omeka-media/src/render/` (the six media renderers plus the
//! registry fallback), `crates/omeka-media/src/file_render/` (the lookup rule
//! and the file renderers), `crates/omeka-media/src/text.rs`, and the
//! [`MediaView`] resolution a renderer reads.
//!
//! Run with `cargo bench -p omeka-media --bench render_html`. The numbers, their
//! limiters and the checks that did not apply are in `crates/omeka-media/README.md`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use omeka_media::data::{HtmlData, MediaData};
use omeka_media::file_render::{FileRendererRegistry, LinkType, ThumbnailType};
use omeka_media::render::html::Html;
use omeka_media::render::{
    FulltextSearchable, MediaView, RenderEnv, RenderOptions, RendererRegistry, View,
};
use omeka_media::testing::media_row;
use omeka_media::text::{escape_html, html_entity_decode, hyperlink, strip_tags, urlencode};

/// A page of stored HTML, the shape the purifier leaves in `media.data.html`.
const FRAGMENT: &str = "<h2>About the port</h2>\n<p>Romeka S is a web publication \
system &mdash; it has sites, pages and blocks &amp; typed values. The port keeps \
&quot;upstream behaviour&quot; where the tests can see it.</p>\n<ul><li>Items</li>\
<li>Item sets</li><li>Media</li></ul>\n<p>Read <a href=\"https://example.org/a?x=1&amp;y=2\">\
the notes</a> for the details.</p>";

fn fixture(json: &str) -> serde_json::Value {
    serde_json::from_str(json).unwrap()
}

fn video_view() -> MediaView {
    let mut media = media_row(7, "upload", "file");
    media.storage_id = Some("abc123".into());
    media.extension = Some("mp4".into());
    media.media_type = Some("video/mp4".into());
    media.has_original = true;
    media.source = Some("clip.mp4".into());
    MediaView::new(
        media,
        MediaData::None,
        RenderEnv::default(),
        "/item/7",
        "/media/7",
    )
}

fn thumbnail_view() -> MediaView {
    let mut media = media_row(8, "upload", "file");
    media.storage_id = Some("def456".into());
    media.extension = Some("pdf".into());
    media.media_type = Some("application/pdf".into());
    media.has_thumbnails = true;
    media.source = Some("document.pdf".into());
    MediaView::new(
        media,
        MediaData::None,
        RenderEnv::default(),
        "/item/8",
        "/media/8",
    )
}

fn html_view() -> MediaView {
    let mut media = media_row(9, "html", "html");
    media.source = Some("a fragment".into());
    MediaView::new(
        media,
        MediaData::Html(HtmlData {
            html: FRAGMENT.to_string(),
        }),
        RenderEnv::default(),
        "/item/9",
        "/media/9",
    )
}

fn youtube_view() -> MediaView {
    let data = MediaData::parse(
        "youtube",
        Some(&serde_json::json!({"id": "dQw4w9WgXcQ", "start": "30", "end": "90"})),
    )
    .unwrap();
    MediaView::new(
        media_row(10, "youtube", "youtube"),
        data,
        RenderEnv::default(),
        "/item/10",
        "/media/10",
    )
}

fn oembed_view() -> MediaView {
    let data = MediaData::parse(
        "oembed",
        Some(&fixture(include_str!(
            "../tests/fixtures/ingest/oembed_response.json"
        ))),
    )
    .unwrap();
    let mut media = media_row(11, "oembed", "oembed");
    media.source = Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ".into());
    MediaView::new(media, data, RenderEnv::default(), "/item/11", "/media/11")
}

fn iiif_view() -> MediaView {
    let data = MediaData::parse(
        "iiif",
        Some(&fixture(include_str!(
            "../tests/fixtures/ingest/iiif_info_v2.json"
        ))),
    )
    .unwrap();
    MediaView::new(
        media_row(12, "iiif", "iiif"),
        data,
        RenderEnv::default(),
        "/item/12",
        "/media/12",
    )
}

fn iiif_presentation_view() -> MediaView {
    let manifest = fixture(include_str!(
        "../tests/fixtures/ingest/iiif_presentation_v2.json"
    ));
    let data = MediaData::parse("iiif_presentation", Some(&manifest)).unwrap();
    let mut media = media_row(13, "iiif_presentation", "iiif_presentation");
    media.source = manifest["@id"].as_str().map(str::to_string);
    MediaView::new(media, data, RenderEnv::default(), "/item/13", "/media/13")
}

fn render_html(c: &mut Criterion) {
    let renderers = RendererRegistry::with_defaults();
    let file_renderers = FileRendererRegistry::with_defaults();
    let view = View {
        file_renderers: &file_renderers,
    };
    let options = RenderOptions::default();
    let sized_options = RenderOptions {
        width: Some("640".to_string()),
        height: Some("480".to_string()),
        thumbnail_type: Some(ThumbnailType::Medium),
        link: Some(LinkType::Item),
        ..RenderOptions::default()
    };

    let video = video_view();
    let thumbnail = thumbnail_view();
    let html = html_view();
    let youtube = youtube_view();
    let oembed = oembed_view();
    let iiif = iiif_view();
    let presentation = iiif_presentation_view();

    // Run once, outside the timed region: an empty fragment means the case has
    // the wrong payload and the bench is timing an early return.
    for (name, media, opts) in [
        ("file", &video, &options),
        ("file", &thumbnail, &sized_options),
        ("html", &html, &options),
        ("youtube", &youtube, &options),
        ("oembed", &oembed, &options),
        ("iiif", &iiif, &options),
        ("iiif_presentation", &presentation, &options),
    ] {
        assert!(
            !renderers.render(name, &view, media, opts).is_empty(),
            "{name} rendered nothing"
        );
    }
    assert_eq!(
        renderers.render("not_a_renderer", &view, &video, &options),
        ""
    );
    assert!(!Html.fulltext_text(&html).is_empty());
    assert_eq!(
        MediaView::new(
            media_row(7, "upload", "file"),
            MediaData::None,
            RenderEnv::default(),
            "/item/7",
            "/media/7",
        )
        .thumbnails
        .len(),
        3
    );

    let mut group = c.benchmark_group("media/render");
    group
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(50);

    group.bench_function("file/video", |b| {
        b.iter(|| renderers.render("file", &view, &video, &options))
    });
    group.bench_function("file/thumbnail", |b| {
        b.iter(|| renderers.render("file", &view, &thumbnail, &sized_options))
    });
    group.bench_function("html", |b| {
        b.iter(|| renderers.render("html", &view, &html, &options))
    });
    group.bench_function("youtube", |b| {
        b.iter(|| renderers.render("youtube", &view, &youtube, &options))
    });
    group.bench_function("oembed", |b| {
        b.iter(|| renderers.render("oembed", &view, &oembed, &options))
    });
    group.bench_function("iiif", |b| {
        b.iter(|| renderers.render("iiif", &view, &iiif, &options))
    });
    group.bench_function("iiif_document_serialize", |b| {
        let document = fixture(include_str!("../tests/fixtures/ingest/iiif_info_v2.json"));
        b.iter(|| serde_json::to_string(black_box(&document)))
    });
    group.bench_function("iiif_presentation", |b| {
        b.iter(|| renderers.render("iiif_presentation", &view, &presentation, &options))
    });
    group.bench_function("unknown_name_fallback", |b| {
        b.iter(|| renderers.render("not_a_renderer", &view, &video, &options))
    });

    group.bench_function("html_fulltext", |b| {
        b.iter(|| Html.fulltext_text(black_box(&html)))
    });

    group.bench_function("media_view_new/file", |b| {
        b.iter(|| {
            MediaView::new(
                black_box(media_row(7, "upload", "file")),
                MediaData::None,
                RenderEnv::default(),
                "/item/7",
                "/media/7",
            )
        })
    });

    group.finish();

    let mut group = c.benchmark_group("media/renderer_lookup");
    group
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(50);
    group.bench_function("media_renderer/known", |b| {
        b.iter(|| renderers.get(black_box("youtube")))
    });
    group.bench_function("media_renderer/unknown", |b| {
        b.iter(|| renderers.get(black_box("not_a_renderer")))
    });
    group.bench_function("file_renderer/by_media_type", |b| {
        b.iter(|| file_renderers.get(black_box("video/mp4")))
    });
    group.bench_function("file_renderer/by_extension", |b| {
        b.iter(|| file_renderers.get(black_box("mp4")))
    });
    group.bench_function("file_renderer/unknown", |b| {
        b.iter(|| file_renderers.get(black_box("application/pdf")))
    });
    group.finish();

    let escaped_input = "a < b & c > d \"quoted\" 'single' \u{e9}\u{fc}";
    let mut group = c.benchmark_group("media/text");
    group
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(50);
    group.bench_function("escape_html/short", |b| {
        b.iter(|| escape_html(black_box(escaped_input)))
    });
    group.bench_function("escape_html/fragment", |b| {
        b.iter(|| escape_html(black_box(FRAGMENT)))
    });
    group.bench_function("strip_tags/fragment", |b| {
        b.iter(|| strip_tags(black_box(FRAGMENT)))
    });
    group.bench_function("html_entity_decode/fragment", |b| {
        b.iter(|| html_entity_decode(black_box(FRAGMENT)))
    });
    group.bench_function("urlencode/url", |b| {
        b.iter(|| {
            urlencode(black_box(
                "https://example.org/iiif/presentation/v2/b18035723?x=1&y=2",
            ))
        })
    });
    group.bench_function("hyperlink/escaped", |b| {
        b.iter(|| {
            hyperlink(
                black_box(Some("A page & title")),
                black_box(Some("https://example.org/a?x=1&y=2")),
                black_box(&[]),
            )
        })
    });
    group.finish();
}

criterion_group!(benches, render_html);
criterion_main!(benches);
