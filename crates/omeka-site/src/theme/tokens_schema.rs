//! The theme token schema: the kinds the admin renders, the token catalogue,
//! the presets, and the two generated files the admin reads.
//!
//! - `web/admin/src/generated/themeSettings.json` ([`registry`]) is what the
//!   Theme page renders: one entry per token, every preset with all its
//!   values, the colour roles and the contrast pairs.
//! - `web/admin/src/generated/themeSamples.json` ([`samples`]) pins the admin's
//!   copy of the palette arithmetic to this crate's: brand colours with the
//!   palettes they derive, colours with their nearest passing suggestion, and
//!   stored maps with the palettes they resolve to. Only the admin's tests
//!   import it, so it never reaches the bundle.
//!
//! A test in `tokens_tests.rs` keeps both files byte-identical.

use serde::Serialize;

use crate::theme::choices::{
    ButtonStyle, ColorScheme, ContentWidth, Density, FontStack, HeaderLayout, HeaderStyle,
};
use crate::theme::color::{AA, HexColor, nearest_passing};
use crate::theme::custom_css::MAX_BYTES;
use crate::theme::palette::{PAIRS, Palette, Role, Shade, derive};
use crate::theme::presets::{NEW_SITE_PRESET, PRESETS, Preset, PresetId};
use crate::theme::tokens::{
    Choices, MAX_BASE_SIZE, MAX_RADIUS, MAX_TYPE_RATIO, MIN_BASE_SIZE, MIN_TYPE_RATIO, resolve,
};

/// The kind of value a theme token holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeSettingKind {
    /// The preset id.
    Preset,
    /// An on or off switch, on by default.
    Toggle,
    /// A hex colour. The preset or the derived palette supplies the default.
    Colour,
    /// A font from the curated list.
    Font,
    /// A bounded whole number.
    Number {
        /// The smallest accepted value.
        min: u32,
        /// The largest accepted value.
        max: u32,
        /// The control's step.
        step: u32,
    },
    /// One of a fixed set of strings.
    Choice(&'static [&'static str]),
    /// An asset id.
    Asset,
    /// A single-line string.
    Text,
    /// An HTML fragment, purified when printed.
    Html,
    /// The site's own CSS, sanitised when written and when printed.
    Css,
}

/// One token the theme declares, with the control the admin renders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeSettingSpec {
    /// The key in the theme settings map.
    pub key: &'static str,
    /// The label a settings editor shows, as source text.
    pub label: &'static str,
    /// The kind of value the token holds.
    pub kind: ThemeSettingKind,
}

const fn spec(key: &'static str, label: &'static str, kind: ThemeSettingKind) -> ThemeSettingSpec {
    ThemeSettingSpec { key, label, kind }
}

/// Every token the bundled theme reads, in the order the Theme page shows
/// them: the preset and the brand, the palette, the type, the layout, the
/// header and footer content, and the custom CSS.
pub static SETTINGS: &[ThemeSettingSpec] = &[
    spec("preset", "Preset", ThemeSettingKind::Preset),
    spec(
        "derive_palette",
        "Build the palette from the brand colour",
        ThemeSettingKind::Toggle,
    ),
    spec("brand_color", "Brand colour", ThemeSettingKind::Colour),
    spec(
        "color_scheme",
        "Colour scheme",
        ThemeSettingKind::Choice(ColorScheme::KEYS),
    ),
    spec("accent_color", "Accent colour", ThemeSettingKind::Colour),
    spec(
        "background_color",
        "Page background",
        ThemeSettingKind::Colour,
    ),
    spec("surface_color", "Surface", ThemeSettingKind::Colour),
    spec("text_color", "Text", ThemeSettingKind::Colour),
    spec("muted_color", "Muted text", ThemeSettingKind::Colour),
    spec("link_color", "Link", ThemeSettingKind::Colour),
    spec("heading_font", "Heading font", ThemeSettingKind::Font),
    spec("body_font", "Body font", ThemeSettingKind::Font),
    spec(
        "base_font_size",
        "Base font size",
        ThemeSettingKind::Number {
            min: MIN_BASE_SIZE,
            max: MAX_BASE_SIZE,
            step: 1,
        },
    ),
    spec(
        "type_ratio",
        "Type scale ratio",
        ThemeSettingKind::Number {
            min: MIN_TYPE_RATIO,
            max: MAX_TYPE_RATIO,
            step: 5,
        },
    ),
    spec(
        "density",
        "Density",
        ThemeSettingKind::Choice(Density::KEYS),
    ),
    spec(
        "corner_radius",
        "Corner radius",
        ThemeSettingKind::Number {
            min: 0,
            max: MAX_RADIUS,
            step: 1,
        },
    ),
    spec(
        "content_width",
        "Content width",
        ThemeSettingKind::Choice(ContentWidth::KEYS),
    ),
    spec(
        "header_layout",
        "Header layout",
        ThemeSettingKind::Choice(HeaderLayout::KEYS),
    ),
    spec(
        "header_style",
        "Header style",
        ThemeSettingKind::Choice(HeaderStyle::KEYS),
    ),
    spec(
        "button_style",
        "Button style",
        ThemeSettingKind::Choice(ButtonStyle::KEYS),
    ),
    spec("logo", "Logo", ThemeSettingKind::Asset),
    spec("banner", "Banner image", ThemeSettingKind::Asset),
    spec("banner_text", "Banner text", ThemeSettingKind::Text),
    spec("footer_text", "Footer text", ThemeSettingKind::Html),
    spec("custom_css", "Custom CSS", ThemeSettingKind::Css),
];

