//! The webfonts the theme serves from its own assets, and the head markup that
//! loads them.
//!
//! Each file is a WOFF2 subset, compiled in and served at `fonts/<file>` like
//! the stylesheet. The token sheet declares an `@font-face` only for the
//! families the site's two font stacks name ([`FontStack::served`]), so a site
//! on system faces loads no font at all. The frame preloads the Latin file of
//! each face the site uses. A Noto face for an Indic script carries its
//! `unicode-range`, so the browser fetches it only when a page holds that
//! script. Every face uses `font-display: swap`: the text shows at once in the
//! fallback face. The licences are in `fonts/OFL.txt`.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::html::escape_html;
use crate::theme::choices::FontStack;
use crate::theme::{AssetKind, ThemeAsset};

/// One WOFF2 file of one face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontFile {
    /// The `font-family` name the stacks use.
    pub family: &'static str,
    /// The path under the theme's asset base.
    pub path: &'static str,
    /// The `font-weight` range the file covers.
    pub weight: &'static str,
    /// The `unicode-range`: the Latin subset, or one Indic script.
    pub range: &'static str,
    /// Whether the file is the Latin subset, which the frame preloads.
    pub latin: bool,
    /// The bytes.
    pub bytes: &'static [u8],
}

/// Fontsource's Latin subset range.
const LATIN: &str = "U+0000-00FF, U+0131, U+0152-0153, U+02BB-02BC, U+02C6, U+02DA, U+02DC, \
                     U+0304, U+0308, U+0329, U+2000-206F, U+20AC, U+2122, U+2191, U+2193, \
                     U+2212, U+2215, U+FEFF, U+FFFD";
const DEVANAGARI: &str =
    "U+0900-097F, U+1CD0-1CF9, U+200C-200D, U+20A8, U+20B9, U+25CC, U+A830-A839, U+A8E0-A8FF";
const TAMIL: &str = "U+0B82-0BFA, U+200C-200D, U+25CC";
const TELUGU: &str = "U+0C00-0C7F, U+200C-200D, U+25CC";
const KANNADA: &str = "U+0C80-0CF3, U+200C-200D, U+25CC";
const MALAYALAM: &str = "U+0D00-0D7F, U+200C-200D, U+25CC";

macro_rules! font {
    ($family:literal, $file:literal, $weight:literal, $range:expr, $latin:literal) => {
        FontFile {
            family: $family,
            path: concat!("fonts/", $file),
            weight: $weight,
            range: $range,
            latin: $latin,
            bytes: include_bytes!(concat!("fonts/", $file)),
        }
    };
}

/// Every webfont file the theme ships.
pub const FONT_FILES: [FontFile; 16] = [
    font!(
        "Fraunces",
        "fraunces-latin-opsz.woff2",
        "100 900",
        LATIN,
        true
    ),
    font!(
        "Source Sans 3",
        "source-sans-3-latin-wght.woff2",
        "200 900",
        LATIN,
        true
    ),
    font!(
        "IBM Plex Sans",
        "ibm-plex-sans-latin-wght.woff2",
        "100 700",
        LATIN,
        true
    ),
    font!("Inter", "inter-latin-opsz.woff2", "100 900", LATIN, true),
    font!(
        "IBM Plex Mono",
        "ibm-plex-mono-latin-400.woff2",
        "400",
        LATIN,
        true
    ),
    font!(
        "IBM Plex Mono",
        "ibm-plex-mono-latin-500.woff2",
        "500",
        LATIN,
        true
    ),
    font!(
        "Noto Sans Devanagari",
        "noto-sans-devanagari-wght.woff2",
        "100 900",
        DEVANAGARI,
        false
    ),
    font!(
        "Noto Sans Tamil",
        "noto-sans-tamil-wght.woff2",
        "100 900",
        TAMIL,
        false
    ),
    font!(
        "Noto Sans Telugu",
        "noto-sans-telugu-wght.woff2",
        "100 900",
        TELUGU,
        false
    ),
    font!(
        "Noto Sans Kannada",
        "noto-sans-kannada-wght.woff2",
        "100 900",
        KANNADA,
        false
    ),
    font!(
        "Noto Sans Malayalam",
        "noto-sans-malayalam-wght.woff2",
        "100 900",
        MALAYALAM,
        false
    ),
    font!(
        "Noto Serif Devanagari",
        "noto-serif-devanagari-wght.woff2",
        "100 900",
        DEVANAGARI,
        false
    ),
    font!(
        "Noto Serif Tamil",
        "noto-serif-tamil-wght.woff2",
        "100 900",
        TAMIL,
        false
    ),
    font!(
        "Noto Serif Telugu",
        "noto-serif-telugu-wght.woff2",
        "100 900",
        TELUGU,
        false
    ),
    font!(
        "Noto Serif Kannada",
        "noto-serif-kannada-wght.woff2",
        "100 900",
        KANNADA,
        false
    ),
    font!(
        "Noto Serif Malayalam",
        "noto-serif-malayalam-wght.woff2",
        "100 900",
        MALAYALAM,
        false
    ),
];

