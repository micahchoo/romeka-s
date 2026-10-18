use super::*;
use crate::theme::tokens_schema::camel;
use serde_json::{Value, json};

/// Where the admin reads the generated token schema.
const ADMIN_FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../web/admin/src/generated/themeSettings.json"
);

/// The admin's file is the registry's serialisation, byte for byte. A
/// difference fails with the command that rewrites it.
#[test]
fn generated_registry_matches_the_admin_file() {
    let rendered = format!(
        "{}\n",
        serde_json::to_string_pretty(&registry()).expect("the registry serialises")
    );
    if std::env::var_os("UPDATE_GENERATED").is_some() {
        let path = std::path::Path::new(ADMIN_FILE);
        std::fs::create_dir_all(path.parent().expect("the file has a parent"))
            .expect("the generated directory exists");
        std::fs::write(path, &rendered).expect("the registry file is writable");
        return;
    }
    let current = std::fs::read_to_string(ADMIN_FILE).unwrap_or_default();
    assert_eq!(
        current, rendered,
        "run UPDATE_GENERATED=1 cargo test -p omeka-site to rewrite {ADMIN_FILE}"
    );
}

/// A site with no stored map renders the documented defaults, and they look
/// untouched.
#[test]
fn defaults_are_the_schema_defaults() {
    let tokens = ThemeSettings::from_value(&Value::Null);
    assert_eq!(tokens, ThemeSettings::default());
    assert_eq!(tokens.color_scheme, ColorScheme::Light);
    assert_eq!(tokens.brand.as_str(), DEFAULT_BRAND);
    assert_eq!(tokens.background.as_str(), "#ffffff");
    assert_eq!(tokens.heading_font, FontStack::SystemSerif);
    assert_eq!(tokens.body_font, FontStack::SystemSans);
    assert_eq!(tokens.base_size, 16);
    assert_eq!(tokens.type_ratio, 125);
    assert_eq!(tokens.density, Density::Comfortable);
    assert_eq!(tokens.radius, 4);
    assert_eq!(tokens.header_layout, HeaderLayout::LogoLeft);
    assert_eq!(tokens.logo, None);
    // Every schema default agrees with the accessor.
    for spec in SETTINGS {
        match spec.default {
            ThemeSettingDefault::Text(text) => {
                let stored = match spec.key {
                    "color_scheme" => tokens.color_scheme.key(),
                    "brand_color" => tokens.brand.as_str(),
                    "accent_color" => tokens.accent.as_str(),
                    "background_color" => tokens.background.as_str(),
                    "surface_color" => tokens.surface.as_str(),
                    "text_color" => tokens.text.as_str(),
                    "muted_color" => tokens.muted.as_str(),
                    "link_color" => tokens.link.as_str(),
                    "heading_font" => tokens.heading_font.key(),
                    "body_font" => tokens.body_font.key(),
                    "density" => tokens.density.key(),
                    "header_layout" => tokens.header_layout.key(),
                    other => panic!("{other} has no text accessor"),
                };
                assert_eq!(stored, text, "{}", spec.key);
            }
            ThemeSettingDefault::Number(number) => {
                let stored = match spec.key {
                    "base_font_size" => tokens.base_size,
                    "type_ratio" => tokens.type_ratio,
                    "corner_radius" => tokens.radius,
                    other => panic!("{other} has no number accessor"),
                };
                assert_eq!(stored, number, "{}", spec.key);
            }
            ThemeSettingDefault::Unset => {
                assert!(
                    matches!(spec.key, "logo" | "banner" | "banner_text" | "footer_text"),
                    "{}",
                    spec.key
                );
            }
        }
    }
}

