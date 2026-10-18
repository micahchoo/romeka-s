//! The bundled theme's design tokens: the values a curator edits, the schema
//! the admin renders, and the CSS custom properties the document frame emits.
//!
//! A site stores the tokens as the theme's own settings map
//! (`theme_settings_<theme>`, served as `o:theme_settings`). That map is
//! untyped storage, so [`ThemeSettings::from_value`] is the boundary: it reads
//! each token, validates it, and falls back to the token's default when the
//! stored value is not one this theme accepts. Downstream code trusts the
//! typed [`ThemeSettings`].
//!
//! The schema lives here, beside the values it describes, and
//! `web/admin/src/generated/themeSettings.json` is its serialisation. A test in
//! this module keeps the two byte-identical, the way the block registry and the
//! site settings registry do.

use serde_json::Value;

/// The font stacks a site may choose, in schema order. Every value is a system
/// stack, so the page requests no font file from another origin.
pub const FONT_KEYS: [&str; 6] = [
    "system-sans",
    "system-serif",
    "humanist",
    "geometric",
    "transitional-serif",
    "mono",
];

/// A curated font stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontStack {
    /// The platform's sans-serif face.
    #[default]
    SystemSans,
    /// The platform's serif face.
    SystemSerif,
    /// A humanist sans stack.
    Humanist,
    /// A geometric sans stack.
    Geometric,
    /// A transitional serif stack.
    TransitionalSerif,
    /// A monospace stack with a sans fallback.
    Mono,
}

impl FontStack {
    /// Every stack, in schema order.
    pub const ALL: [Self; 6] = [
        Self::SystemSans,
        Self::SystemSerif,
        Self::Humanist,
        Self::Geometric,
        Self::TransitionalSerif,
        Self::Mono,
    ];

    /// The stored key.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::SystemSans => "system-sans",
            Self::SystemSerif => "system-serif",
            Self::Humanist => "humanist",
            Self::Geometric => "geometric",
            Self::TransitionalSerif => "transitional-serif",
            Self::Mono => "mono",
        }
    }

    /// The CSS `font-family` value.
    #[must_use]
    pub const fn stack(self) -> &'static str {
        match self {
            Self::SystemSans => "system-ui, -apple-system, \"Segoe UI\", Roboto, sans-serif",
            Self::SystemSerif => "Georgia, \"Times New Roman\", Times, serif",
            Self::Humanist => "\"Gill Sans\", \"Trebuchet MS\", Calibri, sans-serif",
            Self::Geometric => "Futura, \"Century Gothic\", \"Avenir Next\", sans-serif",
            Self::TransitionalSerif => "Charter, \"Bitstream Charter\", Cambria, serif",
            Self::Mono => "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
        }
    }

    /// The stack a stored key names, `None` for a key outside the curated list.
    #[must_use]
    pub fn parse(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|font| font.key() == key)
    }
}

/// How much air the layout leaves around its content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Density {
    /// Tight spacing for a dense catalogue.
    Compact,
    /// The default spacing.
    #[default]
    Comfortable,
    /// Generous spacing for a reading site.
    Spacious,
}

impl Density {
    /// Every density, in schema order.
    pub const ALL: [Self; 3] = [Self::Compact, Self::Comfortable, Self::Spacious];

    /// The stored key.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Comfortable => "comfortable",
            Self::Spacious => "spacious",
        }
    }

    /// The stored key of this density, `None` for an unknown key.
    #[must_use]
    pub fn parse(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|density| density.key() == key)
    }

    /// The five spacing steps (`--space-1` through `--space-5`).
    #[must_use]
    pub const fn spaces(self) -> [&'static str; 5] {
        match self {
            Self::Compact => ["0.125rem", "0.375rem", "0.75rem", "1rem", "2rem"],
            Self::Comfortable => ["0.25rem", "0.5rem", "1rem", "1.5rem", "3rem"],
            Self::Spacious => ["0.375rem", "0.75rem", "1.25rem", "2rem", "4rem"],
        }
    }

    /// The body line height.
    #[must_use]
    pub const fn line_height(self) -> &'static str {
        match self {
            Self::Compact => "1.4",
            Self::Comfortable => "1.5",
            Self::Spacious => "1.65",
        }
    }
}

/// Where the site's logo and title sit in the header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeaderLayout {
    /// The logo and title align left, with the navigation on the same row.
    #[default]
    LogoLeft,
    /// The logo, title and navigation centre.
    Centred,
}

