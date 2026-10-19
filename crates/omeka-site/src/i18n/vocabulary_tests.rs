#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::*;
use crate::i18n::Locale;

/// Where the admin keeps the vocabulary tables, one directory per kind.
const ADMIN_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../web/admin/src/i18n/locales/vocabulary"
);

/// The site's copy of the label tables.
const SITE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/data/vocabulary-labels");

/// The install seed: the English every fresh install stores.
const SEED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../omeka-cli/data/default-vocabularies.json"
);

/// Every `<tag>.json` table of one kind, by tag. `sources.json` is the
/// translation script's record, not a table.
fn admin_tables(kind: &str) -> Vec<(String, PathBuf)> {
    let mut tables: Vec<(String, PathBuf)> = std::fs::read_dir(Path::new(ADMIN_DIR).join(kind))
        .expect("the admin table directory exists")
        .map(|entry| entry.expect("the directory reads").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| {
            let tag = path.file_stem()?.to_str()?.to_string();
            (tag != "sources").then_some((tag, path))
        })
        .collect();
    tables.sort();
    tables
}

/// A table file's terms.
fn terms(path: &Path) -> Vec<(String, String)> {
    let text = std::fs::read_to_string(path).expect("the table reads");
    let file: Value = serde_json::from_str(&text).expect("the table parses");
    file["terms"]
        .as_object()
        .expect("the table has terms")
        .iter()
        .map(|(key, value)| {
            (
                key.clone(),
                value.as_str().expect("a term is text").to_string(),
            )
        })
        .collect()
}

/// The site carries each admin label table byte for byte, and names it in
/// [`TABLES`]. `UPDATE_GENERATED` rewrites the copies.
#[test]
fn the_site_copy_matches_the_admin_label_tables() {
    let admin = admin_tables("labels");
    if std::env::var_os("UPDATE_GENERATED").is_some() {
        std::fs::create_dir_all(SITE_DIR).expect("the site directory exists");
        for (tag, path) in &admin {
            std::fs::copy(path, Path::new(SITE_DIR).join(format!("{tag}.json")))
                .expect("the copy is writable");
        }
        // The copies are embedded at compile time, so the next build checks them.
        return;
    }
    let shipped: Vec<&str> = TABLES.iter().map(|(tag, _)| *tag).collect();
    let tags: Vec<&str> = admin.iter().map(|(tag, _)| tag.as_str()).collect();
    assert_eq!(
        BTreeSet::from_iter(shipped.iter().copied()),
        BTreeSet::from_iter(tags.iter().copied()),
        "TABLES in vocabulary.rs must name every admin label table"
    );
    for (tag, path) in &admin {
        let (_, embedded) = TABLES.iter().find(|(name, _)| name == tag).unwrap();
        let current = std::fs::read_to_string(path).unwrap();
        assert_eq!(
            *embedded, current,
            "run UPDATE_GENERATED=1 cargo test -p omeka-site vocabulary to copy {tag}.json"
        );
    }
}

/// Every locale names every English term, with text, and no other: a term a
/// shipped locale lacks would print in English without a word.
#[test]
fn every_locale_names_every_term() {
    for kind in ["labels", "comments"] {
        let tables = admin_tables(kind);
        let (_, english_path) = tables.iter().find(|(tag, _)| tag == "en").unwrap();
        let english: BTreeSet<String> = terms(english_path)
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        assert!(tables.len() > 1, "{kind}: only English is shipped");
        for (tag, path) in &tables {
            let table = terms(path);
            let keys: BTreeSet<String> = table.iter().map(|(key, _)| key.clone()).collect();
            let missing: Vec<&String> = english.difference(&keys).collect();
            let extra: Vec<&String> = keys.difference(&english).collect();
            assert!(
                missing.is_empty() && extra.is_empty(),
                "{kind}/{tag}.json: missing {missing:?}, not in English {extra:?}"
            );
            for (key, text) in &table {
                assert!(!text.trim().is_empty(), "{kind}/{tag}.json: {key} is empty");
            }
        }
    }
}

/// Upstream's catalogue names 176 property labels, 101 class labels and 4
/// vocabulary labels; 12 more are msgids the application shares, which the
/// pot lists without their vocabulary comment. Comments: 165, 104 and 4.
#[test]
fn the_english_tables_hold_upstreams_terms() {
    let tables = admin_tables("labels");
    let labels = terms(&tables.iter().find(|(tag, _)| tag == "en").unwrap().1);
    let tables = admin_tables("comments");
    let comments = terms(&tables.iter().find(|(tag, _)| tag == "en").unwrap().1);
    assert_eq!(labels.len(), 176 + 101 + 4 + 12);
    assert_eq!(comments.len(), 165 + 104 + 4);
}

/// Every vocabulary, class and property a fresh install stores has its
/// English, so the guard can recognise it.
#[test]
fn every_seeded_term_has_its_english() {
    let tables = admin_tables("labels");
    let labels: BTreeSet<String> = terms(&tables.iter().find(|(tag, _)| tag == "en").unwrap().1)
        .into_iter()
        .map(|(key, _)| key)
        .collect();
    let tables = admin_tables("comments");
    let comments: BTreeSet<String> = terms(&tables.iter().find(|(tag, _)| tag == "en").unwrap().1)
        .into_iter()
        .map(|(key, _)| key)
        .collect();
    let seed: Value = serde_json::from_str(&std::fs::read_to_string(SEED).unwrap()).unwrap();
    for vocabulary in seed["vocabularies"].as_array().unwrap() {
        let prefix = vocabulary["prefix"].as_str().unwrap();
        assert!(labels.contains(prefix), "{prefix}");
        let members = vocabulary["classes"]
            .as_array()
            .unwrap()
            .iter()
            .chain(vocabulary["properties"].as_array().unwrap());
        for member in members {
            let term = format!("{prefix}:{}", member["local_name"].as_str().unwrap());
            assert!(labels.contains(&term), "{term} has no English label");
            if member["comment"].is_string() {
                assert!(comments.contains(&term), "{term} has no English comment");
            }
        }
    }
}

/// `$translate($property->label())` in Spanish: the built-in English label
/// prints as `es.po` translates it.
#[test]
fn a_built_in_label_prints_in_the_locale() {
    let spanish = Locale::Es.translator();
    assert_eq!(spanish.term_label("dcterms:title", "Title"), "Título");
    assert_eq!(spanish.term_label("dctype:Image", "Image"), "Imagen");
    assert_eq!(spanish.term_label("dcterms", "Dublin Core"), "Dublin Core");
    assert_eq!(
        Locale::En.translator().term_label("dcterms:title", "Title"),
        "Title"
    );
}

/// gettext finds no msgid for a label the user wrote, so it prints as stored:
/// a user vocabulary's term, an edited built-in label, and a locale with no
/// table.
#[test]
fn any_other_label_prints_as_stored() {
    let spanish = Locale::Es.translator();
    assert_eq!(
        spanish.term_label("mine:shelfMark", "Shelf mark"),
        "Shelf mark"
    );
    assert_eq!(spanish.term_label("dcterms:title", "Headline"), "Headline");
    assert_eq!(term_label("fr", "dcterms:title", "Title"), "Title");
}
