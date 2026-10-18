//! The YouTube media renderer (`Media/Renderer/Youtube.php`).
//!
//! Builds the embed URL from the stored video id and the `start`/`end` members,
//! and emits the iframe upstream emits.

use crate::data::MediaData;
use crate::render::{MediaView, RenderOptions, Renderer, View};
use crate::text::escape_html;
use crate::uri::HttpUri;

/// The default iframe width (`Renderer/Youtube.php:11`).
pub const DEFAULT_WIDTH: u32 = 420;
/// The default iframe height (`Renderer/Youtube.php:12`).
pub const DEFAULT_HEIGHT: u32 = 315;

/// The YouTube renderer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Youtube;

impl Renderer for Youtube {
    fn name(&self) -> &str {
        "youtube"
    }

    /// `Renderer/Youtube.php:16`. Defaults: 420 by 315, fullscreen allowed.
    fn render(&self, _view: &View<'_>, media: &MediaView, options: &RenderOptions) -> String {
        let MediaData::Youtube(data) = &media.data else {
            return String::new();
        };
        let width = options
            .width
            .clone()
            .unwrap_or_else(|| DEFAULT_WIDTH.to_string());
        let height = options
            .height
            .clone()
            .unwrap_or_else(|| DEFAULT_HEIGHT.to_string());
        let allow_fullscreen = options.allowfullscreen.unwrap_or(true);

        let mut query = Vec::new();
        if let Some(start) = &data.start {
            query.push(("start".to_string(), start.clone()));
        }
        if let Some(end) = &data.end {
            query.push(("end".to_string(), end.clone()));
        }
        let embed = HttpUri::parse(&format!("https://www.youtube.com/embed/{}", data.id))
            .map(|uri| uri.with_query(&query))
            .unwrap_or_default();

        format!(
            "<iframe width=\"{}\" height=\"{}\" src=\"{}\" title=\"{}\" frameborder=\"0\"{}></iframe>",
            escape_html(&width),
            escape_html(&height),
            escape_html(&embed),
            escape_html(&media.display_title),
            if allow_fullscreen {
                " allowfullscreen"
            } else {
                ""
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::YoutubeData;
    use crate::file_render::FileRendererRegistry;
    use crate::testing::media_view_with_data;

    fn youtube_media(start: Option<&str>, end: Option<&str>) -> MediaView {
        media_view_with_data(MediaData::Youtube(YoutubeData {
            id: "abc123".to_string(),
            start: start.map(str::to_string),
            end: end.map(str::to_string),
        }))
    }

    /// `Renderer/Youtube.php:44`: the default iframe, no query.
    #[test]
    fn youtube_renderer_matches_upstream_defaults() {
        let registry = FileRendererRegistry::with_defaults();
        let view = View {
            file_renderers: &registry,
        };
        let html = Youtube.render(&view, &youtube_media(None, None), &RenderOptions::default());
        assert_eq!(
            html,
            "<iframe width=\"420\" height=\"315\" src=\"https://www.youtube.com/embed/abc123\" title=\"[Untitled]\" frameborder=\"0\" allowfullscreen></iframe>"
        );
    }

    /// `Renderer/Youtube.php:32`: `start` and `end` become query parameters, and
    /// `width`, `height` and `allowfullscreen` come from the options.
    #[test]
    fn youtube_renderer_uses_options_and_query_parameters() {
        let registry = FileRendererRegistry::with_defaults();
        let view = View {
            file_renderers: &registry,
        };
        let options = RenderOptions {
            width: Some("640".to_string()),
            height: Some("480".to_string()),
            allowfullscreen: Some(false),
            ..RenderOptions::default()
        };
        let html = Youtube.render(&view, &youtube_media(Some("30"), Some("60")), &options);
        assert_eq!(
            html,
            "<iframe width=\"640\" height=\"480\" src=\"https://www.youtube.com/embed/abc123?start=30&amp;end=60\" title=\"[Untitled]\" frameborder=\"0\"></iframe>"
        );
    }

    /// The title comes from `displayTitle()`, so a source URL is escaped into
    /// the attribute.
    #[test]
    fn youtube_renderer_escapes_the_display_title() {
        let registry = FileRendererRegistry::with_defaults();
        let view = View {
            file_renderers: &registry,
        };
        let mut media = youtube_media(None, None);
        media.display_title = "a & b".to_string();
        let html = Youtube.render(&view, &media, &RenderOptions::default());
        assert!(html.contains("title=\"a &amp; b\""), "{html}");
    }
}
