//! Resource templates and their per-property rules.
//!
//! Mirrors `application/src/Entity/ResourceTemplate.php`,
//! `ResourceTemplateProperty.php`, `application/src/Api/Adapter/ResourceTemplateAdapter.php`
//! (hydrate, one-based positions, data type list normalisation) and the required
//! property rule in `application/src/Api/Adapter/AbstractResourceEntityAdapter.php`
//! (`validateEntity`).
//!
//! Upstream disagreements with DESIGN.md, resolved in upstream's favour:
//! `resource_template` has no `is_private` column (`schema.sql:180`); the private
//! flag lives on each `resource_template_property` row. The template also carries
//! `owner_id`, `title_property_id` and `description_property_id`, and each
//! property row carries `alternate_comment`.

use serde::{Deserialize, Serialize};

use crate::ids::{ClassId, PropertyId, TemplateId, UserId};
use crate::value::{LangTag, ValueError};

/// One entry in `resource_template_property.data_type`, upstream a JSON list of
/// data type names such as `["literal", "uri"]`.
///
/// Normalised the way `ResourceTemplateAdapter::hydrate` normalises the list:
/// trim, drop empty entries, keep the first occurrence of duplicates.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PropertyDataType(String);

impl PropertyDataType {
    /// The data type name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PropertyDataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Normalise a data type list the way `ResourceTemplateAdapter::hydrate` does:
/// `array_unique(array_filter(array_map('trim', $dataTypes)))`.
#[must_use]
pub fn normalize_data_types<I, S>(names: I) -> Vec<PropertyDataType>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut seen = Vec::new();
    for name in names {
        let trimmed = name.as_ref().trim();
        if trimmed.is_empty() {
            continue;
        }
        let candidate = PropertyDataType(trimmed.to_string());
        if !seen.contains(&candidate) {
            seen.push(candidate);
        }
    }
    seen
}

/// A resource template property (`application/src/Entity/ResourceTemplateProperty.php`,
/// `schema.sql:198`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateProp {
    /// `resource_template_property.property_id`.
    pub property: PropertyId,
    /// `alternate_label`, null when blank (`ResourceTemplateAdapter::hydrate`).
    pub alternate_label: Option<String>,
    /// `alternate_comment`, null when blank (`ResourceTemplateAdapter::hydrate`).
    pub alternate_comment: Option<String>,
    /// `position`, one-based, assigned from the order of the submitted list.
    pub position: i64,
    /// `data_type`, a normalised list; empty means "any data type downstream".
    pub data_type: Vec<PropertyDataType>,
    /// `is_required`, default false.
    pub is_required: bool,
    /// `is_private`, default false.
    pub is_private: bool,
    /// `default_lang`, null when blank, otherwise a validated language tag.
    pub default_lang: Option<LangTag>,
}

impl TemplateProp {
    /// A plain, non-required, non-private property with no alternate labels,
    /// data type list or default language.
    #[must_use]
    pub const fn new(property: PropertyId) -> Self {
        Self {
            property,
            alternate_label: None,
            alternate_comment: None,
            position: 0,
            data_type: Vec::new(),
            is_required: false,
            is_private: false,
            default_lang: None,
        }
    }

    /// Set the alternate label with upstream's blank-means-null rule.
    #[must_use]
    pub fn with_alternate_label(mut self, label: impl AsRef<str>) -> Self {
        let trimmed = label.as_ref().trim();
        self.alternate_label = (!trimmed.is_empty()).then(|| trimmed.to_string());
        self
    }

    /// Set the alternate comment with upstream's blank-means-null rule.
    #[must_use]
    pub fn with_alternate_comment(mut self, comment: impl AsRef<str>) -> Self {
        let trimmed = comment.as_ref().trim();
        self.alternate_comment = (!trimmed.is_empty()).then(|| trimmed.to_string());
        self
    }

