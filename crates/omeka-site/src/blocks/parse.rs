//! The block `data` boundary: parse the stored JSON into core's [`BlockData`],
//! with errors that name the failing path and the kind the layout expects.
//!
//! Core owns the typed shape ([`omeka_core::site::BlockData`]) and its
//! `BlockData::parse`, but a serde error names the kind and the column, not the
//! key. This module walks the layout's field list first, so a caller sees
//! `block data data.limit must be a number, found a string`, and then hands the
//! value to core for the typed build.
//!
//! One upstream shape core does not accept: the `asset` layout stores its slots
//! as an object keyed by index (`asset-block-form.phtml` builds
//! `o:data[<index>][id]`, which PHP `json_encode`s as `{"0": {...}}`), while
//! core's `Vec<AssetAttachment>` expects a list. This module orders the object's
//! keys numerically and hands core the list, so a real asset block parses.

use omeka_core::ids::{AssetId, PageId};
use omeka_core::site::{AssetAttachment, BlockData, BlockLayout};
use serde_json::Value as JsonValue;

/// Why a block's `data` JSON did not parse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BlockDataError {
    /// A value has the wrong JSON kind.
    #[error("block data {path} must be {expected}, found {found}")]
    WrongKind {
        /// The JSON path, e.g. `data.limit`.
        path: String,
        /// The kind the layout expects.
        expected: &'static str,
        /// The kind the value has.
        found: &'static str,
    },
    /// Core rejected the value for a reason the kind check cannot see.
    #[error("block data {path} is invalid: {reason}")]
    Invalid {
        /// The layout whose parse failed.
        path: String,
        /// The underlying serde error.
        reason: String,
    },
    /// A layout with no `data` keys received a `data` that is not an object.
    #[error("block data must be an object, found {found}")]
    NotAnObject {
        /// The kind the value has.
        found: &'static str,
    },
}

/// One expected key.
struct Field {
    key: &'static str,
    kind: Kind,
}

/// The JSON kind a key must have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A JSON string.
    Text,
    /// A JSON number, or a numeric string as PHP forms submit.
    Int,
    /// A JSON boolean.
    Bool,
    /// A JSON list of strings.
    TextList,
    /// A JSON object.
    Object,
}

impl Kind {
    fn expected(self) -> &'static str {
        match self {
            Self::Text => "a string",
            Self::Int => "a number",
            Self::Bool => "a boolean",
            Self::TextList => "a list of strings",
            Self::Object => "an object",
        }
    }

    fn accepts(self, value: &JsonValue) -> bool {
        match self {
            Self::Text => value.is_string(),
            Self::Int => value.is_number() || value.as_str().is_some_and(is_numeric),
            Self::Bool => value.is_boolean(),
            Self::TextList => value
                .as_array()
                .is_some_and(|items| items.iter().all(JsonValue::is_string)),
            Self::Object => value.is_object(),
        }
    }
}

fn is_numeric(text: &str) -> bool {
    text.trim().parse::<i64>().is_ok()
}