impl HeaderLayout {
    /// Every layout, in schema order.
    pub const ALL: [Self; 2] = [Self::LogoLeft, Self::Centred];

    /// The stored key.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::LogoLeft => "logo-left",
            Self::Centred => "centred",
        }
    }

    /// The body class the frame adds.
    #[must_use]
    pub const fn class(self) -> &'static str {
        match self {
            Self::LogoLeft => "header-logo-left",
            Self::Centred => "header-centred",
        }
    }

    /// The layout a stored key names.
    #[must_use]
    pub fn parse(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|layout| layout.key() == key)
    }
}

/// Which palette the page asks the browser for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorScheme {
    /// The light palette.
    #[default]
    Light,
    /// The dark palette.
    Dark,
    /// The light palette, with the dark neutrals when the reader's system asks
    /// for dark.
    Auto,
}

impl ColorScheme {
    /// Every scheme, in schema order.
    pub const ALL: [Self; 3] = [Self::Light, Self::Dark, Self::Auto];

    /// The stored key.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::Auto => "auto",
        }
    }

    /// The CSS `color-scheme` value.
    #[must_use]
    pub const fn css(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::Auto => "light dark",
        }
    }

    /// The scheme a stored key names.
    #[must_use]
    pub fn parse(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|scheme| scheme.key() == key)
    }
}

/// A colour a token accepts: `#rgb` or `#rrggbb`, normalised to lower-case
/// `#rrggbb`. The parser is the only constructor, so an emitted value is always
/// a hex colour and can never carry CSS of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexColor(String);

impl HexColor {
    /// Parse a stored colour, `None` when it is not a hex colour.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let hex = raw.trim().strip_prefix('#')?;
        if !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return None;
        }
        let expanded = match hex.len() {
            3 => hex.chars().flat_map(|ch| [ch, ch]).collect::<String>(),
            6 => hex.to_string(),
            _ => return None,
        };
        Some(Self(format!("#{}", expanded.to_ascii_lowercase())))
    }

    /// The normalised value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for HexColor {
    fn default() -> Self {
        Self("#000000".to_string())
    }
}

/// The light defaults a fresh site starts from. They are the values the schema
/// publishes, so a token with no stored row renders the documented default.
/// The brand default. It is light enough that the default text colour clears
/// AA 4.5:1 against it, which the Theme page's contrast gate enforces.
pub const DEFAULT_BRAND: &str = "#6aa9d6";
/// The accent default.
pub const DEFAULT_ACCENT: &str = "#b45309";
/// The page background default.
pub const DEFAULT_BACKGROUND: &str = "#ffffff";
/// The surface default.
pub const DEFAULT_SURFACE: &str = "#f5f7f8";
/// The text default.
pub const DEFAULT_TEXT: &str = "#222222";
/// The muted text default.
pub const DEFAULT_MUTED: &str = "#5a6470";
/// The link default.
pub const DEFAULT_LINK: &str = "#1a5d8f";

/// The background the dark scheme uses when no colour is stored.
pub const DARK_BACKGROUND: &str = "#12161a";
/// The surface the dark scheme uses when no colour is stored.
pub const DARK_SURFACE: &str = "#1b2126";
/// The text the dark scheme uses when no colour is stored.
pub const DARK_TEXT: &str = "#e8ecef";
/// The muted text the dark scheme uses when no colour is stored.
pub const DARK_MUTED: &str = "#9aa6b2";

/// The narrowest base font size the theme offers.
pub const MIN_BASE_SIZE: u32 = 12;
/// The widest base font size the theme offers.
pub const MAX_BASE_SIZE: u32 = 24;
/// The smallest type ratio, as a percentage.
pub const MIN_TYPE_RATIO: u32 = 110;
/// The largest type ratio, as a percentage.
pub const MAX_TYPE_RATIO: u32 = 160;
/// The largest corner radius, in pixels.
pub const MAX_RADIUS: u32 = 24;

