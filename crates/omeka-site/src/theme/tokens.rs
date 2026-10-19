//! The bundled theme's design tokens: what a site stores, how the effective
//! tokens are resolved from it, and the CSS custom properties the document
//! frame emits.
//!
//! A site stores its choices as the theme's own settings map
//! (`theme_settings_<theme>`, served as `o:theme_settings`). That map is
//! untyped storage, so [`Choices::from_value`] is the boundary: it reads each
//! key, keeps the values this theme accepts and drops the rest.
//!
//! [`resolve`] is the one function that turns choices into effective tokens.
//! It starts from the preset, replaces the palette with one derived from the
//! brand colour when derivation is on, then applies the curator's overrides.
//! The renderer and the preview read its answer; the admin repeats it
//! (`web/admin/src/pages/sites/themeTokens.ts`) against the samples
//! `web/admin/src/generated/themeSettings.json` carries.

use serde_json::{Value, json};

use crate::theme::choices::{
    ButtonStyle, ColorScheme, ContentWidth, Density, FontStack, HeaderLayout, HeaderStyle,
};
use crate::theme::color::HexColor;
use crate::theme::custom_css::{self, CustomCss};
use crate::theme::palette::{Palette, Role, Shade, derive};
use crate::theme::presets::{NEW_SITE_PRESET, PresetId};

/// The bounds the type scale clamps to, per heading level, as multiples of the
/// base size: `(level, power, min, max)`. A large ratio grows the small
/// headings but not the page's `h1`, which would otherwise dominate it.
const HEADING_BOUNDS: [(&str, i32, f64, f64); 4] = [
    ("h1", 4, 1.6, 3.2),
    ("h2", 3, 1.35, 2.4),
    ("h3", 2, 1.15, 1.8),
    ("h4", 1, 1.0, 1.4),
];

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

/// What a site chose: a preset, whether to derive the palette from the brand
/// colour, and the tokens it set by hand. `None` means "the preset's".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Choices {
    /// The preset the site starts from.
    pub preset: PresetId,
    /// Whether the palette is derived from the brand colour.
    pub derive: bool,
    /// The colours set by hand, the brand among them.
    pub colours: Vec<(Role, HexColor)>,
    /// The scheme, when set by hand.
    pub scheme: Option<ColorScheme>,
    /// The heading font, when set by hand.
    pub heading_font: Option<FontStack>,
    /// The body font, when set by hand.
    pub body_font: Option<FontStack>,
    /// The base font size, when set by hand.
    pub base_size: Option<u32>,
    /// The type ratio, when set by hand.
    pub type_ratio: Option<u32>,
    /// The density, when set by hand.
    pub density: Option<Density>,
    /// The corner radius, when set by hand.
    pub radius: Option<u32>,
    /// The header layout, when set by hand.
    pub header_layout: Option<HeaderLayout>,
    /// The header style, when set by hand.
    pub header_style: Option<HeaderStyle>,
    /// The button style, when set by hand.
    pub button_style: Option<ButtonStyle>,
    /// The content width, when set by hand.
    pub content_width: Option<ContentWidth>,
}

impl Choices {
    /// Read the choices out of a stored map. A key the map omits, or holds in
    /// a shape the theme does not accept, is left to the preset; a map read
    /// here never fails, so one bad row does not break a page.
    ///
    /// A map with no `preset` was stored before presets existed. It reads as
    /// the plain preset with derivation off, so it renders the tokens it
    /// rendered then.
    #[must_use]
    pub fn from_value(value: &Value) -> Self {
        let preset = value
            .get("preset")
            .and_then(Value::as_str)
            .and_then(PresetId::parse);
        let derive = match preset {
            Some(_) => value
                .get("derive_palette")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            None => false,
        };
        let choice = |key: &str| value.get(key).and_then(Value::as_str);
        Self {
            preset: preset.unwrap_or_default(),
            derive,
            colours: Role::EDITABLE
                .iter()
                .filter_map(|&(role, key)| Some((role, HexColor::parse(choice(key)?)?)))
                .collect(),
            scheme: choice("color_scheme").and_then(ColorScheme::parse),
            heading_font: choice("heading_font").and_then(FontStack::parse),
            body_font: choice("body_font").and_then(FontStack::parse),
            base_size: stored_number(value, "base_font_size", MIN_BASE_SIZE, MAX_BASE_SIZE),
            type_ratio: stored_number(value, "type_ratio", MIN_TYPE_RATIO, MAX_TYPE_RATIO),
            density: choice("density").and_then(Density::parse),
            radius: stored_number(value, "corner_radius", 0, MAX_RADIUS),
            header_layout: choice("header_layout").and_then(HeaderLayout::parse),
            header_style: choice("header_style").and_then(HeaderStyle::parse),
            button_style: choice("button_style").and_then(ButtonStyle::parse),
            content_width: choice("content_width").and_then(ContentWidth::parse),
        }
    }

