//! The bundled theme, behind the [`Theme`] seam, plus the registry that names
//! the themes the port ships.
//!
//! DESIGN.md fixes one bundled theme for public sites and a Rust trait as the
//! seam. [`DefaultTheme`] is that theme: the document frame from
//! `view/layout/layout.phtml` plus the site views core ships in
//! `view/omeka/site/*` (the views a theme overrides). Upstream's `themes/`
//! directory is empty at this SHA, so porting a PHP theme is not the goal; the
//! port renders the core site views as Rust HTML.
//!
//! A caller picks the theme by name and calls [`Theme::render`] with a
//! [`View`](crate::view::View). Everything the views print is escaped except
//! the two fragments upstream trusts: a stored `html` block and a provider's
//! oEmbed `html`.
//!
//! ## The theme contract
//!
//! A theme is a [`Theme`] implementation plus a [`ThemeInfo`] manifest. The
//! manifest carries the three things a caller needs and cannot derive from the
//! render call:
//!
//! - `name`, the value `site.theme` stores, which is the theme's directory name
//!   upstream (`Omeka\Site\Theme\Theme::getId`);
//! - `settings`, the theme's own settings (its `theme.ini` `[config]` spec
//!   upstream), stored under `theme_settings_<name>`. The bundled theme
//!   declares its design tokens here ([`tokens::SETTINGS`]); the site settings
//!   every core view reads are separate, in
//!   [`crate::settings::SITE_SETTINGS`];
//! - `assets`, the files the theme ships with their bytes, so a caller serves
//!   them from [`THEMES`] without a filesystem, and the document frame links
//!   them (upstream reads a theme's `asset/` directory).
//!
//! A second theme implements [`Theme`] and adds itself to [`THEMES`]. Nothing
//! else in the crate names [`DefaultTheme`].

pub mod cards;
pub mod choices;
pub mod color;
pub mod custom_css;
pub mod default;
mod files;
pub mod fonts;
pub mod footer;
pub mod palette;
pub mod presets;
pub mod search;
pub mod style;
pub mod tokens;
pub mod tokens_samples;
pub mod tokens_schema;
mod user_bar;

pub use default::{DEFAULT_INFO, DefaultTheme};
pub use tokens::ThemeSettings;

use crate::view::{RenderContext, View};

/// A public site theme.
///
/// The seam is deliberately narrow: a theme declares its manifest and turns a
/// view plus the render context into one HTML document. `Sync` lets the
/// registry live in a `static`.
pub trait Theme: Sync {
    /// The theme's manifest.
    fn info(&self) -> &'static ThemeInfo;

    /// The theme name, as `site.theme` stores it.
    fn name(&self) -> &'static str {
        self.info().name
    }

    /// Render a view into a full HTML document.
    fn render(&self, view: &View<'_>, ctx: &RenderContext<'_>) -> String;
}

/// What a theme asset is, which decides the media type a caller serves it with
/// and the element that loads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetKind {
    /// A stylesheet, linked in the head and served as `text/css`.
    Stylesheet,
    /// A script, loaded from the head and served as `application/javascript`.
    Script,
    /// A script a view loads where it needs it, served as
    /// `application/javascript`. The frame does not link it, so a library such
    /// as Mirador costs nothing on a page that does not use it.
    Library,
    /// An image a view references by URL, served as `image/svg+xml`. The frame
    /// does not link it; a view does.
    Image,
    /// A WOFF2 webfont, served as `font/woff2`. The token sheet names it in an
    /// `@font-face`, and the frame preloads the ones the site's fonts use.
    Font,
    /// A library's own stylesheet, such as Leaflet's, served as `text/css`.
    /// The frame does not link it; the view that loads the library does.
    LibraryStylesheet,
    /// A PNG image a library references by URL, such as OpenSeadragon's
    /// navigation buttons.
    Png,
}

