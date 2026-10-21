//! The file media renderer (`Media/Renderer/File.php`).
//!
//! Delegates to a file renderer, chosen by media type, then by extension, then
//! by the thumbnail renderer.

use crate::render::{MediaView, RenderOptions, Renderer, View};

/// The file renderer delegate.
#[derive(Debug, Clone, Copy, Default)]
pub struct File;

impl Renderer for File {
    fn name(&self) -> &str {
        "file"
    }

    /// `Renderer/File.php:24`: try the media type, then the extension, then
    /// `thumbnail`.
    ///
    /// Upstream lets the last lookup throw when no `thumbnail` renderer exists;
    /// the port returns an empty fragment instead, because a registry without
    /// the built-ins is a configuration error rather than a media error.
    fn render(&self, view: &View<'_>, media: &MediaView, options: &RenderOptions) -> String {
        let by_media_type = media
            .media
            .media_type
            .as_deref()
            .and_then(|name| view.file_renderers.get(name));
        let by_extension = media
            .media
            .extension
            .as_deref()
            .and_then(|name| view.file_renderers.get(name));
        match by_media_type
            .or(by_extension)
            .or_else(|| view.file_renderers.get("thumbnail"))
        {
            Some(renderer) => renderer.render(media, options),
            None => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_render::FileRendererRegistry;
    use crate::testing::{media_row, media_view_from};

    fn view(registry: &FileRendererRegistry) -> View<'_> {
        View {
            file_renderers: registry,
        }
    }

    /// `Renderer/File.php:24`: `video/mp4` resolves through the alias table to
    /// the video renderer.
    #[test]
    fn file_renderer_prefers_the_media_type() {
        let registry = FileRendererRegistry::with_defaults();
        let mut media = media_row(1, "upload", "file");
        media.media_type = Some("video/mp4".into());
        media.extension = Some("mp4".into());
        media.storage_id = Some("abc123".into());
        media.has_original = true;
        let media = media_view_from(&media);
        let html = File.render(&view(&registry), &media, &RenderOptions::default());
        assert!(html.starts_with("<video "), "{html}");
    }

    /// With no media type match, the extension decides.
    #[test]
    fn file_renderer_falls_back_to_the_extension() {
        let registry = FileRendererRegistry::with_defaults();
        let mut media = media_row(1, "upload", "file");
        media.media_type = Some("application/octet-stream".into());
        media.extension = Some("mp3".into());
        media.storage_id = Some("abc123".into());
        media.has_original = true;
        let media = media_view_from(&media);
        let html = File.render(&view(&registry), &media, &RenderOptions::default());
        assert!(html.starts_with("<audio "), "{html}");
    }

    /// With neither, `Renderer/File.php:30` reaches the thumbnail renderer.
    #[test]
    fn file_renderer_falls_back_to_the_thumbnail() {
        let registry = FileRendererRegistry::with_defaults();
        let mut media = media_row(1, "upload", "file");
        media.media_type = Some("application/msword".into());
        media.extension = Some("doc".into());
        media.storage_id = Some("abc123".into());
        media.has_thumbnails = true;
        let media = media_view_from(&media);
        let html = File.render(&view(&registry), &media, &RenderOptions::default());
        assert!(html.contains("<img "), "{html}");
    }
}
