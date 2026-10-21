//! The 3D model file renderer. Upstream has none: a `.glb` falls through to
//! the thumbnail renderer, so a page offers the file only as a download.
//!
//! The model costs a library and the model's own bytes, so the page shows a
//! facade first: the stored poster (or no picture when none exists), a button
//! that names the viewer's download size, and a link to the original. The
//! button loads the vendored `<model-viewer>` from this origin and only then
//! sets the model's `src`, so nothing but the poster is downloaded until the
//! reader asks for the model.

use crate::file_render::{FileRenderer, ThumbnailType};
use crate::render::{MediaView, RenderOptions};
use crate::text::{escape_html, hyperlink};

/// The 3D model renderer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Model;

impl FileRenderer for Model {
    fn name(&self) -> &'static str {
        "model"
    }

    /// The facade when the site supplies its viewer, a plain link otherwise.
    fn render(&self, media: &MediaView, options: &RenderOptions) -> String {
        let Some(original) = media.original_url.as_deref() else {
            return escape_html(&media.display_title);
        };
        let Some(reader) = &options.model_reader else {
            return hyperlink(Some(&media.display_title), Some(original), &[]);
        };
        let alt = escape_html(media.alt_text.as_deref().unwrap_or(&media.display_title));
        let viewer = format!(
            "<model-viewer class=\"model-viewer\" camera-controls touch-action=\"pan-y\" \
             poster=\"{poster}\" alt=\"{alt}\" data-src=\"{original}\" hidden></model-viewer>",
            poster = poster(media),
            original = escape_html(original)
        );
        format!(
            "<div class=\"model-reader\" data-model-reader data-library=\"{library}\" \
             data-loading=\"{loading}\">\
             <div class=\"model-stage\">{image}{viewer}</div>\
             <div class=\"model-controls\">\
             <button type=\"button\" class=\"model-turn\">{turn}</button>\
             <a class=\"model-download\" href=\"{original}\">{title}</a>\
             </div>\
             <p class=\"model-error\" role=\"status\" hidden>{failed}</p>\
             <noscript><p class=\"model-noscript\">{no_javascript}</p></noscript>\
             </div>",
            library = escape_html(&reader.library_url),
            loading = escape_html(&reader.loading),
            image = image(media),
            turn = escape_html(&reader.turn),
            original = escape_html(original),
            title = escape_html(&media.display_title),
            failed = escape_html(&reader.failed),
            no_javascript = escape_html(&reader.no_javascript),
        )
    }
}

/// The poster the facade shows before the press: the stored large derivative
/// when a curator made one, otherwise nothing, never the theme's placeholder
/// drawn as if it were a view of the model.
fn poster(media: &MediaView) -> String {
    media
        .stored_thumbnail_url(ThumbnailType::Large)
        .map(escape_html)
        .unwrap_or_default()
}

/// The poster as an `<img>`, or an empty string when there is none.
fn image(media: &MediaView) -> String {
    match media.stored_thumbnail_url(ThumbnailType::Large) {
        Some(url) => format!(
            "<img class=\"model-poster\" src=\"{}\" alt=\"\" loading=\"lazy\">",
            escape_html(url)
        ),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::ModelReader;
    use crate::testing::{media_row, media_view_from};

    fn reader() -> ModelReader {
        ModelReader {
            library_url: "/assets/vendor/model-viewer/model-viewer-umd.min.js?v=abc".into(),
            turn: "Turn the model (loads the 3D viewer, 140 KB)".into(),
            loading: "Loading the 3D viewer…".into(),
            failed: "This browser cannot show the model.".into(),
            no_javascript: "Turning the model needs JavaScript.".into(),
        }
    }

    fn model_media() -> MediaView {
        let mut media = media_row(9, "upload", "file");
        media.storage_id = Some("def456".into());
        media.extension = Some("glb".into());
        media.media_type = Some("model/gltf-binary".into());
        media.has_original = true;
        media.has_thumbnails = true;
        media.alt_text = Some("A carved box".into());
        media.source = Some("box.glb".into());
        media_view_from(&media)
    }

    /// The facade states the viewer's cost, shows the poster and the link to
    /// the original, and keeps the model out of the document until the press.
    #[test]
    fn the_facade_names_its_cost_and_defers_the_model() {
        let html = Model.render(
            &model_media(),
            &RenderOptions {
                model_reader: Some(reader()),
                ..RenderOptions::default()
            },
        );
        assert!(
            html.starts_with("<div class=\"model-reader\" data-model-reader"),
            "{html}"
        );
        assert!(
            html.contains(
                "data-library=\"/assets/vendor/model-viewer/model-viewer-umd.min.js?v=abc\""
            ),
            "{html}"
        );
        assert!(
            html.contains(">Turn the model (loads the 3D viewer, 140 KB)</button>"),
            "{html}"
        );
        assert!(
            html.contains("<img class=\"model-poster\" src=\"/files/large/def456.jpg\" alt=\"\""),
            "{html}"
        );
        assert!(
            html.contains("alt=\"A carved box\" data-src=\"/files/original/def456.glb\" hidden"),
            "{html}"
        );
        assert!(
            html.contains(
                "<a class=\"model-download\" href=\"/files/original/def456.glb\">box.glb</a>"
            ),
            "{html}"
        );
        assert!(html.contains("<noscript>"), "{html}");
        assert!(
            html.contains(">Turning the model needs JavaScript.</p>"),
            "{html}"
        );
    }

    /// The facade holds no script of its own: the site loads its facade file
    /// and the library only on the press, so the document runs no inline code.
    #[test]
    fn the_facade_carries_no_script() {
        let html = Model.render(
            &model_media(),
            &RenderOptions {
                model_reader: Some(reader()),
                ..RenderOptions::default()
            },
        );
        assert!(!html.contains("<script"), "{html}");
        assert!(!html.contains("document.createElement"), "{html}");
    }

    /// A model with no poster keeps an empty stage rather than the theme's
    /// placeholder picture, and the poster attribute is left empty.
    #[test]
    fn a_model_without_a_poster_draws_no_picture() {
        let mut media = model_media();
        media.media.has_thumbnails = false;
        let html = Model.render(
            &media,
            &RenderOptions {
                model_reader: Some(reader()),
                ..RenderOptions::default()
            },
        );
        assert!(!html.contains("<img"), "{html}");
        assert!(html.contains("poster=\"\""), "{html}");
    }

    /// A caller that supplies no viewer gets a link to the file, not a button
    /// that cannot work.
    #[test]
    fn a_page_without_the_viewer_links_the_file() {
        assert_eq!(
            Model.render(&model_media(), &RenderOptions::default()),
            "<a href=\"/files/original/def456.glb\">box.glb</a>"
        );
    }
}
