//! The theme token schema: the kinds the admin renders, the token catalogue,
//! and the generated registry `web/admin/src/generated/themeSettings.json`
//! holds.
//!
//! The values these specs describe and the parser that reads them live in
//! [`crate::theme::tokens`]. This module is the data: one [`ThemeSettingSpec`]
//! per token, in the order the Theme page groups them.

use serde::Serialize;

use crate::theme::tokens::{
    DARK_BACKGROUND, DARK_MUTED, DARK_SURFACE, DARK_TEXT, DEFAULT_ACCENT, DEFAULT_BACKGROUND,
    DEFAULT_BRAND, DEFAULT_LINK, DEFAULT_MUTED, DEFAULT_SURFACE, DEFAULT_TEXT, FONT_KEYS,
    FontStack, MAX_BASE_SIZE, MAX_RADIUS, MAX_TYPE_RATIO, MIN_BASE_SIZE, MIN_TYPE_RATIO,
};

/// The kind of value a theme token holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeSettingKind {
    /// A hex colour, with the default the dark scheme uses when no value is
    /// stored.
    Colour {
        /// The dark scheme's default.
        dark: &'static str,
    },
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
}

/// The value a token has when no map stores it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeSettingDefault {
    /// A text default.
    Text(&'static str),
    /// A number default.
    Number(u32),
    /// No default.
    Unset,
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
    /// The value in force when no map stores the key.
    pub default: ThemeSettingDefault,
}

/// Every token the bundled theme reads, in the order the theme page groups
/// them: palette, typography, layout, header and footer.
pub static SETTINGS: &[ThemeSettingSpec] = &[
    ThemeSettingSpec {
        key: "color_scheme",
        label: "Colour scheme",
        kind: ThemeSettingKind::Choice(&["light", "dark", "auto"]),
        default: ThemeSettingDefault::Text("light"),
    },
    ThemeSettingSpec {
        key: "brand_color",
        label: "Brand colour",
        kind: ThemeSettingKind::Colour {
            dark: DEFAULT_BRAND,
        },
        default: ThemeSettingDefault::Text(DEFAULT_BRAND),
    },
    ThemeSettingSpec {
        key: "accent_color",
        label: "Accent colour",
        kind: ThemeSettingKind::Colour {
            dark: DEFAULT_ACCENT,
        },
        default: ThemeSettingDefault::Text(DEFAULT_ACCENT),
    },
    ThemeSettingSpec {
        key: "background_color",
        label: "Page background",
        kind: ThemeSettingKind::Colour {
            dark: DARK_BACKGROUND,
        },
        default: ThemeSettingDefault::Text(DEFAULT_BACKGROUND),
    },
    ThemeSettingSpec {
        key: "surface_color",
        label: "Surface",
        kind: ThemeSettingKind::Colour { dark: DARK_SURFACE },
        default: ThemeSettingDefault::Text(DEFAULT_SURFACE),
    },
    ThemeSettingSpec {
        key: "text_color",
        label: "Text",
        kind: ThemeSettingKind::Colour { dark: DARK_TEXT },
        default: ThemeSettingDefault::Text(DEFAULT_TEXT),
    },
    ThemeSettingSpec {
        key: "muted_color",
        label: "Muted text",
        kind: ThemeSettingKind::Colour { dark: DARK_MUTED },
        default: ThemeSettingDefault::Text(DEFAULT_MUTED),
    },
    ThemeSettingSpec {
        key: "link_color",
        label: "Link",
        kind: ThemeSettingKind::Colour { dark: DEFAULT_LINK },
        default: ThemeSettingDefault::Text(DEFAULT_LINK),
    },
    ThemeSettingSpec {
        key: "heading_font",
        label: "Heading font",
        kind: ThemeSettingKind::Font,
        default: ThemeSettingDefault::Text("system-serif"),
    },
    ThemeSettingSpec {
        key: "body_font",
        label: "Body font",
        kind: ThemeSettingKind::Font,
        default: ThemeSettingDefault::Text("system-sans"),
    },
    ThemeSettingSpec {
        key: "base_font_size",
        label: "Base font size",
        kind: ThemeSettingKind::Number {
            min: MIN_BASE_SIZE,
            max: MAX_BASE_SIZE,
            step: 1,
        },
        default: ThemeSettingDefault::Number(16),
    },
    ThemeSettingSpec {
        key: "type_ratio",
        label: "Type scale ratio",
        kind: ThemeSettingKind::Number {
            min: MIN_TYPE_RATIO,
            max: MAX_TYPE_RATIO,
            step: 5,
        },
        default: ThemeSettingDefault::Number(125),
    },
    ThemeSettingSpec {
        key: "density",
        label: "Density",
        kind: ThemeSettingKind::Choice(&["compact", "comfortable", "spacious"]),
        default: ThemeSettingDefault::Text("comfortable"),
    },
    ThemeSettingSpec {
        key: "corner_radius",
        label: "Corner radius",
        kind: ThemeSettingKind::Number {
            min: 0,
            max: MAX_RADIUS,
            step: 1,
        },
        default: ThemeSettingDefault::Number(4),
    },
    ThemeSettingSpec {
        key: "header_layout",
        label: "Header layout",
        kind: ThemeSettingKind::Choice(&["logo-left", "centred"]),
        default: ThemeSettingDefault::Text("logo-left"),
    },
    ThemeSettingSpec {
        key: "logo",
        label: "Logo",
        kind: ThemeSettingKind::Asset,
        default: ThemeSettingDefault::Unset,
    },
    ThemeSettingSpec {
        key: "banner",
        label: "Banner image",
        kind: ThemeSettingKind::Asset,
        default: ThemeSettingDefault::Unset,
    },
    ThemeSettingSpec {
        key: "banner_text",
        label: "Banner text",
        kind: ThemeSettingKind::Text,
        default: ThemeSettingDefault::Unset,
    },
    ThemeSettingSpec {
        key: "footer_text",
        label: "Footer text",
        kind: ThemeSettingKind::Html,
        default: ThemeSettingDefault::Unset,
    },
];

