//! The data type registry, mirroring `application/src/DataType/*`.
//!
//! One module per upstream file where the implementations differ:
//! `literal` (`Literal.php`), `uri` (`Uri.php`), `resource:item`
//! (`Resource/Item.php`), `resource:itemset` (`Resource/ItemSet.php`),
//! `resource:media` (`Resource/Media.php`) and `resource` (`Resource/All.php`),
//! all registered in `application/config/module.config.php:563`.
//!
//! View-layer members of `DataTypeInterface` (`prepareForm`, `form`, `render`)
//! stay out of core: they render PHP templates, which the port does not have.
//! The `json_ld` output for resource values needs the referenced resource's
//! representation (`AbstractResource::getJsonLd` delegates to
//! `valueResource()->valueRepresentation()`), so it is the API layer's job;
//! the trait returns `None` there.

use serde_json::{Value as JsonValue, json};

use crate::value::{
    LangTag, ResourceDataType, Value, ValueData, ValueError, ValueObject, literal_is_valid,
    uri_is_valid,
};

/// What a data type produces when it hydrates a value object: the typed object
/// plus the language it carries (`@language` for literals, `o:lang` for URIs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HydratedValue {
    /// The value object, typed.
    pub data: ValueData,
    /// The language tag, when the data type carries one.
    pub lang: Option<LangTag>,
}

/// The port's `DataTypeInterface` minus the PHP view members.
pub trait DataType: Sync {
    /// The registered name (`getName`).
    fn name(&self) -> &'static str;

    /// The human-readable label (`getLabel`).
    fn label(&self) -> &'static str;

    /// The optgroup label (`getOptgroupLabel`); only the resource data types
    /// have one.
    fn optgroup_label(&self) -> Option<&'static str> {
        None
    }

    /// Whether the type is registered under `[data_types][value_annotating]`
    /// (`application/config/module.config.php:572`).
    fn value_annotating(&self) -> bool {
        false
    }

    /// `isValid(array $valueObject)`.
    fn is_valid(&self, object: &ValueObject) -> bool;

    /// `hydrate(array $valueObject, Value $value, AbstractEntityAdapter $adapter)`.
    ///
    /// Upstream calls this only after [`DataType::is_valid`] passes. The Rust
    /// signature returns the object instead of mutating a value, so it repeats
    /// the validity check and reports why it failed.
    ///
    /// # Errors
    /// A [`ValueError`] naming the invalid part.
    fn hydrate(&self, object: &ValueObject) -> Result<HydratedValue, ValueError>;

    /// `getJsonLd(ValueRepresentation $value)`, when the core has every input it
    /// needs. `None` means the API layer builds the JSON-LD for this value.
    fn json_ld(&self, value: &Value) -> Option<JsonValue>;

    /// `toString(ValueRepresentation $value)`, the stored `value` column for the
    /// types that keep one.
    fn to_string(&self, value: &Value) -> String;

    /// `getFulltextText(PhpRenderer $view, ValueRepresentation $value)` where core
    /// has the inputs; `None` when the text needs a resource lookup (resource
    /// values) or a renderer.
    fn fulltext(&self, value: &Value) -> Option<String>;

    /// The type as a conversion target, when it implements
    /// `ConversionTargetInterface`.
    fn as_conversion_target(&self) -> Option<&dyn ConversionTarget> {
        None
    }
}

/// Upstream `ConversionTargetInterface`: convert an existing stored value to
/// this data type, without losing data.
pub trait ConversionTarget {
    /// `convert(Value $valueObject, string $dataTypeTarget): bool`. Mutates the
    /// value in place when the conversion succeeds; returns whether it did.
    fn convert(&self, value: &mut Value, target: &str) -> bool;
}

/// `literal` (`application/src/DataType/Literal.php`).
#[derive(Debug, Clone, Copy, Default)]
pub struct LiteralType;