/// A stored map overrides the defaults it names.
#[test]
fn stored_values_override_the_defaults() {
    let tokens = ThemeSettings::from_value(&json!({
        "color_scheme": "dark",
        "brand_color": "#FF6600",
        "background_color": "#000",
        "heading_font": "geometric",
        "body_font": "mono",
        "base_font_size": 20,
        "type_ratio": 140,
        "density": "spacious",
        "corner_radius": 12,
        "header_layout": "centred",
        "logo": 7,
        "banner": "9",
        "banner_text": "Opening hours",
        "footer_text": "<p>Friends of the library</p>",
    }));
    assert_eq!(tokens.brand.as_str(), "#ff6600");
    assert_eq!(tokens.background.as_str(), "#000000");
    assert_eq!(tokens.heading_font, FontStack::Geometric);
    assert_eq!(tokens.body_font, FontStack::Mono);
    assert_eq!(tokens.base_size, 20);
    assert_eq!(tokens.type_ratio, 140);
    assert_eq!(tokens.density, Density::Spacious);
    assert_eq!(tokens.radius, 12);
    assert_eq!(tokens.header_layout, HeaderLayout::Centred);
    assert_eq!(tokens.logo, Some(7));
    assert_eq!(tokens.banner, Some(9));
    assert_eq!(tokens.banner_text.as_deref(), Some("Opening hours"));
    assert_eq!(
        tokens.footer_text.as_deref(),
        Some("<p>Friends of the library</p>")
    );
    // With the dark scheme, a neutral the map does not name takes the dark
    // default rather than the light one.
    assert_eq!(tokens.surface.as_str(), DARK_SURFACE);
    assert_eq!(tokens.text.as_str(), DARK_TEXT);
    assert_eq!(tokens.muted.as_str(), DARK_MUTED);
    assert_eq!(tokens.link.as_str(), DARK_LINK);
    assert_eq!(tokens.accent.as_str(), DARK_ACCENT);
}

/// The type scale clamps: a large ratio grows the page's `h1` to a bound rather
/// than without end, so one rail does not dominate the page.
#[test]
fn heading_sizes_are_clamped() {
    let large = ThemeSettings::from_value(&json!({"type_ratio": 160})).css();
    assert!(large.contains("--font-size-h1: 3.2em;"), "{large}");
    assert!(large.contains("--font-size-h2: 2.4em;"), "{large}");
    assert!(large.contains("--font-size-h3: 1.8em;"), "{large}");
    assert!(large.contains("--font-size-h4: 1.4em;"), "{large}");

    let small = ThemeSettings::from_value(&json!({"type_ratio": 110})).css();
    assert!(small.contains("--font-size-h1: 1.6em;"), "{small}");
    assert!(small.contains("--font-size-h2: 1.35em;"), "{small}");

    // The default ratio sits inside the bounds, so the clamp changes nothing.
    let default = ThemeSettings::from_value(&Value::Null).css();
    assert!(default.contains("--font-size-h1: 2.4414em;"), "{default}");
}

/// A token outside the accepted set takes its default, so no stored string
/// reaches the stylesheet as CSS.
#[test]
fn invalid_values_take_their_defaults() {
    let tokens = ThemeSettings::from_value(&json!({
        "brand_color": "red; background: url(https://example.org/x)",
        "background_color": "#12345",
        "heading_font": "Comic Sans MS",
        "body_font": "",
        "base_font_size": 400,
        "type_ratio": 20,
        "corner_radius": 900,
        "density": "cozy",
        "header_layout": "middle",
        "color_scheme": "sepia",
        "logo": "not-an-id",
    }));
    assert_eq!(tokens, ThemeSettings::default());
}