impl AssetKind {
    /// The media type a caller serves the asset with.
    #[must_use]
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Stylesheet | Self::LibraryStylesheet => "text/css",
            Self::Script | Self::Library => "application/javascript",
            Self::Image => "image/svg+xml",
            Self::Font => "font/woff2",
            Self::Png => "image/png",
        }
    }

    /// Whether the document frame loads the asset from the head.
    #[must_use]
    pub const fn is_head(self) -> bool {
        matches!(self, Self::Stylesheet | Self::Script)
    }

    /// The element that loads the asset with the `href` the caller built.
    #[must_use]
    pub fn element(self, href: &str) -> String {
        match self {
            Self::Stylesheet => format!("<link rel=\"stylesheet\" href=\"{href}\">"),
            Self::Script => format!("<script src=\"{href}\"></script>"),
            Self::Library | Self::LibraryStylesheet | Self::Image | Self::Font | Self::Png => {
                String::new()
            }
        }
    }
}

/// One file a theme ships, with its bytes.
///
/// The bytes live here, compiled in, so a caller can serve `/assets/<path>`
/// straight from the registry and the document frame can link the same path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeAsset {
    /// The path under the theme's asset base, e.g. `css/style.css`.
    pub path: &'static str,
    /// What the asset is.
    pub kind: AssetKind,
    /// The asset's bytes.
    pub content: &'static [u8],
}

impl ThemeAsset {
    /// A digest of the bytes, 16 hex digits. The frame links
    /// `<path>?v=<version>`, so a deploy that changes the bytes changes the URL
    /// and the asset route may let a client keep one URL's bytes for good.
    #[must_use]
    pub fn version(&self) -> String {
        crate::blocks::canonical::byte_hash(self.content)
    }
}

/// A theme's manifest: the id a site stores, the settings it reads and the
/// files it ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeInfo {
    /// The `site.theme` value, the theme's directory name upstream.
    pub name: &'static str,
    /// The display name (`theme.ini` `name`, defaulting to the id).
    pub label: &'static str,
    /// The theme's own settings, its `theme.ini` `[config]` spec upstream, and
    /// the design tokens this theme renders. A site stores them under
    /// `theme_settings_<name>`, one JSON map, separate from the site settings a
    /// `site_setting` row holds per key.
    pub settings: &'static [tokens::ThemeSettingSpec],
    /// The theme's own files.
    pub assets: &'static [ThemeAsset],
}

/// The bundled themes, the registry [`theme_for`] reads.
pub static THEMES: [&dyn Theme; 1] = [&DefaultTheme];

/// Every bundled theme, in registry order.
#[must_use]
pub fn themes() -> &'static [&'static dyn Theme] {
    &THEMES
}

/// The theme for a stored `site.theme` value, `None` when it is not bundled.
#[must_use]
pub fn theme_for(name: &str) -> Option<&'static dyn Theme> {
    THEMES
        .iter()
        .copied()
        .find(|theme| theme.info().name == name)
}

/// The `<script>` element that loads one of the bundled theme's libraries
/// (an [`AssetKind::Library`]) from `asset_base`. The URL carries the bytes'
/// version, so the asset route lets a client keep them.
#[must_use]
pub fn library_script(asset_base: &str, path: &str) -> String {
    format!(
        "<script src=\"{}\"></script>",
        crate::html::escape_html(&library_url(asset_base, path))
    )
}

/// The `<link>` element that loads a library's stylesheet (an
/// [`AssetKind::LibraryStylesheet`]) from `asset_base`, versioned like
/// [`library_script`].
#[must_use]
pub fn library_stylesheet(asset_base: &str, path: &str) -> String {
    format!(
        "<link rel=\"stylesheet\" href=\"{}\">",
        crate::html::escape_html(&library_url(asset_base, path))
    )
}

