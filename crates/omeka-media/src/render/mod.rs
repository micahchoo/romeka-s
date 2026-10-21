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
use crate::store::{Download, FileKind, FileUrls};

/// Site and server configuration a renderer may read.
///
/// The values mirror what upstream reaches through `$view`: `assetUrl` for the
/// OpenSeadragon prefix (`Renderer/IIIF.php:20`), `serverUrl` for absolute
/// fallback thumbnails (`Renderer/MediaRepresentation.php:80`), the file
/// store's URLs, the IIIF viewer route, and the two `iiif_viewer_*` site
/// settings (`Renderer/IiifPresentation.php:18`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderEnv {
    /// The server URL with no trailing slash, e.g. `https://example.org`.
    pub server_url: String,
    /// Where the theme's vendored files are served, e.g. `/assets`.
    pub asset_base_path: String,
    /// The stored files' URLs: `/files/...` on the local store.
    pub files: FileUrls,
    /// The canonical IIIF viewer route.
    pub viewer_url: String,
    /// The `iiif_viewer_sidebar` site setting.
    pub iiif_viewer_sidebar: bool,
    /// The `iiif_viewer_theme` site setting, `light` or `dark`.
    pub iiif_viewer_theme: String,
}

/// Where upstream's application assets are served. The fallback thumbnails are
/// the server's route at this path (`crates/omeka-server/src/assets.rs`), so a
/// renderer links them here beside whatever prefix the theme's own files use.
const APPLICATION_ASSET_PATH: &str = "/application/asset";

impl Default for RenderEnv {
    fn default() -> Self {
        Self {
            server_url: "https://example.org".to_string(),
            asset_base_path: "/assets".to_string(),
            files: FileUrls::local(""),
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
            media.filename().map(|filename| {
                let download = Download::of(
                    media.media_type.as_deref(),
                    media.source.as_deref(),
                    &filename,
                );
                env.files
                    .original(&format!("original/{filename}"), &download)
            })
        } else {
            None
        };
        let thumbnails = thumbnail_urls(&media, &env, media.core.is_public);
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

    /// The same view for a media whose item is public or not: a thumbnail URL
    /// is plain only when the media and its item are both public.
    #[must_use]
    pub fn in_item(mut self, item_is_public: bool) -> Self {
        self.thumbnails = thumbnail_urls(
            &self.media,
            &self.env,
            self.media.core.is_public && item_is_public,
        );
        self
    }

    /// The thumbnail URL for a type.
    #[must_use]
    pub fn thumbnail_url(&self, kind: ThumbnailType) -> Option<&str> {
        self.thumbnails.get(&kind).map(String::as_str)
    }

    /// The thumbnail URL for a type only when a derivative is stored. The plain
    /// [`MediaView::thumbnail_url`] answers the theme's placeholder for a media
    /// with none, which is what a card shows and what a video must not use as a
    /// poster.
    #[must_use]
    pub fn stored_thumbnail_url(&self, kind: ThumbnailType) -> Option<&str> {
        if self.media.has_thumbnails {
            self.thumbnail_url(kind)
        } else {
            None
        }
    }
}

/// `MediaRepresentation::thumbnailUrl`: the stored thumbnail, or the configured
/// fallback asset when the row has no thumbnails
/// (`MediaRepresentation.php:64`, `application/config/module.config.php:240`).
fn thumbnail_urls(media: &Media, env: &RenderEnv, public: bool) -> BTreeMap<ThumbnailType, String> {
    ThumbnailType::ALL
        .into_iter()
        .map(|kind| (kind, thumbnail_url(media, env, kind, public)))
        .collect()
}

fn thumbnail_url(media: &Media, env: &RenderEnv, kind: ThumbnailType, public: bool) -> String {
    if media.has_thumbnails
        && let Some(storage_id) = media.storage_id.as_deref()
    {
        return env.files.url(
            &format!("{}/{storage_id}.jpg", kind.as_str()),
            FileKind::Thumbnail { public },
        );
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
        "{}{APPLICATION_ASSET_PATH}/thumbnails/{fallback}",
        env.server_url
    )
}

/// A playhead offset into an audio or video file, in milliseconds.
///
/// A media fragment writes seconds (`#t=62`, `#t=1.5`) and the site's `?t=`
/// query reads the same unit. Milliseconds are the stored unit and seconds are
/// the printed one, the same split `romeka_media_extent` uses for a duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MediaStart(u64);

impl MediaStart {
    /// A start at `milliseconds` into the file.
    #[must_use]
    pub const fn from_millis(milliseconds: u64) -> Self {
        Self(milliseconds)
    }

