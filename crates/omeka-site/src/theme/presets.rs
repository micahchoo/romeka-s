//! The five presets: complete looks a curator starts from. Each fixes every
//! design token, a light and a dark palette, and the recipe that derives a
//! palette from a brand colour in the preset's own manner.
//!
//! A site stores the id of its preset. Its effective tokens are the preset's,
//! then a palette derived from the brand colour when derivation is on, then the
//! curator's own overrides ([`crate::theme::tokens::resolve`]).

use crate::theme::choices::{
    ButtonStyle, ColorScheme, ContentWidth, Density, FontStack, HeaderLayout, HeaderStyle, choice,
};
use crate::theme::color::HexColor;
use crate::theme::palette::{Palette, Recipe};

choice! {
    /// A preset a site stores by id.
    PresetId {
        /// The theme's original look: system faces, a soft blue brand.
        Plain = "plain",
        /// Warm paper, serif type and a centred masthead.
        Archive = "archive",
        /// A dark, image-first page with almost no chrome.
        Gallery = "gallery",
        /// High-contrast serif headings over a narrow reading measure.
        Editorial = "editorial",
        /// A bright brand band and plain sans type.
        Civic = "civic",
    }
}

/// The preset a site created through the API starts from.
pub const NEW_SITE_PRESET: PresetId = PresetId::Archive;

/// Every token a preset fixes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Preset {
    /// The id a site stores.
    pub id: PresetId,
    /// The name the gallery shows, as source text.
    pub label: &'static str,
    /// The scheme the page shows.
    pub scheme: ColorScheme,
    /// The palette the light scheme shows when derivation is off.
    pub light: Palette,
    /// The palette the dark scheme shows when derivation is off.
    pub dark: Palette,
    /// How a brand colour becomes a palette when derivation is on.
    pub recipe: Recipe,
    /// The heading font stack.
    pub heading_font: FontStack,
    /// The body font stack.
    pub body_font: FontStack,
    /// The body font size in pixels.
    pub base_size: u32,
    /// The type scale as a percentage.
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

impl PresetId {
    /// The preset this id names.
    #[must_use]
    pub fn preset(self) -> &'static Preset {
        match self {
            Self::Plain => &PRESETS[0],
            Self::Archive => &PRESETS[1],
            Self::Gallery => &PRESETS[2],
            Self::Editorial => &PRESETS[3],
            Self::Civic => &PRESETS[4],
        }
    }
}

/// A colour from a `0xrrggbb` literal.
const fn rgb(hex: u32) -> HexColor {
    let [_, r, g, b] = hex.to_be_bytes();
    HexColor::from_rgb([r, g, b])
}

/// A palette from seven literals: brand, accent, background, surface, text,
/// muted, link.
const fn palette(colours: [u32; 7]) -> Palette {
    Palette {
        brand: rgb(colours[0]),
        accent: rgb(colours[1]),
        background: rgb(colours[2]),
        surface: rgb(colours[3]),
        text: rgb(colours[4]),
        muted: rgb(colours[5]),
        link: rgb(colours[6]),
    }
}

