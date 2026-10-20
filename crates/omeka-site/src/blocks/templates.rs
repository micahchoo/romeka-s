//! The built-in starter templates: a page's layout, its layout data and its
//! blocks, defined here beside the block registries.
//!
//! A curator's first page should look like something. A template names only a
//! registered layout or a registered Romeka block, so it cannot reference a
//! block the admin has no form for. [`template_registry`] serialises the
//! templates to `web/admin/src/generated/templates.json`, which the admin reads
//! when it offers a starting point for a new page. A test keeps that file
//! byte-identical and checks every block against the registries.
//!
//! A template is a starting point, not a finished page: its blocks carry
//! placeholder settings and no attachments, and a Romeka block's plain
//! rendering is generated on the first save.

use omeka_core::site::PageLayout;
use serde::Serialize;
use serde_json::{Map, Value as JsonValue, json};

use super::hero::Hero;
use super::romeka::{self, RomkaBlock};
use super::section_templates::section_templates;

/// One block a template lays down.
pub enum TemplateBlock {
    /// A registered upstream layout with its `o:data`.
    Layout {
        /// The registered layout name.
        layout: &'static str,
        /// The block's `o:data`.
        data: JsonValue,
        /// The block's `o:layout_data`, when it carries one.
        layout_data: Option<JsonValue>,
    },
    /// A registered Romeka block, stored under its carrier.
    Romka {
        /// The block's definition, so a template cannot name an unregistered
        /// block.
        block: &'static dyn RomkaBlock,
        /// The block's settings.
        settings: JsonValue,
        /// The block's `o:layout_data` beside the carrier.
        layout_data: Option<JsonValue>,
    },
}

/// A block of a registered layout.
pub(crate) fn layout_block(layout: &'static str, data: JsonValue) -> TemplateBlock {
    TemplateBlock::Layout {
        layout,
        data,
        layout_data: None,
    }
}

/// A Romeka block with its settings.
fn romka_block(block: &'static dyn RomkaBlock, settings: JsonValue) -> TemplateBlock {
    TemplateBlock::Romka {
        block,
        settings,
        layout_data: None,
    }
}

impl TemplateBlock {
    /// The same block placed in a grid cell of `span` columns.
    fn spanning(self, span: i64) -> Self {
        self.with_layout_data(json!({ "grid_column_span": span.to_string() }))
    }

    /// The same block with `extra` added to its `o:layout_data`.
    pub(crate) fn with_layout_data(self, extra: JsonValue) -> Self {
        match self {
            Self::Layout {
                layout,
                data,
                layout_data,
            } => Self::Layout {
                layout,
                data,
                layout_data: Some(merge(layout_data, extra)),
            },
            Self::Romka {
                block,
                settings,
                layout_data,
            } => Self::Romka {
                block,
                settings,
                layout_data: Some(merge(layout_data, extra)),
            },
        }
    }
}

/// The `extra` members on top of an existing `o:layout_data`, as an object.
fn merge(existing: Option<JsonValue>, extra: JsonValue) -> JsonValue {
    let mut map = match existing {
        Some(JsonValue::Object(map)) => map,
        _ => Map::new(),
    };
    if let JsonValue::Object(extra) = extra {
        map.extend(extra);
    }
    JsonValue::Object(map)
}

/// One starter template.
pub struct Template {
    /// The template's id, unique in the registry.
    pub id: &'static str,
    /// The catalogue key for the template's name.
    pub label: &'static str,
    /// The catalogue key for the template's description.
    pub description: &'static str,
    /// The page layout the template starts from.
    pub layout: PageLayout,
    /// The page's `o:layout_data`.
    pub layout_data: JsonValue,
    /// The blocks, in position order.
    pub blocks: Vec<TemplateBlock>,
}

