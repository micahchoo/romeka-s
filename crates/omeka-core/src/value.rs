//! Resource values: the object of an RDF triple, with upstream's validity rules.
//!
//! Mirrors `application/src/Entity/Value.php` (the column set and defaults),
//! `application/src/DataType/Literal.php` and `.../Uri.php` (validity),
//! `application/src/Api/Adapter/ValueHydrator.php` (which keys the API reads),
//! and the language tag rule from `application/asset/js/global.js` (`langIsValid`),
//! which is the only language tag check upstream runs on a value.

use serde::{Deserialize, Serialize};

use crate::ids::{AnnotationId, PropertyId, ResourceId, ValueId};
use crate::resource::ResourceKind;

/// Why a value or one of its parts was rejected at a boundary.
///
/// Upstream rejects an invalid value by silently skipping it
/// (`ValueHydrator::hydrate`, "Ignore an invalid value."). The port parses at the
/// boundary instead, so the caller sees the reason.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValueError {
    /// A literal must be a string that is not empty or whitespace
    /// (`Literal::literalIsValid`).
    #[error("a literal value must not be empty or whitespace")]
    EmptyLiteral,
    /// A URI fails `Uri::uriIsValid`.
    #[error("invalid URI: {0}")]
    InvalidUri(String),
    /// A language tag fails the BCP 47 subset upstream accepts
    /// (`global.js` `langIsValid`).
    #[error("invalid language tag: {0}")]
    InvalidLangTag(String),
    /// A resource value refers to a resource kind the data type rejects
    /// (`DataType\Resource\AbstractResource::hydrate`).
    #[error("data type {data_type} does not accept a {kind:?} value resource")]
    ResourceKindMismatch {
        /// The data type that rejected the resource.
        data_type: String,
        /// The kind of the referenced resource.
        kind: ResourceKind,
    },
    /// A resource value has no id to point at.
    #[error("data type {0} requires a value_resource_id")]
    MissingResourceId(String),
    /// A value object has no data type, or names one the registry does not know
    /// (`ValueHydrator`: an unknown type is skipped).
    #[error("unknown data type: {0}")]
    UnknownDataType(String),
    /// A value object has no `property_id`, which `ValueHydrator` requires.
    #[error("a value object must have a property_id")]
    MissingPropertyId,
}

/// A language tag validated against the BCP 47 subset upstream accepts.
///
/// Upstream rule: `Omeka.langIsValid` in `application/asset/js/global.js`, the
/// regex that also backs the "Please enter a valid language tag" message in
/// `application/config/module.config.php`. Validated once at parse, trusted
/// downstream.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LangTag(String);

impl LangTag {
    /// Parse and validate a language tag.
    ///
    /// # Errors
    /// Returns [`ValueError::InvalidLangTag`] when the tag is empty or does not
    /// match the upstream pattern.
    pub fn parse(tag: &str) -> Result<Self, ValueError> {
        if lang_tag_is_valid(tag) {
            Ok(Self(tag.to_string()))
        } else {
            Err(ValueError::InvalidLangTag(tag.to_string()))
        }
    }

    /// The tag as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for LangTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for LangTag {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for LangTag {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// The language portion of the upstream regex accepts 2 or 3 ASCII letters
/// (the `[A-Za-z]{4}|[A-Za-z]{5,8}` alternatives were removed upstream because
/// they do not represent current usage; see the comment above `langIsValid`).
fn is_alpha(s: &str, min: usize, max: usize) -> bool {
    let len = s.chars().count();
    (min..=max).contains(&len) && s.chars().all(|c| c.is_ascii_alphabetic())
}

fn is_alnum(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric())
}

fn len_between(s: &str, min: usize, max: usize) -> bool {
    let len = s.chars().count();
    (min..=max).contains(&len)
}

/// Upstream's one exception: the extension singleton charset, which is every
/// alphanumeric character except `x` and `X` (`global.js`, `[0-9A-WY-Za-wy-z]`).
fn is_extension_singleton(s: &str) -> bool {
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => {
            c.is_ascii_digit() || (c.is_ascii_alphabetic() && !matches!(c, 'x' | 'X'))
        }
        _ => false,
    }
}

