//! The theme's palette: the seven colours a curator may pick, the one the
//! theme picks for text on the brand colour, the text and background pairs the
//! sheet draws, and the derivation that builds a whole palette from one brand
//! colour.
//!
//! The derivation works in OKLab ([`crate::theme::color`]). It takes the brand's
//! hue for every tint, places each colour at a lightness the preset's
//! [`Recipe`] names, then steps the text colours' lightness until each reaches
//! AA against both backgrounds. So a derived palette passes the contrast pairs
//! by construction, for any brand colour.

use crate::theme::color::{AA, HexColor, Oklab, contrast, passes, step_until};

/// Which half of a colour scheme a palette serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shade {
    /// Dark text on a light background.
    Light,
    /// Light text on a dark background.
    Dark,
}

/// A colour the sheet reads, as a custom property `--color-<key>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// Brand marks, bands and solid buttons.
    Brand,
    /// Text on the brand colour. The theme picks it; no one stores it.
    OnBrand,
    /// Emphasis and small accents.
    Accent,
    /// The page background.
    Background,
    /// Cards, bars and inset surfaces.
    Surface,
    /// Body text.
    Text,
    /// Secondary text.
    Muted,
    /// Link text.
    Link,
}

impl Role {
    /// Every role, in the order the sheet's properties are emitted.
    pub const ALL: [Self; 8] = [
        Self::Brand,
        Self::OnBrand,
        Self::Accent,
        Self::Background,
        Self::Surface,
        Self::Text,
        Self::Muted,
        Self::Link,
    ];

    /// The roles a curator may override, each with its stored key.
    pub const EDITABLE: [(Self, &'static str); 7] = [
        (Self::Brand, "brand_color"),
        (Self::Accent, "accent_color"),
        (Self::Background, "background_color"),
        (Self::Surface, "surface_color"),
        (Self::Text, "text_color"),
        (Self::Muted, "muted_color"),
        (Self::Link, "link_color"),
    ];

    /// The custom property suffix, `--color-<key>`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Brand => "brand",
            Self::OnBrand => "on-brand",
            Self::Accent => "accent",
            Self::Background => "background",
            Self::Surface => "surface",
            Self::Text => "text",
            Self::Muted => "muted",
            Self::Link => "link",
        }
    }
}

/// Every text and background pair `style.css` draws, as `(foreground,
/// background)`. The contrast gate checks each one in every shade the scheme
/// shows. The header and button tokens only ever combine these same pairs.
pub const PAIRS: [(Role, Role); 9] = [
    (Role::Text, Role::Background),
    (Role::Text, Role::Surface),
    (Role::Muted, Role::Background),
    (Role::Muted, Role::Surface),
    (Role::Link, Role::Background),
    (Role::Link, Role::Surface),
    (Role::Accent, Role::Background),
    (Role::Accent, Role::Surface),
    (Role::OnBrand, Role::Brand),
];

/// The seven colours a curator picks. Text on the brand colour is derived from
/// them by [`Palette::get`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Brand marks, bands and solid buttons.
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
}

impl Palette {
    /// The colour a role resolves to.
    #[must_use]
    pub fn get(&self, role: Role) -> HexColor {
        match role {
            Role::Brand => self.brand,
            Role::OnBrand => on_brand(self.brand, self.text, self.background),
            Role::Accent => self.accent,
            Role::Background => self.background,
            Role::Surface => self.surface,
            Role::Text => self.text,
            Role::Muted => self.muted,
            Role::Link => self.link,
        }
    }

    /// Replace an editable role's colour. [`Role::OnBrand`] is derived, so
    /// setting it changes nothing.
    pub fn set(&mut self, role: Role, colour: HexColor) {
        match role {
            Role::Brand => self.brand = colour,
            Role::OnBrand => {}
            Role::Accent => self.accent = colour,
            Role::Background => self.background = colour,
            Role::Surface => self.surface = colour,
            Role::Text => self.text = colour,
            Role::Muted => self.muted = colour,
            Role::Link => self.link = colour,
        }
    }

