//! The field schema: the `data` members a layout edits, declared beside its
//! renderer and serialised for the admin.
//!
//! Each layout module declares a [`LayoutSchema`] and returns it from
//! `BlockLayout::schema`. [`registry`] carries every registered layout's schema
//! and the two layout-data schemas to `web/admin/src/generated/blocks.json`,
//! which the admin reads instead of mirroring the registry by hand. A test
//! ([`tests::generated_registry_matches_the_admin_file`]) writes that file and
//! fails when it is stale, so the two cannot drift.
//!
//! The kinds are the form elements upstream builds in
//! `application/src/Site/BlockLayout/*.php`: a text input, a rich-text HTML
//! input, a bounded number, a checkbox, a select or checkbox group, a page or
//! asset reference, a resource query, an attachments list, a repeat group and
//! the `listOfPages` tree. `label` is a catalogue key, not a string, because
//! only the admin knows a locale. The `const fn`s below keep each layout's
//! table to one line per field.

use serde::Serialize;

use super::{layout_id, layouts, romeka};

/// A layout's editable `data` members, in the order its form shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LayoutSchema {
    /// The fields, in form order.
    pub fields: &'static [Field],
}

/// A layout with no editable `data` members.
pub const EMPTY: LayoutSchema = LayoutSchema { fields: &[] };

/// One editable member of a layout's `data` or of a repeat group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Field {
    /// The stored key, e.g. `resource_type`.
    pub key: &'static str,
    /// The catalogue key for the field's label.
    pub label: &'static str,
    /// What the field holds and how a form renders it, flattened so the JSON
    /// carries one `type` beside the option list.
    #[serde(flatten)]
    pub kind: FieldKind,
    /// The value a form starts from, or absent for an empty control.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<FieldDefault>,
    /// Whether the control is required (`oembed.url`).
    #[serde(skip_serializing_if = "is_false")]
    pub required: bool,
}

/// The value a field starts from, as the JSON kind that carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum FieldDefault {
    /// A string value.
    Text(&'static str),
    /// A whole number.
    Int(i64),
    /// A checkbox state.
    Bool(bool),
    /// The checked values of a multiple choice.
    TextList(&'static [&'static str]),
}

/// The control a field renders as, and the values it accepts. The `type` tag
/// and any options sit beside the field's `key` and `label`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FieldKind {
    /// A text input.
    Text,
    /// A rich-text HTML input (upstream's WYSIWYG).
    Html,
    /// A number input, with a lower bound where upstream sets one.
    Integer {
        /// The smallest accepted value (`min`), if any.
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<i64>,
        /// The largest accepted value (`max`), if any.
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<i64>,
    },
    /// A checkbox.
    Boolean,
    /// A select, or a group of checkboxes when `multiple`.
    Choice {
        /// The accepted values, in form order.
        options: &'static [ChoiceOption],
        /// Whether the field holds a list of the values.
        #[serde(skip_serializing_if = "is_false")]
        multiple: bool,
    },
    /// A reference to a page of the site.
    PageRef,
    /// A reference to an asset.
    AssetRef,
    /// A search query over the site's resources (`Omeka\Form\Element\Query`).
    ItemQuery,
    /// The block's attachments, edited as a separate list rather than in
    /// `data` (`site_page_block.attachment`). `media` marks the layouts whose
    /// attachments may carry a media choice (`blockAttachmentsForm`).
    Attachments {
        /// Whether an attachment may name a media.
        #[serde(skip_serializing_if = "is_false")]
        media: bool,
    },
    /// A repeat group: a list of records, each with these fields.
    Repeat {
        /// The fields of one record.
        fields: &'static [Field],
    },
    /// The `listOfPages` page tree, stored as a JSON string.
    PageList,
}

/// One accepted value of a [`FieldKind::Choice`], with its label key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ChoiceOption {
    /// The stored value; the empty string is a real choice.
    pub value: &'static str,
    /// The catalogue key for the option's label.
    pub label: &'static str,
}

