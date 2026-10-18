//! The HTML media renderer (`Media/Renderer/Html.php`).
//!
//! Returns the stored `html` member verbatim, and supplies its plain text for
//! full-text search.

use crate::data::MediaData;
use crate::render::{FulltextSearchable, MediaView, RenderOptions, Renderer, View};
use crate::text::{html_entity_decode, strip_tags};

/// The HTML renderer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Html;

impl Renderer for Html {
    fn name(&self) -> &str {
        "html"
    }

    /// `Renderer/Html.php:13` returns `$data['html']` unchanged, so the caller
    /// receives whatever the purifier stored.
    fn render(&self, _view: &View<'_>, media: &MediaView, _options: &RenderOptions) -> String {
        match &media.data {
            MediaData::Html(data) => data.html.clone(),
            _ => String::new(),
        }
    }
}

impl FulltextSearchable for Html {
    /// `Renderer/Html.php:19`:
    /// `html_entity_decode(strip_tags($data['html']), ENT_QUOTES | ENT_SUBSTITUTE | ENT_HTML5, 'UTF-8')`.
    fn fulltext_text(&self, media: &MediaView) -> String {
        match &media.data {
            MediaData::Html(data) => html_entity_decode(&strip_tags(&data.html)),
            _ => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::HtmlData;
    use crate::file_render::FileRendererRegistry;
    use crate::testing::media_view_with_data;

    /// The stored fragment is emitted unchanged, entities and all.
    #[test]
    fn html_renderer_returns_the_stored_fragment() {
        let view = View {
            file_renderers: &FileRendererRegistry::with_defaults(),
        };
        let media = media_view_with_data(MediaData::Html(HtmlData {
            html: "<p>a &amp; b</p>".to_string(),
        }));
        assert_eq!(
            Html.render(&view, &media, &RenderOptions::default()),
            "<p>a &amp; b</p>"
        );
    }

    /// `getFulltextText` strips tags and decodes entities.
    #[test]
    fn html_renderer_fulltext_matches_upstream() {
        let media = media_view_with_data(MediaData::Html(HtmlData {
            html: "<p>a &amp; b</p>".to_string(),
        }));
        assert_eq!(Html.fulltext_text(&media), "a & b");
    }

    /// A row whose payload is not HTML renders nothing.
    #[test]
    fn html_renderer_ignores_other_payloads() {
        let view = View {
            file_renderers: &FileRendererRegistry::with_defaults(),
        };
        assert_eq!(
            Html.render(
                &view,
                &media_view_with_data(MediaData::None),
                &RenderOptions::default()
            ),
            ""
        );
    }
}
