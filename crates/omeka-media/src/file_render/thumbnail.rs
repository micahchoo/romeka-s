//! The thumbnail file renderer (`Media/FileRenderer/ThumbnailRenderer.php`).
//!
//! Wraps a thumbnail image in a link to the original, the item or the media.

use crate::file_render::{FileRenderer, LinkType, ThumbnailType};
use crate::render::{MediaView, RenderOptions};
use crate::text::{html_attribs, hyperlink_raw};

/// The thumbnail renderer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Thumbnail;

impl FileRenderer for Thumbnail {
    fn name(&self) -> &'static str {
        "thumbnail"
    }

    /// `FileRenderer/ThumbnailRenderer.php:12`. The default type is `large` and
    /// the default link is the original; a missing link URL leaves the image
    /// unlinked.
    fn render(&self, media: &MediaView, options: &RenderOptions) -> String {
        let kind = options.thumbnail_type.unwrap_or(ThumbnailType::Large);
        let mut attribs = options.thumbnail_attribs.clone();
        attribs.push((
            "src".to_string(),
            media.thumbnail_url(kind).unwrap_or_default().to_string(),
        ));
        if !attribs.iter().any(|(key, _)| key == "alt")
            && let Some(alt) = &media.alt_text
        {
            attribs.push(("alt".to_string(), alt.clone()));
        }
        let img = format!("<img{}>", html_attribs(&attribs));

        let link_type = options.link.unwrap_or(LinkType::Original);
        let Some(url) = link_type.url(media) else {
            return img;
        };
        let mut link_attribs = options.link_attribs.clone();
        if !link_attribs.iter().any(|(key, _)| key == "title") {
            link_attribs.push(("title".to_string(), media.display_title.clone()));
        }
        hyperlink_raw(&img, Some(url), &link_attribs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{media_row, media_view_from};

    fn thumbnail_media() -> MediaView {
        let mut media = media_row(7, "upload", "file");
        media.storage_id = Some("abc123".into());
        media.extension = Some("jpg".into());
        media.media_type = Some("image/jpeg".into());
        media.has_original = true;
        media.has_thumbnails = true;
        media.source = Some("photo.jpg".into());
        media_view_from(&media)
    }

    /// `FileRenderer/ThumbnailRenderer.php:12`: the `large` thumbnail, linked to
    /// the original, titled by the display title. `alt` is omitted because no
    /// alt text is set.
    #[test]
    fn thumbnail_renderer_matches_upstream_defaults() {
        assert_eq!(
            Thumbnail.render(&thumbnail_media(), &RenderOptions::default()),
            "<a title=\"photo.jpg\" href=\"/files/original/abc123.jpg\"><img src=\"/files/large/abc123.jpg\"></a>"
        );
    }

    /// `FileRenderer/ThumbnailRenderer.php:13`: the type and link come from the
    /// options, and extra attributes are emitted before `src`.
    #[test]
    fn thumbnail_renderer_uses_the_options() {
        let options = RenderOptions {
            thumbnail_type: Some(ThumbnailType::Square),
            link: Some(LinkType::Media),
            thumbnail_attribs: vec![("class".to_string(), "thumb".to_string())],
            ..RenderOptions::default()
        };
        assert_eq!(
            Thumbnail.render(&thumbnail_media(), &options),
            "<a title=\"photo.jpg\" href=\"/media/7\"><img class=\"thumb\" src=\"/files/square/abc123.jpg\"></a>"
        );
    }

    /// An explicit `alt` is kept in place; `linkAttribs` supplies the title.
    #[test]
    fn thumbnail_renderer_honours_alt_and_link_attributes() {
        let options = RenderOptions {
            thumbnail_attribs: vec![("alt".to_string(), "A photo".to_string())],
            link_attribs: vec![("class".to_string(), "media-link".to_string())],
            ..RenderOptions::default()
        };
        assert_eq!(
            Thumbnail.render(&thumbnail_media(), &options),
            "<a class=\"media-link\" title=\"photo.jpg\" href=\"/files/original/abc123.jpg\"><img alt=\"A photo\" src=\"/files/large/abc123.jpg\"></a>"
        );
    }

    /// A row with no original file leaves the image unlinked
    /// (`FileRenderer/ThumbnailRenderer.php:23`).
    #[test]
    fn thumbnail_renderer_skips_the_link_without_an_original() {
        let mut media = thumbnail_media();
        media.media.has_original = false;
        media.original_url = None;
        assert_eq!(
            Thumbnail.render(&media, &RenderOptions::default()),
            "<img src=\"/files/large/abc123.jpg\">"
        );
    }
}