/// One accepted value of a choice.
pub const fn option(value: &'static str, label: &'static str) -> ChoiceOption {
    ChoiceOption { value, label }
}

/// A text input.
pub const fn text(key: &'static str, label: &'static str) -> Field {
    Field {
        key,
        label,
        kind: FieldKind::Text,
        default: None,
        required: false,
    }
}

/// A rich-text HTML input.
pub const fn html(key: &'static str, label: &'static str) -> Field {
    Field {
        key,
        label,
        kind: FieldKind::Html,
        default: None,
        required: false,
    }
}

/// A number input with no bound.
pub const fn integer(key: &'static str, label: &'static str) -> Field {
    Field {
        key,
        label,
        kind: FieldKind::Integer {
            min: None,
            max: None,
        },
        default: None,
        required: false,
    }
}

/// A number input whose smallest accepted value is `min`.
pub const fn integer_min(key: &'static str, label: &'static str, min: i64) -> Field {
    Field {
        key,
        label,
        kind: FieldKind::Integer {
            min: Some(min),
            max: None,
        },
        default: None,
        required: false,
    }
}

/// A checkbox.
pub const fn boolean(key: &'static str, label: &'static str) -> Field {
    Field {
        key,
        label,
        kind: FieldKind::Boolean,
        default: None,
        required: false,
    }
}

/// A select over `options`.
pub const fn choice(
    key: &'static str,
    label: &'static str,
    options: &'static [ChoiceOption],
) -> Field {
    Field {
        key,
        label,
        kind: FieldKind::Choice {
            options,
            multiple: false,
        },
        default: None,
        required: false,
    }
}

/// A group of checkboxes over `options`.
pub const fn multi_choice(
    key: &'static str,
    label: &'static str,
    options: &'static [ChoiceOption],
) -> Field {
    Field {
        key,
        label,
        kind: FieldKind::Choice {
            options,
            multiple: true,
        },
        default: None,
        required: false,
    }
}

/// A reference to a page of the site.
pub const fn page_ref(key: &'static str, label: &'static str) -> Field {
    Field {
        key,
        label,
        kind: FieldKind::PageRef,
        default: None,
        required: false,
    }
}

/// A reference to an asset.
pub const fn asset_ref(key: &'static str, label: &'static str) -> Field {
    Field {
        key,
        label,
        kind: FieldKind::AssetRef,
        default: None,
        required: false,
    }
}

/// A search query over the site's resources.
pub const fn item_query(key: &'static str, label: &'static str) -> Field {
    Field {
        key,
        label,
        kind: FieldKind::ItemQuery,
        default: None,
        required: false,
    }
}

/// The block's attachments. `media` marks a layout whose attachments may name
/// a media; the key is not a `data` member, so it names the list itself.
pub const fn attachments(label: &'static str, media: bool) -> Field {
    Field {
        key: "attachments",
        label,
        kind: FieldKind::Attachments { media },
        default: None,
        required: false,
    }
}

/// A repeat group whose records hold `fields`.
pub const fn repeat(label: &'static str, fields: &'static [Field]) -> Field {
    Field {
        key: "attachments",
        label,
        kind: FieldKind::Repeat { fields },
        default: None,
        required: false,
    }
}

/// The `listOfPages` page tree.
pub const fn page_list(key: &'static str, label: &'static str) -> Field {
    Field {
        key,
        label,
        kind: FieldKind::PageList,
        default: None,
        required: false,
    }
}

impl Field {
    /// Starts the control on `value`.
    pub const fn default_text(self, value: &'static str) -> Self {
        Self {
            default: Some(FieldDefault::Text(value)),
            ..self
        }
    }

    /// Starts the number control on `value`.
    pub const fn default_int(self, value: i64) -> Self {
        Self {
            default: Some(FieldDefault::Int(value)),
            ..self
        }
    }

    /// Starts the checkbox on `value`.
    pub const fn default_bool(self, value: bool) -> Self {
        Self {
            default: Some(FieldDefault::Bool(value)),
            ..self
        }
    }

