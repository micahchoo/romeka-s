//! The fallback file renderer (`Media/FileRenderer/FallbackRenderer.php`).
//!
//! A hyperlink to the original file, the item or the media.

use crate::file_render::{FileRenderer, LinkType};
use crate::render::{MediaView, RenderOptions};
use crate::text::hyperlink;

/// The fallback file renderer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Fallback;

impl FileRenderer for Fallback {
    fn name(&self) -> &'static str {
        "fallback"
    }

    /// `FileRenderer/FallbackRenderer.php:8`: a hyperlink titled by the
    /// filename, targeting the original by default.
    fn render(&self, media: &MediaView, options: &RenderOptions) -> String {
        let link_type = options.link.unwrap_or(LinkType::Original);
        let url = link_type.url(media).unwrap_or_default();
        hyperlink(media.media.filename().as_deref(), Some(url), &[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{media_row, media_view_from};

    fn fallback_media() -> MediaView {
        let mut media = media_row(7, "upload", "file");
        media.storage_id = Some("abc123".into());
        media.extension = Some("pdf".into());
        media.media_type = Some("application/pdf".into());
        media.has_original = true;
        media.source = Some("paper.pdf".into());
        media_view_from(&media)
    }

    /// `FileRenderer/FallbackRenderer.php:8`: the filename, linked to the
    /// original.
    #[test]
    fn fallback_renderer_links_the_original_by_default() {
        assert_eq!(
            Fallback.render(&fallback_media(), &RenderOptions::default()),
            "<a href=\"/files/original/abc123.pdf\">abc123.pdf</a>"
        );
    }

    /// The `link` option picks the item or media URL instead
    /// (`FileRenderer/AbstractRenderer.php:19`).
    #[test]
    fn fallback_renderer_honours_the_link_option() {
        let options = RenderOptions {
            link: Some(LinkType::Item),
            ..RenderOptions::default()
        };
        assert_eq!(
            Fallback.render(&fallback_media(), &options),
            "<a href=\"/item/7\">abc123.pdf</a>"
        );
        let options = RenderOptions {
            link: Some(LinkType::Media),
            ..RenderOptions::default()
        };
        assert_eq!(
            Fallback.render(&fallback_media(), &options),
            "<a href=\"/media/7\">abc123.pdf</a>"
        );
    }

    /// A row with no original file has no URL to link to, so the href is empty.
    #[test]
    fn fallback_renderer_emits_an_empty_href_without_a_url() {
        let mut media = fallback_media();
        media.media.has_original = false;
        media.original_url = None;
        assert_eq!(
            Fallback.render(&media, &RenderOptions::default()),
            "<a href=\"\">abc123.pdf</a>"
        );
    }
}
