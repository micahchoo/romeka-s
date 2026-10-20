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
    /// The stored key, e.g. `resource_type`. A field that writes several keys
    /// ([`FieldKind::Sides`], [`FieldKind::Link`]) names its first, or the
    /// group's own name.
    pub key: &'static str,
    /// The catalogue key for the field's label.
    pub label: &'static str,
    /// The catalogue key for the one line under the control that says what the
    /// visitor sees.
    pub help: &'static str,
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
    /// The sibling value the control depends on; absent for a control that
    /// always shows. A hidden control keeps its stored value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<When>,
    /// The section of the styling form the field sits in (layout data only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<Group>,
    /// Whether the form tucks the field under its "Advanced" disclosure.
    #[serde(skip_serializing_if = "is_false")]
    pub advanced: bool,
}

/// A condition on one sibling field's value, which decides whether a control
/// shows. The sibling's schema default stands in for an absent value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct When {
    /// The sibling's key, in the same field list.
    pub field: &'static str,
    /// What the sibling's value must be.
    #[serde(flatten)]
    pub test: Test,
}

/// The test a [`When`] applies to the sibling's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "test", rename_all = "camelCase")]
pub enum Test {
    /// The value is exactly this string (a choice).
    Equals {
        /// The value that shows the control.
        value: &'static str,
    },
    /// The value is present: not empty, not zero, not false.
    Set,
}

impl When {
    /// Shows the control when `field` holds `value`.
    pub const fn equals(field: &'static str, value: &'static str) -> Self {
        Self {
            field,
            test: Test::Equals { value },
        }
    }

    /// Shows the control when `field` holds any value.
    pub const fn set(field: &'static str) -> Self {
        Self {
            field,
            test: Test::Set,
        }
    }
}

/// The sections of the block styling form, in the order it shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Group {
    /// Padding.
    Spacing,
    /// Maximum width and minimum height.
    Size,
    /// Colour and image behind the block.
    Background,
    /// Where the block and its text sit.
    Alignment,
    /// Position and span on a grid page.
    Grid,
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
    Text {
        /// The catalogue key for an example shown in the empty input.
        #[serde(skip_serializing_if = "Option::is_none")]
        placeholder: Option<&'static str>,
    },
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
    ItemQuery {
        /// The sibling choice that names the resource type the query searches
        /// (`browsePreview.resource_type`); absent for an items query.
        #[serde(rename = "resourceKey", skip_serializing_if = "Option::is_none")]
        resource_key: Option<&'static str>,
    },
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
    /// A `<hex-color>` (`#rgb` or `#rrggbb`), the only colour the renderer
    /// accepts.
    Color,
    /// One CSS length per side, each stored under its own key.
    Sides {
        /// The four stored keys.
        keys: SideKeys,
    },
    /// A link target: a page of the site under the field's `key`, or a URL
    /// under `url_key`. The control writes one and clears the other, because
    /// the renderer prefers the page.
    Link {
        /// The key that holds the URL.
        #[serde(rename = "urlKey")]
        url_key: &'static str,
    },
    /// Items picked one by one, stored as a list of item ids in pick order.
    ItemList,
    /// A map's starting view: a `"lat, lng"` centre under the field's `key`
    /// and a zoom under `zoom_key`. An empty centre fits every place.
    MapView {
        /// The key that holds the zoom.
        #[serde(rename = "zoomKey")]
        zoom_key: &'static str,
    },
    /// A property, stored as its term (`dcterms:date`).
    Property,
}

/// The stored keys of a [`FieldKind::Sides`] control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SideKeys {
    /// The top side's key.
    pub top: &'static str,
    /// The right side's key.
    pub right: &'static str,
    /// The bottom side's key.
    pub bottom: &'static str,
    /// The left side's key.
    pub left: &'static str,
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

/// A field of `kind` with no default, no condition and no group.
const fn field(
    key: &'static str,
    label: &'static str,
    help: &'static str,
    kind: FieldKind,
) -> Field {
    Field {
        key,
        label,
        help,
        kind,
        default: None,
        required: false,
        when: None,
        group: None,
        advanced: false,
    }
}