/// The language tag grammar from `application/asset/js/global.js` `langIsValid`,
/// which is case sensitive: the grandfathered alternatives are literal strings
/// and the private-use introducer is lowercase `x` only.
#[must_use]
pub fn lang_tag_is_valid(tag: &str) -> bool {
    const GRANDFATHERED_IRREGULAR: &[&str] = &[
        "en-GB-oed",
        "i-ami",
        "i-bnn",
        "i-default",
        "i-enochian",
        "i-hak",
        "i-klingon",
        "i-lux",
        "i-mingo",
        "i-navajo",
        "i-pwn",
        "i-tao",
        "i-tay",
        "i-tsu",
        "sgn-BE-FR",
        "sgn-BE-NL",
        "sgn-CH-DE",
    ];
    const GRANDFATHERED_REGULAR: &[&str] = &[
        "art-lojban",
        "cel-gaulish",
        "no-bok",
        "no-nyn",
        "zh-guoyu",
        "zh-hakka",
        "zh-min",
        "zh-min-nan",
        "zh-xiang",
    ];
    if GRANDFATHERED_IRREGULAR.contains(&tag) || GRANDFATHERED_REGULAR.contains(&tag) {
        return true;
    }

    let mut parts: Vec<&str> = tag.split('-').collect();
    if parts.first() == Some(&"") || tag.is_empty() {
        return false;
    }

    // `x(-[A-Za-z0-9]{1,8})+` on its own.
    if parts[0] == "x" {
        let rest = &parts[1..];
        return !rest.is_empty() && rest.iter().all(|p| len_between(p, 1, 8) && is_alnum(p));
    }

    // `[A-Za-z]{2,3}` primary language, then up to three `[A-Za-z]{3}` extlangs.
    let Some(primary) = parts.first() else {
        return false;
    };
    if !is_alpha(primary, 2, 3) {
        return false;
    }
    parts.remove(0);
    let mut extlangs = 0;
    while extlangs < 3 && parts.first().is_some_and(|p| is_alpha(p, 3, 3)) {
        parts.remove(0);
        extlangs += 1;
    }

    // `(-([A-Za-z]{4}))?` script.
    if parts.first().is_some_and(|p| is_alpha(p, 4, 4)) {
        parts.remove(0);
    }

    // `(-([A-Za-z]{2}|[0-9]{3}))?` region.
    if parts.first().is_some_and(|p| {
        is_alpha(p, 2, 2) || (p.len() == 3 && p.chars().all(|c| c.is_ascii_digit()))
    }) {
        parts.remove(0);
    }

    // `(-([A-Za-z0-9]{5,8}|[0-9][A-Za-z0-9]{3}))*` variants.
    while parts.first().is_some_and(|p| {
        (len_between(p, 5, 8) && is_alnum(p))
            || (p.chars().count() == 4
                && p.starts_with(|c: char| c.is_ascii_digit())
                && is_alnum(p))
    }) {
        parts.remove(0);
    }

    // `(-([0-9A-WY-Za-wy-z](-[A-Za-z0-9]{2,8})+))*` extensions, 1+ subtags each.
    while parts.first().is_some_and(|p| is_extension_singleton(p)) {
        parts.remove(0);
        let mut subtags = 0;
        while parts
            .first()
            .is_some_and(|p| len_between(p, 2, 8) && is_alnum(p))
        {
            parts.remove(0);
            subtags += 1;
        }
        if subtags == 0 {
            return false;
        }
    }

    // `(-(x(-[A-Za-z0-9]{1,8})+))?` private use.
    if parts.first() == Some(&"x") {
        parts.remove(0);
        let mut subtags = 0;
        while parts
            .first()
            .is_some_and(|p| len_between(p, 1, 8) && is_alnum(p))
        {
            parts.remove(0);
            subtags += 1;
        }
        if subtags == 0 {
            return false;
        }
    }

    parts.is_empty()
}

/// Upstream `Literal::literalIsValid`: a string that is not empty once trimmed.
#[must_use]
pub fn literal_is_valid(literal: &str) -> bool {
    !literal.trim().is_empty()
}

/// Upstream `Uri::uriIsValid`: trim, drop tabs and newlines, reject empty, any
/// control character, and a `javascript` scheme.
#[must_use]
pub fn uri_is_valid(uri: &str) -> bool {
    let cleaned: String = uri
        .trim()
        .chars()
        .filter(|c| !matches!(c, '\t' | '\r' | '\n'))
        .collect();
    if cleaned.is_empty() {
        return false;
    }
    if cleaned.chars().any(|c| c.is_control()) {
        return false;
    }
    !scheme_is_javascript(&cleaned)
}

