//! The video file renderer (`Media/FileRenderer/VideoRenderer.php`).

use crate::file_render::FileRenderer;
use crate::render::{MediaView, RenderOptions};
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
    fn render(&self, media: &MediaView, options: &RenderOptions) -> String {
        let original = media.original_url.clone().unwrap_or_default();
        let mut attrs = vec![format!("src=\"{}\"", escape_html(&original))];
        if let Some(width) = &options.width {
            attrs.push(format!("width=\"{}\"", escape_html(width)));
        }
        if let Some(height) = &options.height {
            attrs.push(format!("height=\"{}\"", escape_html(height)));
        }
        if let Some(poster) = &options.poster {
            attrs.push(format!("poster=\"{}\"", escape_html(poster)));
        }
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
    use crate::testing::{media_row, media_view_from};

    fn video_media() -> MediaView {
        let mut media = media_row(1, "upload", "file");
        media.storage_id = Some("abc123".into());
        media.extension = Some("mp4".into());
        media.media_type = Some("video/mp4".into());
        media.has_original = true;
        media_view_from(&media)
    }

    /// `FileRenderer/VideoRenderer.php:19`: `controls` defaults on and no size
    /// attributes are added.
    #[test]
    fn video_renderer_matches_upstream_defaults() {
        assert_eq!(
            Video.render(&video_media(), &RenderOptions::default()),
            "<video src=\"/files/original/abc123.mp4\" controls><a href=\"/files/original/abc123.mp4\">abc123.mp4</a></video>"
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
            "<video src=\"/files/original/abc123.mp4\" width=\"640\" height=\"480\" poster=\"/files/large/abc123.jpg\" autoplay><a href=\"/files/original/abc123.mp4\">abc123.mp4</a></video>"
        );
    }
}