/// A text input.
pub const fn text(key: &'static str, label: &'static str, help: &'static str) -> Field {
    field(key, label, help, FieldKind::Text { placeholder: None })
}

/// A rich-text HTML input.
pub const fn html(key: &'static str, label: &'static str, help: &'static str) -> Field {
    field(key, label, help, FieldKind::Html)
}

/// A number input with no bound.
pub const fn integer(key: &'static str, label: &'static str, help: &'static str) -> Field {
    field(
        key,
        label,
        help,
        FieldKind::Integer {
            min: None,
            max: None,
        },
    )
}

/// A number input whose smallest accepted value is `min`.
pub const fn integer_min(
    key: &'static str,
    label: &'static str,
    help: &'static str,
    min: i64,
) -> Field {
    field(
        key,
        label,
        help,
        FieldKind::Integer {
            min: Some(min),
            max: None,
        },
    )
}

/// A checkbox.
pub const fn boolean(key: &'static str, label: &'static str, help: &'static str) -> Field {
    field(key, label, help, FieldKind::Boolean)
}

/// A select over `options`.
pub const fn choice(
    key: &'static str,
    label: &'static str,
    help: &'static str,
    options: &'static [ChoiceOption],
) -> Field {
    field(
        key,
        label,
        help,
        FieldKind::Choice {
            options,
            multiple: false,
        },
    )
}

/// A group of checkboxes over `options`.
pub const fn multi_choice(
    key: &'static str,
    label: &'static str,
    help: &'static str,
    options: &'static [ChoiceOption],
) -> Field {
    field(
        key,
        label,
        help,
        FieldKind::Choice {
            options,
            multiple: true,
        },
    )
}

/// A reference to a page of the site.
pub const fn page_ref(key: &'static str, label: &'static str, help: &'static str) -> Field {
    field(key, label, help, FieldKind::PageRef)
}

/// A reference to an asset.
pub const fn asset_ref(key: &'static str, label: &'static str, help: &'static str) -> Field {
    field(key, label, help, FieldKind::AssetRef)
}

/// A search query over the site's items.
pub const fn item_query(key: &'static str, label: &'static str, help: &'static str) -> Field {
    field(
        key,
        label,
        help,
        FieldKind::ItemQuery { resource_key: None },
    )
}

/// The block's attachments. `media` marks a layout whose attachments may name
/// a media; the key is not a `data` member, so it names the list itself.
pub const fn attachments(label: &'static str, help: &'static str, media: bool) -> Field {
    field("attachments", label, help, FieldKind::Attachments { media })
}

/// A repeat group whose records hold `fields`.
pub const fn repeat(label: &'static str, help: &'static str, fields: &'static [Field]) -> Field {
    field("attachments", label, help, FieldKind::Repeat { fields })
}

/// The `listOfPages` page tree.
pub const fn page_list(key: &'static str, label: &'static str, help: &'static str) -> Field {
    field(key, label, help, FieldKind::PageList)
}

/// A hex colour.
pub const fn color(key: &'static str, label: &'static str, help: &'static str) -> Field {
    field(key, label, help, FieldKind::Color)
}

/// Items picked one by one.
pub const fn item_list(key: &'static str, label: &'static str, help: &'static str) -> Field {
    field(key, label, help, FieldKind::ItemList)
}

/// A map's starting centre under `centre_key` and zoom under `zoom_key`.
pub const fn map_view(
    centre_key: &'static str,
    zoom_key: &'static str,
    label: &'static str,
    help: &'static str,
) -> Field {
    field(centre_key, label, help, FieldKind::MapView { zoom_key })
}

/// A property term.
pub const fn property(key: &'static str, label: &'static str, help: &'static str) -> Field {
    field(key, label, help, FieldKind::Property)
}