/// `parse_url($uri, PHP_URL_SCHEME)` lowered: a scheme is the RFC 3986 prefix
/// before `:`, or nothing at all.
fn scheme_is_javascript(uri: &str) -> bool {
    let Some((scheme, _)) = uri.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    let valid = match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {
            chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        }
        _ => false,
    };
    valid && scheme.eq_ignore_ascii_case("javascript")
}

/// The data type names upstream stores for resource values
/// (`application/src/DataType/Resource/*.php`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ResourceDataType {
    /// `resource:item`
    Item,
    /// `resource:itemset`
    ItemSet,
    /// `resource:media`
    Media,
    /// `resource`, the "All" data type
    Any,
}

impl ResourceDataType {
    /// The upstream data type name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Item => "resource:item",
            Self::ItemSet => "resource:itemset",
            Self::Media => "resource:media",
            Self::Any => "resource",
        }
    }

    /// Parse an upstream data type name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "resource:item" => Some(Self::Item),
            "resource:itemset" => Some(Self::ItemSet),
            "resource:media" => Some(Self::Media),
            "resource" => Some(Self::Any),
            _ => None,
        }
    }

    /// The kinds this data type accepts, mirroring
    /// `AbstractResource::getValidValueResources` per subclass:
    /// Item accepts items, ItemSet accepts item sets, Media accepts media,
    /// All accepts all three.
    #[must_use]
    pub const fn accepts(self, kind: ResourceKind) -> bool {
        match self {
            Self::Item => matches!(kind, ResourceKind::Item),
            Self::ItemSet => matches!(kind, ResourceKind::ItemSet),
            Self::Media => matches!(kind, ResourceKind::Media),
            Self::Any => true,
        }
    }

    /// The kind this data type pins, if any. `resource` (All) pins nothing.
    #[must_use]
    pub const fn kind(self) -> Option<ResourceKind> {
        match self {
            Self::Item => Some(ResourceKind::Item),
            Self::ItemSet => Some(ResourceKind::ItemSet),
            Self::Media => Some(ResourceKind::Media),
            Self::Any => None,
        }
    }
}

/// The object of an RDF triple, in the port's typed form (DESIGN.md).
///
/// Upstream keeps all three shapes in one row with nullable `value`, `uri` and
/// `value_resource_id` columns plus a `type` string
/// (`application/data/install/schema.sql:348`, `application/src/Entity/Value.php`).
/// The variant here makes the "exactly one of them is set" rule a type.
///
/// The serde derive is the internal representation, not the API wire shape:
/// the wire shape is [`ValueObject`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueData {
    /// A literal: `@value` on the wire, not empty or whitespace
    /// (`Literal::literalIsValid`).
    Literal {
        /// The literal text.
        text: String,
    },
    /// A URI: `@id` on the wire plus the optional `o:label` stored upstream in
    /// the `value` column (`Uri::hydrate`).
    Uri {
        /// The URI.
        uri: String,
        /// The optional label (`o:label`).
        label: Option<String>,
    },
    /// A value that points at another resource: `value_resource_id` on the wire
    /// plus the data type name that limits its kind.
    Resource {
        /// The referenced resource.
        id: ResourceId,
        /// The data type the value was stored under.
        data_type: ResourceDataType,
    },
}

impl ValueData {
    /// A literal, rejected when empty or whitespace-only.
    ///
    /// # Errors
    /// [`ValueError::EmptyLiteral`].
    pub fn literal(text: impl Into<String>) -> Result<Self, ValueError> {
        let text = text.into();
        if literal_is_valid(&text) {
            Ok(Self::Literal { text })
        } else {
            Err(ValueError::EmptyLiteral)
        }
    }

    /// A URI with no label.
    ///
    /// # Errors
    /// [`ValueError::InvalidUri`].
    pub fn uri(uri: impl Into<String>) -> Result<Self, ValueError> {
        Self::uri_with_label(uri, None)
    }

    /// A URI with an optional label.
    ///
    /// # Errors
    /// [`ValueError::InvalidUri`].
    pub fn uri_with_label(
        uri: impl Into<String>,
        label: Option<String>,
    ) -> Result<Self, ValueError> {
        let uri = uri.into();
        if uri_is_valid(&uri) {
            Ok(Self::Uri { uri, label })
        } else {
            Err(ValueError::InvalidUri(uri))
        }
    }

    /// A resource value. The caller checks the referenced resource's kind
    /// against `data_type` with [`ResourceDataType::accepts`] once it has the row.
    #[must_use]
    pub const fn resource(id: ResourceId, data_type: ResourceDataType) -> Self {
        Self::Resource { id, data_type }
    }