    /// Every pair in [`PAIRS`] that falls below AA, with its ratio.
    #[must_use]
    pub fn failures(&self) -> Vec<(Role, Role, f64)> {
        PAIRS
            .iter()
            .map(|&(fg, bg)| (fg, bg, contrast(self.get(fg), self.get(bg))))
            .filter(|&(_, _, ratio)| ratio < AA)
            .collect()
    }
}

/// Text on the brand colour: the palette's text or background colour,
/// whichever contrasts more, when it reaches AA; otherwise white or black.
/// Any colour reaches AA against one of white and black, so this always does.
#[must_use]
pub fn on_brand(brand: HexColor, text: HexColor, background: HexColor) -> HexColor {
    let best = if contrast(text, brand) >= contrast(background, brand) {
        text
    } else {
        background
    };
    if contrast(best, brand) >= AA {
        return best;
    }
    let white = HexColor::from_rgb([255, 255, 255]);
    let black = HexColor::from_rgb([0, 0, 0]);
    if contrast(white, brand) >= contrast(black, brand) {
        white
    } else {
        black
    }
}

/// How a preset turns one brand colour into a palette.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recipe {
    /// How strongly the backgrounds and greys take the brand's hue, 0 for
    /// pure greys.
    pub tint: f64,
    /// The light background's OKLab lightness.
    pub paper: f64,
    /// The dark background's OKLab lightness.
    pub ink: f64,
}

/// The accent's hue when the brand is a grey and has none: a warm amber.
const GREY_ACCENT_HUE: (f64, f64) = (0.418_6, 0.908_2);

/// The palette for one shade, derived from `brand`.
///
/// The brand keeps its own value. The backgrounds take its hue at the recipe's
/// lightness, the text colours start where a reader expects them and move
/// away from the backgrounds until each pair reaches AA, and the accent takes
/// the opposite hue.
#[must_use]
pub fn derive(brand: HexColor, recipe: Recipe, shade: Shade) -> Palette {
    let lab = brand.oklab();
    let chroma = lab.chroma();
    let (hue_a, hue_b) = if chroma > 1e-4 {
        (lab.a / chroma, lab.b / chroma)
    } else {
        (0.0, 0.0)
    };
    let (accent_a, accent_b) = if chroma > 1e-4 {
        (-hue_a, -hue_b)
    } else {
        GREY_ACCENT_HUE
    };
    let tinted = |l: f64, c: f64| Oklab {
        l,
        a: hue_a * c,
        b: hue_b * c,
    };
    let link_chroma = chroma
        .clamp(0.0, 0.2)
        .max(if chroma > 1e-4 { 0.08 } else { 0.0 });
    let accent = |l: f64| Oklab {
        l,
        a: accent_a * 0.12,
        b: accent_b * 0.12,
    };
    match shade {
        Shade::Light => {
            let background = tinted(recipe.paper, 0.012 * recipe.tint).to_hex();
            let surface = tinted(recipe.paper - 0.035, 0.02 * recipe.tint).to_hex();
            let grounds = [background, surface];
            let darker = |start: Oklab| step_until(start, -1.0, &grounds, AA);
            Palette {
                brand,
                accent: darker(accent(0.56)),
                background,
                surface,
                text: darker(tinted(0.25, 0.02 * recipe.tint)),
                muted: darker(tinted(0.52, 0.03 * recipe.tint)),
                link: darker(tinted(lab.l.min(0.6), link_chroma)),
            }
        }
        Shade::Dark => {
            let background = tinted(recipe.ink, 0.012 * recipe.tint).to_hex();
            let surface = tinted(recipe.ink + 0.05, 0.018 * recipe.tint).to_hex();
            let grounds = [background, surface];
            let lighter = |start: Oklab| step_until(start, 1.0, &grounds, AA);
            Palette {
                brand,
                accent: lighter(accent(0.8)),
                background,
                surface,
                text: lighter(tinted(0.94, 0.01 * recipe.tint)),
                muted: lighter(tinted(0.74, 0.025 * recipe.tint)),
                link: lighter(tinted(lab.l.max(0.78), link_chroma)),
            }
        }
    }
}

/// Whether every pair in [`PAIRS`] reaches AA.
#[must_use]
pub fn is_accessible(palette: &Palette) -> bool {
    PAIRS
        .iter()
        .all(|&(fg, bg)| passes(palette.get(fg), &[palette.get(bg)], AA))
}