/// Every font file as a theme asset, for the manifest.
pub const FONT_ASSETS: [ThemeAsset; 16] = {
    let mut out = [ThemeAsset {
        path: "",
        kind: AssetKind::Font,
        content: &[],
    }; 16];
    let mut index = 0;
    while index < FONT_FILES.len() {
        out[index] = ThemeAsset {
            path: FONT_FILES[index].path,
            kind: AssetKind::Font,
            content: FONT_FILES[index].bytes,
        };
        index += 1;
    }
    out
};

/// Each file's URL version, hashed once: a page names every face it uses.
static VERSIONS: LazyLock<BTreeMap<&'static str, String>> = LazyLock::new(|| {
    FONT_ASSETS
        .iter()
        .map(|asset| (asset.path, asset.version()))
        .collect()
});

/// The files the two stacks need, in [`FONT_FILES`] order, each once.
fn files_for(stacks: [FontStack; 2]) -> impl Iterator<Item = &'static FontFile> {
    let mut families: Vec<&'static str> = Vec::new();
    for stack in stacks {
        let (own, fallbacks) = stack.served();
        families.extend(own);
        families.extend_from_slice(fallbacks);
    }
    FONT_FILES
        .iter()
        .filter(move |file| families.contains(&file.family))
}

fn url(asset_base: &str, file: &FontFile) -> String {
    let version = VERSIONS.get(file.path).map_or("", String::as_str);
    escape_html(&format!(
        "{}/{}?v={version}",
        asset_base.trim_end_matches('/'),
        file.path
    ))
}

/// The `@font-face` rules for the faces the two stacks name. Empty for two
/// system stacks.
#[must_use]
pub fn faces(stacks: [FontStack; 2], asset_base: &str) -> String {
    files_for(stacks)
        .map(|file| {
            format!(
                "@font-face {{ font-family: \"{}\"; src: url({}) format(\"woff2\"); \
                 font-weight: {}; font-display: swap; unicode-range: {}; }}\n",
                file.family,
                url(asset_base, file),
                file.weight,
                file.range,
            )
        })
        .collect()
}

/// The preload links for the Latin files of the faces the two stacks name, so
/// the first paint does not wait on the stylesheet to discover them.
#[must_use]
pub fn preloads(stacks: [FontStack; 2], asset_base: &str) -> String {
    files_for(stacks)
        .filter(|file| file.latin)
        .map(|file| {
            format!(
                "<link rel=\"preload\" href=\"{}\" as=\"font\" type=\"font/woff2\" crossorigin>\n",
                url(asset_base, file)
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::theme_asset;

    /// Every face a stack names is a file the theme ships, and the stack's
    /// `font-family` value names it, so a declared face is a used face.
    #[test]
    fn every_served_face_ships_and_is_named_by_its_stack() {
        for &stack in FontStack::ALL {
            let (own, fallbacks) = stack.served();
            for family in own.iter().chain(fallbacks) {
                assert!(
                    FONT_FILES.iter().any(|file| file.family == *family),
                    "{family} has no file"
                );
                assert!(
                    stack.stack().contains(&format!("\"{family}\"")),
                    "{} does not name {family}",
                    stack.key()
                );
            }
        }
    }

    /// The asset route serves each file as a WOFF2 font with its own bytes.
    #[test]
    fn the_theme_serves_every_font_file() {
        for file in &FONT_FILES {
            let asset = theme_asset("default", file.path).expect("a font asset");
            assert_eq!(asset.kind.media_type(), "font/woff2");
            assert_eq!(asset.content, file.bytes);
            assert!(file.bytes.starts_with(b"wOF2"), "{} is WOFF2", file.path);
        }
    }

    /// System stacks load nothing; a webfont pair declares its own faces and
    /// the Noto fallbacks, and preloads only its Latin files.
    #[test]
    fn faces_follow_the_two_stacks() {
        let none = [FontStack::SystemSans, FontStack::SystemSerif];
        assert_eq!(faces(none, "/assets"), "");
        assert_eq!(preloads(none, "/assets"), "");

        let pair = [FontStack::Fraunces, FontStack::SourceSans3];
        let declared = faces(pair, "/assets");
        assert!(declared.contains(
            "font-family: \"Fraunces\"; src: url(/assets/fonts/fraunces-latin-opsz.woff2?v="
        ));
        assert!(declared.contains("\"Noto Serif Tamil\""));
        assert!(declared.contains("\"Noto Sans Tamil\""));
        assert!(!declared.contains("\"Inter\""));
        assert!(declared.contains("font-display: swap"));
        let preloaded = preloads(pair, "/assets");
        assert_eq!(preloaded.matches("rel=\"preload\"").count(), 2);
        assert!(preloaded.contains("/assets/fonts/source-sans-3-latin-wght.woff2?v="));
        assert!(!preloaded.contains("noto"));
    }
}