    /// Upstream's stored `type` column (`ValueHydrator::hydrate`).
    #[must_use]
    pub const fn data_type_name(&self) -> &'static str {
        match self {
            Self::Literal { .. } => "literal",
            Self::Uri { .. } => "uri",
            Self::Resource { data_type, .. } => data_type.name(),
        }
    }

    /// The value column upstream stores: the literal, or a URI value's label.
    #[must_use]
    pub fn stored_value(&self) -> Option<&str> {
        match self {
            Self::Literal { text } => Some(text),
            Self::Uri { label, .. } => label.as_deref(),
            Self::Resource { .. } => None,
        }
    }

    /// The URI column upstream stores.
    #[must_use]
    pub fn stored_uri(&self) -> Option<&str> {
        match self {
            Self::Uri { uri, .. } => Some(uri),
            Self::Literal { .. } | Self::Resource { .. } => None,
        }
    }
}

/// A property reference in a value object. Upstream accepts a numeric id or the
/// string `auto`, which the hydrator resolves from the enclosing term
/// (`ValueHydrator::hydrate`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyRef {
    /// `property_id: <n>`
    Id(PropertyId),
    /// `property_id: "auto"`
    Auto,
}

impl Serialize for PropertyRef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Id(id) => serializer.serialize_u64(id.get()),
            Self::Auto => serializer.serialize_str("auto"),
        }
    }
}

impl<'de> Deserialize<'de> for PropertyRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Id(u64),
            Name(String),
        }
        match Raw::deserialize(deserializer)? {
            Raw::Id(id) => Ok(Self::Id(PropertyId::new(id))),
            Raw::Name(name) if name == "auto" => Ok(Self::Auto),
            Raw::Name(other) => Err(serde::de::Error::custom(format!(
                "property_id must be a number or \"auto\", got \"{other}\""
            ))),
        }
    }
}

/// The value object the API reads and writes, key for key
/// (`application/src/Api/Adapter/ValueHydrator.php` for input,
/// `application/src/Api/Representation/ValueRepresentation.php` for output).
///
/// Unknown keys are ignored, matching upstream, which only reads the keys it
/// knows. Parse this at the HTTP boundary, never inside the domain.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ValueObject {
    /// The data type name (`type`).
    #[serde(rename = "type", skip_serializing_if = "Option::is_none", default)]
    pub data_type: Option<String>,
    /// The property this value belongs to (`property_id`).
    #[serde(
        rename = "property_id",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub property: Option<PropertyRef>,
    /// The property label, written on output (`property_label`).
    #[serde(
        rename = "property_label",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub property_label: Option<String>,
    /// A literal's text (`@value`).
    #[serde(rename = "@value", skip_serializing_if = "Option::is_none", default)]
    pub value: Option<String>,
    /// A literal's language (`@language`).
    #[serde(rename = "@language", skip_serializing_if = "Option::is_none", default)]
    pub language: Option<String>,
    /// A URI (`@id`).
    #[serde(rename = "@id", skip_serializing_if = "Option::is_none", default)]
    pub id: Option<String>,
    /// A URI value's label (`o:label`).
    #[serde(rename = "o:label", skip_serializing_if = "Option::is_none", default)]
    pub label: Option<String>,
    /// A URI value's language (`o:lang`).
    #[serde(rename = "o:lang", skip_serializing_if = "Option::is_none", default)]
    pub lang: Option<String>,
    /// A resource value's target (`value_resource_id`).
    #[serde(
        rename = "value_resource_id",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub value_resource_id: Option<ResourceId>,
    /// Visibility (`is_public`); upstream defaults to true
    /// (`Value::$isPublic = true`, set only when the key is present).
    #[serde(rename = "is_public", skip_serializing_if = "Option::is_none", default)]
    pub is_public: Option<bool>,
}

impl ValueObject {
    /// Parse a value object from JSON. The JSON is external data, so this is the
    /// only place shape errors surface.
    ///
    /// # Errors
    /// A serde error naming the offending key.
    pub fn parse(json: &serde_json::Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(json.clone())
    }
}

/// A value, representing the object in an RDF triple
/// (`application/src/Entity/Value.php`).
///
/// The serde derive is the internal representation; the API wire shape is
/// [`ValueObject`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Value {
    /// `value.id`; absent until stored.
    pub id: Option<ValueId>,
    /// `value.resource_id`, the subject.
    pub resource: ResourceId,
    /// `value.property_id`, the predicate.
    pub property: PropertyId,
    /// The object, typed.
    pub data: ValueData,
    /// `value.lang`, validated (`@language` for literals, `o:lang` for URIs).
    pub lang: Option<LangTag>,
    /// `value.is_public`, default true upstream.
    pub is_public: bool,
    /// `value.value_annotation_id`.
    pub annotation: Option<AnnotationId>,
}

