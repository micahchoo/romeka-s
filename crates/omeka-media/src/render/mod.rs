//! Media renderers.
//!
//! One module per upstream class under `application/src/Media/Renderer/`:
//! [`file`], [`html`], [`iiif`], [`iiif_presentation`], [`oembed`], [`youtube`]
//! and [`fallback`]. [`RendererRegistry`] mirrors `Manager.php`.
//!
//! A renderer is a pure function from a [`MediaView`] and [`RenderOptions`] to
//! an HTML fragment. [`MediaView`] is the boundary: it resolves the URLs,
//! display title and typed payload once, so no renderer reaches a store, a
//! database or a settings service.

pub mod fallback;
pub mod file;
pub mod html;
pub mod iiif;
pub mod iiif_presentation;
pub mod oembed;
pub mod youtube;

use std::collections::BTreeMap;
use std::sync::Arc;

use omeka_core::resource::Media;

use crate::data::MediaData;
use crate::file_render::{FileRendererRegistry, LinkType, ThumbnailType};

/// Site and server configuration a renderer may read.
///
/// The values mirror what upstream reaches through `$view`: `assetUrl` for the
/// OpenSeadragon prefix (`Renderer/IIIF.php:20`), `serverUrl` plus `basePath`
/// for absolute fallback thumbnails (`Renderer/MediaRepresentation.php:80`),
/// the file store base URI, the IIIF viewer route, and the two `iiif_viewer_*`
/// site settings (`Renderer/IiifPresentation.php:18`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderEnv {
    /// The server URL with no trailing slash, e.g. `https://example.org`.
    pub server_url: String,
    /// The Omeka application asset path, e.g. `/application/asset`.
    pub asset_base_path: String,
    /// The file store base URI, e.g. `/files`.
    pub file_base_uri: String,
    /// The canonical IIIF viewer route.
    pub viewer_url: String,
    /// The `iiif_viewer_sidebar` site setting.
    pub iiif_viewer_sidebar: bool,
    /// The `iiif_viewer_theme` site setting, `light` or `dark`.
    pub iiif_viewer_theme: String,
}

impl Default for RenderEnv {
    fn default() -> Self {
        Self {
            server_url: "https://example.org".to_string(),
            asset_base_path: "/application/asset".to_string(),
            file_base_uri: "/files".to_string(),
            viewer_url: "https://example.org/iiif-viewer".to_string(),
            iiif_viewer_sidebar: false,
            iiif_viewer_theme: "light".to_string(),
        }
    }
}

/// Everything a renderer can observe about one media row.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaView {
    /// The `media` row.
    pub media: Media,
    /// The typed `media.data` payload.
    pub data: MediaData,
    /// `displayTitle()`: the resource title, else the source, else `[Untitled]`
    /// (`MediaRepresentation.php:305`, `AbstractResourceEntityRepresentation.php:639`).
    pub display_title: String,
    /// The resolved alt text (`MediaRepresentation::altTextResolved`).
    pub alt_text: Option<String>,
    /// The parent item URL.
    pub item_url: String,
    /// The media URL.
    pub media_url: String,
    /// The original file URL, present only when the row has an original
    /// (`MediaRepresentation::originalUrl`).
    pub original_url: Option<String>,
    /// Thumbnail URLs keyed by type (`MediaRepresentation::thumbnailUrl`).
    pub thumbnails: BTreeMap<ThumbnailType, String>,
    /// The environment the renderer reads.
    pub env: RenderEnv,
}

impl MediaView {
    /// Resolve a media row into everything a renderer reads.
    #[must_use]
    pub fn new(
        media: Media,
        data: MediaData,
        env: RenderEnv,
        item_url: impl Into<String>,
        media_url: impl Into<String>,
    ) -> Self {
        let display_title = media
            .core
            .title
            .clone()
            .or_else(|| media.source.clone())
            .unwrap_or_else(|| "[Untitled]".to_string());
        let original_url = if media.has_original {
            media
                .filename()
                .map(|filename| format!("{}/original/{filename}", env.file_base_uri))
        } else {
            None
        };
        let thumbnails = ThumbnailType::ALL
            .into_iter()
            .map(|kind| (kind, thumbnail_url(&media, &env, kind)))
            .collect();
        let alt_text = media.alt_text.clone();
        Self {
            media,
            data,
            display_title,
            alt_text,
            item_url: item_url.into(),
            media_url: media_url.into(),
            original_url,
            thumbnails,
            env,
        }
    }

    /// The thumbnail URL for a type.
    #[must_use]
    pub fn thumbnail_url(&self, kind: ThumbnailType) -> Option<&str> {
        self.thumbnails.get(&kind).map(String::as_str)
    }
}

/// `MediaRepresentation::thumbnailUrl`: the stored thumbnail, or the configured
/// fallback asset when the row has no thumbnails
/// (`MediaRepresentation.php:64`, `application/config/module.config.php:240`).
fn thumbnail_url(media: &Media, env: &RenderEnv, kind: ThumbnailType) -> String {
    if media.has_thumbnails
        && let Some(storage_id) = media.storage_id.as_deref()
    {
        return format!("{}/{}/{storage_id}.jpg", env.file_base_uri, kind.as_str());
    }
    let fallback = match media
        .media_type
        .as_deref()
        .and_then(|value| value.split('/').next())
    {
        Some("image") => "image.png",
        Some("video") => "video.png",
        Some("audio") => "audio.png",
        _ => "default.png",
    };
    format!(
        "{}{}/thumbnails/{fallback}",
        env.server_url, env.asset_base_path
    )
}

