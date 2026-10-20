use super::*;
use crate::theme::choices::{CardStyle, HeroStyle};
use crate::theme::color::{AA, contrast};
use crate::theme::palette::{self, PAIRS, is_accessible};
use crate::theme::presets::PRESETS;
use crate::theme::tokens_samples::{SAMPLE_BRANDS, samples};
use crate::theme::tokens_schema::camel;
use serde_json::{Value, json};

/// Where the admin reads the generated token schema.
const ADMIN_FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../web/admin/src/generated/themeSettings.json"
);

/// Where the admin's tests read the palette samples.
const SAMPLES_FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../web/admin/src/generated/themeSamples.json"
);

/// Compare a generated file with its serialisation, or rewrite it under
/// `UPDATE_GENERATED`.
fn check_generated(path: &str, value: &impl serde::Serialize) {
    let rendered = format!(
        "{}\n",
        serde_json::to_string_pretty(value).expect("the value serialises")
    );
    if std::env::var_os("UPDATE_GENERATED").is_some() {
        let file = std::path::Path::new(path);
        std::fs::create_dir_all(file.parent().expect("the file has a parent"))
            .expect("the generated directory exists");
        std::fs::write(file, &rendered).expect("the generated file is writable");
        return;
    }
    let current = std::fs::read_to_string(path).unwrap_or_default();
    assert_eq!(
        current, rendered,
        "run UPDATE_GENERATED=1 cargo test -p omeka-site to rewrite {path}"
    );
}

/// The admin's files are the registry's and the samples' serialisations, byte
/// for byte. A difference fails with the command that rewrites them.
#[test]
fn generated_files_match_the_admin_copies() {
    check_generated(ADMIN_FILE, &registry());
    check_generated(SAMPLES_FILE, &samples());
}

fn hex(raw: &str) -> HexColor {
    HexColor::parse(raw).expect("a hex colour")
}

fn tokens(value: &Value) -> Tokens {
    ThemeSettings::from_value(value).tokens
}

/// A site with no stored map, or a map stored before presets, renders the
/// fixed palette of the preset that replaced plain.
#[test]
fn a_map_from_before_presets_renders_the_catalogue_look() {
    let empty = tokens(&Value::Null);
    assert_eq!(empty.preset, PresetId::Catalogue);
    assert_eq!(empty.light, PresetId::Catalogue.preset().light);
    let css = empty.css();
    for needle in [
        "color-scheme: light;",
        "--color-brand: #1d2a33;",
        "--color-background: #fafaf9;",
        "--color-text: #1b2227;",
        "--color-link: #0b5a63;",
        "--color-on-brand: #fafaf9;",
        "--content-width: 76rem;",
        "--font-size-h1: 2.0736em;",
    ] {
        assert!(css.contains(needle), "{needle} missing from {css}");
    }
    let legacy = tokens(&json!({"brand_color": "#abcdef", "color_scheme": "dark"}));
    assert_eq!(legacy.dark.background, hex("#12171a"));
    assert_eq!(legacy.dark.brand, hex("#abcdef"));
}

/// The ids the first five presets stored read as the presets that replaced
/// them, and what a site set by hand still beats the new preset.
#[test]
fn legacy_preset_ids_read_as_their_replacements() {
    for (stored, preset) in [
        ("plain", PresetId::Catalogue),
        ("archive", PresetId::Exhibit),
        ("gallery", PresetId::Gallery),
        ("editorial", PresetId::Journal),
        ("civic", PresetId::Community),
    ] {
        assert_eq!(
            tokens(&json!({"preset": stored})).preset,
            preset,
            "{stored}"
        );
    }
    let kept = tokens(&json!({
        "preset": "archive",
        "card_style": "row",
        "base_font_size": 19,
    }));
    assert_eq!(kept.preset, PresetId::Exhibit);
    assert_eq!(kept.structure.cards, CardStyle::Row);
    assert_eq!(kept.structure.hero, HeroStyle::Cover);
    assert_eq!(kept.base_size, 19);
}

/// Each structural token is a class on the root element; a hand-set value
/// replaces the preset's.
#[test]
fn structural_tokens_become_root_classes() {
    assert_eq!(
        tokens(&json!({"preset": "exhibit"})).classes(),
        "hero-cover cards-caption search-icon footer-columns item-stacked"
    );
    assert_eq!(
        tokens(&json!({"preset": "gallery", "item_layout": "side"})).classes(),
        "hero-framed cards-overlay search-icon footer-split item-side"
    );
    let inverse = tokens(&json!({"preset": "catalogue", "footer_tone": "inverse"})).css();
    assert!(
        inverse.contains("--footer-background: var(--color-text);"),
        "{inverse}"
    );
}