    /// Starts a multiple choice with `value` checked.
    pub const fn default_list(self, value: &'static [&'static str]) -> Self {
        Self {
            default: Some(FieldDefault::TextList(value)),
            ..self
        }
    }

    /// Marks the control required.
    pub const fn required(self) -> Self {
        Self {
            required: true,
            ..self
        }
    }
}

/// `Form/BlockLayoutDataForm.php`: the styling every block's `layout_data`
/// carries. `grid_column_position` and `grid_column_span` come from the page
/// editor's grid rather than that form, and are declared here because
/// `AbstractLayout` reads them.
pub const BLOCK_LAYOUT_DATA: LayoutSchema = LayoutSchema {
    fields: &[
        text("template_name", "blockLayoutData.template_name"),
        text("class", "blockLayoutData.class"),
        text(
            "grid_column_position",
            "blockLayoutData.grid_column_position",
        ),
        text("grid_column_span", "blockLayoutData.grid_column_span"),
        choice(
            "alignment_block",
            "blockLayoutData.alignment_block",
            &[
                option("left", "blockChoice.left"),
                option("center", "blockChoice.center"),
                option("right", "blockChoice.right"),
            ],
        ),
        choice(
            "alignment_text",
            "blockLayoutData.alignment_text",
            &[
                option("left", "blockChoice.left"),
                option("center", "blockChoice.center"),
                option("right", "blockChoice.right"),
                option("justify", "blockChoice.justify"),
            ],
        ),
        text("max_width", "blockLayoutData.max_width"),
        text("min_height", "blockLayoutData.min_height"),
        text("padding_top", "blockLayoutData.padding_top"),
        text("padding_right", "blockLayoutData.padding_right"),
        text("padding_bottom", "blockLayoutData.padding_bottom"),
        text("padding_left", "blockLayoutData.padding_left"),
        text("background_color", "blockLayoutData.background_color"),
        asset_ref(
            "background_image_asset",
            "blockLayoutData.background_image_asset",
        ),
        choice(
            "background_image_position_y",
            "blockLayoutData.background_image_position_y",
            &[
                option("top", "blockChoice.top"),
                option("center", "blockChoice.center"),
                option("bottom", "blockChoice.bottom"),
            ],
        ),
        choice(
            "background_image_position_x",
            "blockLayoutData.background_image_position_x",
            &[
                option("left", "blockChoice.left"),
                option("center", "blockChoice.center"),
                option("right", "blockChoice.right"),
            ],
        ),
        choice(
            "background_image_size",
            "blockLayoutData.background_image_size",
            &[
                option("cover", "blockChoice.cover"),
                option("contain", "blockChoice.contain"),
            ],
        ),
    ],
};

/// `Form/PageLayoutDataForm.php`: the page's own `layout_data`. `grid_columns`
/// comes from the page editor's grid rather than that form, and is declared
/// here because `View/Helper/PageLayout::render` reads it.
pub const PAGE_LAYOUT_DATA: LayoutSchema = LayoutSchema {
    fields: &[
        text("template_name", "pageLayoutData.template_name"),
        text("class", "pageLayoutData.class"),
        integer_min("grid_columns", "pageLayoutData.grid_columns", 0),
        integer_min("grid_column_gap", "pageLayoutData.grid_column_gap", 0).default_int(10),
        integer_min("grid_row_gap", "pageLayoutData.grid_row_gap", 0).default_int(10),
    ],
};

/// One registered layout as the admin reads it.
#[derive(Debug, Serialize)]
pub struct LayoutEntry {
    /// The `site_page_block.layout` value.
    pub name: &'static str,
    /// The catalogue key for the layout's name (`layout.<name>`).
    pub label: String,
    /// The layout's `data` fields, flattened beside its name.
    #[serde(flatten)]
    pub schema: LayoutSchema,
}

