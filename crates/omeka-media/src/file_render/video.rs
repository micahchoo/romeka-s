//! The video file renderer (`Media/FileRenderer/VideoRenderer.php`).

use crate::file_render::{FileRenderer, ThumbnailType};
use crate::render::{MediaView, RenderOptions, media_fragment};
use crate::text::{escape_html, hyperlink};

/// The video renderer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Video;

impl FileRenderer for Video {
    fn name(&self) -> &'static str {
        "video"
    }

    /// `FileRenderer/VideoRenderer.php:19`: a `<video>` element with `src`, the
    /// sizing attributes and the truthy playback attributes, wrapping a
    /// hyperlink to the original.
    ///
    /// Two deliberate differences: the poster defaults to the stored large
    /// derivative, so a video Romeka S made a still for never shows an empty
    /// black rectangle, and `preload="metadata"` is always sent, so a page with
    /// a two hour recording fetches its header rather than the whole file
    /// before anyone presses play. `options.poster` still wins.
    ///
    /// A third: `options.start` becomes the media fragment `#t=62` on `src`, so
    /// a `?t=` link opens at that moment with no script.
    fn render(&self, media: &MediaView, options: &RenderOptions) -> String {
        let original = media.original_url.clone().unwrap_or_default();
        let source = media_fragment(&original, options.start);
        let mut attrs = vec![format!("src=\"{}\"", escape_html(&source))];
        if let Some(width) = &options.width {
            attrs.push(format!("width=\"{}\"", escape_html(width)));
        }
        if let Some(height) = &options.height {
            attrs.push(format!("height=\"{}\"", escape_html(height)));
        }
        let poster = options.poster.clone().or_else(|| {
            media
                .stored_thumbnail_url(ThumbnailType::Large)
                .map(str::to_string)
        });
        if let Some(poster) = &poster {
            attrs.push(format!("poster=\"{}\"", escape_html(poster)));
        }
        attrs.push("preload=\"metadata\"".to_string());
        if options.autoplay.unwrap_or(false) {
            attrs.push("autoplay".to_string());
        }
        if options.controls.unwrap_or(true) {
            attrs.push("controls".to_string());
        }
        if options.loop_playback.unwrap_or(false) {
            attrs.push("loop".to_string());
        }
        if options.muted.unwrap_or(false) {
            attrs.push("muted".to_string());
        }
        let link = hyperlink(media.media.filename().as_deref(), Some(&original), &[]);
        format!("<video {}>{link}</video>", attrs.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::MediaStart;
    use crate::testing::{media_row, media_view_from};

    fn video_media() -> MediaView {
        let mut media = media_row(1, "upload", "file");
        media.storage_id = Some("abc123".into());
        media.extension = Some("mp4".into());
        media.media_type = Some("video/mp4".into());
        media.has_original = true;
        media_view_from(&media)
    }

    /// `FileRenderer/VideoRenderer.php:19`: `controls` defaults on, no size
    /// attributes, the placeholder is not a poster, and the header is preloaded
    /// rather than the whole file.
    #[test]
    fn video_renderer_matches_upstream_defaults() {
        assert_eq!(
            Video.render(&video_media(), &RenderOptions::default()),
            "<video src=\"/files/original/abc123.mp4\" preload=\"metadata\" controls><a href=\"/files/original/abc123.mp4\">abc123.mp4</a></video>"
        );
    }

    /// `FileRenderer/VideoRenderer.php:26`: width, height and poster are
    /// emitted in upstream order, before the playback attributes.
    #[test]
    fn video_renderer_emits_size_and_poster() {
        let options = RenderOptions {
            width: Some("640".to_string()),
            height: Some("480".to_string()),
            poster: Some("/files/large/abc123.jpg".to_string()),
            autoplay: Some(true),
            controls: Some(false),
            ..RenderOptions::default()
        };
        assert_eq!(
            Video.render(&video_media(), &options),
            "<video src=\"/files/original/abc123.mp4\" width=\"640\" height=\"480\" poster=\"/files/large/abc123.jpg\" preload=\"metadata\" autoplay><a href=\"/files/original/abc123.mp4\">abc123.mp4</a></video>"
        );
    }

    /// A video with a stored large derivative uses it as the poster with no
    /// caller passing one: the media page, the item page and a media block all
    /// render through this renderer.
    #[test]
    fn a_stored_large_derivative_is_the_default_poster() {
        let mut media = media_row(1, "upload", "file");
        media.storage_id = Some("abc123".into());
        media.extension = Some("mp4".into());
        media.media_type = Some("video/mp4".into());
        media.has_original = true;
        media.has_thumbnails = true;
        let html = Video.render(&media_view_from(&media), &RenderOptions::default());
        assert!(
            html.contains("poster=\"/files/large/abc123.jpg\""),
            "{html}"
        );
    }

    /// A video whose derivatives failed shows no poster, rather than the
    /// theme's video placeholder drawn as if it were a frame.
    #[test]
    fn a_video_without_derivatives_has_no_poster() {
        let html = Video.render(&video_media(), &RenderOptions::default());
        assert!(!html.contains("poster="), "{html}");
    }

    /// A start reaches the browser as a media fragment on the source, so a
    /// `?t=` link plays from that moment with no script.
    #[test]
    fn a_start_cues_the_video_with_a_media_fragment() {
        let options = RenderOptions {
            start: Some(MediaStart::from_millis(62_000)),
            ..RenderOptions::default()
        };
        assert_eq!(
            Video.render(&video_media(), &options),
            "<video src=\"/files/original/abc123.mp4#t=62\" preload=\"metadata\" controls><a href=\"/files/original/abc123.mp4\">abc123.mp4</a></video>"
        );
    }
}
