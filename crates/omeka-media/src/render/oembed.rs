//! The oEmbed media renderer (`Media/Renderer/OEmbed.php`).
//!
//! Uses the stored response for markup, and falls back to a hyperlink when the
//! response carries nothing renderable.

use serde_json::Map;

use crate::data::{MediaData, OembedData};
use crate::oembed::render_oembed;
use crate::render::{MediaView, RenderOptions, Renderer, View};
use crate::text::hyperlink;

/// The oEmbed renderer.
#[derive(Debug, Clone, Copy, Default)]
pub struct OEmbed;

impl Renderer for OEmbed {
    fn name(&self) -> &str {
        "oembed"
    }

    /// `Renderer/OEmbed.php:29`: the response markup, else a hyperlink titled by
    /// the response title or the source.
    fn render(&self, _view: &View<'_>, media: &MediaView, _options: &RenderOptions) -> String {
        let data = match &media.data {
            MediaData::Oembed(data) => data.clone(),
            _ => OembedData { map: Map::new() },
        };
        if let Some(markup) = render_oembed(&data) {
            return markup;
        }
        let source = media.media.source.clone().unwrap_or_default();
        let title = data.title().unwrap_or_else(|| source.clone());
        hyperlink(Some(&title), Some(&source), &[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_render::FileRendererRegistry;
    use crate::testing::{media_row, media_view_with_data};

    fn view(registry: &FileRendererRegistry) -> View<'_> {
        View {
            file_renderers: registry,
        }
    }

    fn oembed_media(value: serde_json::Value) -> MediaView {
        let data = MediaData::parse("oembed", Some(&value)).expect("oembed data");
        media_view_with_data(data)
    }

    /// A response with an `html` member is emitted verbatim
    /// (`Renderer/OEmbed.php:31`).
    #[test]
    fn oembed_renderer_returns_response_html() {
        let registry = FileRendererRegistry::with_defaults();
        let media =
            oembed_media(serde_json::json!({"html": "<iframe src=\"https://x\"></iframe>"}));
        assert_eq!(
            OEmbed.render(&view(&registry), &media, &RenderOptions::default()),
            "<iframe src=\"https://x\"></iframe>"
        );
    }

    /// A response with no renderable member falls back to a hyperlink on the
    /// source, titled by `title` when it is non-empty (`Renderer/OEmbed.php:34`).
    #[test]
    fn oembed_renderer_falls_back_to_a_hyperlink() {
        let registry = FileRendererRegistry::with_defaults();
        let mut media = oembed_media(serde_json::json!({"type": "link", "title": "A page"}));
        media.media.source = Some("https://example.org/page".to_string());
        assert_eq!(
            OEmbed.render(&view(&registry), &media, &RenderOptions::default()),
            "<a href=\"https://example.org/page\">A page</a>"
        );
    }

    /// With no title, the source is both text and href, and both are escaped.
    #[test]
    fn oembed_renderer_uses_the_source_as_the_fallback_title() {
        let registry = FileRendererRegistry::with_defaults();
        let mut media = media_view_with_data(MediaData::None);
        media.media.source = Some("https://example.org/a?x=1&y=2".to_string());
        assert_eq!(
            OEmbed.render(&view(&registry), &media, &RenderOptions::default()),
            "<a href=\"https://example.org/a?x=1&amp;y=2\">https://example.org/a?x=1&amp;y=2</a>"
        );
        let _ = media_row(1, "oembed", "oembed");
    }
}
