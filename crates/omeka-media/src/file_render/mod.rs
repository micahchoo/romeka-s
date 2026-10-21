//! File renderers: the HTML for a stored file, keyed by media type.
//!
//! One module per upstream class under `application/src/Media/FileRenderer/`:
//! [`audio`] (`AudioRenderer.php`), [`video`] (`VideoRenderer.php`),
//! [`thumbnail`] (`ThumbnailRenderer.php`) and [`fallback`]
//! (`FallbackRenderer.php`), plus this fork's [`pdf`] and [`model`] renderers.
//! [`FileRendererRegistry`] mirrors `Manager.php`, including its alias table
//! from `application/config/module.config.php:897`.

pub mod audio;
pub mod fallback;
pub mod model;
pub mod pdf;
pub mod thumbnail;
pub mod video;

use std::collections::BTreeMap;

use crate::render::{MediaView, RenderOptions};

/// The three thumbnail types upstream configures under `thumbnails.types`
/// (`application/config/module.config.php:222`).
///
/// Each type carries its pixel constraint, so the file store and the thumbnail
/// renderer agree on the same three names without a second list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThumbnailType {
    /// `large`, constraint 800.
    Large,
    /// `medium`, constraint 400.
    Medium,
    /// `square`, constraint 200 with `gravity: center`.
    Square,
}

impl ThumbnailType {
    /// All three types in upstream configuration order.
    pub const ALL: [Self; 3] = [Self::Large, Self::Medium, Self::Square];

    /// The configuration key, which is also the storage prefix and the
    /// thumbnail URL segment.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Large => "large",
            Self::Medium => "medium",
            Self::Square => "square",
        }
    }

    /// The configured pixel constraint.
    #[must_use]
    pub const fn constraint(self) -> u32 {
        match self {
            Self::Large => 800,
            Self::Medium => 400,
            Self::Square => 200,
        }
    }

    /// Parse a configuration key back into a type.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.as_str() == name)
    }
}

/// The link target a file renderer wraps its markup in
/// (`FileRenderer/AbstractRenderer.php:19`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkType {
    /// The original file URL.
    Original,
    /// The parent item URL.
    Item,
    /// The media URL.
    Media,
}

impl LinkType {
    /// The option value upstream accepts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Item => "item",
            Self::Media => "media",
        }
    }

    /// Parse the `link` option. An unknown value is an error upstream
    /// (`AbstractRenderer::getLinkUrl` throws `InvalidArgumentException`).
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "original" => Some(Self::Original),
            "item" => Some(Self::Item),
            "media" => Some(Self::Media),
            _ => None,
        }
    }

    /// The URL this link type resolves to for a media row.
    #[must_use]
    pub fn url(self, media: &MediaView) -> Option<&str> {
        match self {
            Self::Original => media.original_url.as_deref(),
            Self::Item => Some(media.item_url.as_str()),
            Self::Media => Some(media.media_url.as_str()),
        }
    }
}

/// The one sentence a player stands in for while its copy is being made, or
/// after the copy failed. The site layer supplies the words.
pub(crate) fn access_wait(text: &str) -> String {
    format!(
        "<p class=\"media-access-wait\">{}</p>",
        crate::text::escape_html(text)
    )
}

/// A file renderer (`Media/FileRenderer/RendererInterface.php`).
pub trait FileRenderer {
    /// The registry name, e.g. `thumbnail`.
    fn name(&self) -> &'static str;

    /// Emit the HTML fragment for one media row.
    fn render(&self, media: &MediaView, options: &RenderOptions) -> String;
}

/// The file renderer registry, mirroring `Media/FileRenderer/Manager.php`.
///
/// Unlike the ingester and media renderer managers, this one has no fallback
/// override: a missing name is `None`, and `Media/Renderer/File.php:24` decides
/// what to do about it.
#[derive(Default)]
pub struct FileRendererRegistry {
    renderers: BTreeMap<&'static str, Box<dyn FileRenderer>>,
    aliases: BTreeMap<&'static str, &'static str>,
}

