//! The five presets: complete looks a curator starts from. Each fixes every
//! design token, a light and a dark palette, and the recipe that derives a
//! palette from a brand colour in the preset's own manner. The presets differ
//! in structure, not only in colour: the hero, the cards, the search, the
//! footer and the item page each take the preset's own layout.
//!
//! A site stores the id of its preset. Its effective tokens are the preset's,
//! then a palette derived from the brand colour when derivation is on, then the
//! curator's own overrides ([`crate::theme::tokens::resolve`]).
//!
//! The ids a site stored before these presets existed still read: each names
//! the new preset nearest the look it had ([`PresetId::from_stored`]).

use crate::theme::choices::{
    ButtonStyle, CardStyle, ColorScheme, ContentWidth, Density, FontStack, FooterLayout,
    FooterTone, HeaderLayout, HeaderStyle, HeroStyle, ItemLayout, SearchDisplay, choice,
};
use crate::theme::color::HexColor;
use crate::theme::palette::{Palette, Recipe};

choice! {
    /// A preset a site stores by id.
    PresetId {
        /// Warm paper, a serif masthead, a photograph across the screen and
        /// captioned cards over a dark footer.
        Exhibit = "exhibit",
        /// A quiet, dense catalogue: a dark bar header with the search open,
        /// one row per resource, and a sticky image beside the record.
        Catalogue = "catalogue",
        /// A dark image wall: square tiles, a framed photograph, a minimal
        /// header.
        Gallery = "gallery",
        /// Long-form reading: the Exhibit masthead and serif type on a narrow
        /// column.
        Journal = "journal",
        /// Bright and open: a brand-colour bar, Inter, large buttons and a
        /// footer in columns.
        Community = "community",
    }
}

/// The preset a site created through the API starts from.
pub const NEW_SITE_PRESET: PresetId = PresetId::Exhibit;

/// The ids the theme's first five presets stored, each with the preset that
/// now stands for it. `archive` named the warm serif look that became Exhibit,
/// so the dense catalogue look has an id of its own.
pub const LEGACY_IDS: [(&str, PresetId); 4] = [
    ("plain", PresetId::Catalogue),
    ("archive", PresetId::Exhibit),
    ("editorial", PresetId::Journal),
    ("civic", PresetId::Community),
];

impl PresetId {
    /// The preset a stored id names: a current id, or a legacy one read as
    /// the preset that replaced it. `None` for any other string.
    #[must_use]
    pub fn from_stored(key: &str) -> Option<Self> {
        Self::parse(key).or_else(|| {
            LEGACY_IDS
                .iter()
                .find(|(legacy, _)| *legacy == key)
                .map(|&(_, preset)| preset)
        })
    }

    /// The preset this id names.
    #[must_use]
    pub fn preset(self) -> &'static Preset {
        match self {
            Self::Exhibit => &PRESETS[0],
            Self::Catalogue => &PRESETS[1],
            Self::Gallery => &PRESETS[2],
            Self::Journal => &PRESETS[3],
            Self::Community => &PRESETS[4],
        }
    }
}

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
    /// The structural layouts.
    pub structure: Structure,
}

