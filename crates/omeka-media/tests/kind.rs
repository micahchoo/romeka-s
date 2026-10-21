//! One test per row of the annotation design's per-kind table (section 7).
//!
//! Each row states what the media is, what a selector may point at, and what
//! its canvas paints: an image service, a PDF's first page, sound, video, or
//! nothing.

use omeka_media::kind::{CanvasBody, KindInput, MediaKind, Surface};

/// The kind of a media row, through the public table.
fn kind(
    renderer: &str,
    ingester: &str,
    media_type: Option<&str>,
    has_thumbnails: bool,
) -> MediaKind {
    MediaKind::of(&KindInput {
        renderer,
        ingester,
        media_type,
        has_thumbnails,
    })
}

/// One row of the table: the kind, the surface it accepts and the canvas it
/// paints.
fn assert_row(
    renderer: &str,
    ingester: &str,
    media_type: Option<&str>,
    has_thumbnails: bool,
    expected: MediaKind,
    surface: Surface,
    canvas: Option<CanvasBody>,
) {
    let kind = kind(renderer, ingester, media_type, has_thumbnails);
    assert_eq!(kind, expected, "{renderer}/{ingester} {media_type:?}");
    assert_eq!(kind.surface(), surface, "surface of {kind:?}");
    assert_eq!(kind.canvas_body(), canvas, "canvas of {kind:?}");
}

#[test]
fn a_local_raster_image_with_derivatives_is_an_image_with_an_image_canvas() {
    assert_row(
        "file",
        "upload",
        Some("image/jpeg"),
        true,
        MediaKind::LocalImage,
        Surface::Image,
        Some(CanvasBody::Image),
    );
}

#[test]
fn a_remote_iiif_image_is_an_image_with_an_image_canvas() {
    assert_row(
        "iiif",
        "iiif",
        Some("image/jpeg"),
        false,
        MediaKind::IiifImage,
        Surface::Image,
        Some(CanvasBody::Image),
    );
}

#[test]
fn audio_takes_time_ranges_and_paints_sound() {
    assert_row(
        "file",
        "upload",
        Some("audio/mpeg"),
        false,
        MediaKind::Audio,
        Surface::Time,
        Some(CanvasBody::Sound),
    );
}

#[test]
fn video_takes_time_ranges_and_paints_video() {
    assert_row(
        "file",
        "upload",
        Some("video/mp4"),
        false,
        MediaKind::Video,
        Surface::Time,
        Some(CanvasBody::Video),
    );
}

#[test]
fn a_pdf_takes_whole_notes_and_paints_its_first_page_once_it_exists() {
    assert_row(
        "file",
        "upload",
        Some("application/pdf"),
        false,
        MediaKind::Pdf { first_page: false },
        Surface::Whole,
        None,
    );
    assert_row(
        "file",
        "upload",
        Some("application/pdf"),
        true,
        MediaKind::Pdf { first_page: true },
        Surface::Whole,
        Some(CanvasBody::Still),
    );
}

#[test]
fn a_gltf_model_takes_whole_notes_and_paints_no_canvas() {
    for media_type in ["model/gltf-binary", "model/gltf+json"] {
        assert_row(
            "file",
            "upload",
            Some(media_type),
            false,
            MediaKind::Model3d,
            Surface::Whole,
            None,
        );
    }
}

#[test]
fn a_caption_file_takes_whole_notes_and_paints_no_canvas() {
    for media_type in ["text/vtt", "application/x-subrip"] {
        assert_row(
            "file",
            "upload",
            Some(media_type),
            false,
            MediaKind::Captions,
            Surface::Whole,
            None,
        );
    }
}

#[test]
fn a_youtube_oembed_or_html_embed_takes_whole_notes_and_paints_no_canvas() {
    for renderer in ["youtube", "oembed", "html"] {
        assert_row(
            renderer,
            renderer,
            None,
            false,
            MediaKind::Embed,
            Surface::Whole,
            None,
        );
    }
}

#[test]
fn a_remote_presentation_takes_whole_notes_and_paints_no_canvas() {
    assert_row(
        "iiif_presentation",
        "iiif_presentation",
        None,
        false,
        MediaKind::IiifPresentation,
        Surface::Whole,
        None,
    );
}

#[test]
fn any_other_file_takes_whole_notes_and_paints_no_canvas() {
    for (renderer, media_type, has_thumbnails) in [
        (
            "file",
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
            false,
        ),
        // A raster whose derivatives failed is not a local image.
        ("file", Some("image/jpeg"), false),
        ("", None, false),
    ] {
        assert_row(
            renderer,
            "upload",
            media_type,
            has_thumbnails,
            MediaKind::OtherFile,
            Surface::Whole,
            None,
        );
    }
}
