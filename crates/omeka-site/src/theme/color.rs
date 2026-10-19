//! Colour values and the arithmetic the palette needs: hex parsing, OKLab, and
//! WCAG contrast.
//!
//! Every function here is plain `f64` arithmetic with no trigonometry, and the
//! admin repeats it in `web/admin/src/pages/sites/themeColour.ts`. The two must
//! give the same hex for the same input, and the derivation samples in
//! `web/admin/src/generated/themeSettings.json` pin them together: a Rust test
//! writes the samples and an admin test derives them again.

/// The WCAG AA ratio for body text.
pub const AA: f64 = 4.5;

/// The step the lightness searches take, in OKLab `L`.
const STEP: f64 = 0.005;

/// A colour a token accepts: `#rgb` or `#rrggbb`, normalised to lower-case
/// `#rrggbb`. The parser and [`HexColor::from_rgb`] are the only constructors,
/// so an emitted value is always a hex colour and never carries CSS of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HexColor([u8; 3]);

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
        let channel = |at: usize| u8::from_str_radix(&expanded[at..at + 2], 16).ok();
        Some(Self([channel(0)?, channel(2)?, channel(4)?]))
    }

    /// A colour from its three 8-bit channels.
    #[must_use]
    pub const fn from_rgb(rgb: [u8; 3]) -> Self {
        Self(rgb)
    }

    /// The three 8-bit channels.
    #[must_use]
    pub const fn rgb(self) -> [u8; 3] {
        self.0
    }

    /// The normalised `#rrggbb` form.
    #[must_use]
    pub fn hex(self) -> String {
        let [r, g, b] = self.0;
        format!("#{r:02x}{g:02x}{b:02x}")
    }

    /// The colour in OKLab.
    #[must_use]
    pub fn oklab(self) -> Oklab {
        let [r, g, b] = self.0.map(|channel| linear(f64::from(channel) / 255.0));
        let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
        let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
        let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
        Oklab {
            l: 0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
            a: 1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
            b: 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
        }
    }

    /// WCAG 2 relative luminance.
    #[must_use]
    pub fn luminance(self) -> f64 {
        let [r, g, b] = self.0.map(|channel| linear(f64::from(channel) / 255.0));
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }
}

/// The WCAG 2 contrast ratio of two colours, 1 to 21.
#[must_use]
pub fn contrast(first: HexColor, second: HexColor) -> f64 {
    let (x, y) = (first.luminance(), second.luminance());
    let (lighter, darker) = if x >= y { (x, y) } else { (y, x) };
    (lighter + 0.05) / (darker + 0.05)
}

/// Whether `colour` reaches `ratio` against every colour in `against`.
#[must_use]
pub fn passes(colour: HexColor, against: &[HexColor], ratio: f64) -> bool {
    against
        .iter()
        .all(|other| contrast(colour, *other) >= ratio)
}

/// A colour in OKLab: perceptual lightness `l` from 0 to 1, and the two
/// opponent axes `a` (green to red) and `b` (blue to yellow).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklab {
    /// Lightness, 0 to 1.
    pub l: f64,
    /// Green to red.
    pub a: f64,
    /// Blue to yellow.
    pub b: f64,
}

impl Oklab {
    /// The chroma, the distance from grey.
    #[must_use]
    pub fn chroma(self) -> f64 {
        (self.a * self.a + self.b * self.b).sqrt()
    }

    /// The same colour at another lightness.
    #[must_use]
    pub fn with_l(self, l: f64) -> Self {
        Self { l, ..self }
    }