/// The token schema, the presets, the colour roles and the contrast pairs, as
/// the admin reads them.
#[must_use]
pub fn registry() -> ThemeSettingsRegistry {
    ThemeSettingsRegistry {
        new_site_preset: NEW_SITE_PRESET.key(),
        settings: SETTINGS.iter().map(entry).collect(),
        presets: PRESETS.iter().map(preset_entry).collect(),
        colours: Role::EDITABLE
            .iter()
            .map(|&(role, key)| ColourEntry {
                role: role.key(),
                key,
            })
            .collect(),
        pairs: PAIRS.iter().map(|&(fg, bg)| [fg.key(), bg.key()]).collect(),
        aa: AA,
    }
}

/// One spec as the admin reads it.
fn entry(spec: &ThemeSettingSpec) -> ThemeSettingEntry {
    let camel = camel(spec.key);
    ThemeSettingEntry {
        key: spec.key,
        label: spec.label,
        label_key: format!("themeSetting.{camel}"),
        help_key: format!("themeSetting.{camel}Help"),
        schema: match spec.kind {
            ThemeSettingKind::Preset => ThemeSettingSchema::Choice {
                choices: PresetId::KEYS,
            },
            ThemeSettingKind::Toggle => ThemeSettingSchema::Toggle,
            ThemeSettingKind::Colour => ThemeSettingSchema::Colour,
            ThemeSettingKind::Font => ThemeSettingSchema::Font {
                choices: FontStack::KEYS,
            },
            ThemeSettingKind::Number { min, max, step } => {
                ThemeSettingSchema::Number { min, max, step }
            }
            ThemeSettingKind::Choice(choices) => ThemeSettingSchema::Choice { choices },
            ThemeSettingKind::Asset => ThemeSettingSchema::Asset,
            ThemeSettingKind::Text => ThemeSettingSchema::Text,
            ThemeSettingKind::Html => ThemeSettingSchema::Html,
            ThemeSettingKind::Css => ThemeSettingSchema::Css {
                max_bytes: MAX_BYTES,
            },
        },
    }
}

fn preset_entry(preset: &Preset) -> PresetEntry {
    let id = preset.id.key();
    PresetEntry {
        id,
        label_key: format!("themePreset.{id}"),
        description_key: format!("themePreset.{id}Description"),
        recipe: RecipeEntry {
            tint: preset.recipe.tint,
            paper: preset.recipe.paper,
            ink: preset.recipe.ink,
        },
        light: PaletteEntry::new(&preset.light),
        dark: PaletteEntry::new(&preset.dark),
        tokens: PresetTokens {
            color_scheme: preset.scheme.key(),
            heading_font: preset.heading_font.key(),
            body_font: preset.body_font.key(),
            base_font_size: preset.base_size,
            type_ratio: preset.type_ratio,
            density: preset.density.key(),
            corner_radius: preset.radius,
            content_width: preset.content_width.key(),
            header_layout: preset.header_layout.key(),
            header_style: preset.header_style.key(),
            button_style: preset.button_style.key(),
        },
    }
}