/// The `data` keys each layout reads, from its class's `dataValue` calls and
/// form element names. `Null` counts as absent for every key.
fn fields(layout: &BlockLayout) -> &'static [Field] {
    const HTML: &[Field] = &[Field {
        key: "html",
        kind: Kind::Text,
    }];
    const MEDIA: &[Field] = &[
        Field {
            key: "layout",
            kind: Kind::Text,
        },
        Field {
            key: "media_display",
            kind: Kind::Text,
        },
        Field {
            key: "thumbnail_type",
            kind: Kind::Text,
        },
        Field {
            key: "show_title_option",
            kind: Kind::Text,
        },
    ];
    const BROWSE_PREVIEW: &[Field] = &[
        Field {
            key: "resource_type",
            kind: Kind::Text,
        },
        Field {
            key: "query",
            kind: Kind::Text,
        },
        Field {
            key: "heading",
            kind: Kind::Text,
        },
        Field {
            key: "limit",
            kind: Kind::Int,
        },
        Field {
            key: "components",
            kind: Kind::TextList,
        },
        Field {
            key: "link-text",
            kind: Kind::Text,
        },
    ];
    const LIST_OF_SITES: &[Field] = &[
        Field {
            key: "sort",
            kind: Kind::Text,
        },
        Field {
            key: "limit",
            kind: Kind::Int,
        },
        Field {
            key: "pagination",
            kind: Kind::Bool,
        },
        Field {
            key: "summaries",
            kind: Kind::Bool,
        },
        Field {
            key: "thumbnails",
            kind: Kind::Bool,
        },
        Field {
            key: "exclude_current",
            kind: Kind::Bool,
        },
    ];
    const TABLE_OF_CONTENTS: &[Field] = &[Field {
        key: "depth",
        kind: Kind::Int,
    }];
    const LINE_BREAK: &[Field] = &[Field {
        key: "break_type",
        kind: Kind::Text,
    }];
    const PAGE_DATE_TIME: &[Field] = &[
        Field {
            key: "display",
            kind: Kind::Text,
        },
        Field {
            key: "date_format",
            kind: Kind::Text,
        },
        Field {
            key: "time_format",
            kind: Kind::Text,
        },
    ];
    const BLOCK_GROUP: &[Field] = &[Field {
        key: "span",
        kind: Kind::Text,
    }];
    const IIIF_PRESENTATION: &[Field] = &[
        Field {
            key: "manifest_url",
            kind: Kind::Text,
        },
        Field {
            key: "title",
            kind: Kind::Text,
        },
        Field {
            key: "show_title",
            kind: Kind::Bool,
        },
    ];
    const IIIF_IMAGE: &[Field] = &[
        Field {
            key: "image_url",
            kind: Kind::Text,
        },
        Field {
            key: "title",
            kind: Kind::Text,
        },
        Field {
            key: "show_title",
            kind: Kind::Bool,
        },
    ];
    const OEMBED: &[Field] = &[
        Field {
            key: "url",
            kind: Kind::Text,
        },
        Field {
            key: "url_old",
            kind: Kind::Text,
        },
        Field {
            key: "refresh",
            kind: Kind::Bool,
        },
        Field {
            key: "oembed",
            kind: Kind::Object,
        },
    ];
    const LIST_OF_PAGES: &[Field] = &[Field {
        key: "pagelist",
        kind: Kind::Text,
    }];

    match layout {
        BlockLayout::Html => HTML,
        BlockLayout::Media => MEDIA,
        BlockLayout::BrowsePreview => BROWSE_PREVIEW,
        BlockLayout::ListOfSites => LIST_OF_SITES,
        BlockLayout::TableOfContents => TABLE_OF_CONTENTS,
        BlockLayout::LineBreak => LINE_BREAK,
        BlockLayout::PageDateTime => PAGE_DATE_TIME,
        BlockLayout::BlockGroup => BLOCK_GROUP,
        BlockLayout::IiifPresentation => IIIF_PRESENTATION,
        BlockLayout::IiifImage => IIIF_IMAGE,
        BlockLayout::Oembed => OEMBED,
        BlockLayout::ListOfPages => LIST_OF_PAGES,
        BlockLayout::ItemWithMetadata
        | BlockLayout::PageTitle
        | BlockLayout::Asset
        | BlockLayout::Fallback(_) => &[],
    }
}