    /// The brand colour set by hand.
    fn brand(&self) -> Option<HexColor> {
        self.colours
            .iter()
            .find(|(role, _)| *role == Role::Brand)
            .map(|(_, colour)| *colour)
    }
}

/// The theme settings map a site created through the API starts with: the
/// new-site preset, its palette derived from its own brand colour.
#[must_use]
pub fn new_site_settings() -> Value {
    json!({ "preset": NEW_SITE_PRESET.key(), "derive_palette": true })
}

/// Every token the sheet reads, resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tokens {
    /// The preset the tokens start from.
    pub preset: PresetId,
    /// The scheme the page shows.
    pub scheme: ColorScheme,
    /// The palette the light scheme shows.
    pub light: Palette,
    /// The palette the dark scheme shows.
    pub dark: Palette,
    /// The heading font stack.
    pub heading_font: FontStack,
    /// The body font stack.
    pub body_font: FontStack,
    /// The body font size in pixels.
    pub base_size: u32,
    /// The type scale as a percentage (125 is 1.25).
    pub type_ratio: u32,
    /// The layout density.
    pub density: Density,
    /// The corner radius in pixels.
    pub radius: u32,
    /// The header layout.
    pub header_layout: HeaderLayout,
    /// How the header sets itself off.
    pub header_style: HeaderStyle,
    /// How a button looks.
    pub button_style: ButtonStyle,
    /// The widest the content grows.
    pub content_width: ContentWidth,
}

/// The effective tokens for a site's choices: the preset, then a palette
/// derived from the brand when derivation is on, then the overrides.
///
/// A colour override applies to the palette the page shows first: the dark one
/// under the dark scheme, the light one otherwise. Under the automatic scheme
/// the dark palette stays the preset's or the derived one.
#[must_use]
pub fn resolve(choices: &Choices) -> Tokens {
    let preset = choices.preset.preset();
    let scheme = choices.scheme.unwrap_or(preset.scheme);
    let brand = choices.brand();
    let (mut light, mut dark) = if choices.derive {
        let brand = brand.unwrap_or(preset.light.brand);
        (
            derive(brand, preset.recipe, Shade::Light),
            derive(brand, preset.recipe, Shade::Dark),
        )
    } else {
        let (mut light, mut dark) = (preset.light, preset.dark);
        if let Some(brand) = brand {
            light.brand = brand;
            dark.brand = brand;
        }
        (light, dark)
    };
    let shown = if scheme == ColorScheme::Dark {
        &mut dark
    } else {
        &mut light
    };
    for &(role, colour) in &choices.colours {
        shown.set(role, colour);
    }
    Tokens {
        preset: choices.preset,
        scheme,
        light,
        dark,
        heading_font: choices.heading_font.unwrap_or(preset.heading_font),
        body_font: choices.body_font.unwrap_or(preset.body_font),
        base_size: choices.base_size.unwrap_or(preset.base_size),
        type_ratio: choices.type_ratio.unwrap_or(preset.type_ratio),
        density: choices.density.unwrap_or(preset.density),
        radius: choices.radius.unwrap_or(preset.radius),
        header_layout: choices.header_layout.unwrap_or(preset.header_layout),
        header_style: choices.header_style.unwrap_or(preset.header_style),
        button_style: choices.button_style.unwrap_or(preset.button_style),
        content_width: choices.content_width.unwrap_or(preset.content_width),
    }
}

impl Tokens {
    /// The palettes the page can show, each with its shade: one for the light
    /// or dark scheme, both for the automatic one.
    #[must_use]
    pub fn shown(&self) -> Vec<(Shade, &Palette)> {
        match self.scheme {
            ColorScheme::Light => vec![(Shade::Light, &self.light)],
            ColorScheme::Dark => vec![(Shade::Dark, &self.dark)],
            ColorScheme::Auto => vec![(Shade::Light, &self.light), (Shade::Dark, &self.dark)],
        }
    }

