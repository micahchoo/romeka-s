//! The machine-translated catalogues: Hindi, Marathi, Malayalam, Tamil, Kannada,
//! Telugu and Nepali.
//!
//! Each lives in `locales/<tag>.json`, a flat object from a message key to its
//! text, which `web/admin/scripts/translate-locales.mjs` fills with sarvam-30b
//! and md-translator's checks, as it fills the admin's catalogues. The English
//! it translates from is `locales/en.json`, the serialisation of [`EN`] and
//! [`EN_SEARCH`] that a test keeps current. A key is a [`Messages`] field name,
//! `search.<field>` for a [`SearchLabels`] field, and
//! `search.comparisons.<value>` or `search.sort_fields.<value>` for an option.
//!
//! The files are compiled in with `include_str!` and parsed once, on first use.
//! A file that lacks a key, or names one the site does not render, fails the
//! parse, and `every_shipped_locale_parses` fails with it, so a shipped locale is
//! complete. At run time a file that fails to parse renders English.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use super::{EN, EN_SEARCH, Messages, SearchLabels};

/// The key of the note a file carries for its reader; it is not a message.
const NOTE: &str = "//";

/// One locale's parsed catalogue.
pub(super) struct Catalogue {
    pub(super) messages: Messages,
    pub(super) search: SearchLabels,
}

/// Why a locale file is not a catalogue.
#[derive(Debug, thiserror::Error)]
pub(super) enum CatalogueError {
    /// The file is not a JSON object of strings.
    #[error("not a JSON object of strings: {0}")]
    Json(#[from] serde_json::Error),
    /// A key the site renders has no text.
    #[error("no text for `{0}`")]
    Missing(String),
    /// A key names no message the site renders.
    #[error("`{0}` is not a message the site renders")]
    Unknown(String),
}

/// Both directions of the flat key list, from one field list: the struct
/// literals in `parse` make a field missing from the list a compile error.
macro_rules! flat_catalogue {
    (messages: [$($m:ident),* $(,)?], search: [$($s:ident),* $(,)?] $(,)?) => {
        /// English as the flat entries `locales/en.json` holds, in field order.
        #[cfg(test)]
        fn english_entries() -> Vec<(String, &'static str)> {
            let mut out = vec![$((stringify!($m).to_string(), EN.$m),)*];
            $(out.push((format!("search.{}", stringify!($s)), EN_SEARCH.$s));)*
            for (value, label) in EN_SEARCH.comparisons {
                out.push((format!("search.comparisons.{value}"), label));
            }
            for (value, label) in EN_SEARCH.sort_fields {
                out.push((format!("search.sort_fields.{value}"), label));
            }
            out
        }

        /// A locale file as a catalogue. Every key must be present, and no other.
        pub(super) fn parse(text: &str) -> Result<Catalogue, CatalogueError> {
            let mut map: BTreeMap<String, String> = serde_json::from_str(text)?;
            map.remove(NOTE);
            let messages = Messages {
                $($m: take(&mut map, stringify!($m))?,)*
            };
            // The option values are wire names, so only the labels translate.
            let mut comparisons = EN_SEARCH.comparisons;
            for (value, label) in &mut comparisons {
                *label = take(&mut map, &format!("search.comparisons.{value}"))?;
            }
            let mut sort_fields = EN_SEARCH.sort_fields;
            for (value, label) in &mut sort_fields {
                *label = take(&mut map, &format!("search.sort_fields.{value}"))?;
            }
            let search = SearchLabels {
                $($s: take(&mut map, &format!("search.{}", stringify!($s)))?,)*
                comparisons,
                sort_fields,
            };
            match map.into_keys().next() {
                Some(key) => Err(CatalogueError::Unknown(key)),
                None => Ok(Catalogue { messages, search }),
            }
        }
    };
}

flat_catalogue! {
    messages: [
        powered_by, no_pages, prev, next, advanced_search, item, item_set, media, items,
        item_sets, no_item_selected, part_of, created, modified, user_bar_label, signed_in_as,
        admin_dashboard, edit_page, edit_item, edit_item_set, edit_media, resource_class,
        log_out, log_in, skip_to_main, menu, home, browse, browse_item_sets, custom_url,
        missing_page, untitled, site_pages, title, class, linked_caption, page_not_found,
        page_not_found_message, forbidden, pager_count, no_title_recorded, no_media_attached,
        previous, file_of, files_count,
        model_turn, model_no_javascript, model_loading, model_failed,
        kind_image, kind_audio, kind_video, kind_pdf, kind_model_3d, kind_captions,
        kind_embed, kind_other_file, no_file,
        footer_links, all_rights_reserved, search_placeholder, iiif_poster, iiif_view_in_viewer,
        iiif_manifest, no_site_items_preview, no_site_items,
        map, map_places, map_place_count, map_poster, map_no_places_preview,
        gallery, gallery_close, gallery_previous, gallery_next, gallery_position,
        gallery_empty_preview, featured_view_item, featured_empty_preview,
        button_empty_preview, quote_empty_preview, accordion_empty_preview,
        timeline_undated, timeline_empty_preview,
        no_matches, no_matches_preview,
        notes_one, notes_count, notes_region, notes_whole_image, notes_whole_recording,
        notes_whole_file, notes_show, notes_facts, notes_facts_time, notes_next, notes_all,
        notes_link, notes_range, notes_play, notes_missing,
    ],
    search: [
        search, query, submit, sort, select_sort_by, select_sort_order, ascending, descending,
        search_full_text, search_by_value, query_text, property, query_type, joiner, and, or,
        any_property, search_by_class, search_by_item_set, in_set, not_in_set,
        search_by_media_presence, select_media_presence, has_media, has_no_media, search_by_id,
        search_results, no_result_found, view_all_results, item_results, item_set_results,
        site_page_results, advanced_item_search, advanced_item_set_search,
        advanced_item_set_form_title, pages, no_pages_found, site_pages, items, item_sets,
    ],
}

/// Remove one key's text. A catalogue is parsed once per process and lives as
/// long as it, so its strings are leaked into `'static` rather than copied
/// into every render.
fn take(map: &mut BTreeMap<String, String>, key: &str) -> Result<&'static str, CatalogueError> {
    map.remove(key)
        .map(|text| &*Box::leak(text.into_boxed_str()))
        .ok_or_else(|| CatalogueError::Missing(key.to_string()))
}