/// The theme's design tokens, validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeSettings {
    /// The palette the page asks the browser for.
    pub color_scheme: ColorScheme,
    /// Links, buttons and other brand marks.
    pub brand: HexColor,
    /// Emphasis and small accents.
    pub accent: HexColor,
    /// The page background.
    pub background: HexColor,
    /// Cards, bars and inset surfaces.
    pub surface: HexColor,
    /// Body text.
    pub text: HexColor,
    /// Secondary text.
    pub muted: HexColor,
    /// Link text.
    pub link: HexColor,
    /// The heading font stack.
    pub heading_font: FontStack,
    /// The body font stack.
    pub body_font: FontStack,
    /// The body font size in pixels, 12 to 24.
    pub base_size: u32,
    /// The type scale as a percentage, 110 to 160 (125 is 1.25).
    pub type_ratio: u32,
    /// The layout density.
    pub density: Density,
    /// The corner radius in pixels, 0 to 24.
    pub radius: u32,
    /// The header layout.
    pub header_layout: HeaderLayout,
    /// The logo's asset id, when the site has one.
    pub logo: Option<u64>,
    /// The banner image's asset id, when the site has one.
    pub banner: Option<u64>,
    /// The text beside the banner, escaped when it is printed.
    pub banner_text: Option<String>,
    /// The footer's HTML, purified when it is printed.
    pub footer_text: Option<String>,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self::from_value(&Value::Null)
    }
}

impl ThemeSettings {
    /// Read the tokens out of a stored map. A token the map omits, or holds in
    /// a shape the theme does not accept, takes its default; a map read here
    /// can never fail, so one bad row does not break a page.
    #[must_use]
    pub fn from_value(value: &Value) -> Self {
        let scheme = value
            .get("color_scheme")
            .and_then(Value::as_str)
            .and_then(ColorScheme::parse)
            .unwrap_or_default();
        let dark = scheme == ColorScheme::Dark;
        let mut out = Self {
            color_scheme: scheme,
            brand: color(DEFAULT_BRAND),
            accent: color(DEFAULT_ACCENT),
            background: color(if dark {
                DARK_BACKGROUND
            } else {
                DEFAULT_BACKGROUND
            }),
            surface: color(if dark { DARK_SURFACE } else { DEFAULT_SURFACE }),
            text: color(if dark { DARK_TEXT } else { DEFAULT_TEXT }),
            muted: color(if dark { DARK_MUTED } else { DEFAULT_MUTED }),
            link: color(DEFAULT_LINK),
            heading_font: FontStack::SystemSerif,
            body_font: FontStack::SystemSans,
            base_size: 16,
            type_ratio: 125,
            density: Density::Comfortable,
            radius: 4,
            header_layout: HeaderLayout::LogoLeft,
            logo: None,
            banner: None,
            banner_text: None,
            footer_text: None,
        };
        out.brand = stored_color(value, "brand_color").unwrap_or(out.brand);
        out.accent = stored_color(value, "accent_color").unwrap_or(out.accent);
        out.background = stored_color(value, "background_color").unwrap_or(out.background);
        out.surface = stored_color(value, "surface_color").unwrap_or(out.surface);
        out.text = stored_color(value, "text_color").unwrap_or(out.text);
        out.muted = stored_color(value, "muted_color").unwrap_or(out.muted);
        out.link = stored_color(value, "link_color").unwrap_or(out.link);
        out.heading_font = stored_font(value, "heading_font").unwrap_or(out.heading_font);
        out.body_font = stored_font(value, "body_font").unwrap_or(out.body_font);
        out.base_size = stored_number(value, "base_font_size", MIN_BASE_SIZE, MAX_BASE_SIZE)
            .unwrap_or(out.base_size);
        out.type_ratio = stored_number(value, "type_ratio", MIN_TYPE_RATIO, MAX_TYPE_RATIO)
            .unwrap_or(out.type_ratio);
        out.density = value
            .get("density")
            .and_then(Value::as_str)
            .and_then(Density::parse)
            .unwrap_or(out.density);
        out.radius = stored_number(value, "corner_radius", 0, MAX_RADIUS).unwrap_or(out.radius);
        out.header_layout = value
            .get("header_layout")
            .and_then(Value::as_str)
            .and_then(HeaderLayout::parse)
            .unwrap_or(out.header_layout);
        out.logo = stored_id(value, "logo");
        out.banner = stored_id(value, "banner");
        out.banner_text = stored_text(value, "banner_text");
        out.footer_text = stored_text(value, "footer_text");
        out
    }

    /// The type scale as a decimal.
    #[must_use]
    pub fn ratio(&self) -> f64 {
        f64::from(self.type_ratio) / 100.0
    }