/// Parse a block's `data` for its layout.
///
/// # Errors
/// [`BlockDataError`] naming the path and the expected kind.
pub fn parse_block_data(
    layout: &BlockLayout,
    data: &JsonValue,
) -> Result<BlockData, BlockDataError> {
    if matches!(layout, BlockLayout::Asset) {
        return parse_asset_slots(data);
    }
    if !data.is_object() {
        return Err(BlockDataError::NotAnObject {
            found: kind_name(data),
        });
    }
    for field in fields(layout) {
        let Some(field_value) = data.get(field.key) else {
            continue;
        };
        if field_value.is_null() || field.kind.accepts(field_value) {
            continue;
        }
        return Err(BlockDataError::WrongKind {
            path: format!("data.{}", field.key),
            expected: field.kind.expected(),
            found: kind_name(field_value),
        });
    }
    BlockData::parse(layout, data).map_err(|error| BlockDataError::Invalid {
        path: format!("{layout:?}"),
        reason: error.to_string(),
    })
}

/// Build the `asset` layout's slots without delegating to core.
///
/// The stored shape is an object keyed by slot index, and a slot's `id` and
/// `page` are empty strings when unset. Core's `AssetAttachment` is a `u64`
/// newtype that neither shape satisfies, so the boundary builds the vector in
/// slot order itself.
fn parse_asset_slots(data: &JsonValue) -> Result<BlockData, BlockDataError> {
    let slots: Vec<&JsonValue> = match data {
        JsonValue::Object(map) => {
            let mut keys: Vec<(&String, &JsonValue)> = map.iter().collect();
            keys.sort_by_key(|(key, _)| key.parse::<u64>().unwrap_or(u64::MAX));
            keys.into_iter().map(|(_, slot)| slot).collect()
        }
        JsonValue::Array(items) => items.iter().collect(),
        JsonValue::Null => Vec::new(),
        other => {
            return Err(BlockDataError::WrongKind {
                path: "data".to_string(),
                expected: "an object",
                found: kind_name(other),
            });
        }
    };
    let mut attachments = Vec::with_capacity(slots.len());
    for (index, slot) in slots.iter().enumerate() {
        let path = format!("data.{index}");
        if !slot.is_object() {
            return Err(BlockDataError::WrongKind {
                path,
                expected: "an object",
                found: kind_name(slot),
            });
        }
        attachments.push(AssetAttachment {
            id: asset_id(slot, &path, "id")?.map(AssetId::new),
            page: asset_id(slot, &path, "page")?.map(PageId::new),
            alt_link_title: optional_text(slot, &path, "alt_link_title")?,
            caption: optional_text(slot, &path, "caption")?,
        });
    }
    Ok(BlockData::Asset(attachments))
}

fn asset_id(slot: &JsonValue, path: &str, key: &str) -> Result<Option<u64>, BlockDataError> {
    match slot.get(key) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::Number(number)) => Ok(number.as_u64()),
        Some(JsonValue::String(text)) if text.is_empty() => Ok(None),
        Some(JsonValue::String(text)) => {
            text.parse::<u64>()
                .ok()
                .map(Some)
                .ok_or_else(|| BlockDataError::WrongKind {
                    path: format!("{path}.{key}"),
                    expected: "a number or a numeric string",
                    found: "a string",
                })
        }
        Some(other) => Err(BlockDataError::WrongKind {
            path: format!("{path}.{key}"),
            expected: "a number or string",
            found: kind_name(other),
        }),
    }
}

fn optional_text(
    slot: &JsonValue,
    path: &str,
    key: &str,
) -> Result<Option<String>, BlockDataError> {
    match slot.get(key) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::String(text)) => Ok(Some(text.clone())),
        Some(other) => Err(BlockDataError::WrongKind {
            path: format!("{path}.{key}"),
            expected: "a string",
            found: kind_name(other),
        }),
    }
}