/// The built-in templates, in the order the chooser lists them.
#[must_use]
pub fn templates() -> Vec<Template> {
    vec![
        Template {
            id: "exhibit-landing",
            label: "template.exhibitLanding.label",
            description: "template.exhibitLanding.description",
            layout: PageLayout::Normal,
            layout_data: json!({}),
            blocks: vec![
                romka_block(
                    &Hero,
                    json!({
                        "heading": "Your exhibit title",
                        "subheading": "A sentence about what this exhibit holds",
                        "overlay": "medium",
                        "alignment": "left",
                    }),
                ),
                layout_block(
                    "html",
                    json!({
                        "html": "<p>Introduce the exhibit here. This page came from a template, so every block is yours to edit or remove.</p>",
                    }),
                ),
                layout_block(
                    "browsePreview",
                    json!({
                        "resource_type": "items",
                        "limit": 12,
                        "components": ["resource-heading", "resource-body", "thumbnail"],
                        "heading": "Browse the collection",
                    }),
                ),
                layout_block("listOfPages", json!({ "pagelist": "[]" })),
            ],
        },
        Template {
            id: "collection-highlight",
            label: "template.collectionHighlight.label",
            description: "template.collectionHighlight.description",
            layout: PageLayout::Normal,
            layout_data: json!({}),
            blocks: vec![
                romka_block(
                    &Hero,
                    json!({
                        "heading": "Collection highlight",
                        "subheading": "One object, up close",
                        "overlay": "medium",
                        "alignment": "left",
                    }),
                ),
                layout_block(
                    "media",
                    json!({
                        "layout": "",
                        "media_display": "",
                        "thumbnail_type": "square",
                        "show_title_option": "item_title",
                    }),
                ),
                layout_block("itemWithMetadata", json!({})),
            ],
        },
        Template {
            id: "about",
            label: "template.about.label",
            description: "template.about.description",
            layout: PageLayout::Normal,
            layout_data: json!({}),
            blocks: vec![
                layout_block("pageTitle", json!({})),
                layout_block(
                    "html",
                    json!({
                        "html": "<p>About this site. Replace this text with the story behind the collection.</p>",
                    }),
                ),
                layout_block(
                    "asset",
                    json!({
                        "0": {
                            "id": "",
                            "page": "",
                            "alt_link_title": "",
                            "caption": "",
                        },
                    }),
                ),
            ],
        },
        Template {
            id: "gallery",
            label: "template.gallery.label",
            description: "template.gallery.description",
            layout: PageLayout::Grid,
            layout_data: json!({
                "grid_columns": 3,
                "grid_column_gap": 10,
                "grid_row_gap": 10,
            }),
            blocks: vec![
                layout_block("pageTitle", json!({})).spanning(3),
                layout_block(
                    "media",
                    json!({
                        "layout": "",
                        "media_display": "",
                        "thumbnail_type": "square",
                        "show_title_option": "item_title",
                    }),
                )
                .spanning(1),
                layout_block(
                    "media",
                    json!({
                        "layout": "",
                        "media_display": "",
                        "thumbnail_type": "square",
                        "show_title_option": "item_title",
                    }),
                )
                .spanning(1),
                layout_block(
                    "media",
                    json!({
                        "layout": "",
                        "media_display": "",
                        "thumbnail_type": "square",
                        "show_title_option": "item_title",
                    }),
                )
                .spanning(1),
            ],
        },
        Template {
            id: "blank",
            label: "template.blank.label",
            description: "template.blank.description",
            layout: PageLayout::Normal,
            layout_data: json!({}),
            blocks: Vec::new(),
        },
    ]
}