impl DataType for LiteralType {
    fn name(&self) -> &'static str {
        "literal"
    }

    fn label(&self) -> &'static str {
        "Text"
    }

    fn value_annotating(&self) -> bool {
        true
    }

    fn is_valid(&self, object: &ValueObject) -> bool {
        object.value.as_deref().is_some_and(literal_is_valid)
    }

    /// `Literal::hydrate`: `@value` becomes the value, `@language` the language,
    /// `uri` and `value_resource_id` are cleared.
    fn hydrate(&self, object: &ValueObject) -> Result<HydratedValue, ValueError> {
        let text = object.value.as_deref().unwrap_or_default();
        let data = ValueData::literal(text)?;
        let lang = match object.language.as_deref() {
            Some(tag) => Some(LangTag::parse(tag)?),
            None => None,
        };
        Ok(HydratedValue { data, lang })
    }

    /// `Literal::getJsonLd`: `@value` plus `@language` when set.
    fn json_ld(&self, value: &Value) -> Option<JsonValue> {
        let text = value.data.stored_value()?;
        let mut object = json!({ "@value": text });
        if let Some(lang) = &value.lang {
            object["@language"] = JsonValue::String(lang.as_str().to_string());
        }
        Some(object)
    }

    /// `AbstractDataType::toString`: the stored value.
    fn to_string(&self, value: &Value) -> String {
        value.data.stored_value().unwrap_or_default().to_string()
    }

    /// `AbstractDataType::getFulltextText`: the stored value.
    fn fulltext(&self, value: &Value) -> Option<String> {
        Some(value.data.stored_value().unwrap_or_default().to_string())
    }

    fn as_conversion_target(&self) -> Option<&dyn ConversionTarget> {
        Some(self)
    }
}

impl ConversionTarget for LiteralType {
    /// `Literal::convert`: to avoid data loss it only converts when the URI is a
    /// valid literal and the stored value is not, or when the stored value is
    /// already valid.
    fn convert(&self, value: &mut Value, _target: &str) -> bool {
        let stored = value.data.stored_value().unwrap_or_default().to_string();
        let uri = value.data.stored_uri().map(str::to_string);
        if uri.as_deref().is_some_and(literal_is_valid) && !literal_is_valid(&stored) {
            let text = uri.unwrap_or_default();
            value.data = ValueData::Literal { text };
            return true;
        }
        if literal_is_valid(&stored) && !uri.as_deref().is_some_and(literal_is_valid) {
            return true;
        }
        false
    }
}

/// `uri` (`application/src/DataType/Uri.php`).
#[derive(Debug, Clone, Copy, Default)]
pub struct UriType;

impl DataType for UriType {
    fn name(&self) -> &'static str {
        "uri"
    }

    fn label(&self) -> &'static str {
        "URI"
    }

    fn value_annotating(&self) -> bool {
        true
    }

    fn is_valid(&self, object: &ValueObject) -> bool {
        object.id.as_deref().is_some_and(uri_is_valid)
    }

    /// `Uri::hydrate`: `@id` becomes the URI, `o:label` the stored value,
    /// `o:lang` the language, `value_resource_id` is cleared.
    fn hydrate(&self, object: &ValueObject) -> Result<HydratedValue, ValueError> {
        let uri = object.id.clone().unwrap_or_default();
        let label = object.label.clone();
        let data = ValueData::uri_with_label(uri, label)?;
        let lang = match object.lang.as_deref() {
            Some(tag) => Some(LangTag::parse(tag)?),
            None => None,
        };
        Ok(HydratedValue { data, lang })
    }

    /// `Uri::getJsonLd`: `@id`, plus `o:label` and `o:lang` when set.
    fn json_ld(&self, value: &Value) -> Option<JsonValue> {
        let uri = value.data.stored_uri()?;
        let mut object = json!({ "@id": uri });
        if let Some(label) = value.data.stored_value() {
            object["o:label"] = JsonValue::String(label.to_string());
        }
        if let Some(lang) = &value.lang {
            object["o:lang"] = JsonValue::String(lang.as_str().to_string());
        }
        Some(object)
    }

    fn to_string(&self, value: &Value) -> String {
        value.data.stored_value().unwrap_or_default().to_string()
    }

    /// `Uri::getFulltextText`: the URI and the label, space separated.
    fn fulltext(&self, value: &Value) -> Option<String> {
        Some(format!(
            "{} {}",
            value.data.stored_uri().unwrap_or_default(),
            value.data.stored_value().unwrap_or_default()
        ))
    }

    fn as_conversion_target(&self) -> Option<&dyn ConversionTarget> {
        Some(self)
    }
}