    /// Set the data type list, normalised like the adapter.
    #[must_use]
    pub fn with_data_types<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.data_type = normalize_data_types(names);
        self
    }

    /// Mark the property required.
    #[must_use]
    pub const fn required(mut self) -> Self {
        self.is_required = true;
        self
    }

    /// Mark the property private.
    #[must_use]
    pub const fn private(mut self) -> Self {
        self.is_private = true;
        self
    }

    /// Set the default language. Blank input clears it, like the adapter treats
    /// `'' !== trim($value)`.
    ///
    /// # Errors
    /// [`ValueError::InvalidLangTag`] when the tag fails `LangTag` validation.
    pub fn with_default_lang(self, lang: &str) -> Result<Self, ValueError> {
        let trimmed = lang.trim();
        if trimmed.is_empty() {
            return Ok(Self {
                default_lang: None,
                ..self
            });
        }
        Ok(Self {
            default_lang: Some(LangTag::parse(trimmed)?),
            ..self
        })
    }

    /// The label to show for this property: the alternate label when set, else
    /// the property's own label. Upstream uses the same fallback in the
    /// required-property error (`AbstractResourceEntityAdapter::validateEntity`).
    #[must_use]
    pub fn display_label<'a>(&'a self, property_label: &'a str) -> &'a str {
        self.alternate_label.as_deref().unwrap_or(property_label)
    }
}

/// A resource template (`application/src/Entity/ResourceTemplate.php`,
/// `schema.sql:180`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceTemplate {
    /// `resource_template.id`.
    pub id: TemplateId,
    /// `resource_template.owner_id`, nullable.
    pub owner: Option<UserId>,
    /// `resource_template.resource_class_id`.
    pub class: Option<ClassId>,
    /// `resource_template.title_property_id`.
    pub title_property: Option<PropertyId>,
    /// `resource_template.description_property_id`.
    pub description_property: Option<PropertyId>,
    /// `resource_template.label`, unique upstream, non-empty.
    pub label: String,
    /// `resource_template_property` rows, ordered by position.
    pub props: Vec<TemplateProp>,
}

impl ResourceTemplate {
    /// A template with the given label and no properties.
    #[must_use]
    pub fn new(id: TemplateId, label: impl Into<String>) -> Self {
        Self {
            id,
            owner: None,
            class: None,
            title_property: None,
            description_property: None,
            label: label.into(),
            props: Vec::new(),
        }
    }

    /// Replace the property list, assigning one-based positions in list order
    /// the way `ResourceTemplateAdapter::hydrate` assigns them ("Position is
    /// one-based.").
    #[must_use]
    pub fn with_props(mut self, props: Vec<TemplateProp>) -> Self {
        self.props = props;
        for (index, prop) in self.props.iter_mut().enumerate() {
            prop.position = index as i64 + 1;
        }
        self
    }

    /// The required properties that have no value among `present`
    /// (`AbstractResourceEntityAdapter::validateEntity`: every required template
    /// property must have a value with the same property id).
    #[must_use]
    pub fn missing_required_props(&self, present: &[PropertyId]) -> Vec<&TemplateProp> {
        self.props
            .iter()
            .filter(|prop| prop.is_required && !present.contains(&prop.property))
            .collect()
    }

    /// The default language configured for a property, if any
    /// (`resource-form.js` fills a new value's language from it).
    #[must_use]
    pub fn default_lang_for(&self, property: PropertyId) -> Option<&LangTag> {
        self.props
            .iter()
            .find(|prop| prop.property == property)
            .and_then(|prop| prop.default_lang.as_ref())
    }