impl FileRendererRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The four upstream invokables plus the alias table
    /// (`application/config/module.config.php:891,897`), this fork's aliases
    /// for the formats upstream's table predates, and its two renderers
    /// upstream has none of.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(fallback::Fallback));
        registry.register(Box::new(thumbnail::Thumbnail));
        registry.register(Box::new(audio::Audio));
        registry.register(Box::new(video::Video));
        registry.register(Box::new(pdf::Pdf));
        registry.register(Box::new(model::Model));
        for (alias, target) in UPSTREAM_ALIASES {
            registry.alias(alias, target);
        }
        for (alias, target) in FORK_RENDERER_ALIASES {
            registry.alias(alias, target);
        }
        for (alias, target) in ACCESS_CONTAINER_ALIASES {
            registry.alias(alias, target);
        }
        registry
    }

    /// Add a renderer under its own name.
    pub fn register(&mut self, renderer: Box<dyn FileRenderer>) {
        let name = renderer.name();
        self.renderers.insert(name, renderer);
    }

    /// Point a media type or extension at a renderer name.
    pub fn alias(&mut self, alias: &'static str, target: &'static str) {
        self.aliases.insert(alias, target);
    }

    /// Look up a renderer by name or alias.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn FileRenderer> {
        let key: &str = self.aliases.get(name).copied().unwrap_or(name);
        self.renderers.get(key).map(Box::as_ref)
    }

    /// The names of the registered renderers, sorted.
    #[must_use]
    pub fn names(&self) -> Vec<&'static str> {
        self.renderers.keys().copied().collect()
    }

    /// The alias table, for tests and diagnostics.
    #[must_use]
    pub fn aliases(&self) -> &BTreeMap<&'static str, &'static str> {
        &self.aliases
    }
}

/// The `file_renderers.aliases` map from `application/config/module.config.php:897`,
/// then the formats this fork accepts that upstream's table predates: FLAC and
/// Matroska by type, and their extensions for a row that stored no type.
const UPSTREAM_ALIASES: &[(&str, &str)] = &[
    ("audio/ogg", "audio"),
    ("audio/x-aac", "audio"),
    ("audio/mpeg", "audio"),
    ("audio/mp4", "audio"),
    ("audio/x-wav", "audio"),
    ("audio/x-aiff", "audio"),
    ("application/ogg", "video"),
    ("video/mp4", "video"),
    ("video/quicktime", "video"),
    ("video/x-msvideo", "video"),
    ("video/ogg", "video"),
    ("video/webm", "video"),
    ("mp3", "audio"),
    ("audio/flac", "audio"),
    ("video/x-matroska", "video"),
    ("flac", "audio"),
    ("mkv", "video"),
    ("ogv", "video"),
    ("opus", "audio"),
];

/// The media types and extensions this fork's two extra file renderers answer
/// to. Upstream has no PDF or 3D renderer, so a PDF drew its thumbnail and a
/// model fell through to the fallback link.
///
/// The office document types reach the PDF renderer too: once the document's
/// access copy exists, the reader opens that PDF, and until then the renderer
/// draws the thumbnail card the page had.
const FORK_RENDERER_ALIASES: &[(&str, &str)] = &[
    ("application/pdf", "pdf"),
    ("pdf", "pdf"),
    ("application/msword", "pdf"),
    ("application/rtf", "pdf"),
    ("application/vnd.ms-excel", "pdf"),
    ("application/vnd.ms-powerpoint", "pdf"),
    ("application/vnd.oasis.opendocument.presentation", "pdf"),
    ("application/vnd.oasis.opendocument.spreadsheet", "pdf"),
    ("application/vnd.oasis.opendocument.text", "pdf"),
    (
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "pdf",
    ),
    (
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pdf",
    ),
    (
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "pdf",
    ),
    ("doc", "pdf"),
    ("docx", "pdf"),
    ("odp", "pdf"),
    ("ods", "pdf"),
    ("odt", "pdf"),
    ("ppt", "pdf"),
    ("pptx", "pdf"),
    ("rtf", "pdf"),
    ("xls", "pdf"),
    ("xlsx", "pdf"),
    ("model/gltf-binary", "model"),
    ("model/gltf+json", "model"),
    ("glb", "model"),
    ("gltf", "model"),
];