/// A stored preset derives its palette from the brand unless derivation is
/// switched off.
#[test]
fn a_preset_derives_its_palette_by_default() {
    let exhibit = PresetId::Exhibit.preset();
    let derived = tokens(&json!({"preset": "exhibit"}));
    assert_eq!(
        derived.light,
        palette::derive(exhibit.light.brand, exhibit.recipe, Shade::Light)
    );
    assert_eq!(derived.heading_font, exhibit.heading_font);
    let fixed = tokens(&json!({"preset": "exhibit", "derive_palette": false}));
    assert_eq!(fixed.light, exhibit.light);
    assert_eq!(fixed.dark, exhibit.dark);
}

/// Preset, then derived palette, then overrides: a hand-set brand drives the
/// derivation and a hand-set colour beats the derived one.
#[test]
fn overrides_beat_the_derived_palette_and_the_brand_drives_it() {
    let civic = PresetId::Community.preset();
    let resolved = tokens(&json!({
        "preset": "community",
        "brand_color": "#ffcc00",
        "link_color": "#003366",
        "base_font_size": 20,
    }));
    let derived = palette::derive(hex("#ffcc00"), civic.recipe, Shade::Light);
    assert_eq!(resolved.light.brand, hex("#ffcc00"));
    assert_eq!(resolved.light.background, derived.background);
    assert_eq!(resolved.light.link, hex("#003366"));
    assert_eq!(resolved.base_size, 20);
    assert_eq!(resolved.header_style, civic.header_style);
    // Derivation off: the preset's own palette, with the brand swapped in.
    let fixed = tokens(&json!({
        "preset": "community",
        "derive_palette": false,
        "brand_color": "#ffcc00",
    }));
    assert_eq!(fixed.light.background, civic.light.background);
    assert_eq!(fixed.light.brand, hex("#ffcc00"));
    assert_eq!(fixed.dark.brand, hex("#ffcc00"));
}

/// A colour override lands on the palette the page shows first.
#[test]
fn colour_overrides_follow_the_scheme() {
    let dark = tokens(&json!({
        "preset": "journal",
        "color_scheme": "dark",
        "muted_color": "#999999",
    }));
    assert_eq!(dark.dark.muted, hex("#999999"));
    assert_ne!(dark.light.muted, hex("#999999"));
    let auto = tokens(&json!({
        "preset": "journal",
        "color_scheme": "auto",
        "muted_color": "#444444",
    }));
    assert_eq!(auto.light.muted, hex("#444444"));
    assert_ne!(auto.dark.muted, hex("#444444"));
    assert_eq!(auto.shown().len(), 2);
}

/// Under the automatic scheme a colour has two values: the plain key is the
/// light one, `<key>_dark` the dark one. Under a fixed scheme the page shows
/// one palette, so the plain key is the only one read.
#[test]
fn a_dark_override_reaches_the_dark_palette_under_auto_only() {
    let auto = tokens(&json!({
        "preset": "editorial",
        "color_scheme": "auto",
        "muted_color": "#444444",
        "muted_color_dark": "#bbbbbb",
        "link_color_dark": "#99ccff",
    }));
    assert_eq!(auto.light.muted, hex("#444444"));
    assert_eq!(auto.dark.muted, hex("#bbbbbb"));
    assert_eq!(auto.dark.link, hex("#99ccff"));
    assert_ne!(auto.light.link, hex("#99ccff"));
    let media = auto
        .css()
        .split("@media (prefers-color-scheme: dark)")
        .nth(1)
        .map(str::to_string)
        .expect("a dark media query");
    assert!(media.contains("--color-muted: #bbbbbb;"), "{media}");

    let preset = tokens(&json!({"preset": "editorial", "color_scheme": "auto"}));
    for scheme in ["light", "dark"] {
        let fixed = tokens(&json!({
            "preset": "editorial",
            "color_scheme": scheme,
            "muted_color_dark": "#bbbbbb",
        }));
        assert_eq!(fixed.dark.muted, preset.dark.muted, "{scheme}");
        assert_eq!(fixed.light.muted, preset.light.muted, "{scheme}");
    }
    let refused = tokens(&json!({
        "preset": "editorial",
        "color_scheme": "auto",
        "muted_color_dark": "grey",
        "brand_color_dark": "#000000",
    }));
    assert_eq!(refused.dark, preset.dark);
}