    /// The CSS custom properties the document frame puts in a `<style>` in the
    /// head. Every value is a parsed colour, a number, or a constant this
    /// crate holds, so no stored text reaches the sheet here.
    #[must_use]
    pub fn css(&self) -> String {
        let mut out = String::from(":root {\n");
        out.push_str(&format!("  color-scheme: {};\n", self.scheme.css()));
        let first = if self.scheme == ColorScheme::Dark {
            &self.dark
        } else {
            &self.light
        };
        push_colours(&mut out, first, "  ");
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
        let ratio = f64::from(self.type_ratio) / 100.0;
        for (level, power, min, max) in HEADING_BOUNDS {
            let size = ratio.powi(power).clamp(min, max);
            out.push_str(&format!("  --font-size-{level}: {}em;\n", decimal(size)));
        }
        out.push_str(&format!(
            "  --line-height: {};\n",
            self.density.line_height()
        ));
        for (index, space) in self.density.spaces().iter().enumerate() {
            out.push_str(&format!("  --space-{}: {space};\n", index + 1));
        }
        out.push_str(&format!(
            "  --radius: {}px;\n  --radius-sm: {}px;\n  --content-width: {};\n",
            self.radius,
            self.radius.saturating_sub(2),
            self.content_width.css()
        ));
        for (name, value) in self
            .header_style
            .properties()
            .iter()
            .chain(self.button_style.properties().iter())
        {
            out.push_str(&format!("  --{name}: {value};\n"));
        }
        out.push_str("}\n");
        if self.scheme == ColorScheme::Auto {
            out.push_str(
                "@media (prefers-color-scheme: dark) {\n  :root {\n    color-scheme: dark;\n",
            );
            push_colours(&mut out, &self.dark, "    ");
            out.push_str("  }\n}\n");
        }
        out
    }
}

/// One `--color-<role>` line per role.
fn push_colours(out: &mut String, palette: &Palette, indent: &str) {
    for role in Role::ALL {
        out.push_str(&format!(
            "{indent}--color-{}: {};\n",
            role.key(),
            palette.get(role).hex()
        ));
    }
}

/// What the document frame reads from a site's theme settings: the effective
/// tokens, the header and footer content, and the custom CSS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeSettings {
    /// The effective design tokens.
    pub tokens: Tokens,
    /// The logo's asset id, when the site has one.
    pub logo: Option<u64>,
    /// The banner image's asset id, when the site has one.
    pub banner: Option<u64>,
    /// The text beside the banner, escaped when it is printed.
    pub banner_text: Option<String>,
    /// The footer's HTML, purified when it is printed.
    pub footer_text: Option<String>,
    /// The site's own CSS, when it has any that passes the sanitiser.
    pub custom_css: Option<CustomCss>,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self::from_value(&Value::Null)
    }
}

impl ThemeSettings {
    /// Read a stored map. The tokens resolve through [`resolve`]; custom CSS
    /// that the sanitiser refuses is dropped whole, so text stored by a path
    /// other than the site write still cannot reach the page.
    #[must_use]
    pub fn from_value(value: &Value) -> Self {
        Self {
            tokens: resolve(&Choices::from_value(value)),
            logo: stored_id(value, "logo"),
            banner: stored_id(value, "banner"),
            banner_text: stored_text(value, "banner_text"),
            footer_text: stored_text(value, "footer_text"),
            custom_css: stored_text(value, "custom_css")
                .and_then(|text| custom_css::sanitize(&text).ok()),
        }
    }

    /// The token custom properties, as [`Tokens::css`] writes them.
    #[must_use]
    pub fn css(&self) -> String {
        self.tokens.css()
    }
}

/// A stored number inside `min..=max`, from a number or the string a form
/// control sends.
fn stored_number(value: &Value, key: &str, min: u32, max: u32) -> Option<u32> {
    let number = match value.get(key)? {
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
    SETTINGS, ThemeSettingEntry, ThemeSettingKind, ThemeSettingSpec, ThemeSettingsRegistry,
    registry,
};

#[cfg(test)]
#[path = "tokens_tests.rs"]
mod tests;
