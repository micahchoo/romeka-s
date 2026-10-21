//! The oEmbed media renderer (`Media/Renderer/OEmbed.php`).
//!
//! Uses the stored response for markup, and falls back to a hyperlink when the
//! response carries nothing renderable.

use serde_json::Map;

use crate::data::{MediaData, OembedData};
use crate::oembed::{OembedAllowList, render_oembed};
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
    /// the response title or the source. The response's `html` and a `photo`'s
    /// alt come from [`render_oembed`].
    fn render(&self, _view: &View<'_>, media: &MediaView, _options: &RenderOptions) -> String {
        let data = match &media.data {
            MediaData::Oembed(data) => data.clone(),
            _ => OembedData { map: Map::new() },
        };
        let source = media.media.source.as_deref().unwrap_or_default();
        let providers = OembedAllowList::shipped().hosts();
        if let Some(markup) = render_oembed(&data, source, media.alt_text.as_deref(), &providers) {
            return markup;
        }
        let title = data.title().unwrap_or_else(|| source.to_string());
        hyperlink(Some(&title), Some(source), &[])
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
        let mut media = media_view_with_data(data);
        media.media.source = Some("https://www.youtube.com/watch?v=x".to_string());
        media
    }

    /// A response with an `html` member keeps its provider frame, now with the
    /// sandbox the sanitizer sets (`Renderer/OEmbed.php:31`).
    #[test]
    fn oembed_renderer_keeps_a_provider_frame_with_a_sandbox() {
        let registry = FileRendererRegistry::with_defaults();
        let media = oembed_media(
            serde_json::json!({"html": "<iframe src=\"https://www.youtube.com/embed/x\"></iframe>"}),
        );
        assert_eq!(
            OEmbed.render(&view(&registry), &media, &RenderOptions::default()),
            "<iframe src=\"https://www.youtube.com/embed/x\" sandbox=\"allow-scripts allow-same-origin allow-popups allow-presentation\"></iframe>"
        );
    }

    /// A provider response carrying a script and an event handler renders
    /// neither, and a frame from a host the provider rules do not allow is
    /// dropped with it.
    #[test]
    fn oembed_renderer_sanitizes_the_provider_html() {
        let registry = FileRendererRegistry::with_defaults();
        let media = oembed_media(serde_json::json!({
            "html": "<p onmouseover=\"steal()\">Watch</p><script>alert(1)</script><iframe src=\"https://evil.example/embed\"></iframe>",
        }));
        let out = OEmbed.render(&view(&registry), &media, &RenderOptions::default());
        assert_eq!(out, "<p>Watch</p>");
    }

    /// A `photo` takes its alt from the response title, else the media's
    /// resolved alt, else nothing.
    #[test]
    fn oembed_renderer_takes_the_photo_alt_from_the_title_then_the_media() {
        let registry = FileRendererRegistry::with_defaults();
        let photo =
            serde_json::json!({"type": "photo", "url": "https://x/y.jpg", "title": "A cat"});
        let titled = oembed_media(photo);
        assert!(
            OEmbed
                .render(&view(&registry), &titled, &RenderOptions::default())
                .contains("alt=\"A cat\"")
        );

        let mut untitled =
            oembed_media(serde_json::json!({"type": "photo", "url": "https://x/y.jpg"}));
        untitled.alt_text = Some("A photo of a cat".to_string());
        assert!(
            OEmbed
                .render(&view(&registry), &untitled, &RenderOptions::default())
                .contains("alt=\"A photo of a cat\"")
        );

        let mut no_alt =
            oembed_media(serde_json::json!({"type": "photo", "url": "https://x/y.jpg"}));
        no_alt.alt_text = None;
        assert!(
            OEmbed
                .render(&view(&registry), &no_alt, &RenderOptions::default())
                .contains("alt=\"\"")
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