/// The blocks of the first page a new site is born with: a banner, a line of
/// introduction and the latest items, so the site looks finished before a
/// curator edits it. The chooser does not offer them; the site create stores
/// them once ([`welcome_blocks`]).
///
/// The call to action is a relative URL because a page's own URL is always
/// `/s/<site>/page/<page>`, so `../item` reaches the item browse after a slug
/// change and on an upstream installation alike.
fn welcome() -> Vec<TemplateBlock> {
    vec![
        romka_block(
            &Hero,
            json!({
                "heading": "Welcome",
                "subheading": "A new collection, ready for its first visitors.",
                "overlay": "medium",
                "alignment": "left",
                "cta_label": "Explore the collection",
                "cta_url": "../item",
            }),
        ),
        layout_block(
            "html",
            json!({
                "html": "<p>This is the first page of your site. Change these words, give the banner a background image, or start a new page from a template. Everything here is yours to edit.</p>",
            }),
        ),
        layout_block(
            "browsePreview",
            json!({
                "resource_type": "items",
                "limit": 6,
                "components": ["resource-heading", "resource-body", "thumbnail"],
                "heading": "Recently added",
                "link-text": "Browse all items",
            }),
        ),
    ]
}

/// The welcome page's blocks in the shape a site page stores them. A Romeka
/// block arrives as its carrier with an empty body, which the write path fills
/// with the plain rendering.
#[must_use]
pub fn welcome_blocks() -> Vec<TemplateBlockEntry> {
    welcome().iter().map(block_entry).collect()
}

/// The whole template registry, as `templates.json` holds it.
#[derive(Debug, Serialize)]
pub struct TemplateRegistry {
    /// Every template, in registry order.
    pub templates: Vec<TemplateEntry>,
    /// Every section template, in the order the block picker lists them.
    pub sections: Vec<SectionEntry>,
}

/// One section template as the admin reads it: a group and its blocks, which
/// the block picker inserts where the curator chose.
#[derive(Debug, Serialize)]
pub struct SectionEntry {
    /// The template's id.
    pub id: &'static str,
    /// The catalogue key for the template's name.
    pub label: &'static str,
    /// The catalogue key for the template's description.
    pub description: &'static str,
    /// The group block first, then its members.
    pub blocks: Vec<TemplateBlockEntry>,
}

/// One template as the admin reads it.
#[derive(Debug, Serialize)]
pub struct TemplateEntry {
    /// The template's id.
    pub id: &'static str,
    /// The catalogue key for the template's name.
    pub label: &'static str,
    /// The catalogue key for the template's description.
    pub description: &'static str,
    /// `normal` or `grid`, the value the draft's page layout takes.
    pub layout: &'static str,
    /// The page's `o:layout_data`.
    #[serde(rename = "layoutData")]
    pub layout_data: JsonValue,
    /// The blocks, in position order.
    pub blocks: Vec<TemplateBlockEntry>,
}

/// One block of a template as the admin reads it: the draft shape without a key
/// or attachments.
#[derive(Debug, Serialize)]
pub struct TemplateBlockEntry {
    /// The stored layout name, the carrier's for a Romeka block.
    pub layout: &'static str,
    /// The block's `o:data`.
    pub data: JsonValue,
    /// The block's `o:layout_data`, or `null`.
    #[serde(rename = "layoutData")]
    pub layout_data: JsonValue,
}

/// The registry the admin reads, built from the templates' own blocks.
#[must_use]
pub fn template_registry() -> TemplateRegistry {
    TemplateRegistry {
        templates: templates().into_iter().map(template_entry).collect(),
        sections: section_templates()
            .into_iter()
            .map(|section| SectionEntry {
                id: section.id,
                label: section.label,
                description: section.description,
                blocks: section.blocks().iter().map(block_entry).collect(),
            })
            .collect(),
    }
}

fn template_entry(template: Template) -> TemplateEntry {
    TemplateEntry {
        id: template.id,
        label: template.label,
        description: template.description,
        layout: layout_name(template.layout),
        layout_data: canonical(template.layout_data),
        blocks: template.blocks.iter().map(block_entry).collect(),
    }
}

/// The value with every object's keys sorted.
///
/// `serde_json`'s `Map` is a `BTreeMap` unless the `preserve_order` feature is
/// on, and that feature is unified across a build: `cargo test --workspace`
/// turns it on where `cargo test -p omeka-site` does not. Sorting the keys makes
/// the generated file the same bytes either way, so the byte-identical test
/// cannot fail on how the crate was built.
fn canonical(value: JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(map) => {
            let mut entries: Vec<(String, JsonValue)> = map
                .into_iter()
                .map(|(key, value)| (key, canonical(value)))
                .collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            JsonValue::Object(entries.into_iter().collect())
        }
        JsonValue::Array(items) => JsonValue::Array(items.into_iter().map(canonical).collect()),
        other => other,
    }
}