/// The options a caller passes to a renderer.
///
/// Upstream passes an associative array; the port names each key it reads.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RenderOptions {
    /// `width`, for the video and IIIF viewers.
    pub width: Option<String>,
    /// `height`, for the video and IIIF viewers.
    pub height: Option<String>,
    /// `link`, the file renderer link target.
    pub link: Option<LinkType>,
    /// `thumbnailType`, the file renderer thumbnail type.
    pub thumbnail_type: Option<ThumbnailType>,
    /// `thumbnailAttribs`, extra attributes on the thumbnail image.
    pub thumbnail_attribs: Vec<(String, String)>,
    /// `linkAttribs`, extra attributes on the wrapping link.
    pub link_attribs: Vec<(String, String)>,
    /// `autoplay`.
    pub autoplay: Option<bool>,
    /// `controls`, default true for audio and video.
    pub controls: Option<bool>,
    /// `loop`.
    pub loop_playback: Option<bool>,
    /// `muted`.
    pub muted: Option<bool>,
    /// `poster`, for the video renderer.
    pub poster: Option<String>,
    /// `allowfullscreen`, default true for the YouTube renderer.
    pub allowfullscreen: Option<bool>,
    /// `title`, for the IIIF viewer iframe.
    pub title: Option<String>,
}

/// The registries a renderer may reach, the port's shape for `$view`.
pub struct View<'a> {
    /// The file renderer registry, which `Renderer/File.php` delegates to.
    pub file_renderers: &'a FileRendererRegistry,
}

/// A media renderer (`Media/Renderer/RendererInterface.php`).
pub trait Renderer {
    /// The registry name, e.g. `html`.
    fn name(&self) -> &str;

    /// Emit the HTML fragment for one media row.
    fn render(&self, view: &View<'_>, media: &MediaView, options: &RenderOptions) -> String;
}

/// A renderer that can also supply full-text search text
/// (`Media/Renderer/FulltextSearchableInterface.php`).
pub trait FulltextSearchable {
    /// The plain text of the media.
    fn fulltext_text(&self, media: &MediaView) -> String;
}

/// The media renderer registry, mirroring `Media/Renderer/Manager.php`.
pub struct RendererRegistry {
    renderers: BTreeMap<String, Arc<dyn Renderer>>,
}

impl Default for RendererRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RendererRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            renderers: BTreeMap::new(),
        }
    }

    /// The six renderers from `media_renderers`
    /// (`application/config/module.config.php:872`).
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(youtube::Youtube));
        registry.register(Box::new(html::Html));
        registry.register(Box::new(iiif::Iiif));
        registry.register(Box::new(iiif_presentation::IiifPresentation));
        registry.register(Box::new(file::File));
        registry.register(Box::new(oembed::OEmbed));
        registry
    }

    /// Add a renderer under its own name.
    pub fn register(&mut self, renderer: Box<dyn Renderer>) {
        let name = renderer.name().to_string();
        self.renderers.insert(name, Arc::from(renderer));
    }

    /// Get a renderer by name, falling back to [`fallback::Fallback`] for an
    /// unknown name, exactly as `Manager::get` does (`Renderer/Manager.php:18`).
    #[must_use]
    pub fn get(&self, name: &str) -> Arc<dyn Renderer> {
        match self.renderers.get(name) {
            Some(renderer) => Arc::clone(renderer),
            None => Arc::new(fallback::Fallback),
        }
    }

    /// Borrow a registered renderer, or `None` when the name is unknown.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&dyn Renderer> {
        self.renderers.get(name).map(AsRef::as_ref)
    }

    /// The registered names, sorted.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.renderers.keys().cloned().collect()
    }

    /// Render a named renderer, or `""` for an unknown name.
    #[must_use]
    pub fn render(
        &self,
        name: &str,
        view: &View<'_>,
        media: &MediaView,
        options: &RenderOptions,
    ) -> String {
        self.get(name).render(view, media, options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::media_view;

    /// `Manager::get` falls back to `Renderer\Fallback`, which renders nothing
    /// (`Renderer/Manager.php:18`, `Renderer/Fallback.php:12`).
    #[test]
    fn registry_falls_back_for_an_unknown_name() {
        let registry = RendererRegistry::with_defaults();
        let fallback = registry.get("does_not_exist");
        assert_eq!(fallback.name(), "fallback");
        let view = View {
            file_renderers: &FileRendererRegistry::with_defaults(),
        };
        assert_eq!(
            fallback.render(&view, &media_view(), &RenderOptions::default()),
            ""
        );
    }

    /// The six upstream renderers are registered under their configuration keys
    /// (`application/config/module.config.php:872`).
    #[test]
    fn registry_holds_the_upstream_renderers() {
        let registry = RendererRegistry::with_defaults();
        let names = registry.names();
        assert_eq!(
            names,
            vec![
                "file",
                "html",
                "iiif",
                "iiif_presentation",
                "oembed",
                "youtube"
            ]
        );
    }

    /// `MediaRepresentation::displayTitle` on a media row falls back to the
    /// source and then to `[Untitled]`.
    #[test]
    fn display_title_falls_back_to_source_then_untitled() {
        let mut media = crate::testing::media_row(1, "upload", "file");
        assert_eq!(media_view_for(&media).display_title, "[Untitled]");
        media.source = Some("https://example.org/a.jpg".into());
        assert_eq!(
            media_view_for(&media).display_title,
            "https://example.org/a.jpg"
        );
        media.core = media.core.clone().with_title("A title");
        assert_eq!(media_view_for(&media).display_title, "A title");
    }

    fn media_view_for(media: &Media) -> MediaView {
        MediaView::new(
            media.clone(),
            MediaData::None,
            RenderEnv::default(),
            "/item/1",
            "/media/1",
        )
    }
}