/// Four CSS lengths, one per side, edited as one control named `key`.
pub const fn sides(
    key: &'static str,
    label: &'static str,
    help: &'static str,
    keys: SideKeys,
) -> Field {
    field(key, label, help, FieldKind::Sides { keys })
}

/// A page of the site under `page_key`, or a URL under `url_key`.
pub const fn link(
    page_key: &'static str,
    url_key: &'static str,
    label: &'static str,
    help: &'static str,
) -> Field {
    field(page_key, label, help, FieldKind::Link { url_key })
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

    /// Shows the control only while `when` holds.
    pub const fn when(self, when: When) -> Self {
        Self {
            when: Some(when),
            ..self
        }
    }

    /// Puts the field in a section of the styling form.
    pub const fn group(self, group: Group) -> Self {
        Self {
            group: Some(group),
            ..self
        }
    }

    /// Tucks the field under the form's "Advanced" disclosure.
    pub const fn advanced(self) -> Self {
        Self {
            advanced: true,
            ..self
        }
    }

    /// Shows the catalogue example `key` in the empty text input. Only a text
    /// field has one, so any other kind fails the build.
    pub const fn placeholder(self, key: &'static str) -> Self {
        match self.kind {
            FieldKind::Text { .. } => Self {
                kind: FieldKind::Text {
                    placeholder: Some(key),
                },
                ..self
            },
            _ => panic!("only a text field has a placeholder"),
        }
    }

    /// Searches the resource type the sibling choice `key` names. Only an item
    /// query has one, so any other kind fails the build.
    pub const fn resource_from(self, key: &'static str) -> Self {
        match self.kind {
            FieldKind::ItemQuery { .. } => Self {
                kind: FieldKind::ItemQuery {
                    resource_key: Some(key),
                },
                ..self
            },
            _ => panic!("only an item query searches a resource type"),
        }
    }

    /// Whether the field writes the stored key `key`.
    #[must_use]
    pub fn writes(&self, key: &str) -> bool {
        match self.kind {
            FieldKind::Sides { keys } => {
                [keys.top, keys.right, keys.bottom, keys.left].contains(&key)
            }
            FieldKind::Link { url_key } => self.key == key || url_key == key,
            FieldKind::MapView { zoom_key } => self.key == key || zoom_key == key,
            _ => self.key == key,
        }
    }
}

