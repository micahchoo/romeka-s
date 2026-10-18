//! Shared JSON-LD pieces.
//!
//! Every shape here mirrors one PHP class or trait under
//! `application/src/Api/Representation/` and `application/src/DataType/`.
//! Field order is the insertion order upstream `json_encode` produces, so the
//! serializer can be plain `serde_json` over these structs.

use indexmap::IndexMap;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// `ResourceReference::jsonSerialize`: `{"@id": "...", "o:id": N}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceRef {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "o:id")]
    pub o_id: u64,
}

impl ResourceRef {
    pub fn new(id: impl Into<String>, o_id: u64) -> Self {
        Self {
            id: id.into(),
            o_id,
        }
    }
}

/// `{"@id": "..."}` for generated links such as `o:items` and `o:site_item`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiUrl {
    #[serde(rename = "@id")]
    pub id: String,
}

impl ApiUrl {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

/// One entry of the `@reverse` block,
/// `AbstractResourceEntityRepresentation::subjectValuesForReverse`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReverseRef {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "o:title")]
    pub title: Option<String>,
}

/// The JSON-LD `@type` value: a string, or a list when `getJsonLdType`
/// appends the resource class term (`AbstractResourceEntityRepresentation`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonLdType {
    Single(String),
    Many(Vec<String>),
}

impl JsonLdType {
    pub fn single(term: impl Into<String>) -> Self {
        Self::Single(term.into())
    }

    /// Upstream: primary term, plus the resource class term when one is set.
    pub fn with_resource_class(primary: impl Into<String>, class: Option<String>) -> Self {
        match class {
            Some(class) => Self::Many(vec![primary.into(), class]),
            None => Self::Single(primary.into()),
        }
    }
}

/// The fixed `@type` IRI of JSON-LD date-time value objects.
pub const XSD_DATETIME: &str = "http://www.w3.org/2001/XMLSchema#dateTime";

macro_rules! wire_const_str {
    ($(#[$meta:meta])* $name:ident = $value:expr) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name;

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str($value)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(deserializer)?;
                if raw == $value {
                    Ok(Self)
                } else {
                    Err(serde::de::Error::custom(format!(
                        "expected {:?}, got {:?}",
                        $value, raw
                    )))
                }
            }
        }
    };
}

wire_const_str!(
    /// The only IRI allowed on `o:created` / `o:modified` value objects.
    XsdDateTime = XSD_DATETIME
);

wire_const_str!(
    /// Marker for the fixed `"type": "literal"` member of a value object.
    LiteralType = "literal"
);

wire_const_str!(
    /// Marker for the fixed `"type": "uri"` member of a value object.
    UriType = "uri"
);

/// The `type` of a resource-valued value object, from
/// `Application/config/module.config.php` `data_types` and the
/// `DataType\Resource\*` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceValueType {
    #[serde(rename = "resource")]
    All,
    #[serde(rename = "resource:item")]
    Item,
    #[serde(rename = "resource:itemset")]
    ItemSet,
    #[serde(rename = "resource:media")]
    Media,
}

/// `{"@value": <ISO 8601>, "@type": "xsd:dateTime"}`.
///
/// Used by `AbstractResourceEntityRepresentation::getJsonLd`,
/// `SiteRepresentation::getJsonLd`, `SitePageRepresentation::getJsonLd`,
/// and `JobRepresentation::getJsonLd`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonLdDate {
    #[serde(rename = "@value")]
    pub value: String,
    #[serde(rename = "@type")]
    pub type_: XsdDateTime,
}

impl JsonLdDate {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            type_: XsdDateTime,
        }
    }
}

/// Value objects keyed by term, in the order upstream `values()` returns them.
pub type ValueMap = IndexMap<String, Vec<Value>>;

/// `ValueRepresentation::jsonSerialize`, split by the three data type shapes
/// under `application/src/DataType/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    /// Tried first: a resource value has `@id` plus `value_resource_id`, so a
    /// looser variant would swallow it.
    Resource(ValueResource),
    Uri(ValueUri),
    Literal(ValueLiteral),
}

