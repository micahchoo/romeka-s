//! The YouTube media renderer (`Media/Renderer/Youtube.php`).
//!
//! Builds the embed URL from the stored video id and the `start`/`end` members,
//! and emits the iframe upstream emits, with two departures: the embed goes
//! through `youtube-nocookie.com`, and the frame loads lazily.

use crate::data::{MediaData, YoutubeData};
use crate::render::{MediaView, RenderOptions, Renderer, View};
use crate::text::escape_html;
use crate::uri::HttpUri;

/// The default iframe width (`Renderer/Youtube.php:11`).
pub const DEFAULT_WIDTH: u32 = 420;
/// The default iframe height (`Renderer/Youtube.php:12`).
pub const DEFAULT_HEIGHT: u32 = 315;

/// The host the embed goes through: no cookie reaches the player's origin
/// until the viewer plays the video.
pub const EMBED_HOST: &str = "www.youtube-nocookie.com";

/// The URL a stored video plays at, with the `start` and `end` the ingester
/// kept, or `None` when the id is not one YouTube video id.
///
/// The port's own renderer and the API's player route both call this, so the
/// player the public site embeds and the player the admin frames are one URL.
/// `data.id` is whatever an ingester read out of a URL, and a copied or edited
/// row could hold anything, so an id outside YouTube's alphabet is refused
/// here rather than escaped into another path or a second host.
#[must_use]
pub fn embed_url(data: &YoutubeData) -> Option<String> {
    let id = video_id(&data.id)?;
    let mut query = Vec::new();
    if let Some(start) = &data.start {
        query.push(("start".to_string(), start.clone()));
    }
    if let Some(end) = &data.end {
        query.push(("end".to_string(), end.clone()));
    }
    let uri = HttpUri::parse(&format!("https://{EMBED_HOST}/embed/{id}"))?;
    Some(uri.with_query(&query))
}

/// YouTube video ids are letters, digits, `_` and `-`, one path segment long.
fn video_id(id: &str) -> Option<&str> {
    (!id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'))
    .then_some(id)
}

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
        let Some(embed) = embed_url(data) else {
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

        format!(
            "<iframe width=\"{}\" height=\"{}\" src=\"{}\" title=\"{}\" frameborder=\"0\" loading=\"lazy\"{}></iframe>",
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

    /// `Renderer/Youtube.php:44` with two departures this fork makes: the embed
    /// goes through `youtube-nocookie.com`, and the frame is lazy so a page
    /// with many videos defers the ones below the fold.
    #[test]
    fn youtube_renderer_embeds_the_nocookie_host_and_loads_lazily() {
        let registry = FileRendererRegistry::with_defaults();
        let view = View {
            file_renderers: &registry,
        };
        let html = Youtube.render(&view, &youtube_media(None, None), &RenderOptions::default());
        assert_eq!(
            html,
            "<iframe width=\"420\" height=\"315\" src=\"https://www.youtube-nocookie.com/embed/abc123\" title=\"[Untitled]\" frameborder=\"0\" loading=\"lazy\" allowfullscreen></iframe>"
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
            "<iframe width=\"640\" height=\"480\" src=\"https://www.youtube-nocookie.com/embed/abc123?start=30&amp;end=60\" title=\"[Untitled]\" frameborder=\"0\" loading=\"lazy\"></iframe>"
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

    /// The player URL the public site embeds is the one the admin frames, and
    /// it carries the times the ingester kept.
    #[test]
    fn embed_url_is_the_nocookie_url_with_the_stored_times() {
        assert_eq!(
            embed_url(&YoutubeData {
                id: "abc123".to_string(),
                start: Some("30".to_string()),
                end: Some("60".to_string()),
            }),
            Some("https://www.youtube-nocookie.com/embed/abc123?start=30&end=60".to_string())
        );
        assert_eq!(
            embed_url(&YoutubeData {
                id: "abc123".to_string(),
                start: None,
                end: None,
            }),
            Some("https://www.youtube-nocookie.com/embed/abc123".to_string())
        );
    }

    /// An id that is not one YouTube video id names no URL: a slash would add a
    /// path, a quote would end the frame's `src`, and a full URL a host.
    #[test]
    fn embed_url_refuses_an_id_outside_the_video_id_alphabet() {
        for id in [
            "abc/def",
            "abc\"def",
            "abc def",
            "abc?x=1",
            "https://example.org/embed/x",
            "",
        ] {
            assert_eq!(
                embed_url(&YoutubeData {
                    id: id.to_string(),
                    start: Some("30".to_string()),
                    end: None,
                }),
                None,
                "{id}"
            );
        }
    }
}