/// One registered Romeka block as the admin reads it.
#[derive(Debug, Serialize)]
pub struct RomkaEntry {
    /// The name `o:layout_data.romeka.block` holds.
    pub name: &'static str,
    /// The catalogue key for the block's label (`romeka.<name>`).
    pub label: &'static str,
    /// The upstream layout that stores the block.
    pub carrier: &'static str,
    /// The settings fields, flattened beside the entry's name.
    #[serde(flatten)]
    pub schema: LayoutSchema,
}

/// The plain-hash algorithm and one worked example.
///
/// The admin recomputes the hash of a stored plain rendering to mark a block
/// that was edited outside Romeka S. It ships this sample so its own
/// implementation is tested against the generator's, not against a copied
/// constant. `hash` is the canonical-content hash and `legacyHash` is the
/// raw-byte hash a carrier version 1 stored.
#[derive(Debug, Serialize)]
pub struct RomkaHash {
    /// The canonical algorithm's name (`fnv1a-64-canonical`).
    pub algorithm: &'static str,
    /// The sample input.
    pub input: &'static str,
    /// The canonical hash of the sample input, as this crate computes it.
    pub hash: String,
    /// The legacy algorithm's name (`fnv1a-64`).
    #[serde(rename = "legacyAlgorithm")]
    pub legacy_algorithm: &'static str,
    /// The raw-byte hash of the sample input.
    #[serde(rename = "legacyHash")]
    pub legacy_hash: String,
}

/// The sample the admin hashes in its test.
pub const HASH_SAMPLE: &str = "<h2>Romeka &amp; Co</h2><p>Hello <em>world</em>.</p>\
     <img src=\"/files/asset/a.png\" alt=\"A shelf\">\
     <a href=\"/s/main/page/about\">About</a>";

/// The whole registry, as `web/admin/src/generated/blocks.json` holds it.
#[derive(Debug, Serialize)]
pub struct Registry {
    /// Every registered layout, in upstream's order. The fallback is not a
    /// layout the admin offers, so it is absent.
    pub layouts: Vec<LayoutEntry>,
    /// Every registered Romeka block, in registry order.
    pub romeka: Vec<RomkaEntry>,
    /// The carrier version a new carrier writes.
    #[serde(rename = "romekaVersion")]
    pub romeka_version: i64,
    /// The plain-hash algorithm and a worked sample.
    #[serde(rename = "romekaHash")]
    pub romeka_hash: RomkaHash,
    /// The schema of a block's `o:layout_data`.
    #[serde(rename = "blockLayoutData")]
    pub block_layout_data: LayoutSchema,
    /// The schema of a page's `o:layout_data`.
    #[serde(rename = "pageLayoutData")]
    pub page_layout_data: LayoutSchema,
}

/// The registry the admin reads, built from the layouts' own schemas.
#[must_use]
pub fn registry() -> Registry {
    let entries = layouts()
        .iter()
        .filter(|layout| layout_id(layout.name()).is_some())
        .map(|layout| LayoutEntry {
            name: layout.name(),
            label: format!("layout.{}", layout.name()),
            schema: *layout.schema(),
        })
        .collect();
    let romeka_entries = romeka::blocks()
        .iter()
        .map(|block| RomkaEntry {
            name: block.name(),
            label: block.label(),
            carrier: block.carrier(),
            schema: *block.schema(),
        })
        .collect();
    Registry {
        layouts: entries,
        romeka: romeka_entries,
        romeka_version: romeka::CARRIER_VERSION,
        romeka_hash: RomkaHash {
            algorithm: "fnv1a-64-canonical",
            input: HASH_SAMPLE,
            hash: romeka::plain_hash(HASH_SAMPLE),
            legacy_algorithm: "fnv1a-64",
            legacy_hash: romeka::byte_hash(HASH_SAMPLE),
        },
        block_layout_data: BLOCK_LAYOUT_DATA,
        page_layout_data: PAGE_LAYOUT_DATA,
    }
}

/// `skip_serializing_if` for the flags whose `false` is the default reading.
fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod tests {
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

    fn assert_keys_are_read(layout: &str, fields: &'static [Field], read: &[&'static str]) {
        for field in fields {
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
}