/// The audio and video containers the access-copy table names. A browser may
/// not play one as stored, and its access copy plays; the page still needs a
/// player element to open either, so each names `audio` or `video`. Without
/// this a WMA page drew the fallback thumbnail instead of a player.
const ACCESS_CONTAINER_ALIASES: &[(&str, &str)] = &[
    ("audio/x-ms-wma", "audio"),
    ("audio/x-ms-wax", "audio"),
    ("audio/x-ape", "audio"),
    ("audio/x-monkeys-audio", "audio"),
    ("audio/x-realaudio", "audio"),
    ("audio/amr", "audio"),
    ("audio/x-aiff", "audio"),
    ("video/mpeg", "video"),
    ("video/x-ms-wmv", "video"),
    ("video/x-ms-asf", "video"),
    ("video/x-ms-vob", "video"),
    ("video/x-dv", "video"),
    ("video/x-flv", "video"),
    ("video/divx", "video"),
    ("video/dvd", "video"),
    ("video/3gpp", "video"),
    ("wma", "audio"),
    ("wax", "audio"),
    ("ape", "audio"),
    ("ra", "audio"),
    ("amr", "audio"),
    ("aif", "audio"),
    ("aiff", "audio"),
    ("mpg", "video"),
    ("mpeg", "video"),
    ("mp2", "video"),
    ("wmv", "video"),
    ("asf", "video"),
    ("vob", "video"),
    ("dv", "video"),
    ("flv", "video"),
    ("divx", "video"),
    ("3gp", "video"),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The three types and their constraints come from
    /// `application/config/module.config.php:222`.
    #[test]
    fn thumbnail_types_match_upstream_configuration() {
        assert_eq!(ThumbnailType::Large.constraint(), 800);
        assert_eq!(ThumbnailType::Medium.constraint(), 400);
        assert_eq!(ThumbnailType::Square.constraint(), 200);
        assert_eq!(
            ThumbnailType::ALL.map(ThumbnailType::as_str),
            ["large", "medium", "square"]
        );
    }

    /// `Media/FileRenderer/Manager.php` aliases a media type or extension to a
    /// renderer, so `get('audio/mpeg')` and `get('mp3')` both reach `audio`.
    #[test]
    fn registry_resolves_upstream_aliases() {
        let registry = FileRendererRegistry::with_defaults();
        assert_eq!(
            registry.get("audio/mpeg").map(FileRenderer::name),
            Some("audio")
        );
        assert_eq!(registry.get("mp3").map(FileRenderer::name), Some("audio"));
        assert_eq!(
            registry.get("video/webm").map(FileRenderer::name),
            Some("video")
        );
        assert_eq!(
            registry.get("application/ogg").map(FileRenderer::name),
            Some("video")
        );
    }

    /// A WMA, an MPEG-2, a DV and every other container the access-copy table
    /// names reaches a player, by type and by extension, so its page opens
    /// either the original or the copy.
    #[test]
    fn registry_resolves_the_containers_the_copy_table_names() {
        let registry = FileRendererRegistry::with_defaults();
        for (name, renderer) in ACCESS_CONTAINER_ALIASES {
            assert_eq!(
                registry.get(name).map(FileRenderer::name),
                Some(*renderer),
                "{name}"
            );
        }
    }

    /// A name upstream does not register is `None`, which is what makes
    /// `Media/Renderer/File.php` fall back to `thumbnail`.
    #[test]
    fn registry_returns_none_for_unknown_name() {
        let registry = FileRendererRegistry::with_defaults();
        assert!(registry.get("application/zip").is_none());
        assert_eq!(
            registry.names(),
            vec!["audio", "fallback", "model", "pdf", "thumbnail", "video"]
        );
    }

    /// A PDF and a glTF model reach this fork's renderers by media type and by
    /// extension, so a row that stored no type still gets the reader.
    #[test]
    fn registry_resolves_the_pdf_and_model_renderers() {
        let registry = FileRendererRegistry::with_defaults();
        for (name, renderer) in [
            ("application/pdf", "pdf"),
            ("pdf", "pdf"),
            ("model/gltf-binary", "model"),
            ("model/gltf+json", "model"),
            ("glb", "model"),
            ("gltf", "model"),
        ] {
            assert_eq!(
                registry.get(name).map(FileRenderer::name),
                Some(renderer),
                "{name}"
            );
        }
    }

    /// An office document reaches the PDF renderer by its type, by the MIME
    /// types of the older Word, Excel and PowerPoint formats, and by every
    /// extension in the corpus, so its page can open the PDF copy.
    #[test]
    fn registry_resolves_every_office_document() {
        let registry = FileRendererRegistry::with_defaults();
        for extension in crate::kind::OFFICE_DOCUMENT_EXTENSIONS {
            assert_eq!(
                registry.get(extension).map(FileRenderer::name),
                Some("pdf"),
                "{extension}"
            );
        }
        for media_type in crate::kind::OFFICE_DOCUMENT_MEDIA_TYPES {
            assert_eq!(
                registry.get(media_type).map(FileRenderer::name),
                Some("pdf"),
                "{media_type}"
            );
        }
    }

    /// FLAC plays through the audio renderer and Matroska through the video
    /// one, by type and by extension.
    #[test]
    fn registry_resolves_the_widened_formats() {
        let registry = FileRendererRegistry::with_defaults();
        for (name, renderer) in [
            ("audio/flac", "audio"),
            ("flac", "audio"),
            ("opus", "audio"),
            ("video/x-matroska", "video"),
            ("mkv", "video"),
            ("ogv", "video"),
            ("video/ogg", "video"),
            ("audio/ogg", "audio"),
        ] {
            assert_eq!(
                registry.get(name).map(FileRenderer::name),
                Some(renderer),
                "{name}"
            );
        }
    }
}