/// Every preset, in the order the gallery shows them.
pub static PRESETS: [Preset; 5] = [
    Preset {
        id: PresetId::Plain,
        label: "Plain",
        scheme: ColorScheme::Light,
        light: palette([
            0x6aa9d6, 0xb45309, 0xffffff, 0xf5f7f8, 0x222222, 0x5a6470, 0x1a5d8f,
        ]),
        dark: palette([
            0x6aa9d6, 0xe8b27a, 0x12161a, 0x1b2126, 0xe8ecef, 0x9aa6b2, 0x8ab6e8,
        ]),
        recipe: Recipe {
            tint: 0.3,
            paper: 1.0,
            ink: 0.19,
        },
        heading_font: FontStack::SystemSerif,
        body_font: FontStack::SystemSans,
        base_size: 16,
        type_ratio: 125,
        density: Density::Comfortable,
        radius: 4,
        header_layout: HeaderLayout::LogoLeft,
        header_style: HeaderStyle::Plain,
        button_style: ButtonStyle::Soft,
        content_width: ContentWidth::Standard,
    },
    Preset {
        id: PresetId::Archive,
        label: "Archive",
        scheme: ColorScheme::Light,
        light: palette([
            0x7a4a14, 0x6b5420, 0xfaf6ee, 0xf0e8d8, 0x2b241c, 0x675a49, 0x7a4a14,
        ]),
        dark: palette([
            0xd08a5a, 0xd9bf84, 0x1d1915, 0x27221c, 0xefe7da, 0xb5a893, 0xe3a477,
        ]),
        recipe: Recipe {
            tint: 1.8,
            paper: 0.975,
            ink: 0.2,
        },
        heading_font: FontStack::TransitionalSerif,
        body_font: FontStack::SystemSerif,
        base_size: 17,
        type_ratio: 120,
        density: Density::Comfortable,
        radius: 2,
        header_layout: HeaderLayout::Centred,
        header_style: HeaderStyle::Rule,
        button_style: ButtonStyle::Outline,
        content_width: ContentWidth::Standard,
    },
    Preset {
        id: PresetId::Gallery,
        label: "Gallery",
        scheme: ColorScheme::Dark,
        light: palette([
            0x1c1c1c, 0xa3540f, 0xfafafa, 0xf0f0f0, 0x161616, 0x5c5c5c, 0x1c1c1c,
        ]),
        dark: palette([
            0xf2f2f2, 0xf0b35a, 0x0f0f10, 0x1a1a1c, 0xededed, 0xa6a6a6, 0xc9d7ff,
        ]),
        recipe: Recipe {
            tint: 0.15,
            paper: 0.985,
            ink: 0.16,
        },
        heading_font: FontStack::Geometric,
        body_font: FontStack::SystemSans,
        base_size: 15,
        type_ratio: 130,
        density: Density::Compact,
        radius: 0,
        header_layout: HeaderLayout::LogoLeft,
        header_style: HeaderStyle::Minimal,
        button_style: ButtonStyle::Outline,
        content_width: ContentWidth::Wide,
    },
    Preset {
        id: PresetId::Editorial,
        label: "Editorial",
        scheme: ColorScheme::Light,
        light: palette([
            0x111111, 0xb3001b, 0xffffff, 0xf4f3ef, 0x121212, 0x545454, 0xb3001b,
        ]),
        dark: palette([
            0xf5f5f5, 0xff6b7a, 0x141414, 0x1f1f1f, 0xf2f2f2, 0xa8a8a8, 0xff8a96,
        ]),
        recipe: Recipe {
            tint: 0.0,
            paper: 1.0,
            ink: 0.17,
        },
        heading_font: FontStack::Didone,
        body_font: FontStack::TransitionalSerif,
        base_size: 18,
        type_ratio: 140,
        density: Density::Spacious,
        radius: 0,
        header_layout: HeaderLayout::Centred,
        header_style: HeaderStyle::Rule,
        button_style: ButtonStyle::Solid,
        content_width: ContentWidth::Narrow,
    },
    Preset {
        id: PresetId::Civic,
        label: "Civic",
        scheme: ColorScheme::Light,
        light: palette([
            0x0b57d0, 0xb93c0b, 0xffffff, 0xeef3fc, 0x18202e, 0x4a5568, 0x0a4ec2,
        ]),
        dark: palette([
            0x0b57d0, 0xf59e6b, 0x0f1724, 0x172236, 0xe8eef8, 0x9fb0c8, 0x8fb7ff,
        ]),
        recipe: Recipe {
            tint: 0.6,
            paper: 1.0,
            ink: 0.2,
        },
        heading_font: FontStack::Humanist,
        body_font: FontStack::Humanist,
        base_size: 16,
        type_ratio: 125,
        density: Density::Comfortable,
        radius: 8,
        header_layout: HeaderLayout::LogoLeft,
        header_style: HeaderStyle::Band,
        button_style: ButtonStyle::Solid,
        content_width: ContentWidth::Wide,
    },
];

/// A map stored before presets existed reads as the plain preset, the look
/// those sites had.
impl Default for PresetId {
    fn default() -> Self {
        Self::Plain
    }
}