fn library_url(asset_base: &str, path: &str) -> String {
    let base = asset_base.trim_end_matches('/');
    match theme_asset(DEFAULT_INFO.name, path) {
        Some(asset) => format!("{base}/{path}?v={}", asset.version()),
        None => format!("{base}/{path}"),
    }
}

/// A theme's asset by path, `None` when the theme is not bundled or does not
/// ship that path. The caller serves the bytes with
/// [`ThemeAsset::kind`]'s media type.
#[must_use]
pub fn theme_asset(theme: &str, path: &str) -> Option<&'static ThemeAsset> {
    theme_for(theme)?
        .info()
        .assets
        .iter()
        .find(|asset| asset.path == path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bundled theme answers to the name `default`.
    #[test]
    fn theme_for_resolves_the_bundled_theme() {
        let theme = theme_for("default").expect("the bundled theme is registered");
        assert_eq!(theme.name(), "default");
        assert!(theme_for("someThirdPartyTheme").is_none());
    }

    /// The registry lists the bundled theme with its manifest.
    #[test]
    fn theme_registry_lists_the_bundled_theme() {
        assert_eq!(themes().len(), 1);
        let info = themes()[0].info();
        assert_eq!(info.name, "default");
        assert_eq!(info.label, "Default");
        // The bundled theme declares its design tokens as its own settings.
        assert_eq!(info.settings, tokens::SETTINGS);
        assert_eq!(
            info.assets
                .iter()
                .filter(|asset| asset.kind == AssetKind::Font)
                .count(),
            fonts::FONT_FILES.len()
        );
        assert!(theme_asset("default", "vendor/mirador/mirador.min.js").is_some());
        assert!(theme_asset("default", "vendor/leaflet/leaflet.js").is_some());
    }

    /// The bundled theme ships its own stylesheet and the placeholder image a
    /// resource without a thumbnail shows.
    #[test]
    fn theme_asset_serves_the_bundled_stylesheet() {
        let asset = theme_asset("default", "css/style.css").expect("the stylesheet");
        assert_eq!(asset.kind, AssetKind::Stylesheet);
        assert_eq!(asset.kind.media_type(), "text/css");
        assert_eq!(asset.content, style::STYLE_CSS.as_bytes());
        assert!(!asset.content.is_empty());
        let placeholder = theme_asset("default", "img/placeholder.svg").expect("the placeholder");
        assert_eq!(placeholder.kind, AssetKind::Image);
        assert_eq!(placeholder.kind.media_type(), "image/svg+xml");
        assert!(placeholder.content.starts_with(b"<svg"));
        assert!(theme_asset("default", "css/missing.css").is_none());
        assert!(theme_asset("other", "css/style.css").is_none());
    }

    /// The element an asset is loaded with follows its kind, and an image is
    /// not loaded from the head.
    #[test]
    fn asset_kind_picks_the_element() {
        assert_eq!(
            AssetKind::Stylesheet.element("/assets/css/style.css"),
            "<link rel=\"stylesheet\" href=\"/assets/css/style.css\">"
        );
        assert_eq!(
            AssetKind::Script.element("/assets/js/global.js"),
            "<script src=\"/assets/js/global.js\"></script>"
        );
        assert_eq!(AssetKind::Script.media_type(), "application/javascript");
        assert_eq!(AssetKind::Image.element("/assets/img/placeholder.svg"), "");
        assert!(!AssetKind::Image.is_head());
        assert_eq!(
            AssetKind::Library.element("/assets/vendor/mirador/mirador.min.js"),
            ""
        );
        assert!(!AssetKind::Library.is_head());
        assert!(!AssetKind::Png.is_head());
        assert!(AssetKind::Stylesheet.is_head());
        assert_eq!(
            AssetKind::LibraryStylesheet.element("/assets/vendor/leaflet/leaflet.css"),
            ""
        );
        assert!(!AssetKind::LibraryStylesheet.is_head());
        assert_eq!(AssetKind::LibraryStylesheet.media_type(), "text/css");
    }
}