    /// Parse a `?t=` value: seconds, with a fraction to the millisecond.
    /// `None` for a value that is not a finite, non-negative number, which a
    /// hand-written or stale link may carry.
    #[must_use]
    pub fn parse_seconds(value: &str) -> Option<Self> {
        let seconds: f64 = value.trim().parse().ok()?;
        if !seconds.is_finite() || seconds < 0.0 {
            return None;
        }
        let milliseconds = (seconds * 1000.0).round();
        if milliseconds > u64::MAX as f64 {
            return None;
        }
        Some(Self(milliseconds as u64))
    }

    /// This start, held inside a known run time. A link past the end plays the
    /// last moment rather than nothing.
    #[must_use]
    pub fn clamped_to(self, duration_ms: Option<u64>) -> Self {
        match duration_ms {
            Some(duration) if self.0 > duration => Self(duration),
            _ => self,
        }
    }

    /// The seconds a media fragment and a `?t=` link state.
    #[must_use]
    pub fn seconds(self) -> String {
        let whole = self.0 / 1000;
        let fraction = self.0 % 1000;
        if fraction == 0 {
            whole.to_string()
        } else {
            format!("{whole}.{fraction:03}")
                .trim_end_matches('0')
                .to_string()
        }
    }
}

/// The source URL a player opens at `start`: `url#t=62`, the media fragment a
/// browser honours with no script. One builder, so the video and the audio
/// player cue the same way.
#[must_use]
pub fn media_fragment(url: &str, start: Option<MediaStart>) -> String {
    let Some(start) = start else {
        return url.to_string();
    };
    let base = url.split_once('#').map_or(url, |(base, _)| base);
    format!("{base}#t={}", start.seconds())
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
    /// The moment a `?t=` link asks the player to open at. A recording writes
    /// it as a media fragment on its source; a kind a browser cannot play
    /// ignores it.
    pub start: Option<MediaStart>,
    /// `allowfullscreen`, default true for the YouTube renderer.
    pub allowfullscreen: Option<bool>,
    /// `title`, for the IIIF viewer iframe.
    pub title: Option<String>,
    /// Whether the embed is the file's own page, where a browser's document
    /// reader may draw. An item page or a block shows the file's card instead,
    /// so a page of files never loads one reader per file.
    pub reader: bool,
    /// The 3D viewer a `model` file renderer offers on the reader's press, with
    /// the words the site renders it in. `None` on a surface that has no
    /// viewer: the renderer then links the file instead of offering a button.
    pub model_reader: Option<ModelReader>,
}

/// The deferred 3D viewer: where the vendored library is served from, and the
/// words the facade says before and during the download. The site layer owns
/// the library's URL and the translated text; the renderer owns the markup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelReader {
    /// The library's URL on this origin, versioned by its bytes.
    pub library_url: String,
    /// The button's label, which states the library's download size.
    pub turn: String,
    /// The button's label once pressed, while the library loads.
    pub loading: String,
    /// The line shown when the library cannot load.
    pub failed: String,
    /// The line a reader without JavaScript sees.
    pub no_javascript: String,
}

