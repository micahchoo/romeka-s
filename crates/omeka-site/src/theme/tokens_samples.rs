//! The samples file, `web/admin/src/generated/themeSamples.json`: it pins the
//! admin's copy of the palette arithmetic to this crate's. Brand colours with
//! the palettes they derive, colours with their nearest passing suggestion,
//! and stored maps with the palettes they resolve to. Only the admin's tests
//! import it, so it never reaches the bundle.

use serde::Serialize;

use crate::theme::color::{AA, HexColor, nearest_passing};
use crate::theme::palette::{Shade, derive};
use crate::theme::presets::PRESETS;
use crate::theme::tokens::{Choices, resolve};
use crate::theme::tokens_schema::PaletteEntry;

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