/// The brief's spread: the theme's brands, near white, near black, pure
/// yellow and a ring of hues. Every preset's recipe derives a palette that
/// passes every pair in both shades.
#[test]
fn derived_palettes_pass_aa_for_any_brand() {
    for raw in SAMPLE_BRANDS {
        for preset in &PRESETS {
            for shade in [Shade::Light, Shade::Dark] {
                let derived = palette::derive(hex(raw), preset.recipe, shade);
                assert!(
                    derived.failures().is_empty(),
                    "{raw} in {:?} {shade:?}: {:?} from {derived:?}",
                    preset.id,
                    derived.failures()
                );
            }
        }
    }
}

/// Every preset's own palettes pass every pair, so switching derivation off
/// never starts a curator from a failing palette.
#[test]
fn every_preset_palette_passes_aa() {
    for preset in &PRESETS {
        for (shade, palette) in [("light", &preset.light), ("dark", &preset.dark)] {
            assert!(
                is_accessible(palette),
                "{:?} {shade}: {:?}",
                preset.id,
                palette.failures()
            );
        }
    }
}

/// Text on the brand colour always reaches AA, even on a mid-tone brand where
/// neither the text nor the background colour would.
#[test]
fn text_on_brand_always_passes() {
    let mut palette = PresetId::Catalogue.preset().light;
    for raw in [
        "#777777", "#ffff00", "#0000ff", "#6aa9d6", "#000000", "#ffffff",
    ] {
        palette.brand = hex(raw);
        let on = palette.get(Role::OnBrand);
        assert!(contrast(on, palette.brand) >= AA, "{raw}: {on:?}");
    }
}

/// The new tokens reach the sheet: content width, header and button styles.
#[test]
fn css_emits_the_layout_header_and_button_tokens() {
    let css = tokens(&json!({
        "preset": "community",
        "content_width": "narrow",
        "button_style": "outline",
        "heading_font": "didone",
    }))
    .css();
    for needle in [
        "--content-width: 44rem;",
        "--header-background: var(--color-brand);",
        "--header-link: var(--color-on-brand);",
        "--button-background: transparent;",
        "--button-text: var(--color-link);",
        "--font-heading: Didot,",
        "--radius: 8px;",
    ] {
        assert!(css.contains(needle), "{needle} missing from {css}");
    }
}

/// Heading sizes follow the ratio but stay inside each level's bounds.
#[test]
fn heading_sizes_are_clamped() {
    let large = tokens(&json!({"type_ratio": 160})).css();
    assert!(large.contains("--font-size-h1: 3.2em;"), "{large}");
    assert!(large.contains("--font-size-h4: 1.4em;"), "{large}");
    let small = tokens(&json!({"type_ratio": 110})).css();
    assert!(small.contains("--font-size-h1: 1.6em;"), "{small}");
    assert!(small.contains("--font-size-h2: 1.35em;"), "{small}");
}

/// A value outside the accepted set is left to the preset, so no stored
/// string reaches the stylesheet as CSS.
#[test]
fn invalid_values_are_left_to_the_preset() {
    let settings = ThemeSettings::from_value(&json!({
        "preset": "renaissance",
        "brand_color": "red; background: url(https://example.org/x)",
        "background_color": "#12345",
        "heading_font": "Comic Sans MS",
        "base_font_size": 400,
        "type_ratio": 20,
        "corner_radius": 900,
        "density": "cozy",
        "header_style": "}</style>",
        "content_width": "huge",
        "color_scheme": "sepia",
        "logo": "not-an-id",
    }));
    assert_eq!(settings, ThemeSettings::default());
    assert!(!settings.css().contains("</style>"));
}

/// The automatic scheme emits the whole dark palette in a media query, and the
/// explicit schemes do not.
#[test]
fn auto_scheme_emits_the_dark_media_query() {
    let css = tokens(&json!({"color_scheme": "auto"})).css();
    assert!(css.contains("color-scheme: light dark;"), "{css}");
    let media = css
        .split("@media (prefers-color-scheme: dark)")
        .nth(1)
        .expect("a dark media query");
    for role in Role::ALL {
        assert!(
            media.contains(&format!("--color-{}:", role.key())),
            "{media}"
        );
    }
    assert!(media.contains("--color-background: #12171a;"), "{media}");
    for scheme in ["light", "dark"] {
        assert!(
            !tokens(&json!({"color_scheme": scheme}))
                .css()
                .contains("@media")
        );
    }
}