    /// Template properties whose data type list names something outside the
    /// given set of registered data type names. Upstream stores the list as
    /// text and ignores unknown names at value hydration; the port reports them
    /// at the template boundary.
    #[must_use]
    pub fn unknown_data_types(
        &self,
        is_registered: impl Fn(&str) -> bool,
    ) -> Vec<(&TemplateProp, &str)> {
        self.props
            .iter()
            .flat_map(|prop| {
                prop.data_type
                    .iter()
                    .filter(|name| !is_registered(name.as_str()))
                    .map(move |name| (prop, name.as_str()))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template() -> ResourceTemplate {
        ResourceTemplate::new(TemplateId::new(1), "Base Resource")
    }

    /// `ResourceTemplateProperty` defaults from application/src/Entity/ResourceTemplateProperty.php:
    /// `isRequired = false`, `isPrivate = false`, nullable data type and default language.
    #[test]
    fn template_prop_defaults_match_upstream_entity() {
        let prop = TemplateProp::new(PropertyId::new(7));
        assert!(!prop.is_required);
        assert!(!prop.is_private);
        assert_eq!(prop.default_lang, None);
        assert_eq!(prop.alternate_label, None);
        assert_eq!(prop.alternate_comment, None);
        assert!(prop.data_type.is_empty());
    }

    /// `ResourceTemplateAdapter::hydrate`: positions come from list order and are
    /// one-based ("Position is one-based.").
    #[test]
    fn template_props_get_one_based_positions_from_hydration_order() {
        let template = template().with_props(vec![
            TemplateProp::new(PropertyId::new(1)),
            TemplateProp::new(PropertyId::new(2)).required(),
            TemplateProp::new(PropertyId::new(3)).private(),
        ]);
        assert_eq!(template.props[0].position, 1);
        assert_eq!(template.props[1].position, 2);
        assert_eq!(template.props[2].position, 3);
        assert!(template.props[1].is_required);
        assert!(template.props[2].is_private);
    }

    /// `ResourceTemplateAdapter::hydrate` normalises the data type list:
    /// trim, drop empties, unique.
    #[test]
    fn template_data_type_list_normalisation_matches_upstream_adapter() {
        let prop = TemplateProp::new(PropertyId::new(1)).with_data_types([
            " literal ",
            "uri",
            "",
            "  ",
            "literal",
            "uri",
        ]);
        let names: Vec<&str> = prop
            .data_type
            .iter()
            .map(PropertyDataType::as_str)
            .collect();
        assert_eq!(names, ["literal", "uri"]);
    }

    /// `ResourceTemplateAdapter::hydrate` blanks alternate label/comment and
    /// default language on whitespace-only input.
    #[test]
    fn template_blank_strings_become_null_like_upstream_adapter() {
        let prop = TemplateProp::new(PropertyId::new(1))
            .with_alternate_label("  ")
            .with_alternate_comment("")
            .with_default_lang("   ")
            .unwrap();
        assert_eq!(prop.alternate_label, None);
        assert_eq!(prop.alternate_comment, None);
        assert_eq!(prop.default_lang, None);

        let prop = TemplateProp::new(PropertyId::new(1))
            .with_alternate_label(" Title ")
            .with_default_lang("en-US")
            .unwrap();
        assert_eq!(prop.alternate_label.as_deref(), Some("Title"));
        assert_eq!(
            prop.default_lang.as_ref().map(LangTag::as_str),
            Some("en-US")
        );
    }

    /// `AbstractResourceEntityAdapter::validateEntity`: a required template
    /// property must have a value for the same property id, and the error names
    /// the alternate label when set.
    #[test]
    fn template_required_property_rule_matches_upstream_validate_entity() {
        let template = template().with_props(vec![
            TemplateProp::new(PropertyId::new(1)).required(),
            TemplateProp::new(PropertyId::new(2))
                .required()
                .with_alternate_label("Preferred label"),
            TemplateProp::new(PropertyId::new(3)),
        ]);
        let missing = template.missing_required_props(&[PropertyId::new(2)]);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].property, PropertyId::new(1));
        assert_eq!(missing[0].display_label("Title"), "Title");

        let none = template.missing_required_props(&[PropertyId::new(1), PropertyId::new(2)]);
        assert!(none.is_empty());

        let prop = &template.props[1];
        assert_eq!(prop.display_label("Title"), "Preferred label");
    }

    /// `resource-form.js` fills a new value's language from the template
    /// property's default language.
    #[test]
    fn template_default_lang_lookup_serves_value_defaults() {
        let template = template().with_props(vec![
            TemplateProp::new(PropertyId::new(1))
                .with_default_lang("fr")
                .unwrap(),
            TemplateProp::new(PropertyId::new(2)),
        ]);
        assert_eq!(
            template
                .default_lang_for(PropertyId::new(1))
                .map(LangTag::as_str),
            Some("fr")
        );
        assert_eq!(template.default_lang_for(PropertyId::new(2)), None);
        assert_eq!(template.default_lang_for(PropertyId::new(9)), None);
    }

    /// An unknown data type name in a template list is reported against the
    /// registry, not silently dropped.
    #[test]
    fn template_unknown_data_types_are_reported() {
        let template = template().with_props(vec![
            TemplateProp::new(PropertyId::new(1)).with_data_types(["literal", "http://ex"]),
        ]);
        let unknown = template.unknown_data_types(|name| name == "literal");
        assert_eq!(unknown.len(), 1);
        assert_eq!(unknown[0].1, "http://ex");
    }
}