/// The emitted properties carry every token, and no stored value can escape
/// as CSS.
#[test]
fn css_emits_the_tokens_as_custom_properties() {
    let tokens = ThemeSettings::from_value(&json!({
        "brand_color": "#123456",
        "heading_font": "humanist",
        "body_font": "transitional-serif",
        "base_font_size": 18,
        "type_ratio": 150,
        "density": "compact",
        "corner_radius": 6,
    }));
    let css = tokens.css();
    for needle in [
        "--color-brand: #123456;",
        "--color-background: #ffffff;",
        "--color-link: #1a5d8f;",
        "--font-heading: \"Gill Sans\", \"Trebuchet MS\", Calibri, sans-serif;",
        "--font-size-base: 18px;",
        "--font-size-h1: 3.2em;",
        "--line-height: 1.4;",
        "--space-1: 0.125rem;",
        "--radius: 6px;",
        "--radius-sm: 4px;",
        "color-scheme: light;",
    ] {
        assert!(css.contains(needle), "{needle} missing from {css}");
    }
    // A hostile value never reaches the sheet; the default stands.
    let hostile = ThemeSettings::from_value(&json!({"brand_color": "}</style><script>"}));
    assert!(hostile.css().contains("--color-brand: #6aa9d6;"));
    assert!(!hostile.css().contains("<script>"));
}

/// The auto scheme emits a dark media query for the neutral colours, and
/// the explicit schemes do not.
#[test]
fn auto_scheme_emits_the_dark_media_query() {
    let auto = ThemeSettings::from_value(&json!({"color_scheme": "auto"}));
    let css = auto.css();
    assert!(css.contains("color-scheme: light dark;"), "{css}");
    assert!(css.contains("@media (prefers-color-scheme: dark)"), "{css}");
    assert!(css.contains("--color-background: #12161a;"), "{css}");
    assert!(
        !ThemeSettings::from_value(&json!({"color_scheme": "light"}))
            .css()
            .contains("@media")
    );
    assert!(
        !ThemeSettings::from_value(&json!({"color_scheme": "dark"}))
            .css()
            .contains("@media")
    );
}

/// Every schema key is read by the boundary parser, and every parser key is
/// declared. The labels are non-empty and the catalogue keys are camel case.
#[test]
fn the_schema_and_the_parser_name_the_same_keys() {
    let tokens = ThemeSettings::from_value(&json!({
        "color_scheme": "dark",
        "brand_color": "#111111",
        "accent_color": "#222222",
        "background_color": "#333333",
        "surface_color": "#444444",
        "text_color": "#555555",
        "muted_color": "#666666",
        "link_color": "#777777",
        "heading_font": "mono",
        "body_font": "geometric",
        "base_font_size": 14,
        "type_ratio": 110,
        "density": "compact",
        "corner_radius": 0,
        "header_layout": "centred",
        "logo": 1,
        "banner": 2,
        "banner_text": "x",
        "footer_text": "y",
    }));
    assert_ne!(tokens, ThemeSettings::default());
    assert_eq!(tokens.link.as_str(), "#777777");
    assert_eq!(tokens.radius, 0);
    assert_eq!(tokens.logo, Some(1));
    assert_eq!(tokens.banner, Some(2));
    assert_eq!(tokens.banner_text.as_deref(), Some("x"));
    assert_eq!(tokens.footer_text.as_deref(), Some("y"));
    let registry = registry();
    assert_eq!(registry.settings.len(), SETTINGS.len());
    for (entry, spec) in registry.settings.iter().zip(SETTINGS) {
        assert_eq!(entry.key, spec.key);
        assert!(!entry.label.is_empty());
        assert_eq!(entry.label_key, format!("themeSetting.{}", camel(spec.key)));
    }
    assert_eq!(
        registry.settings[0].label_key,
        "themeSetting.colorScheme".to_string()
    );
    let keys: std::collections::BTreeSet<&str> = SETTINGS.iter().map(|s| s.key).collect();
    assert_eq!(keys.len(), SETTINGS.len(), "a key appears twice");
}

/// The curated list is closed: each key parses back to itself and a key
/// outside the list does not.
#[test]
fn the_font_list_is_curated() {
    for font in FontStack::ALL {
        assert_eq!(FontStack::parse(font.key()), Some(font));
        assert!(!font.stack().is_empty());
    }
    assert_eq!(FontStack::parse("Papyrus"), None);
}