/// `/^[a-z]+(_[a-z0-9]+)*$/` to camelCase, for the catalogue key.
pub(crate) fn camel(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    let mut upper = false;
    for ch in key.chars() {
        if ch == '_' {
            upper = true;
            continue;
        }
        if upper {
            out.extend(ch.to_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// The brand colours the samples derive from: the theme's own brands, the
/// extremes (near white, near black, pure yellow) and a spread of hues.
pub const SAMPLE_BRANDS: [&str; 16] = [
    "#6aa9d6", "#7a4a14", "#0b57d0", "#fafafa", "#050505", "#ffff00", "#ff0000", "#00ff00",
    "#0000ff", "#808080", "#f2f2f2", "#111111", "#ff69b4", "#00ced1", "#7c3aed", "#c0c0c0",
];

/// The stored maps the resolution samples cover: a map from before presets, a
/// preset with derivation, a hand-set brand and colour, and the dark and
/// automatic schemes. Each map's keys are in alphabetical order, so the file
/// is the same whether or not `serde_json` keeps insertion order.
fn sample_maps() -> Vec<serde_json::Value> {
    use serde_json::json;
    vec![
        json!({}),
        json!({"brand_color": "#abcdef", "color_scheme": "auto", "text_color": "#111111"}),
        json!({"preset": "archive"}),
        json!({"brand_color": "#ffcc00", "link_color": "#003366", "preset": "civic"}),
        json!({"brand_color": "#ff0000", "derive_palette": false, "preset": "gallery"}),
        json!({"color_scheme": "dark", "muted_color": "#999999", "preset": "editorial"}),
        json!({"color_scheme": "auto", "derive_palette": true, "preset": "plain"}),
    ]
}

/// The samples that pin the admin's palette arithmetic to this crate's.
#[must_use]
pub fn samples() -> ThemeSamples {
    let brands = SAMPLE_BRANDS.iter().filter_map(|raw| HexColor::parse(raw));
    let derive_samples = brands
        .clone()
        .flat_map(|brand| {
            PRESETS.iter().map(move |preset| DeriveSample {
                brand: brand.hex(),
                preset: preset.id.key(),
                light: PaletteEntry::full(&derive(brand, preset.recipe, Shade::Light)),
                dark: PaletteEntry::full(&derive(brand, preset.recipe, Shade::Dark)),
            })
        })
        .collect();
    let grounds = [
        ["#ffffff"].as_slice(),
        ["#ffffff", "#f5f7f8"].as_slice(),
        ["#12161a"].as_slice(),
        ["#808080"].as_slice(),
    ];
    let nearest = brands
        .flat_map(|colour| {
            grounds.iter().map(move |against| {
                let against: Vec<HexColor> = against
                    .iter()
                    .filter_map(|raw| HexColor::parse(raw))
                    .collect();
                NearestSample {
                    colour: colour.hex(),
                    against: against.iter().map(|c| c.hex()).collect(),
                    suggestion: nearest_passing(colour, &against, AA).hex(),
                }
            })
        })
        .collect();
    let resolve_samples = sample_maps()
        .into_iter()
        .map(|stored| {
            let tokens = resolve(&Choices::from_value(&stored));
            ResolveSample {
                light: PaletteEntry::full(&tokens.light),
                dark: PaletteEntry::full(&tokens.dark),
                scheme: tokens.scheme.key(),
                stored,
            }
        })
        .collect();
    ThemeSamples {
        derive: derive_samples,
        nearest,
        resolve: resolve_samples,
    }
}

/// The whole theme settings registry, as `themeSettings.json` holds it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSettingsRegistry {
    /// The preset a site created through the API starts from.
    pub new_site_preset: &'static str,
    /// Every token, in schema order.
    pub settings: Vec<ThemeSettingEntry>,
    /// Every preset, in gallery order.
    pub presets: Vec<PresetEntry>,
    /// The colour roles a curator may set, each with its stored key.
    pub colours: Vec<ColourEntry>,
    /// The `(foreground, background)` role pairs the contrast gate checks.
    pub pairs: Vec<[&'static str; 2]>,
    /// The ratio every pair must reach.
    pub aa: f64,
}

/// One token as the admin reads it.
#[derive(Debug, Serialize)]
pub struct ThemeSettingEntry {
    /// The key in the theme settings map.
    pub key: &'static str,
    /// The English label, not serialised; the admin reads `labelKey`.
    #[serde(skip)]
    pub label: &'static str,
    /// The catalogue key for the label.
    #[serde(rename = "labelKey")]
    pub label_key: String,
    /// The catalogue key for the one-line help.
    #[serde(rename = "helpKey")]
    pub help_key: String,
    /// The control kind and its options, flattened beside the key.
    #[serde(flatten)]
    pub schema: ThemeSettingSchema,
}

/// The control a token renders as: a `kind` tag plus that kind's fields.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ThemeSettingSchema {
    /// An on or off switch.
    Toggle,
    /// A colour picker.
    Colour,
    /// A font select over the curated keys.
    Font {
        /// The curated keys, in schema order.
        choices: &'static [&'static str],
    },
    /// A bounded number input.
    Number {
        /// The smallest accepted value.
        min: u32,
        /// The largest accepted value.
        max: u32,
        /// The control's step.
        step: u32,
    },
    /// A select over a fixed set.
    Choice {
        /// The accepted values, in schema order.
        choices: &'static [&'static str],
    },
    /// An asset picker.
    Asset,
    /// A single-line text input.
    Text,
    /// A textarea holding HTML.
    Html,
    /// A code editor holding CSS.
    Css {
        /// The most bytes the sanitiser accepts.
        #[serde(rename = "maxBytes")]
        max_bytes: usize,
    },
}

/// A colour role and the key a site stores it under.
#[derive(Debug, Serialize)]
pub struct ColourEntry {
    /// The role, as `--color-<role>` names it.
    pub role: &'static str,
    /// The stored key.
    pub key: &'static str,
}

/// One preset as the admin reads it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetEntry {
    /// The stored id.
    pub id: &'static str,
    /// The catalogue key for the name.
    pub label_key: String,
    /// The catalogue key for the one-line description.
    pub description_key: String,
    /// How a brand colour becomes a palette.
    pub recipe: RecipeEntry,
    /// The light palette with derivation off.
    pub light: PaletteEntry,
    /// The dark palette with derivation off.
    pub dark: PaletteEntry,
    /// Every other token, by stored key.
    pub tokens: PresetTokens,
}