impl ConversionTarget for UriType {
    /// `Uri::convert`: keep a valid stored URI, otherwise move the stored value
    /// into the URI column.
    fn convert(&self, value: &mut Value, _target: &str) -> bool {
        if value.data.stored_uri().is_some_and(uri_is_valid) {
            return true;
        }
        let stored = value.data.stored_value().unwrap_or_default().to_string();
        if uri_is_valid(&stored) {
            value.data = ValueData::Uri {
                uri: stored,
                label: None,
            };
            return true;
        }
        false
    }
}

/// `resource:item`, `resource:itemset`, `resource:media` and `resource`
/// (`application/src/DataType/Resource/AbstractResource.php` and its four
/// subclasses).
#[derive(Debug, Clone, Copy)]
pub struct ResourceType {
    kind: ResourceDataType,
}

/// `resource:item`.
pub const RESOURCE_ITEM: ResourceType = ResourceType {
    kind: ResourceDataType::Item,
};
/// `resource:itemset`.
pub const RESOURCE_ITEM_SET: ResourceType = ResourceType {
    kind: ResourceDataType::ItemSet,
};
/// `resource:media`.
pub const RESOURCE_MEDIA: ResourceType = ResourceType {
    kind: ResourceDataType::Media,
};
/// `resource`, the "All" data type.
pub const RESOURCE_ANY: ResourceType = ResourceType {
    kind: ResourceDataType::Any,
};

impl ResourceType {
    /// The kind restriction this data type carries.
    #[must_use]
    pub const fn resource_data_type(&self) -> ResourceDataType {
        self.kind
    }
}

impl DataType for ResourceType {
    fn name(&self) -> &'static str {
        self.kind.name()
    }

    fn label(&self) -> &'static str {
        match self.kind {
            ResourceDataType::Item => "Item",
            ResourceDataType::ItemSet => "Item Set",
            ResourceDataType::Media => "Media",
            ResourceDataType::Any => "All",
        }
    }

    fn optgroup_label(&self) -> Option<&'static str> {
        Some("Resource")
    }

    /// Only the three kind-specific resource types are value annotating
    /// (`module.config.php:572`); `resource` (All) is not.
    fn value_annotating(&self) -> bool {
        !matches!(self.kind, ResourceDataType::Any)
    }

    /// `AbstractResource::isValid`: a numeric `value_resource_id` is present.
    /// Serde has already rejected a non-numeric id at the boundary.
    fn is_valid(&self, object: &ValueObject) -> bool {
        object.value_resource_id.is_some()
    }

    /// `AbstractResource::hydrate`: the value, language and URI columns are
    /// cleared and `value_resource_id` is kept. The lookup and the
    /// `getValidValueResources` check happen where the resource table is
    /// reachable; [`ResourceDataType::accepts`] is that check.
    fn hydrate(&self, object: &ValueObject) -> Result<HydratedValue, ValueError> {
        let Some(id) = object.value_resource_id else {
            return Err(ValueError::MissingResourceId(self.name().to_string()));
        };
        Ok(HydratedValue {
            data: ValueData::resource(id, self.kind),
            lang: None,
        })
    }

    /// `AbstractResource::getJsonLd` delegates to the referenced resource's
    /// representation, which core cannot build.
    fn json_ld(&self, _value: &Value) -> Option<JsonValue> {
        None
    }

    /// `AbstractResource::toString` delegates to `valueResource()->url()`, which
    /// core cannot build; the stored `value` column is null for resource values.
    fn to_string(&self, _value: &Value) -> String {
        String::new()
    }

    /// `AbstractResource::getFulltextText` is the referenced resource's title.
    fn fulltext(&self, _value: &Value) -> Option<String> {
        None
    }

    fn as_conversion_target(&self) -> Option<&dyn ConversionTarget> {
        Some(self)
    }
}

impl ConversionTarget for ResourceType {
    /// `AbstractResource::convert` tests `is_numeric($valueObject->getValueResource())`
    /// on the Value entity, where the property holds a Resource object or null
    /// (application/src/DataType/Resource/AbstractResource.php:112). Both fail
    /// `is_numeric`, so the method always returns false. Mirrored as false.
    fn convert(&self, _value: &mut Value, _target: &str) -> bool {
        false
    }
}

/// The registered data types, one entry per upstream registration
/// (`application/config/module.config.php:563`).
#[derive(Clone)]
pub struct Registry {
    types: Vec<&'static dyn DataType>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field(
                "types",
                &self.types.iter().map(|t| t.name()).collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::core()
    }
}