fn kind_name(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "a boolean",
        JsonValue::Number(_) => "a number",
        JsonValue::String(_) => "a string",
        JsonValue::Array(_) => "a list",
        JsonValue::Object(_) => "an object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn layout(name: &str) -> BlockLayout {
        BlockLayout::from_wire_name(name)
    }

    /// Every layout's upstream `data` parses, including the asset object shape.
    #[test]
    fn parse_block_data_accepts_every_layout_data() {
        let cases: &[(&str, JsonValue)] = &[
            ("html", json!({"html": "<p>Hi</p>"})),
            (
                "media",
                json!({"layout": "horizontal", "show_title_option": "file_name"}),
            ),
            (
                "browsePreview",
                json!({"resource_type": "items", "limit": "12", "components": ["thumbnail"], "link-text": "All"}),
            ),
            (
                "listOfSites",
                json!({"sort": "newest", "limit": 5, "pagination": true}),
            ),
            ("tableOfContents", json!({"depth": 2})),
            ("lineBreak", json!({"break_type": "opaque"})),
            ("itemWithMetadata", json!({})),
            ("pageDateTime", json!({"display": "modified"})),
            ("pageTitle", json!({})),
            ("blockGroup", json!({"span": "6"})),
            (
                "iiifPresentation",
                json!({"manifest_url": "https://example.org/m", "show_title": true}),
            ),
            ("iiifImage", json!({"image_url": "https://example.org/i"})),
            (
                "oembed",
                json!({"url": "https://example.org/x", "oembed": {"type": "photo"}}),
            ),
            ("listOfPages", json!({"pagelist": "[]"})),
            (
                "asset",
                json!({"1": {"id": "4", "page": "", "alt_link_title": "", "caption": "second"}, "0": {"id": "", "caption": "first"}}),
            ),
            ("myModuleLayout", json!({"anything": [1, 2]})),
        ];
        for (name, data) in cases {
            parse_block_data(&layout(name), data)
                .unwrap_or_else(|error| panic!("{name} failed: {error}"));
        }
    }

    /// A wrong kind names the path and the expected kind.
    #[test]
    fn parse_block_data_names_the_path_and_expected_kind() {
        let error =
            parse_block_data(&layout("browsePreview"), &json!({"limit": "twelve"})).unwrap_err();
        assert_eq!(
            error.to_string(),
            "block data data.limit must be a number, found a string"
        );
        let error =
            parse_block_data(&layout("listOfSites"), &json!({"pagination": "yes"})).unwrap_err();
        assert_eq!(
            error.to_string(),
            "block data data.pagination must be a boolean, found a string"
        );
        let error = parse_block_data(
            &layout("browsePreview"),
            &json!({"components": "thumbnail"}),
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "block data data.components must be a list of strings, found a string"
        );
        let error = parse_block_data(&layout("oembed"), &json!({"oembed": "x"})).unwrap_err();
        assert_eq!(
            error.to_string(),
            "block data data.oembed must be an object, found a string"
        );
    }

    /// The asset layout orders its keyed slots and names a bad slot's path.
    #[test]
    fn parse_block_data_orders_asset_slots() {
        let parsed = parse_block_data(
            &layout("asset"),
            &json!({"2": {"caption": "c"}, "0": {"caption": "a"}, "1": {"caption": "b"}}),
        )
        .unwrap();
        let BlockData::Asset(slots) = parsed else {
            panic!("expected asset data");
        };
        let captions: Vec<&str> = slots
            .iter()
            .map(|slot| slot.caption.as_deref().unwrap_or_default())
            .collect();
        assert_eq!(captions, ["a", "b", "c"]);

        let error = parse_block_data(&layout("asset"), &json!({"0": {"caption": 3}})).unwrap_err();
        assert_eq!(
            error.to_string(),
            "block data data.0.caption must be a string, found a number"
        );
    }

    /// `Null` counts as absent, and data that is not an object is rejected for
    /// every layout but the asset slots.
    #[test]
    fn parse_block_data_treats_null_as_absent() {
        parse_block_data(&layout("listOfSites"), &json!({"limit": null})).unwrap();
        let error = parse_block_data(&layout("html"), &json!("nope")).unwrap_err();
        assert_eq!(
            error.to_string(),
            "block data must be an object, found a string"
        );
    }
}
