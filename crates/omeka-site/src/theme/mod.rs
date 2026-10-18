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
//! - `settings`, the `site_setting` keys the theme reads, as
//!   [`crate::settings::SETTINGS`] for the bundled theme, so a settings editor
//!   knows what to offer;
//! - `assets`, the files the theme ships with their bytes, so a caller serves
//!   them from [`THEMES`] without a filesystem, and the document frame links
//!   them (upstream reads a theme's `asset/` directory).
//!
//! A second theme implements [`Theme`] and adds itself to [`THEMES`]. Nothing
//! else in the crate names [`DefaultTheme`].

pub mod default;
pub mod search;
pub mod style;

pub use default::{DEFAULT_INFO, DefaultTheme};

use crate::settings::ThemeSetting;
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
}

impl AssetKind {
    /// The media type a caller serves the asset with.
    #[must_use]
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Stylesheet => "text/css",
            Self::Script => "application/javascript",
        }
    }

    /// The element that loads the asset with the `href` the caller built.
    #[must_use]
    pub fn element(self, href: &str) -> String {
        match self {
            Self::Stylesheet => format!("<link rel=\"stylesheet\" href=\"{href}\">"),
            Self::Script => format!("<script src=\"{href}\"></script>"),
        }
    }
}

/// One file a theme ships, with its bytes.
///
/// The bytes live here, as `&'static str`, so a caller can serve
/// `/assets/<path>` straight from the registry and the document frame can link
/// the same path. A theme stylesheet or script is UTF-8 text; a binary asset a
/// later theme adds would need a byte variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeAsset {
    /// The path under the theme's asset base, e.g. `css/style.css`.
    pub path: &'static str,
    /// What the asset is.
    pub kind: AssetKind,
    /// The asset's bytes.
    pub content: &'static str,
}

/// A theme's manifest: the id a site stores, the settings it reads and the
/// files it ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeInfo {
    /// The `site.theme` value, the theme's directory name upstream.
    pub name: &'static str,
    /// The display name (`theme.ini` `name`, defaulting to the id).
    pub label: &'static str,
    /// The `site_setting` keys the theme reads.
    pub settings: &'static [ThemeSetting],
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
        assert_eq!(info.settings, &crate::settings::SETTINGS);
        assert_eq!(info.assets.len(), 1);
    }

    /// The bundled theme ships its own stylesheet, and a path it does not ship
    /// has no asset. The asset's bytes are the sheet the module includes.
    #[test]
    fn theme_asset_serves_the_bundled_stylesheet() {
        let asset = theme_asset("default", "css/style.css").expect("the stylesheet");
        assert_eq!(asset.kind, AssetKind::Stylesheet);
        assert_eq!(asset.kind.media_type(), "text/css");
        assert_eq!(asset.content, style::STYLE_CSS);
        assert!(!asset.content.is_empty());
        assert!(theme_asset("default", "css/missing.css").is_none());
        assert!(theme_asset("other", "css/style.css").is_none());
    }

    /// The element an asset is loaded with follows its kind.
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
    }
}
