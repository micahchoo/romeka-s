//! Values: the types a value is written as, and the cell rules every reader
//! of a sheet or a decision shares (`docs/import-v2.md`, "Values" and "Slot
//! rules a worker must not guess").

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use super::plan::RecordKind;

/// What a value is written as.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "ValueTypeWire", into = "ValueTypeWire")]
pub enum ValueType {
    Literal,
    /// A cell `http://x.org/a Label` gives the URI and the label.
    Uri,
    /// A link to another resource.
    Resource(ResourceBy),
    /// Any data type the install knows, by its name.
    DataType(String),
}

/// How a `resource` value names the resource it links to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceBy {
    /// In the match scope, then among this import's records.
    Identifier,
    InternalId,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum ValueTypeWire {
    Simple(SimpleType),
    Resource(ResourceWire),
    DataType(DataTypeWire),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum SimpleType {
    Literal,
    Uri,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceWire {
    resource: ResourceBy,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DataTypeWire {
    data_type: String,
}

impl TryFrom<ValueTypeWire> for ValueType {
    type Error = String;

    fn try_from(wire: ValueTypeWire) -> Result<Self, String> {
        Ok(match wire {
            ValueTypeWire::Simple(SimpleType::Literal) => Self::Literal,
            ValueTypeWire::Simple(SimpleType::Uri) => Self::Uri,
            ValueTypeWire::Resource(ResourceWire { resource }) => Self::Resource(resource),
            ValueTypeWire::DataType(DataTypeWire { data_type }) if data_type.trim().is_empty() => {
                return Err("a dataType names a data type".to_string());
            }
            ValueTypeWire::DataType(DataTypeWire { data_type }) => Self::DataType(data_type),
        })
    }
}

impl From<ValueType> for ValueTypeWire {
    fn from(value: ValueType) -> Self {
        match value {
            ValueType::Literal => Self::Simple(SimpleType::Literal),
            ValueType::Uri => Self::Simple(SimpleType::Uri),
            ValueType::Resource(resource) => Self::Resource(ResourceWire { resource }),
            ValueType::DataType(data_type) => Self::DataType(DataTypeWire { data_type }),
        }
    }
}

/// A value as the run writes it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Value {
    #[serde(rename = "type")]
    pub value_type: ValueType,
    pub text: String,
    pub lang: Option<String>,
    pub label: Option<String>,
    pub is_public: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Visibility {
    Public,
    Private,
}

impl Visibility {
    /// A visibility cell: `false`, `no`, `off`, `0` and `private` mean
    /// private, any other text public. An empty cell is no value.
    #[must_use]
    pub fn from_cell(cell: &str) -> Option<Self> {
        let cell = cell.trim().to_lowercase();
        match cell.as_str() {
            "" => None,
            "false" | "no" | "off" | "0" | "private" => Some(Self::Private),
            _ => Some(Self::Public),
        }
    }
}

/// A kind-column cell, case-insensitive. `None` flags `unknown-kind`.
#[must_use]
pub fn row_kind_from_cell(cell: &str) -> Option<RecordKind> {
    match cell.trim().to_lowercase().as_str() {
        "item" | "items" => Some(RecordKind::Item),
        "media" => Some(RecordKind::Media),
        "item set" | "itemset" | "item sets" | "itemsets" => Some(RecordKind::ItemSet),
        _ => None,
    }
}

/// A `uri` cell's URI and label: the text up to the first space, and the
/// rest as the label (upstream CSVImport's rule).
#[must_use]
pub fn uri_and_label(cell: &str) -> (&str, Option<&str>) {
    let cell = cell.trim();
    match cell.split_once(' ') {
        Some((uri, label)) if !label.trim().is_empty() => (uri, Some(label.trim())),
        Some((uri, _)) => (uri, None),
        None => (cell, None),
    }
}

/// The form titles and labels compare in: trimmed, NFC, lower case.
///
/// Lower case stands in for Unicode case folding, which no crate in the
/// workspace provides; the two differ on a few letters such as `ß`.
#[must_use]
pub fn fold_title(text: &str) -> String {
    text.trim().nfc().collect::<String>().to_lowercase()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use serde_json::json;

    #[test]
    fn value_types_read_and_write_the_contract_shapes() {
        let shapes = [
            (json!("literal"), ValueType::Literal),
            (json!("uri"), ValueType::Uri),
            (
                json!({ "resource": "identifier" }),
                ValueType::Resource(ResourceBy::Identifier),
            ),
            (
                json!({ "resource": "internalId" }),
                ValueType::Resource(ResourceBy::InternalId),
            ),
            (
                json!({ "dataType": "numeric:timestamp" }),
                ValueType::DataType("numeric:timestamp".into()),
            ),
        ];
        for (wire, typed) in shapes {
            assert_eq!(
                serde_json::from_value::<ValueType>(wire.clone()).ok(),
                Some(typed.clone())
            );
            assert_eq!(serde_json::to_value(&typed).expect("json"), wire);
        }
        for bad in [
            json!("html"),
            json!({ "resource": "title" }),
            json!({ "dataType": "" }),
            json!({ "dataType": "x", "y": 1 }),
        ] {
            assert!(
                serde_json::from_value::<ValueType>(bad.clone()).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn visibility_cells_follow_the_contract() {
        for private in ["false", "No", " off ", "0", "PRIVATE"] {
            assert_eq!(
                Visibility::from_cell(private),
                Some(Visibility::Private),
                "{private}"
            );
        }
        for public in ["public", "yes", "1", "true"] {
            assert_eq!(
                Visibility::from_cell(public),
                Some(Visibility::Public),
                "{public}"
            );
        }
        assert_eq!(Visibility::from_cell("  "), None);
    }

    #[test]
    fn row_kinds_are_the_eight_spellings_in_any_case() {
        let kinds = [
            ("item", RecordKind::Item),
            ("Items", RecordKind::Item),
            ("MEDIA", RecordKind::Media),
            ("item set", RecordKind::ItemSet),
            ("ItemSet", RecordKind::ItemSet),
            ("item sets", RecordKind::ItemSet),
            ("itemsets", RecordKind::ItemSet),
        ];
        for (cell, kind) in kinds {
            assert_eq!(row_kind_from_cell(cell), Some(kind), "{cell}");
        }
        for unknown in ["", "set", "file", "item-set"] {
            assert_eq!(row_kind_from_cell(unknown), None, "{unknown}");
        }
    }

    #[test]
    fn a_uri_cell_gives_the_uri_and_its_label() {
        assert_eq!(
            uri_and_label("http://x.org/a Label"),
            ("http://x.org/a", Some("Label"))
        );
        assert_eq!(
            uri_and_label(" http://x.org/a  Two words "),
            ("http://x.org/a", Some("Two words"))
        );
        assert_eq!(uri_and_label("http://x.org/a"), ("http://x.org/a", None));
    }

    #[test]
    fn titles_compare_trimmed_nfc_and_lower_case() {
        assert_eq!(
            fold_title(" Cafe\u{301} Photos "),
            fold_title("CAFÉ PHOTOS")
        );
        assert_ne!(fold_title("Photos"), fold_title("Photo"));
    }
}
