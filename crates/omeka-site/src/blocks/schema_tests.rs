//! The field schema tests: the generated admin file, and the invariants every
//! field list keeps.

use super::*;
use crate::blocks::parse;

/// Where the admin reads the generated registry.
const ADMIN_FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../web/admin/src/generated/blocks.json"
);

/// The admin's file is the registry's serialisation, byte for byte. A
/// difference fails with the command that rewrites it, so the boundary is
/// enforced by a test rather than a convention.
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

/// Every registered layout the admin offers declares a schema, and only the
/// two layouts that carry no data leave it empty. The registry lists them
/// all, in the registry's order, and never the fallback.
#[test]
fn every_registered_layout_declares_a_schema() {
    let names: Vec<&str> = layouts()
        .iter()
        .filter(|layout| layout_id(layout.name()).is_some())
        .map(|layout| layout.name())
        .collect();
    let listed: Vec<&str> = registry().layouts.iter().map(|entry| entry.name).collect();
    assert_eq!(listed, names);
    assert!(!listed.contains(&"fallback"), "{listed:?}");
    for layout in layouts() {
        let name = layout.name();
        let empty = matches!(name, "pageTitle" | "fallback");
        assert_eq!(
            layout.schema().fields.is_empty(),
            empty,
            "{name} schema emptiness"
        );
    }
}

/// The registry lists every registered Romeka block, with the carrier and
/// the catalogue key the admin reads.
#[test]
fn the_registry_lists_every_romeka_block() {
    let listed: Vec<&str> = registry().romeka.iter().map(|entry| entry.name).collect();
    let names: Vec<&str> = romeka::blocks().iter().map(|block| block.name()).collect();
    assert_eq!(listed, names);
    for entry in &registry().romeka {
        assert_eq!(entry.label, format!("romeka.{}", entry.name));
        assert_eq!(entry.carrier, "html", "{} carrier", entry.name);
        assert!(!entry.schema.fields.is_empty(), "{} fields", entry.name);
    }
}

/// Every field key a schema names is one its layout's `data` parse reads.
/// An attachments list is stored as rows beside `data` and a repeat group
/// names its records rather than a key, so the check descends to the keys
/// that carry values.
#[test]
fn every_schema_field_names_a_key_its_layout_reads() {
    for layout in layouts() {
        let Some(id) = layout_id(layout.name()) else {
            continue;
        };
        let read = parse::data_keys(&id);
        assert_keys_are_read(layout.name(), layout.schema().fields, &read);
    }
}