    /// The nearest sRGB colour: chroma is reduced until the colour fits the
    /// sRGB gamut, keeping lightness and hue, then each channel is rounded.
    #[must_use]
    pub fn to_hex(self) -> HexColor {
        let l = self.l.clamp(0.0, 1.0);
        let fits = |scale: f64| in_gamut(linear_rgb(l, self.a * scale, self.b * scale));
        let scale = if fits(1.0) {
            1.0
        } else {
            let (mut low, mut high) = (0.0, 1.0);
            for _ in 0..24 {
                let middle = (low + high) / 2.0;
                if fits(middle) {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            low
        };
        let rgb = linear_rgb(l, self.a * scale, self.b * scale);
        HexColor(rgb.map(|channel| byte(gamma(channel.clamp(0.0, 1.0)))))
    }
}

/// The colour on `colour`'s hue and chroma whose lightness is nearest to its
/// own and reaches `ratio` against every colour in `against`.
///
/// The search steps lightness both ways from the start; at an equal distance
/// the darker candidate wins. When no lightness passes, the answer is the
/// lighter or darker extreme, whichever contrasts more.
#[must_use]
pub fn nearest_passing(colour: HexColor, against: &[HexColor], ratio: f64) -> HexColor {
    if passes(colour, against, ratio) {
        return colour;
    }
    let lab = colour.oklab();
    for index in 1..=200 {
        let offset = STEP * f64::from(index);
        for l in [lab.l - offset, lab.l + offset] {
            if (0.0..=1.0).contains(&l) {
                let candidate = lab.with_l(l).to_hex();
                if passes(candidate, against, ratio) {
                    return candidate;
                }
            }
        }
    }
    extreme(against)
}

/// Step `start`'s lightness in `direction` (`-1.0` darker, `1.0` lighter)
/// until the colour reaches `ratio` against every colour in `against`.
#[must_use]
pub fn step_until(start: Oklab, direction: f64, against: &[HexColor], ratio: f64) -> HexColor {
    for index in 0..=200 {
        let l = (start.l + direction * STEP * f64::from(index)).clamp(0.0, 1.0);
        let candidate = start.with_l(l).to_hex();
        if passes(candidate, against, ratio) {
            return candidate;
        }
        if l <= 0.0 || l >= 1.0 {
            break;
        }
    }
    extreme(against)
}

/// White or black, whichever contrasts more with the worst of `against`.
fn extreme(against: &[HexColor]) -> HexColor {
    let white = HexColor([255, 255, 255]);
    let black = HexColor([0, 0, 0]);
    let worst = |colour: HexColor| {
        against
            .iter()
            .map(|other| contrast(colour, *other))
            .fold(f64::INFINITY, f64::min)
    };
    if worst(white) >= worst(black) {
        white
    } else {
        black
    }
}

/// OKLab to linear sRGB.
fn linear_rgb(l: f64, a: f64, b: f64) -> [f64; 3] {
    let l_ = l + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m_ = l - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s_ = l - 0.089_484_177_5 * a - 1.291_485_548 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    [
        4.076_741_662_1 * l3 - 3.307_711_591_3 * m3 + 0.230_969_929_2 * s3,
        -1.268_438_004_6 * l3 + 2.609_757_401_1 * m3 - 0.341_319_396_5 * s3,
        -0.004_196_086_3 * l3 - 0.703_418_614_7 * m3 + 1.707_614_701 * s3,
    ]
}

/// Whether linear sRGB channels sit inside the gamut, allowing for rounding.
fn in_gamut(rgb: [f64; 3]) -> bool {
    rgb.iter()
        .all(|channel| (-1e-7..=1.000_000_1).contains(channel))
}

/// An sRGB channel, 0 to 1, to linear light.
fn linear(channel: f64) -> f64 {
    if channel <= 0.040_45 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear light to an sRGB channel, 0 to 1.
fn gamma(channel: f64) -> f64 {
    if channel <= 0.003_130_8 {
        channel * 12.92
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    }
}

/// A 0 to 1 channel as a byte.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn byte(channel: f64) -> u8 {
    // The channel is clamped to 0..=1 by the caller, so the product is 0..=255.
    (channel * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(raw: &str) -> HexColor {
        HexColor::parse(raw).expect("a hex colour")
    }

    #[test]
    fn hex_parses_short_and_long_forms_and_normalises() {
        assert_eq!(hex("#ABC").hex(), "#aabbcc");
        assert_eq!(hex(" #1A5D8F ").hex(), "#1a5d8f");
        for bad in ["red", "#12345", "#ggg", "123456", "#1234567", "", "#"] {
            assert_eq!(HexColor::parse(bad), None, "{bad}");
        }
    }

    /// WCAG's own reference points: black on white is 21:1, a colour on itself
    /// is 1:1, and `#767676` on white is the well-known 4.54:1.
    #[test]
    fn contrast_matches_the_wcag_reference_points() {
        assert!((contrast(hex("#000"), hex("#fff")) - 21.0).abs() < 1e-9);
        assert!((contrast(hex("#6aa9d6"), hex("#6aa9d6")) - 1.0).abs() < 1e-9);
        let grey = contrast(hex("#767676"), hex("#fff"));
        assert!((grey - 4.54).abs() < 0.01, "{grey}");
    }

    /// A colour survives the trip through OKLab unchanged.
    #[test]
    fn oklab_round_trips_every_sampled_colour() {
        for raw in [
            "#000000", "#ffffff", "#6aa9d6", "#ffff00", "#b45309", "#12161a",
        ] {
            assert_eq!(hex(raw).oklab().to_hex(), hex(raw), "{raw}");
        }
    }

    /// OKLab's published reference: white is L 1 with no chroma.
    #[test]
    fn oklab_white_is_lightness_one() {
        let white = hex("#fff").oklab();
        assert!((white.l - 1.0).abs() < 1e-6);
        assert!(white.chroma() < 1e-6);
    }

    /// A colour outside sRGB keeps its lightness and loses chroma.
    #[test]
    fn out_of_gamut_colours_are_mapped_by_chroma() {
        let vivid = Oklab {
            l: 0.6,
            a: 0.4,
            b: 0.4,
        };
        let mapped = vivid.to_hex().oklab();
        assert!((mapped.l - 0.6).abs() < 0.01, "{mapped:?}");
        assert!(mapped.chroma() < vivid.chroma());
    }

    #[test]
    fn nearest_passing_keeps_a_passing_colour() {
        assert_eq!(
            nearest_passing(hex("#222"), &[hex("#fff")], AA),
            hex("#222")
        );
    }

    /// A failing link on white moves darker just far enough to pass: the ratio
    /// lands just over the line, not at black.
    #[test]
    fn nearest_passing_moves_lightness_just_far_enough() {
        let start = hex("#6aa9d6");
        let suggestion = nearest_passing(start, &[hex("#fff")], AA);
        let ratio = contrast(suggestion, hex("#fff"));
        assert!((AA..4.8).contains(&ratio), "{suggestion:?} at {ratio}");
        assert!(suggestion.oklab().l < start.oklab().l);
    }

    /// Against a dark background the suggestion moves lighter.
    #[test]
    fn nearest_passing_moves_lighter_on_a_dark_background() {
        let dark = hex("#12161a");
        let suggestion = nearest_passing(hex("#1a5d8f"), &[dark], AA);
        assert!(suggestion.oklab().l > hex("#1a5d8f").oklab().l);
        assert!(contrast(suggestion, dark) >= AA);
    }
}