/// The structural tokens: how the page is built rather than how it is
/// coloured or set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Structure {
    /// How a hero with a photograph lays it out.
    pub hero: HeroStyle,
    /// How a resource list draws each resource.
    pub cards: CardStyle,
    /// How the header shows the search on a wide screen.
    pub search: SearchDisplay,
    /// How the footer arranges its parts.
    pub footer: FooterLayout,
    /// The footer's ground.
    pub footer_tone: FooterTone,
    /// Where an item page puts its media.
    pub item: ItemLayout,
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
        id: PresetId::Exhibit,
        label: "Exhibit",
        scheme: ColorScheme::Light,
        light: palette([
            0x8c2f1b, 0x8c2f1b, 0xf6f2ea, 0xebe4d6, 0x1f1b16, 0x6a6052, 0x8c2f1b,
        ]),
        dark: palette([
            0xd9876a, 0xe9a07f, 0x1c1915, 0x26221c, 0xefe8dc, 0xb8ad9a, 0xeba287,
        ]),
        recipe: Recipe {
            tint: 1.6,
            paper: 0.965,
            ink: 0.2,
        },
        heading_font: FontStack::Fraunces,
        body_font: FontStack::SourceSans3,
        base_size: 17,
        type_ratio: 125,
        density: Density::Spacious,
        radius: 2,
        header_layout: HeaderLayout::Centred,
        header_style: HeaderStyle::Rule,
        button_style: ButtonStyle::Solid,
        content_width: ContentWidth::Wide,
        structure: Structure {
            hero: HeroStyle::Cover,
            cards: CardStyle::Caption,
            search: SearchDisplay::Icon,
            footer: FooterLayout::Columns,
            footer_tone: FooterTone::Inverse,
            item: ItemLayout::Stacked,
        },
    },
    Preset {
        id: PresetId::Catalogue,
        label: "Catalogue",
        scheme: ColorScheme::Light,
        light: palette([
            0x1d2a33, 0x0b5a63, 0xfafaf9, 0xeef0f1, 0x1b2227, 0x56616b, 0x0b5a63,
        ]),
        dark: palette([
            0x2b3f4c, 0x7cc4cc, 0x12171a, 0x1b2227, 0xe6ebee, 0xa0acb5, 0x7cc4cc,
        ]),
        recipe: Recipe {
            tint: 0.4,
            paper: 0.985,
            ink: 0.19,
        },
        heading_font: FontStack::IbmPlexSans,
        body_font: FontStack::IbmPlexSans,
        base_size: 15,
        type_ratio: 120,
        density: Density::Compact,
        radius: 3,
        header_layout: HeaderLayout::LogoLeft,
        header_style: HeaderStyle::Band,
        button_style: ButtonStyle::Solid,
        content_width: ContentWidth::Wide,
        structure: Structure {
            hero: HeroStyle::Split,
            cards: CardStyle::Row,
            search: SearchDisplay::Open,
            footer: FooterLayout::Inline,
            footer_tone: FooterTone::Surface,
            item: ItemLayout::Side,
        },
    },
    Preset {
        id: PresetId::Gallery,
        label: "Gallery",
        scheme: ColorScheme::Dark,
        light: palette([
            0x1c1c1c, 0x8a5a12, 0xfafafa, 0xf0f0f0, 0x161616, 0x5c5c5c, 0x7a4f0e,
        ]),
        dark: palette([
            0xf0c27a, 0xf0c27a, 0x111214, 0x1c1e22, 0xecebe8, 0xa3a29d, 0xf0c27a,
        ]),
        recipe: Recipe {
            tint: 0.15,
            paper: 0.985,
            ink: 0.16,
        },
        heading_font: FontStack::Inter,
        body_font: FontStack::Inter,
        base_size: 16,
        type_ratio: 140,
        density: Density::Comfortable,
        radius: 0,
        header_layout: HeaderLayout::LogoLeft,
        header_style: HeaderStyle::Minimal,
        button_style: ButtonStyle::Solid,
        content_width: ContentWidth::Full,
        structure: Structure {
            hero: HeroStyle::Framed,
            cards: CardStyle::Overlay,
            search: SearchDisplay::Icon,
            footer: FooterLayout::Split,
            footer_tone: FooterTone::Page,
            item: ItemLayout::Stacked,
        },
    },
    Preset {
        id: PresetId::Journal,
        label: "Journal",
        scheme: ColorScheme::Light,
        light: palette([
            0x7a2318, 0x7a2318, 0xfffdf8, 0xf3efe6, 0x1d1a16, 0x5f574b, 0x7a2318,
        ]),
        dark: palette([
            0xe39a86, 0xeba287, 0x1a1815, 0x24211d, 0xeee8dd, 0xb5ab9b, 0xeba287,
        ]),
        recipe: Recipe {
            tint: 0.8,
            paper: 0.99,
            ink: 0.19,
        },
        heading_font: FontStack::Fraunces,
        body_font: FontStack::Fraunces,
        base_size: 18,
        type_ratio: 125,
        density: Density::Spacious,
        radius: 0,
        header_layout: HeaderLayout::Centred,
        header_style: HeaderStyle::Rule,
        button_style: ButtonStyle::Outline,
        content_width: ContentWidth::Narrow,
        structure: Structure {
            hero: HeroStyle::Cover,
            cards: CardStyle::Caption,
            search: SearchDisplay::Icon,
            footer: FooterLayout::Inline,
            footer_tone: FooterTone::Page,
            item: ItemLayout::Stacked,
        },
    },
    Preset {
        id: PresetId::Community,
        label: "Community",
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
        heading_font: FontStack::Inter,
        body_font: FontStack::Inter,
        base_size: 17,
        type_ratio: 125,
        density: Density::Spacious,
        radius: 8,
        header_layout: HeaderLayout::LogoLeft,
        header_style: HeaderStyle::Band,
        button_style: ButtonStyle::Solid,
        content_width: ContentWidth::Wide,
        structure: Structure {
            hero: HeroStyle::Split,
            cards: CardStyle::Caption,
            search: SearchDisplay::Open,
            footer: FooterLayout::Columns,
            footer_tone: FooterTone::Surface,
            item: ItemLayout::Side,
        },
    },
];

/// A map stored before presets existed reads as the preset that replaced
/// plain, the theme's first look.
impl Default for PresetId {
    fn default() -> Self {
        Self::Catalogue
    }
}