/// The token schema as the admin reads it.
#[must_use]
pub fn registry() -> ThemeSettingsRegistry {
    ThemeSettingsRegistry {
        settings: SETTINGS.iter().map(entry).collect(),
    }
}

/// One spec as the admin reads it.
fn entry(spec: &ThemeSettingSpec) -> ThemeSettingEntry {
    ThemeSettingEntry {
        key: spec.key,
        label: spec.label,
        label_key: format!("themeSetting.{}", camel(spec.key)),
        schema: schema(spec),
    }
}

/// The control schema a spec describes.
fn schema(spec: &ThemeSettingSpec) -> ThemeSettingSchema {
    match (spec.kind, spec.default) {
        (ThemeSettingKind::Colour { dark }, ThemeSettingDefault::Text(default)) => {
            ThemeSettingSchema::Colour { default, dark }
        }
        (ThemeSettingKind::Colour { dark }, _) => ThemeSettingSchema::Colour {
            default: DEFAULT_BACKGROUND,
            dark,
        },
        (ThemeSettingKind::Font, default) => ThemeSettingSchema::Font {
            choices: &FONT_KEYS,
            default: text_default(default, FontStack::SystemSans.key()),
        },
        (ThemeSettingKind::Number { min, max, step }, ThemeSettingDefault::Number(default)) => {
            ThemeSettingSchema::Number {
                min,
                max,
                step,
                default,
            }
        }
        (ThemeSettingKind::Number { min, max, step }, _) => ThemeSettingSchema::Number {
            min,
            max,
            step,
            default: min,
        },
        (ThemeSettingKind::Choice(choices), default) => ThemeSettingSchema::Choice {
            choices,
            default: text_default(default, choices.first().copied().unwrap_or("")),
        },
        (ThemeSettingKind::Asset, default) => ThemeSettingSchema::Asset {
            default: match default {
                ThemeSettingDefault::Number(id) => Some(u64::from(id)),
                _ => None,
            },
        },
        (ThemeSettingKind::Text, default) => ThemeSettingSchema::Text {
            default: match default {
                ThemeSettingDefault::Text(text) => Some(text),
                _ => None,
            },
        },
        (ThemeSettingKind::Html, default) => ThemeSettingSchema::Html {
            default: match default {
                ThemeSettingDefault::Text(text) => Some(text),
                _ => None,
            },
        },
    }
}

/// A text default, with a fallback for a spec that names another kind.
fn text_default(default: ThemeSettingDefault, fallback: &'static str) -> &'static str {
    match default {
        ThemeSettingDefault::Text(text) => text,
        _ => fallback,
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

/// The whole theme settings registry, as `themeSettings.json` holds it.
#[derive(Debug, Serialize)]
pub struct ThemeSettingsRegistry {
    /// Every token, in schema order.
    pub settings: Vec<ThemeSettingEntry>,
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
    /// The control kind and its options, flattened beside the key.
    #[serde(flatten)]
    pub schema: ThemeSettingSchema,
}

/// The control a token renders as: a `kind` tag plus that kind's fields.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ThemeSettingSchema {
    /// A colour picker.
    Colour {
        /// The light default.
        default: &'static str,
        /// The dark scheme's default.
        dark: &'static str,
    },
    /// A font select over the curated keys.
    Font {
        /// The curated keys, in schema order.
        choices: &'static [&'static str],
        /// The default key.
        default: &'static str,
    },
    /// A bounded number input.
    Number {
        /// The smallest accepted value.
        min: u32,
        /// The largest accepted value.
        max: u32,
        /// The control's step.
        step: u32,
        /// The default value.
        default: u32,
    },
    /// A select over a fixed set.
    Choice {
        /// The accepted values, in schema order.
        choices: &'static [&'static str],
        /// The default value.
        default: &'static str,
    },
    /// An asset picker.
    Asset {
        /// The default asset id, `null` when unset.
        default: Option<u64>,
    },
    /// A single-line text input.
    Text {
        /// The default text, or `null`.
        default: Option<&'static str>,
    },
    /// A textarea holding HTML.
    Html {
        /// The default HTML, or `null`.
        default: Option<&'static str>,
    },
}
