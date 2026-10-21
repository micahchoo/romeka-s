//! The audio file renderer (`Media/FileRenderer/AudioRenderer.php`).

use crate::file_render::FileRenderer;
use crate::render::{MediaView, RenderOptions, media_fragment};
use crate::text::{escape_html, hyperlink};

/// The audio renderer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Audio;

impl FileRenderer for Audio {
    fn name(&self) -> &'static str {
        "audio"
    }

    /// `FileRenderer/AudioRenderer.php:17`: a `<audio>` element with `src` and
    /// the truthy playback attributes, wrapping a hyperlink to the original.
    /// A start is the media fragment `#t=62` on `src`, so a `?t=` link opens at
    /// that moment with no script.
    fn render(&self, media: &MediaView, options: &RenderOptions) -> String {
        let original = media.original_url.clone().unwrap_or_default();
        let source = media_fragment(&original, options.start);
        let mut attrs = vec![format!("src=\"{}\"", escape_html(&source))];
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
        format!("<audio {}>{link}</audio>", attrs.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::MediaStart;
    use crate::testing::{media_row, media_view_from};

    fn audio_media() -> MediaView {
        let mut media = media_row(1, "upload", "file");
        media.storage_id = Some("abc123".into());
        media.extension = Some("mp3".into());
        media.media_type = Some("audio/mpeg".into());
        media.has_original = true;
        media.source = Some("song.mp3".into());
        media_view_from(&media)
    }

    /// `FileRenderer/AudioRenderer.php:17`: `controls` defaults on, and the
    /// hyperlink text is the filename.
    #[test]
    fn audio_renderer_matches_upstream_defaults() {
        assert_eq!(
            Audio.render(&audio_media(), &RenderOptions::default()),
            "<audio src=\"/files/original/abc123.mp3\" controls><a href=\"/files/original/abc123.mp3\">abc123.mp3</a></audio>"
        );
    }

    /// `FileRenderer/AudioRenderer.php:22`: the truthy flags become bare
    /// attributes, in upstream order.
    #[test]
    fn audio_renderer_adds_playback_attributes() {
        let options = RenderOptions {
            autoplay: Some(true),
            loop_playback: Some(true),
            muted: Some(true),
            ..RenderOptions::default()
        };
        assert_eq!(
            Audio.render(&audio_media(), &options),
            "<audio src=\"/files/original/abc123.mp3\" autoplay controls loop muted><a href=\"/files/original/abc123.mp3\">abc123.mp3</a></audio>"
        );
    }

    /// A start reaches the browser as a media fragment on the source, so a
    /// `?t=` link plays from that moment with no script.
    #[test]
    fn a_start_cues_the_audio_with_a_media_fragment() {
        let options = RenderOptions {
            start: Some(MediaStart::from_millis(1_500)),
            ..RenderOptions::default()
        };
        assert_eq!(
            Audio.render(&audio_media(), &options),
            "<audio src=\"/files/original/abc123.mp3#t=1.5\" controls><a href=\"/files/original/abc123.mp3\">abc123.mp3</a></audio>"
        );
    }
}