fn load(text: &str) -> Catalogue {
    parse(text).unwrap_or(Catalogue {
        messages: EN,
        search: EN_SEARCH,
    })
}

pub(super) static HI: LazyLock<Catalogue> = LazyLock::new(|| load(include_str!("locales/hi.json")));
pub(super) static MR: LazyLock<Catalogue> = LazyLock::new(|| load(include_str!("locales/mr.json")));
pub(super) static ML: LazyLock<Catalogue> = LazyLock::new(|| load(include_str!("locales/ml.json")));
pub(super) static TA: LazyLock<Catalogue> = LazyLock::new(|| load(include_str!("locales/ta.json")));
pub(super) static KN: LazyLock<Catalogue> = LazyLock::new(|| load(include_str!("locales/kn.json")));
pub(super) static TE: LazyLock<Catalogue> = LazyLock::new(|| load(include_str!("locales/te.json")));
pub(super) static NE: LazyLock<Catalogue> = LazyLock::new(|| load(include_str!("locales/ne.json")));

/// The shipped files, by tag, for the completeness test.
#[cfg(test)]
const SHIPPED: [(&str, &str); 7] = [
    ("hi", include_str!("locales/hi.json")),
    ("mr", include_str!("locales/mr.json")),
    ("ml", include_str!("locales/ml.json")),
    ("ta", include_str!("locales/ta.json")),
    ("kn", include_str!("locales/kn.json")),
    ("te", include_str!("locales/te.json")),
    ("ne", include_str!("locales/ne.json")),
];

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    const ENGLISH_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/i18n/locales/en.json");

    /// `locales/en.json` as the English statics serialise, with a note first.
    fn english_file() -> String {
        let note = "Generated from EN and EN_SEARCH by `UPDATE_GENERATED=1 cargo test -p \
                    omeka-site`. translate-locales.mjs translates from it.";
        let mut lines = vec![format!("  {}: {}", json(NOTE), json(note))];
        lines.extend(
            english_entries()
                .into_iter()
                .map(|(key, text)| format!("  {}: {}", json(&key), json(text))),
        );
        format!("{{\n{}\n}}\n", lines.join(",\n"))
    }

    fn json(text: &str) -> String {
        serde_json::to_string(text).unwrap()
    }

    /// The translation script's English is the statics', byte for byte. A
    /// difference fails with the command that rewrites it.
    #[test]
    fn english_file_matches_the_statics() {
        let rendered = english_file();
        if std::env::var_os("UPDATE_GENERATED").is_some() {
            std::fs::write(ENGLISH_FILE, &rendered).unwrap();
            return;
        }
        let current = std::fs::read_to_string(ENGLISH_FILE).unwrap_or_default();
        assert_eq!(
            current, rendered,
            "run UPDATE_GENERATED=1 cargo test -p omeka-site to rewrite {ENGLISH_FILE}"
        );
    }

    /// The English file parses back to the English statics, so the key list
    /// covers every field in both directions.
    #[test]
    fn english_file_parses_to_the_statics() {
        let catalogue = parse(&english_file()).unwrap();
        assert_eq!(catalogue.messages, EN);
        assert_eq!(catalogue.search, EN_SEARCH);
    }

    /// Every shipped locale names every message and no other, and none renders
    /// the English it was translated from in the footer.
    #[test]
    fn every_shipped_locale_parses() {
        for (tag, text) in SHIPPED {
            let catalogue = parse(text).unwrap_or_else(|error| panic!("{tag}.json: {error}"));
            assert_ne!(catalogue.messages.powered_by, EN.powered_by, "{tag}");
            assert!(catalogue.messages.powered_by.contains("Romeka S"), "{tag}");
        }
    }

    /// A file that drops a key, or adds one, is not a catalogue.
    #[test]
    fn a_missing_or_unknown_key_fails_the_parse() {
        let english = english_file();
        let missing = english.replace("  \"menu\": \"Menu\",\n", "");
        assert!(matches!(
            parse(&missing),
            Err(CatalogueError::Missing(key)) if key == "menu"
        ));
        let unknown = english.replace("\"menu\":", "\"menu\": \"Menu\",\n  \"menus\":");
        assert!(matches!(
            parse(&unknown),
            Err(CatalogueError::Unknown(key)) if key == "menus"
        ));
        let missing_option = english.replace("  \"search.comparisons.eq\": \"is exactly\",\n", "");
        assert!(matches!(
            parse(&missing_option),
            Err(CatalogueError::Missing(key)) if key == "search.comparisons.eq"
        ));
    }
}