/// Custom CSS that passes the sanitiser is kept; CSS it refuses is dropped
/// whole on the way out, whoever stored it.
#[test]
fn stored_custom_css_is_sanitised_on_read() {
    let kept = ThemeSettings::from_value(&json!({"custom_css": "a { color: red }"}));
    assert_eq!(
        kept.custom_css.as_ref().map(CustomCss::as_str),
        Some("a { color: red }")
    );
    let dropped = ThemeSettings::from_value(&json!({
        "custom_css": "a { color: red } </style><script>alert(1)</script>"
    }));
    assert_eq!(dropped.custom_css, None);
}

/// A new site starts from Exhibit with the palette it was drawn with.
#[test]
fn a_new_site_starts_from_the_new_site_preset() {
    let resolved = tokens(&new_site_settings());
    assert_eq!(resolved.preset, PresetId::Exhibit);
    assert_eq!(resolved.light, PresetId::Exhibit.preset().light);
}

/// Every schema key changes what the theme renders, so the schema names no
/// key the parser ignores. The registry lists the keys once each, with
/// camel-case catalogue keys.
#[test]
fn every_schema_key_reaches_the_render() {
    let samples: [(&str, Value); 35] = [
        ("preset", json!("gallery")),
        ("derive_palette", json!(true)),
        ("brand_color", json!("#123456")),
        ("color_scheme", json!("dark")),
        ("accent_color", json!("#123456")),
        ("background_color", json!("#123456")),
        ("surface_color", json!("#123456")),
        ("text_color", json!("#123456")),
        ("muted_color", json!("#123456")),
        ("link_color", json!("#123456")),
        ("heading_font", json!("mono")),
        ("body_font", json!("mono")),
        ("base_font_size", json!(20)),
        ("type_ratio", json!(150)),
        ("density", json!("spacious")),
        ("corner_radius", json!(12)),
        ("content_width", json!("full")),
        ("header_layout", json!("centred")),
        ("header_style", json!("rule")),
        ("button_style", json!("outline")),
        ("hero_style", json!("framed")),
        ("card_style", json!("caption")),
        ("search_display", json!("icon")),
        ("item_layout", json!("stacked")),
        ("footer_layout", json!("split")),
        ("footer_tone", json!("inverse")),
        ("logo", json!(1)),
        ("banner", json!(2)),
        ("banner_text", json!("x")),
        ("footer_columns", json!(["<p>y</p>"])),
        (
            "footer_links",
            json!([{"type": "url", "data": {"label": "A", "url": "/a"}}]),
        ),
        ("footer_contact", json!("z")),
        ("footer_licence", json!("cc-by")),
        ("show_powered_by", json!(false)),
        ("custom_css", json!("a { color: red }")),
    ];
    // Catalogue with derivation off is the baseline, so switching derivation
    // on is a visible change too.
    let base = json!({"preset": "catalogue", "derive_palette": false});
    let baseline = ThemeSettings::from_value(&base);
    for (key, value) in &samples {
        let mut stored = base.clone();
        stored[*key] = value.clone();
        assert_ne!(
            ThemeSettings::from_value(&stored),
            baseline,
            "{key} is ignored"
        );
    }
    let names: Vec<&str> = samples.iter().map(|(key, _)| *key).collect();
    let declared: Vec<&str> = SETTINGS.iter().map(|spec| spec.key).collect();
    assert_eq!(names, declared);
    let registry = registry();
    for (entry, spec) in registry.settings.iter().zip(SETTINGS) {
        assert_eq!(entry.key, spec.key);
        assert!(!entry.label.is_empty());
        assert_eq!(entry.label_key, format!("themeSetting.{}", camel(spec.key)));
    }
    assert_eq!(registry.pairs.len(), PAIRS.len());
}

/// The curated font list is closed: each key parses back to itself and a key
/// outside the list does not.
#[test]
fn the_font_list_is_curated() {
    for font in FontStack::ALL {
        assert_eq!(FontStack::parse(font.key()), Some(*font));
        assert!(!font.stack().is_empty());
    }
    assert_eq!(FontStack::parse("Papyrus"), None);
}