/// A recipe as the admin reads it.
#[derive(Debug, Serialize)]
pub struct RecipeEntry {
    /// The tint strength.
    pub tint: f64,
    /// The light background lightness.
    pub paper: f64,
    /// The dark background lightness.
    pub ink: f64,
}

/// A palette by role. `on-brand` is present only where the palette is a
/// result, not a preset's input.
#[derive(Debug, Serialize)]
pub struct PaletteEntry {
    /// Brand marks.
    pub brand: String,
    /// Text on the brand colour.
    #[serde(rename = "on-brand", skip_serializing_if = "Option::is_none")]
    pub on_brand: Option<String>,
    /// Accents.
    pub accent: String,
    /// The page background.
    pub background: String,
    /// Inset surfaces.
    pub surface: String,
    /// Body text.
    pub text: String,
    /// Secondary text.
    pub muted: String,
    /// Links.
    pub link: String,
}

impl PaletteEntry {
    fn new(palette: &Palette) -> Self {
        Self {
            brand: palette.brand.hex(),
            on_brand: None,
            accent: palette.accent.hex(),
            background: palette.background.hex(),
            surface: palette.surface.hex(),
            text: palette.text.hex(),
            muted: palette.muted.hex(),
            link: palette.link.hex(),
        }
    }

    fn full(palette: &Palette) -> Self {
        Self {
            on_brand: Some(palette.get(Role::OnBrand).hex()),
            ..Self::new(palette)
        }
    }
}

/// A preset's tokens other than its palette, by stored key.
#[derive(Debug, Serialize)]
pub struct PresetTokens {
    /// The scheme.
    pub color_scheme: &'static str,
    /// The heading font key.
    pub heading_font: &'static str,
    /// The body font key.
    pub body_font: &'static str,
    /// The base font size in pixels.
    pub base_font_size: u32,
    /// The type ratio as a percentage.
    pub type_ratio: u32,
    /// The density key.
    pub density: &'static str,
    /// The corner radius in pixels.
    pub corner_radius: u32,
    /// The content width key.
    pub content_width: &'static str,
    /// The header layout key.
    pub header_layout: &'static str,
    /// The header style key.
    pub header_style: &'static str,
    /// The button style key.
    pub button_style: &'static str,
}

/// The samples file.
#[derive(Debug, Serialize)]
pub struct ThemeSamples {
    /// Brand colours with the palettes each preset derives from them.
    pub derive: Vec<DeriveSample>,
    /// Colours with the nearest colour that passes against a background set.
    pub nearest: Vec<NearestSample>,
    /// Stored maps with the palettes they resolve to.
    pub resolve: Vec<ResolveSample>,
}

/// One derivation.
#[derive(Debug, Serialize)]
pub struct DeriveSample {
    /// The brand colour.
    pub brand: String,
    /// The preset whose recipe derived it.
    pub preset: &'static str,
    /// The light palette.
    pub light: PaletteEntry,
    /// The dark palette.
    pub dark: PaletteEntry,
}

/// One nearest-passing suggestion.
#[derive(Debug, Serialize)]
pub struct NearestSample {
    /// The colour that may fail.
    pub colour: String,
    /// The backgrounds it must pass against.
    pub against: Vec<String>,
    /// The nearest colour that passes.
    pub suggestion: String,
}

/// One resolution.
#[derive(Debug, Serialize)]
pub struct ResolveSample {
    /// The stored map.
    pub stored: serde_json::Value,
    /// The scheme it resolves to.
    pub scheme: &'static str,
    /// The light palette it resolves to.
    pub light: PaletteEntry,
    /// The dark palette it resolves to.
    pub dark: PaletteEntry,
}