impl Registry {
    /// A registry with the six core data types upstream ships.
    #[must_use]
    pub fn core() -> Self {
        Self {
            types: vec![
                &LiteralType,
                &UriType,
                &RESOURCE_ANY,
                &RESOURCE_ITEM,
                &RESOURCE_ITEM_SET,
                &RESOURCE_MEDIA,
            ],
        }
    }

    /// A data type by its registered name (`Manager::get`), `None` where
    /// upstream would throw `ServiceNotFoundException`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&'static dyn DataType> {
        self.types.iter().copied().find(|t| t.name() == name)
    }

    /// Whether a name is registered; the template rule uses this to report
    /// unknown data types.
    #[must_use]
    pub fn is_registered(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// The data type to extract a stored value with (`Manager::getForExtract`).
    /// The stored data type wins; the typed value always has one, so the
    /// upstream fallback chain (`uri`, then `resource`, then `literal`) for rows
    /// whose type name is missing collapses to a lookup the variant already
    /// answers.
    #[must_use]
    pub fn get_for_extract(&self, data: &ValueData) -> &'static dyn DataType {
        match data {
            ValueData::Literal { .. } => &LiteralType,
            ValueData::Uri { .. } => &UriType,
            ValueData::Resource { data_type, .. } => match data_type {
                ResourceDataType::Item => &RESOURCE_ITEM,
                ResourceDataType::ItemSet => &RESOURCE_ITEM_SET,
                ResourceDataType::Media => &RESOURCE_MEDIA,
                ResourceDataType::Any => &RESOURCE_ANY,
            },
        }
    }

    /// `ValueHydrator::hydrate` for one value object: look up the named type,
    /// check validity, then hydrate.
    ///
    /// # Errors
    /// [`ValueError::UnknownDataType`] for a missing or unregistered type, or the
    /// type's own hydration error.
    pub fn hydrate(&self, object: &ValueObject) -> Result<HydratedValue, ValueError> {
        let name = object.data_type.clone().unwrap_or_default();
        let Some(data_type) = self.get(&name) else {
            return Err(ValueError::UnknownDataType(name));
        };
        if !data_type.is_valid(object) {
            return match data_type.name() {
                "literal" => Err(ValueError::EmptyLiteral),
                "uri" => Err(ValueError::InvalidUri(
                    object.id.clone().unwrap_or_default(),
                )),
                _ => Err(ValueError::MissingResourceId(data_type.name().to_string())),
            };
        }
        data_type.hydrate(object)
    }

    /// `ValueHydrator` data type conversion: get the target, skip it when it is
    /// not a conversion target, convert, report whether it worked.
    pub fn convert(&self, value: &mut Value, target: &str) -> bool {
        let Some(data_type) = self.get(target) else {
            return false;
        };
        let Some(conversion_target) = data_type.as_conversion_target() else {
            return false;
        };
        conversion_target.convert(value, target)
    }

    /// Every registered data type, in registration order.
    pub fn iter(&self) -> impl Iterator<Item = &'static dyn DataType> {
        self.types.iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{PropertyId, ResourceId};

    fn value(data: ValueData) -> Value {
        Value::new(ResourceId::new(1), PropertyId::new(2), data)
    }

    fn object(json: JsonValue) -> ValueObject {
        ValueObject::parse(&json).unwrap()
    }

    /// Names, labels and optgroups are the ones registered in
    /// application/config/module.config.php:563 and set by the classes'
    /// `getName`/`getLabel`/`getOptgroupLabel`.
    #[test]
    fn registry_names_and_labels_match_upstream_data_types() {
        let registry = Registry::core();
        let listed: Vec<(&str, &str, Option<&str>)> = registry
            .iter()
            .map(|t| (t.name(), t.label(), t.optgroup_label()))
            .collect();
        assert_eq!(
            listed,
            vec![
                ("literal", "Text", None),
                ("uri", "URI", None),
                ("resource", "All", Some("Resource")),
                ("resource:item", "Item", Some("Resource")),
                ("resource:itemset", "Item Set", Some("Resource")),
                ("resource:media", "Media", Some("Resource")),
            ]
        );
        assert!(registry.get("literal").is_some());
        assert!(registry.get("Literal").is_none());
        assert!(!registry.is_registered("http://example.org/type"));
    }

    /// `module.config.php:572`: the value annotating data types are literal, uri,
    /// resource:item, resource:itemset, resource:media.
    #[test]
    fn registry_value_annotating_match_upstream_config() {
        let registry = Registry::core();
        let annotating: Vec<&str> = registry
            .iter()
            .filter(|t| t.value_annotating())
            .map(DataType::name)
            .collect();
        assert_eq!(
            annotating,
            [
                "literal",
                "uri",
                "resource:item",
                "resource:itemset",
                "resource:media"
            ]
        );
    }

    /// `ValueHydrator::hydrate`: the `type` key selects the data type and an
    /// unknown or missing type is rejected.
    #[test]
    fn registry_hydrate_matches_upstream_value_hydrator_types() {
        let registry = Registry::core();

        let hydrated = registry
            .hydrate(&object(
                json!({"type": "literal", "property_id": 2, "@value": "text", "@language": "en"}),
            ))
            .unwrap();
        assert_eq!(
            hydrated.data,
            ValueData::Literal {
                text: "text".into()
            }
        );
        assert_eq!(hydrated.lang.as_ref().map(LangTag::as_str), Some("en"));

        let hydrated = registry
            .hydrate(&object(
                json!({"type": "uri", "property_id": 2, "@id": "https://example.org/", "o:label": "Ex"}),
            ))
            .unwrap();
        assert_eq!(
            hydrated.data,
            ValueData::Uri {
                uri: "https://example.org/".into(),
                label: Some("Ex".into())
            }
        );

        let hydrated = registry
            .hydrate(&object(
                json!({"type": "resource:itemset", "property_id": 2, "value_resource_id": 9}),
            ))
            .unwrap();
        assert_eq!(
            hydrated.data,
            ValueData::Resource {
                id: ResourceId::new(9),
                data_type: ResourceDataType::ItemSet
            }
        );

        assert!(matches!(
            registry.hydrate(&object(
                json!({"type": "literal", "property_id": 2, "@value": "  "})
            )),
            Err(ValueError::EmptyLiteral)
        ));
        assert!(matches!(
            registry.hydrate(&object(json!({"type": "nope", "property_id": 2, "@value": "x"}))),
            Err(ValueError::UnknownDataType(name)) if name == "nope"
        ));
        assert!(matches!(
            registry.hydrate(&object(json!({"property_id": 2, "@value": "x"}))),
            Err(ValueError::UnknownDataType(name)) if name.is_empty()
        ));
    }

    /// `Literal::getJsonLd` and `Uri::getJsonLd` key for key.
    #[test]
    fn data_type_json_ld_matches_upstream_get_json_ld() {
        let registry = Registry::core();

        let literal =
            value(ValueData::literal("a text").unwrap()).with_lang(LangTag::parse("en").unwrap());
        let json = registry.get("literal").unwrap().json_ld(&literal).unwrap();
        assert_eq!(json, json!({"@value": "a text", "@language": "en"}));

        let uri = value(
            ValueData::uri_with_label("https://example.org/", Some("Example".into())).unwrap(),
        );
        let json = registry.get("uri").unwrap().json_ld(&uri).unwrap();
        assert_eq!(
            json,
            json!({"@id": "https://example.org/", "o:label": "Example"})
        );

        let resource = value(ValueData::resource(
            ResourceId::new(9),
            ResourceDataType::Item,
        ));
        assert!(
            registry
                .get("resource:item")
                .unwrap()
                .json_ld(&resource)
                .is_none()
        );
    }

    /// `Manager::getForExtract` resolves the stored data type; the typed value
    /// always names the right one.
    #[test]
    fn registry_extract_type_matches_upstream_manager_get_for_extract() {
        let registry = Registry::core();
        assert_eq!(
            registry
                .get_for_extract(&ValueData::literal("x").unwrap())
                .name(),
            "literal"
        );
        assert_eq!(
            registry
                .get_for_extract(&ValueData::uri("https://example.org/").unwrap())
                .name(),
            "uri"
        );
        assert_eq!(
            registry
                .get_for_extract(&ValueData::resource(
                    ResourceId::new(1),
                    ResourceDataType::Any
                ))
                .name(),
            "resource"
        );
    }

    /// `Literal::convert` (application/src/DataType/Literal.php:71): a valid URI
    /// and no valid literal converts; a valid literal stays; both or neither
    /// returns false.
    #[test]
    fn literal_conversion_matches_upstream_conversion_target() {
        let registry = Registry::core();

        let mut from_uri = value(ValueData::uri("https://example.org/").unwrap());
        assert!(registry.convert(&mut from_uri, "literal"));
        assert_eq!(
            from_uri.data,
            ValueData::Literal {
                text: "https://example.org/".into()
            }
        );

        let mut already_literal = value(ValueData::literal("text").unwrap());
        assert!(registry.convert(&mut already_literal, "literal"));

        let mut from_resource = value(ValueData::resource(
            ResourceId::new(9),
            ResourceDataType::Item,
        ));
        assert!(!registry.convert(&mut from_resource, "literal"));
    }

    /// `Uri::convert` (application/src/DataType/Uri.php:97): a valid URI stays,
    /// otherwise a valid stored value moves into the URI column.
    #[test]
    fn uri_conversion_matches_upstream_conversion_target() {
        let registry = Registry::core();

        let mut uri_like = value(ValueData::literal("https://example.org/").unwrap());
        assert!(registry.convert(&mut uri_like, "uri"));
        assert_eq!(
            uri_like.data,
            ValueData::Uri {
                uri: "https://example.org/".into(),
                label: None
            }
        );

        // `uriIsValid` needs no scheme, so any non-empty, control-free text is a
        // valid URI and the stored value moves into the URI column unchanged.
        let mut plain = value(ValueData::literal("just text").unwrap());
        assert!(registry.convert(&mut plain, "uri"));
        assert_eq!(
            plain.data,
            ValueData::Uri {
                uri: "just text".into(),
                label: None
            }
        );

        let mut already_uri = value(ValueData::uri("https://example.org/").unwrap());
        assert!(registry.convert(&mut already_uri, "uri"));
    }

    /// `AbstractResource::convert` always returns false upstream, because the
    /// Value entity's `valueResource` is an object or null and `is_numeric`
    /// rejects both. Mirrored, including the no-op verdict.
    #[test]
    fn resource_conversion_matches_upstream_no_op() {
        let registry = Registry::core();
        let mut resource = value(ValueData::resource(
            ResourceId::new(9),
            ResourceDataType::Item,
        ));
        let before = resource.clone();
        assert!(!registry.convert(&mut resource, "resource:item"));
        assert_eq!(resource, before);
        assert!(!registry.convert(&mut resource, "resource"));
    }

    /// `AbstractDataType::toString` and the `Uri`/`Resource` overrides.
    #[test]
    fn data_type_to_string_matches_upstream() {
        let registry = Registry::core();
        assert_eq!(
            registry
                .get("literal")
                .unwrap()
                .to_string(&value(ValueData::literal("text").unwrap())),
            "text"
        );
        assert_eq!(
            registry.get("uri").unwrap().to_string(&value(
                ValueData::uri_with_label("https://example.org/", Some("Label".into())).unwrap()
            )),
            "Label"
        );
        assert_eq!(
            registry
                .get("uri")
                .unwrap()
                .to_string(&value(ValueData::uri("https://example.org/").unwrap())),
            ""
        );
        assert_eq!(
            registry
                .get("resource:item")
                .unwrap()
                .to_string(&value(ValueData::resource(
                    ResourceId::new(1),
                    ResourceDataType::Item
                ))),
            ""
        );
    }

    /// `Uri::getFulltextText`: "uri label" with a space, no separators.
    #[test]
    fn uri_fulltext_matches_upstream_uri_data_type() {
        let registry = Registry::core();
        assert_eq!(
            registry.get("uri").unwrap().fulltext(&value(
                ValueData::uri_with_label("https://example.org/", Some("Example".into())).unwrap()
            )),
            Some("https://example.org/ Example".to_string())
        );
    }

    /// The resource data types' kind restrictions come from
    /// `getValidValueResources` in `application/src/DataType/Resource/*.php`.
    #[test]
    fn resource_data_type_kinds_match_upstream_subclasses() {
        let registry = Registry::core();
        assert_eq!(
            registry.get("resource:item").unwrap().name(),
            RESOURCE_ITEM.name()
        );
        assert!(
            RESOURCE_ITEM
                .resource_data_type()
                .accepts(crate::resource::ResourceKind::Item)
        );
        assert!(
            !RESOURCE_ITEM
                .resource_data_type()
                .accepts(crate::resource::ResourceKind::Media)
        );
        assert!(
            RESOURCE_ANY
                .resource_data_type()
                .accepts(crate::resource::ResourceKind::Media)
        );
    }
}