/// Every field list the admin renders: each layout's, each Romeka
/// block's, the two layout-data schemas, and every repeat group inside them.
fn every_field_list() -> Vec<(String, &'static [Field])> {
    let mut lists: Vec<(String, &'static [Field])> = layouts()
        .iter()
        .map(|layout| (layout.name().to_string(), layout.schema().fields))
        .collect();
    lists.extend(
        romeka::blocks()
            .iter()
            .map(|block| (block.name().to_string(), block.schema().fields)),
    );
    lists.push(("blockLayoutData".to_string(), BLOCK_LAYOUT_DATA.fields));
    lists.push(("pageLayoutData".to_string(), PAGE_LAYOUT_DATA.fields));
    lists.push(("sectionLayoutData".to_string(), SECTION_LAYOUT_DATA.fields));
    let mut index = 0;
    while index < lists.len() {
        let (name, fields) = lists[index].clone();
        for field in fields {
            if let FieldKind::Repeat { fields: inner } = field.kind {
                lists.push((format!("{name}.{}", field.key), inner));
            }
        }
        index += 1;
    }
    lists
}

/// A condition, a resource-type source and a link's URL key each name a
/// sibling in the same list, so the admin never reads a key nobody writes.
/// A condition's `equals` value is one of the sibling's choices.
#[test]
fn every_sibling_reference_names_a_real_sibling() {
    for (list, fields) in every_field_list() {
        let sibling = |key: &str| fields.iter().find(|field| field.key == key);
        for field in fields {
            if let Some(when) = field.when {
                let target = sibling(when.field).unwrap_or_else(|| {
                    panic!(
                        "{list}.{}: `when` names no sibling `{}`",
                        field.key, when.field
                    )
                });
                assert_ne!(
                    target.key, field.key,
                    "{list}.{}: `when` names itself",
                    field.key
                );
                if let (Test::Equals { value }, FieldKind::Choice { options, .. }) =
                    (when.test, target.kind)
                {
                    assert!(
                        options.iter().any(|option| option.value == value),
                        "{list}.{}: `{value}` is not a choice of `{}`",
                        field.key,
                        target.key
                    );
                }
            }
            if let FieldKind::ItemQuery {
                resource_key: Some(key),
            } = field.kind
            {
                let target = sibling(key)
                    .unwrap_or_else(|| panic!("{list}.{}: no resource sibling `{key}`", field.key));
                assert!(
                    matches!(target.kind, FieldKind::Choice { .. }),
                    "{list}.{}: `{key}` is not a choice",
                    field.key
                );
            }
            if let FieldKind::Link { url_key } = field.kind {
                assert!(
                    sibling(url_key).is_none(),
                    "{list}.{}: `{url_key}` is also its own field",
                    field.key
                );
            }
        }
    }
}

/// One `assetRef` or `mediaRef` field the admin reads: where it is, what it
/// accepts, and whether it is a background.
type Reference = (String, String, String, bool);

/// The reference fields of one serialised field list, descending into repeat
/// groups, as `(list, key, accepts, decorative)` records.
fn references_of(fields: &serde_json::Value, list: &str, out: &mut Vec<Reference>) {
    let Some(entries) = fields.as_array() else {
        return;
    };
    for field in entries {
        let key = field["key"].as_str().expect("a field has a key");
        match field["type"].as_str().expect("a field has a type") {
            "assetRef" | "mediaRef" => out.push((
                list.to_string(),
                key.to_string(),
                field["accepts"]
                    .as_str()
                    .expect("a reference accepts some kinds")
                    .to_string(),
                field["decorative"].as_bool().unwrap_or(false),
            )),
            "repeat" => references_of(&field["fields"], &format!("{list}.{key}"), out),
            _ => {}
        }
    }
}

/// Every field that names an asset or a media states what the slot shows and
/// whether it is a background, in the JSON the admin parses. The list is
/// exact, so a new reference field has to answer both, and a reader of the
/// registry knows the hero's slot takes images while the asset block's takes
/// any file.
#[test]
fn every_reference_field_states_what_it_accepts() {
    let json = serde_json::to_value(registry()).expect("the registry serialises");
    let mut found: Vec<Reference> = Vec::new();
    for list in ["layouts", "romeka"] {
        for entry in json[list].as_array().expect("a list of blocks") {
            let name = entry["name"].as_str().expect("a block has a name");
            references_of(&entry["fields"], name, &mut found);
        }
    }
    for list in ["blockLayoutData", "pageLayoutData", "sectionLayoutData"] {
        references_of(&json[list]["fields"], list, &mut found);
    }
    let mut expected: Vec<Reference> = [
        // A hero's background is a picture behind the text, read by nobody.
        ("hero", "background", "image", true),
        ("hero", "background_media", "image", true),
        // A IIIF viewer zooms an image this archive serves, and reads no file.
        ("iiifImage", "romeka.media", "imageService", false),
        // A gallery shows its picture with a caption.
        ("gallery.assets", "asset", "image", false),
        // A block's own background is a picture behind the block.
        ("blockLayoutData", "background_image_asset", "image", true),
        // The asset block links to the file or draws it, so any file fits.
        ("asset.attachments", "id", "any", false),
    ]
    .into_iter()
    .map(|(list, key, accepts, decorative)| {
        (
            list.to_string(),
            key.to_string(),
            accepts.to_string(),
            decorative,
        )
    })
    .collect();
    found.sort();
    expected.sort();
    assert_eq!(found, expected);
}

/// Every field carries a help key apart from its label, and a group or the
/// advanced flag only in the layout-data schemas.
#[test]
fn every_field_has_help_and_layout_data_alone_groups() {
    for (list, fields) in every_field_list() {
        let styling = list == "blockLayoutData" || list == "pageLayoutData";
        for field in fields {
            assert!(!field.help.is_empty(), "{list}.{} help", field.key);
            assert_ne!(
                field.help, field.label,
                "{list}.{} help is its label",
                field.key
            );
            if !styling {
                assert!(
                    field.group.is_none() && !field.advanced,
                    "{list}.{} is grouped outside layout data",
                    field.key
                );
            }
        }
    }
    for field in BLOCK_LAYOUT_DATA.fields {
        assert!(
            field.group.is_some() != field.advanced,
            "blockLayoutData.{}: exactly one of a group or advanced",
            field.key
        );
    }
}

fn assert_keys_are_read(layout: &str, fields: &'static [Field], read: &[&'static str]) {
    // A Romeka setting lives in the layout data, which the data parse never reads.
    for field in fields
        .iter()
        .filter(|field| romeka_setting(field.key).is_none())
    {
        match field.kind {
            FieldKind::Attachments { .. } => {}
            FieldKind::Repeat { fields: inner } => assert_keys_are_read(layout, inner, read),
            _ => assert!(
                read.contains(&field.key),
                "{} declares `{}`, which its parse does not read",
                layout,
                field.key
            ),
        }
    }
}