/// The admin's page layout name: `normal` or `grid`.
fn layout_name(layout: PageLayout) -> &'static str {
    match layout {
        PageLayout::Grid => "grid",
        PageLayout::Normal => "normal",
    }
}

/// A template block as the admin reads it. A Romeka block becomes its carrier
/// with the block's settings under `o:layout_data.romeka`; the carrier's own
/// content is empty, because the first save generates the plain rendering.
pub(crate) fn block_entry(block: &TemplateBlock) -> TemplateBlockEntry {
    match block {
        TemplateBlock::Layout {
            layout,
            data,
            layout_data,
        } => TemplateBlockEntry {
            layout,
            data: canonical(data.clone()),
            layout_data: canonical(layout_data.clone().unwrap_or(JsonValue::Null)),
        },
        TemplateBlock::Romka {
            block,
            settings,
            layout_data,
        } => {
            let mut carrier = match layout_data {
                Some(JsonValue::Object(map)) => map.clone(),
                _ => Map::new(),
            };
            carrier.insert(
                "romeka".to_string(),
                json!({
                    "block": block.name(),
                    "v": romeka::CARRIER_VERSION,
                    "settings": settings,
                }),
            );
            TemplateBlockEntry {
                layout: block.carrier(),
                data: json!({ "html": "" }),
                layout_data: canonical(JsonValue::Object(carrier)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::blocks::schema::{Field, FieldKind};
    use crate::blocks::{layout_id, parse};

    /// Where the admin reads the generated templates.
    const ADMIN_FILE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/admin/src/generated/templates.json"
    );

    /// The admin's file is the registry's serialisation, byte for byte. A
    /// difference fails with the command that rewrites it, so the two cannot
    /// drift.
    #[test]
    fn generated_registry_matches_the_admin_file() {
        let rendered = format!(
            "{}\n",
            serde_json::to_string_pretty(&template_registry()).expect("the registry serialises")
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

    /// The registry ships the five named starting points, each with a unique id.
    #[test]
    fn the_registry_ships_the_five_named_templates() {
        let ids: Vec<&str> = templates().iter().map(|template| template.id).collect();
        assert_eq!(
            ids,
            [
                "exhibit-landing",
                "collection-highlight",
                "about",
                "gallery",
                "blank"
            ]
        );
        let unique: std::collections::BTreeSet<&str> = ids.iter().copied().collect();
        assert_eq!(unique.len(), ids.len(), "a template id appears twice");
    }

    /// Every template block names a registered layout or Romeka block, and its
    /// `o:data` or settings are the values that block reads.
    #[test]
    fn every_template_block_is_registered_and_its_values_parse() {
        for template in templates() {
            assert!(!template.label.is_empty(), "{} label", template.id);
            assert!(
                !template.description.is_empty(),
                "{} description",
                template.id
            );
            for block in &template.blocks {
                check_block(template.id, block);
            }
        }
        for block in &welcome() {
            check_block("welcome", block);
        }
        for section in section_templates() {
            let id = section.id;
            for block in &section.blocks() {
                check_block(id, block);
            }
        }
    }

    fn check_block(template: &str, block: &TemplateBlock) {
        match block {
            TemplateBlock::Layout { layout, data, .. } => {
                let id = layout_id(layout)
                    .unwrap_or_else(|| panic!("{template}: `{layout}` is not a registered layout"));
                parse::parse_block_data(&id, data)
                    .unwrap_or_else(|error| panic!("{template}: {layout}: {error}"));
                check_data_keys(template, layout, data);
            }
            TemplateBlock::Romka {
                block, settings, ..
            } => {
                assert!(
                    romeka::block(block.name()).is_some(),
                    "{template}: `{}` is not a registered Romeka block",
                    block.name()
                );
                check_settings(template, *block, settings);
            }
        }
    }

    /// Every `o:data` key a template sets names a field its layout declares. The
    /// asset layout stores its slots by index, so its keys are positions.
    fn check_data_keys(template: &str, layout: &str, data: &JsonValue) {
        if layout == "asset" {
            return;
        }
        let schema = crate::blocks::layout_for(layout).schema();
        let Some(object) = data.as_object() else {
            return;
        };
        for key in object.keys() {
            assert!(
                schema.fields.iter().any(|field| field.writes(key)),
                "{template}: {layout} has no data field `{key}`"
            );
        }
    }

    /// Every Romeka setting names a schema field and carries the JSON kind that
    /// field declares.
    fn check_settings(template: &str, block: &dyn RomkaBlock, settings: &JsonValue) {
        let fields = block.schema().fields;
        let Some(object) = settings.as_object() else {
            panic!("{template}: {} settings are not an object", block.name());
        };
        for (key, value) in object {
            let field = fields
                .iter()
                .find(|field| field.writes(key))
                .unwrap_or_else(|| panic!("{template}: {} has no setting `{key}`", block.name()));
            assert!(
                kind_accepts(field, key, value),
                "{template}: {} setting `{key}` has the wrong kind",
                block.name()
            );
        }
    }

    /// Whether a value has the JSON kind a field declares.
    fn kind_accepts(field: &Field, key: &str, value: &JsonValue) -> bool {
        match field.kind {
            FieldKind::Text { .. }
            | FieldKind::Html
            | FieldKind::PageList
            | FieldKind::Color { .. }
            | FieldKind::Anchor
            | FieldKind::ManifestSource
            | FieldKind::Sides { .. } => value.is_string(),
            FieldKind::Link { url_key } if key == url_key => value.is_string(),
            FieldKind::Link { .. } => value.is_number() || numeric_string(value),
            FieldKind::Integer { .. } => value.is_number() || numeric_string(value),
            FieldKind::Boolean => value.is_boolean(),
            FieldKind::Choice { .. } => value.is_string(),
            FieldKind::PageRef
            | FieldKind::AssetRef
            | FieldKind::MediaRef
            | FieldKind::FocalPoint { .. } => value.is_number() || numeric_string(value),
            FieldKind::ItemQuery { .. } => value.is_string() || value.is_object(),
            FieldKind::Attachments { .. } | FieldKind::Repeat { .. } => {
                value.is_object() || value.is_array()
            }
            FieldKind::ItemList => value.is_array(),
            FieldKind::MapView { zoom_key } if key == zoom_key => value.is_number(),
            FieldKind::MapView { .. } | FieldKind::Property => value.is_string(),
        }
    }

    /// A numeric string, the shape an upstream form submits.
    fn numeric_string(value: &JsonValue) -> bool {
        value
            .as_str()
            .is_some_and(|text| text.trim().parse::<i64>().is_ok())
    }

    /// The generated entries carry the carrier expansion the admin's draft model
    /// builds a block from.
    #[test]
    fn a_romka_block_becomes_its_carrier() {
        let registry = template_registry();
        let exhibit = registry
            .templates
            .iter()
            .find(|template| template.id == "exhibit-landing")
            .expect("the exhibit landing template");
        let hero = exhibit.blocks.first().expect("the hero block");
        assert_eq!(hero.layout, "html");
        assert_eq!(hero.data, json!({ "html": "" }));
        assert_eq!(
            hero.layout_data.pointer("/romeka/block"),
            Some(&json!("hero"))
        );
        assert_eq!(
            hero.layout_data.pointer("/romeka/v"),
            Some(&json!(romeka::CARRIER_VERSION))
        );
        assert_eq!(
            hero.layout_data.pointer("/romeka/settings/heading"),
            Some(&json!("Your exhibit title"))
        );
    }
}