impl Value {
    pub fn is_public(&self) -> bool {
        match self {
            Value::Resource(v) => v.is_public,
            Value::Uri(v) => v.is_public,
            Value::Literal(v) => v.is_public,
        }
    }
}

/// `DataType\Literal::getJsonLd`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueLiteral {
    #[serde(rename = "type")]
    pub data_type: LiteralType,
    pub property_id: u64,
    pub property_label: String,
    pub is_public: bool,
    #[serde(
        rename = "@annotation",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub annotation: Option<ValueMap>,
    #[serde(rename = "@value")]
    pub value: String,
    #[serde(rename = "@language", default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// `DataType\Uri::getJsonLd`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueUri {
    #[serde(rename = "type")]
    pub data_type: UriType,
    pub property_id: u64,
    pub property_label: String,
    pub is_public: bool,
    #[serde(
        rename = "@annotation",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub annotation: Option<ValueMap>,
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "o:label", default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(rename = "o:lang", default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

/// `AbstractResourceEntityRepresentation::valueRepresentation` after the
/// `$valueObject + $jsonLd` union in `ValueRepresentation::jsonSerialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueResource {
    #[serde(rename = "type")]
    pub data_type: ResourceValueType,
    pub property_id: u64,
    pub property_label: String,
    pub is_public: bool,
    #[serde(
        rename = "@annotation",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub annotation: Option<ValueMap>,
    #[serde(rename = "@id")]
    pub id: String,
    pub value_resource_id: u64,
    pub value_resource_name: String,
    pub url: Option<String>,
    pub display_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_type: Option<String>,
}

/// `AbstractRepresentation::thumbnailDisplayUrls` keys, from the `thumbnails`
/// config in `application/config/module.config.php`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThumbnailDisplayUrls {
    pub large: Option<String>,
    pub medium: Option<String>,
    pub square: Option<String>,
}

/// `MediaRepresentation::thumbnailUrls()`: a map keyed by thumbnail type, or
/// the PHP empty array when the media has no thumbnails.
///
/// An empty Rust map serializes as `{}`, which is not what upstream emits:
/// `thumbnailUrls()` returns `[]`, and PHP `json_encode` writes that as `[]`.
/// The two forms are not interchangeable on the wire, so this type owns both.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThumbnailUrls(pub IndexMap<String, String>);

impl ThumbnailUrls {
    pub fn empty() -> Self {
        Self(IndexMap::new())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn insert(&mut self, kind: &str, url: String) {
        self.0.insert(kind.to_string(), url);
    }

    pub fn get(&self, kind: &str) -> Option<&String> {
        self.0.get(kind)
    }
}

impl Serialize for ThumbnailUrls {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.0.is_empty() {
            serializer.serialize_seq(Some(0))?.end()
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for ThumbnailUrls {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UrlsVisitor;

        impl<'de> serde::de::Visitor<'de> for UrlsVisitor {
            type Value = ThumbnailUrls;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a thumbnail URL map, or an empty list")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
                Ok(ThumbnailUrls::empty())
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut urls = IndexMap::new();
                while let Some((kind, url)) = map.next_entry::<String, String>()? {
                    urls.insert(kind, url);
                }
                Ok(ThumbnailUrls(urls))
            }
        }

        deserializer.deserialize_any(UrlsVisitor)
    }
}

/// One entry of a site's stored navigation JSON (`o:navigation`).
///
/// Stored shape from `SiteAdapter::getDefaultNavigation` and
/// `Site\Navigation\Translator`: `type`, `data`, `links`. The `data` members
/// differ per link type, so they stay untyped until the site unit owns them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavLink {
    #[serde(rename = "type")]
    pub kind: String,
    pub data: serde_json::Value,
    #[serde(default)]
    pub links: Vec<NavLink>,
}