/// `Form/BlockLayoutDataForm.php`: the styling every block's `layout_data`
/// carries, in the sections the styling form shows. `grid_column_position` and
/// `grid_column_span` come from the page editor's grid rather than that form,
/// and are declared here because `AbstractLayout` reads them. The four padding
/// keys are one [`sides`] control.
pub const BLOCK_LAYOUT_DATA: LayoutSchema = LayoutSchema {
    fields: &[
        sides(
            "padding",
            "blockLayoutData.padding",
            "fieldHelp.layout.padding",
            SideKeys {
                top: "padding_top",
                right: "padding_right",
                bottom: "padding_bottom",
                left: "padding_left",
            },
        )
        .group(Group::Spacing),
        text(
            "max_width",
            "blockLayoutData.max_width",
            "fieldHelp.layout.length",
        )
        .placeholder("fieldExample.layout.max_width")
        .group(Group::Size),
        text(
            "min_height",
            "blockLayoutData.min_height",
            "fieldHelp.layout.length",
        )
        .placeholder("fieldExample.layout.min_height")
        .group(Group::Size),
        color(
            "background_color",
            "blockLayoutData.background_color",
            "fieldHelp.layout.background_color",
        )
        .group(Group::Background),
        asset_ref(
            "background_image_asset",
            "blockLayoutData.background_image_asset",
            "fieldHelp.layout.background_image_asset",
        )
        .group(Group::Background),
        choice(
            "background_image_size",
            "blockLayoutData.background_image_size",
            "fieldHelp.layout.background_image_size",
            &[
                option("cover", "blockChoice.cover"),
                option("contain", "blockChoice.contain"),
            ],
        )
        .group(Group::Background)
        .when(When::set("background_image_asset")),
        choice(
            "background_image_position_y",
            "blockLayoutData.background_image_position_y",
            "fieldHelp.layout.background_image_position",
            &[
                option("top", "blockChoice.top"),
                option("center", "blockChoice.center"),
                option("bottom", "blockChoice.bottom"),
            ],
        )
        .group(Group::Background)
        .when(When::set("background_image_asset")),
        choice(
            "background_image_position_x",
            "blockLayoutData.background_image_position_x",
            "fieldHelp.layout.background_image_position",
            &[
                option("left", "blockChoice.left"),
                option("center", "blockChoice.center"),
                option("right", "blockChoice.right"),
            ],
        )
        .group(Group::Background)
        .when(When::set("background_image_asset")),
        choice(
            "alignment_block",
            "blockLayoutData.alignment_block",
            "fieldHelp.layout.alignment_block",
            &[
                option("left", "blockChoice.left"),
                option("center", "blockChoice.center"),
                option("right", "blockChoice.right"),
            ],
        )
        .group(Group::Alignment),
        choice(
            "alignment_text",
            "blockLayoutData.alignment_text",
            "fieldHelp.layout.alignment_text",
            &[
                option("left", "blockChoice.left"),
                option("center", "blockChoice.center"),
                option("right", "blockChoice.right"),
                option("justify", "blockChoice.justify"),
            ],
        )
        .group(Group::Alignment),
        text(
            "grid_column_position",
            "blockLayoutData.grid_column_position",
            "fieldHelp.layout.grid_column_position",
        )
        .placeholder("fieldExample.layout.grid_column_position")
        .group(Group::Grid),
        text(
            "grid_column_span",
            "blockLayoutData.grid_column_span",
            "fieldHelp.layout.grid_column_span",
        )
        .group(Group::Grid),
        text(
            "template_name",
            "blockLayoutData.template_name",
            "fieldHelp.layout.template_name",
        )
        .advanced(),
        text("class", "blockLayoutData.class", "fieldHelp.layout.class").advanced(),
    ],
};

/// `Form/PageLayoutDataForm.php`: the page's own `layout_data`. `grid_columns`
/// comes from the page editor's grid rather than that form, and is declared
/// here because `View/Helper/PageLayout::render` reads it.
pub const PAGE_LAYOUT_DATA: LayoutSchema = LayoutSchema {
    fields: &[
        text(
            "template_name",
            "pageLayoutData.template_name",
            "fieldHelp.layout.template_name",
        )
        .advanced(),
        text("class", "pageLayoutData.class", "fieldHelp.layout.class").advanced(),
        integer_min(
            "grid_columns",
            "pageLayoutData.grid_columns",
            "fieldHelp.page.grid_columns",
            0,
        )
        .group(Group::Grid),
        integer_min(
            "grid_column_gap",
            "pageLayoutData.grid_column_gap",
            "fieldHelp.page.grid_gap",
            0,
        )
        .default_int(10)
        .group(Group::Grid),
        integer_min(
            "grid_row_gap",
            "pageLayoutData.grid_row_gap",
            "fieldHelp.page.grid_gap",
            0,
        )
        .default_int(10)
        .group(Group::Grid),
    ],
};

/// One registered layout as the admin reads it.
#[derive(Debug, Serialize)]
pub struct LayoutEntry {
    /// The `site_page_block.layout` value.
    pub name: &'static str,
    /// The catalogue key for the layout's name (`layout.<name>`).
    pub label: String,
    /// The catalogue key for the one line the insert picker shows under the
    /// name (`layoutHelp.<name>`).
    pub description: String,
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
    /// The catalogue key for the picker's one-line description
    /// (`romekaHelp.<name>`).
    pub description: String,
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
            description: format!("layoutHelp.{}", layout.name()),
            schema: *layout.schema(),
        })
        .collect();
    let romeka_entries = romeka::blocks()
        .iter()
        .map(|block| RomkaEntry {
            name: block.name(),
            label: block.label(),
            description: format!("romekaHelp.{}", block.name()),
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
#[path = "schema_tests.rs"]
mod tests;