/// A byte count as a page states it: `812 B`, `140 KB`, `2.3 MB`, `1.2 GB`.
/// One decimal from a megabyte up, none below it, 1024 bytes to the next unit.
#[must_use]
pub fn human_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    let value = bytes as f64;
    if bytes >= GB {
        format!("{:.1} GB", value / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", value / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", value / KB as f64)
    } else {
        format!("{bytes} B")
    }
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

    /// A file with no derivative of its own links upstream's fallback
    /// thumbnail, which the server serves under `/application/asset`, not
    /// under the theme's own `/assets` prefix.
    #[test]
    fn a_file_without_a_derivative_links_the_application_fallback() {
        let mut media = crate::testing::media_row(7, "upload", "file");
        media.media_type = Some("application/msword".into());
        media.extension = Some("doc".into());
        media.storage_id = Some("abc123".into());
        media.has_original = true;
        let env = RenderEnv {
            asset_base_path: "/assets".into(),
            ..RenderEnv::default()
        };
        let view = MediaView::new(media, MediaData::None, env, "/item/7", "/media/7");
        let file_renderers = FileRendererRegistry::with_defaults();
        let html = RendererRegistry::with_defaults().render(
            "file",
            &View {
                file_renderers: &file_renderers,
            },
            &view,
            &RenderOptions::default(),
        );
        assert!(
            html.contains(
                "<img src=\"https://example.org/application/asset/thumbnails/default.png\">"
            ),
            "{html}"
        );
    }

    /// A page states a download's size in whole bytes, whole kilobytes, or one
    /// decimal above, so "140 KB" is what the reader pays.
    #[test]
    fn a_size_is_stated_in_the_unit_a_reader_reads() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(812), "812 B");
        assert_eq!(human_size(143_378), "140 KB");
        assert_eq!(human_size(2_411_724), "2.3 MB");
        assert_eq!(human_size(1_288_490_188), "1.2 GB");
    }

    /// A `?t=` value is seconds, and a value that is not a finite,
    /// non-negative number is no start at all.
    #[test]
    fn a_start_reads_seconds_and_refuses_what_is_not_a_number() {
        assert_eq!(
            MediaStart::parse_seconds("62"),
            Some(MediaStart::from_millis(62_000))
        );
        assert_eq!(
            MediaStart::parse_seconds(" 1.5 "),
            Some(MediaStart::from_millis(1_500))
        );
        assert_eq!(
            MediaStart::parse_seconds("0"),
            Some(MediaStart::from_millis(0))
        );
        for value in ["", "abc", "-1", "NaN", "inf", "1 second"] {
            assert_eq!(MediaStart::parse_seconds(value), None, "{value:?}");
        }
    }

    /// A start opens the file with no script: the media fragment carries the
    /// seconds, and no start leaves the URL byte for byte as it was.
    #[test]
    fn a_start_is_a_media_fragment_on_the_source() {
        assert_eq!(media_fragment("/files/a.mp4", None), "/files/a.mp4");
        assert_eq!(
            media_fragment("/files/a.mp4", Some(MediaStart::from_millis(62_000))),
            "/files/a.mp4#t=62"
        );
        assert_eq!(
            media_fragment(
                "https://s3.example/a.mp4?X-Amz-Signature=abc",
                Some(MediaStart::from_millis(1_500))
            ),
            "https://s3.example/a.mp4?X-Amz-Signature=abc#t=1.5"
        );
    }

    /// A link past the end plays the last moment; with no probed run time the
    /// link is kept as it was written.
    #[test]
    fn a_start_past_the_duration_is_held_at_the_duration() {
        let start = MediaStart::from_millis(100_000);
        assert_eq!(
            start.clamped_to(Some(62_346)),
            MediaStart::from_millis(62_346)
        );
        assert_eq!(start.clamped_to(None), start);
        assert_eq!(
            MediaStart::from_millis(2_000).clamped_to(Some(62_346)),
            MediaStart::from_millis(2_000)
        );
    }
}