    /// The CSS custom properties the document frame puts in a `<style>` in the
    /// head. Every value is validated or derived from a number, so this string
    /// carries no value a caller supplied verbatim except the curated font
    /// stacks.
    #[must_use]
    pub fn css(&self) -> String {
        let spaces = self.density.spaces();
        let mut out = String::from(":root {\n");
        out.push_str(&format!("  color-scheme: {};\n", self.color_scheme.css()));
        for (name, colour) in [
            ("brand", &self.brand),
            ("accent", &self.accent),
            ("background", &self.background),
            ("surface", &self.surface),
            ("text", &self.text),
            ("muted", &self.muted),
            ("link", &self.link),
        ] {
            out.push_str(&format!("  --color-{name}: {};\n", colour.as_str()));
        }
        out.push_str(
            "  --color-border: color-mix(in srgb, var(--color-text) 18%, var(--color-background));\n",
        );
        out.push_str(&format!(
            "  --font-heading: {};\n  --font-body: {};\n",
            self.heading_font.stack(),
            self.body_font.stack()
        ));
        out.push_str(&format!(
            "  --font-size-base: {}px;\n  --font-size-sm: 0.875em;\n  --font-size-lg: 1.125em;\n",
            self.base_size
        ));
        let ratio = self.ratio();
        for (level, power) in [("h1", 4), ("h2", 3), ("h3", 2), ("h4", 1)] {
            out.push_str(&format!(
                "  --font-size-{level}: {}em;\n",
                decimal(ratio.powi(power))
            ));
        }
        out.push_str(&format!(
            "  --line-height: {};\n",
            self.density.line_height()
        ));
        for (index, space) in spaces.iter().enumerate() {
            out.push_str(&format!("  --space-{}: {space};\n", index + 1));
        }
        out.push_str(&format!(
            "  --radius: {}px;\n  --radius-sm: {}px;\n  --content-width: 60rem;\n",
            self.radius,
            self.radius.saturating_sub(2)
        ));
        out.push_str("}\n");
        if self.color_scheme == ColorScheme::Auto {
            out.push_str(
                "@media (prefers-color-scheme: dark) {\n  :root {\n    color-scheme: dark;\n",
            );
            for (name, value) in [
                ("background", DARK_BACKGROUND),
                ("surface", DARK_SURFACE),
                ("text", DARK_TEXT),
                ("muted", DARK_MUTED),
            ] {
                out.push_str(&format!("    --color-{name}: {value};\n"));
            }
            out.push_str("  }\n}\n");
        }
        out
    }
}

/// A colour literal the schema already validated.
fn color(value: &'static str) -> HexColor {
    HexColor(format!("#{}", value.trim_start_matches('#')))
}

/// A stored colour, `None` when the key is absent or not a hex colour.
fn stored_color(value: &Value, key: &str) -> Option<HexColor> {
    HexColor::parse(value.get(key)?.as_str()?)
}

/// A stored font key.
fn stored_font(value: &Value, key: &str) -> Option<FontStack> {
    FontStack::parse(value.get(key)?.as_str()?)
}

/// A stored number inside `min..=max`.
fn stored_number(value: &Value, key: &str, min: u32, max: u32) -> Option<u32> {
    let raw = value.get(key)?;
    let number = match raw {
        Value::Number(number) => number.as_u64()?,
        Value::String(text) => text.trim().parse().ok()?,
        _ => return None,
    };
    u32::try_from(number)
        .ok()
        .filter(|n| (min..=max).contains(n))
}

/// A stored asset id, from a number or the string a form control sends.
fn stored_id(value: &Value, key: &str) -> Option<u64> {
    match value.get(key)? {
        Value::Number(number) => number.as_u64().filter(|id| *id > 0),
        Value::String(text) => text.trim().parse().ok().filter(|id| *id > 0),
        _ => None,
    }
}

/// A stored text value, `None` when absent or blank.
fn stored_text(value: &Value, key: &str) -> Option<String> {
    let text = value.get(key)?.as_str()?.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// A float as a short decimal, without a trailing zero run.
fn decimal(value: f64) -> String {
    let formatted = format!("{value:.4}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

pub use crate::theme::tokens_schema::{
    SETTINGS, ThemeSettingDefault, ThemeSettingEntry, ThemeSettingKind, ThemeSettingSchema,
    ThemeSettingSpec, ThemeSettingsRegistry, registry,
};

#[cfg(test)]
#[path = "tokens_tests.rs"]
mod tests;