impl Value {
    /// A public, unannotated value.
    #[must_use]
    pub const fn new(resource: ResourceId, property: PropertyId, data: ValueData) -> Self {
        Self {
            id: None,
            resource,
            property,
            data,
            lang: None,
            is_public: true,
            annotation: None,
        }
    }

    /// Set the language tag.
    #[must_use]
    pub fn with_lang(mut self, lang: LangTag) -> Self {
        self.lang = Some(lang);
        self
    }

    /// Set visibility.
    #[must_use]
    pub fn with_public(mut self, is_public: bool) -> Self {
        self.is_public = is_public;
        self
    }

    /// The value object the API writes for this value, key for key
    /// (`ValueRepresentation::jsonSerialize` plus the data type's `getJsonLd`).
    #[must_use]
    pub fn to_object(&self) -> ValueObject {
        let mut object = ValueObject {
            data_type: Some(self.data.data_type_name().to_string()),
            property: Some(PropertyRef::Id(self.property)),
            is_public: Some(self.is_public),
            ..ValueObject::default()
        };
        match &self.data {
            ValueData::Literal { text } => {
                object.value = Some(text.clone());
                object.language = self.lang.as_ref().map(|tag| tag.as_str().to_string());
            }
            ValueData::Uri { uri, label } => {
                object.id = Some(uri.clone());
                object.label = label.clone();
                object.lang = self.lang.as_ref().map(|tag| tag.as_str().to_string());
            }
            ValueData::Resource { id, .. } => {
                object.value_resource_id = Some(*id);
            }
        }
        object
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The accepted tags are the ones `Omeka.langIsValid` accepts; the pattern
    /// is copied from `application/asset/js/global.js` (Comment above the
    /// function documents the `[A-Za-z]{4}|[A-Za-z]{5,8}` removal).
    #[test]
    fn value_lang_tag_accepts_upstream_global_js_valid_tags() {
        for tag in [
            "en",
            "en-US",
            "en-GB-oed",
            "i-klingon",
            "zh-hakka",
            "zh-min-nan",
            "x-private",
            "en-x-private",
            "en-a-bbb",
            "de-DE-1996",
            "sr-Latn-RS",
            "en-US-u-ca-gregory",
            "en-abc-def-ghi",
        ] {
            assert!(
                LangTag::parse(tag).is_ok(),
                "{tag} should be a valid language tag per global.js langIsValid"
            );
        }
    }

    /// Mirrors the removals and case sensitivity of the upstream regex.
    #[test]
    fn value_lang_tag_rejects_upstream_global_js_invalid_tags() {
        for tag in [
            "",
            "e",
            "english",
            "en--US",
            "en-",
            "-en",
            "en US",
            "123",
            "en-gb-oed",
            "en-X-private",
            "en-a",
            "en-x",
            "x",
            "en-Latn-abc",
            "en-abc-def-ghi-jkl",
        ] {
            assert!(
                LangTag::parse(tag).is_err(),
                "{tag} should be rejected per global.js langIsValid"
            );
        }
    }

    #[test]
    fn value_lang_tag_serde_rejects_invalid_input() {
        assert!(serde_json::from_str::<LangTag>("\"en-US\"").is_ok());
        assert!(serde_json::from_str::<LangTag>("\"english\"").is_err());
        assert_eq!(
            serde_json::to_string(&LangTag::parse("en-US").unwrap()).unwrap(),
            "\"en-US\""
        );
    }

    /// `Literal::literalIsValid` in application/src/DataType/Literal.php.
    #[test]
    fn value_literal_validity_matches_upstream_literal_datatype() {
        assert!(literal_is_valid("text"));
        assert!(literal_is_valid("  text  "));
        assert!(!literal_is_valid(""));
        assert!(!literal_is_valid("   "));
        assert!(!literal_is_valid("\t\n"));
    }

    /// `Uri::uriIsValid` in application/src/DataType/Uri.php.
    #[test]
    fn value_uri_validity_matches_upstream_uri_datatype() {
        assert!(uri_is_valid("https://example.org/"));
        assert!(uri_is_valid("  https://example.org/a\tb\r\n"));
        assert!(!uri_is_valid(""));
        assert!(!uri_is_valid("   "));
        assert!(!uri_is_valid("JAvaScRiPt:alert(1)"));
        assert!(!uri_is_valid("javascript:alert(1)"));
        assert!(!uri_is_valid("https://example.org/\u{1}"));
    }

    /// `Value::$isPublic = true`; `ValueHydrator` only overrides it when the key
    /// is present.
    #[test]
    fn value_defaults_match_upstream_value_entity() {
        let value = Value::new(
            ResourceId::new(1),
            PropertyId::new(2),
            ValueData::literal("text").unwrap(),
        );
        assert!(value.is_public);
        assert_eq!(value.lang, None);
        assert_eq!(value.annotation, None);
        assert_eq!(value.id, None);
    }

    /// The output keys are the ones `ValueRepresentation::jsonSerialize` and the
    /// data types' `getJsonLd` write: `type`, `@value`/`@language` for literals,
    /// `@id`/`o:label`/`o:lang` for URIs, `value_resource_id` for resources.
    #[test]
    fn value_object_roundtrip_matches_upstream_json_keys() {
        let literal = Value::new(
            ResourceId::new(1),
            PropertyId::new(2),
            ValueData::literal("a text").unwrap(),
        )
        .with_lang(LangTag::parse("en-US").unwrap());
        let json = serde_json::to_value(literal.to_object()).unwrap();
        assert_eq!(json["type"], "literal");
        assert_eq!(json["property_id"], 2);
        assert_eq!(json["@value"], "a text");
        assert_eq!(json["@language"], "en-US");
        assert_eq!(json["is_public"], true);

        let uri = Value::new(
            ResourceId::new(1),
            PropertyId::new(3),
            ValueData::uri_with_label("https://example.org/", Some("Example".into())).unwrap(),
        )
        .with_lang(LangTag::parse("fr").unwrap())
        .with_public(false);
        let json = serde_json::to_value(uri.to_object()).unwrap();
        assert_eq!(json["type"], "uri");
        assert_eq!(json["@id"], "https://example.org/");
        assert_eq!(json["o:label"], "Example");
        assert_eq!(json["o:lang"], "fr");
        assert_eq!(json["is_public"], false);

        let resource = Value::new(
            ResourceId::new(1),
            PropertyId::new(4),
            ValueData::resource(ResourceId::new(99), ResourceDataType::Item),
        );
        let json = serde_json::to_value(resource.to_object()).unwrap();
        assert_eq!(json["type"], "resource:item");
        assert_eq!(json["value_resource_id"], 99);

        // The same JSON parses back to the same object shape.
        let parsed = ValueObject::parse(&json).unwrap();
        assert_eq!(parsed.data_type.as_deref(), Some("resource:item"));
        assert_eq!(parsed.value_resource_id, Some(ResourceId::new(99)));
        assert_eq!(parsed.property, Some(PropertyRef::Id(PropertyId::new(4))));
    }

    /// `ValueHydrator` accepts `property_id: "auto"` and resolves it from the
    /// enclosing term; the port keeps `Auto` as a distinct reference.
    #[test]
    fn value_object_property_ref_accepts_upstream_auto() {
        let parsed = ValueObject::parse(&serde_json::json!({
            "type": "literal",
            "property_id": "auto",
            "@value": "x",
        }))
        .unwrap();
        assert_eq!(parsed.property, Some(PropertyRef::Auto));

        assert!(ValueObject::parse(&serde_json::json!({"property_id": "dcterms:title"})).is_err());
    }

    /// `AbstractResource::getValidValueResources`: item, item set, media, all.
    #[test]
    fn value_resource_data_types_accept_upstream_resource_kinds() {
        assert!(ResourceDataType::Item.accepts(ResourceKind::Item));
        assert!(!ResourceDataType::Item.accepts(ResourceKind::ItemSet));
        assert!(ResourceDataType::ItemSet.accepts(ResourceKind::ItemSet));
        assert!(ResourceDataType::Media.accepts(ResourceKind::Media));
        assert!(ResourceDataType::Any.accepts(ResourceKind::Item));
        assert!(ResourceDataType::Any.accepts(ResourceKind::ItemSet));
        assert!(ResourceDataType::Any.accepts(ResourceKind::Media));

        assert_eq!(
            ResourceDataType::from_name("resource:itemset"),
            Some(ResourceDataType::ItemSet)
        );
        assert_eq!(
            ResourceDataType::from_name("resource:item"),
            Some(ResourceDataType::Item)
        );
        assert_eq!(ResourceDataType::from_name("literal"), None);
    }
}
