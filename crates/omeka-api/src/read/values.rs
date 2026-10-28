//! `value` rows to JSON-LD value objects.
//!
//! `ValueRepresentation::jsonSerialize` builds the base object (`type`,
//! `property_id`, `property_label`, `is_public`, `@annotation`) and unions the
//! data type's `getJsonLd` onto it. `AbstractResourceEntityRepresentation::values`
//! groups those objects by property term, ordered by the resource template when
//! one is set and by `dcterms:title` then `dcterms:description` otherwise.
//!
//! Every row this module reads comes from a [`Relations`] value the caller
//! loaded for the whole page, so building a document issues no queries.

use indexmap::IndexMap;
use omeka_core::term_list;
use omeka_core::value_shape::ValueShape;
use omeka_db::ids::{AnnotationId, ResourceId, TemplateId};
use omeka_db::rows::{ValueRow, resource_type};

use super::relations::Relations;
use super::{Api, UNTITLED};
use crate::wire::{
    LiteralType, ResourceValueType, ReverseRef, UriType, Value, ValueLiteral, ValueMap,
    ValueResource, ValueUri,
};

/// Guard for an annotation that carries an annotated value in turn.
pub(crate) const MAX_ANNOTATION_DEPTH: u8 = 8;

/// Every value on a resource, keyed by property term.
pub(crate) fn value_map(
    api: &Api<'_>,
    relations: &Relations,
    resource: ResourceId,
    template: Option<TemplateId>,
) -> ValueMap {
    let rows = relations.values_of(resource.0);
    let mut map = build_map(api, relations, rows, 0);
    order_by_template(api, relations, &mut map, template);
    map
}

/// Values whose `value_resource_id` points at this resource, keyed by term.
///
/// The subject is always addressed through the `resources` adapter
/// (`subjectValuesForReverse`), which is why the reference is not the kind's
/// own collection.
pub(crate) fn reverse_map(
    api: &Api<'_>,
    relations: &Relations,
    resource: ResourceId,
) -> Option<IndexMap<String, Vec<ReverseRef>>> {
    let mut map: IndexMap<String, Vec<ReverseRef>> = IndexMap::new();
    for row in relations.pointing_at(resource.0) {
        let Some(property) = api.catalog().property(row.property_id) else {
            continue;
        };
        let Some(subject) = relations.resource(row.resource_id.0) else {
            continue;
        };
        if !api.sees(subject.is_public, subject.owner_id) {
            continue;
        }
        map.entry(property.term.clone())
            .or_default()
            .push(ReverseRef {
                id: api.base().api_resource("resources", subject.id.0),
                title: relations.title(subject),
            });
    }
    if map.is_empty() { None } else { Some(map) }
}

fn build_map(api: &Api<'_>, relations: &Relations, rows: &[ValueRow], depth: u8) -> ValueMap {
    let mut map = ValueMap::new();
    for row in rows {
        if let Some((term, value)) = build_value(api, relations, row, depth) {
            map.entry(term).or_default().push(value);
        }
    }
    map
}

/// One value object, or `None` when it should be hidden from the document.
fn build_value(
    api: &Api<'_>,
    relations: &Relations,
    row: &ValueRow,
    depth: u8,
) -> Option<(String, Value)> {
    let property = api.catalog().property(row.property_id)?;
    let term = property.term.clone();
    let label = property.label.clone();
    let annotation = match row.value_annotation_id {
        Some(id) if depth < MAX_ANNOTATION_DEPTH => annotation_map(api, relations, id, depth + 1),
        _ => None,
    };
    // `DataType\CustomVocab::getJsonLd` builds each shape the way the core type
    // does, except that a URI carries no `o:lang`.
    let list = term_list::list_id(&row.r#type);
    let shape = ValueShape::of(
        &row.r#type,
        row.uri.is_some(),
        row.value_resource_id.is_some(),
    );
    let value = match shape {
        ValueShape::Uri => Value::Uri(ValueUri {
            data_type: list.map_or(UriType::Uri, UriType::TermList),
            property_id: row.property_id.0,
            property_label: label,
            is_public: row.is_public,
            annotation,
            id: row.uri.clone().unwrap_or_default(),
            label: row.value.clone().filter(|value| !value.is_empty()),
            lang: row.lang.clone().filter(|_| list.is_none()),
        }),
        ValueShape::Resource => {
            Value::Resource(resource_value(api, relations, row, label, annotation)?)
        }
        ValueShape::Literal => Value::Literal(ValueLiteral {
            data_type: list.map_or(LiteralType::Literal, LiteralType::TermList),
            property_id: row.property_id.0,
            property_label: label,
            is_public: row.is_public,
            annotation,
            value: row.value.clone().unwrap_or_default(),
            language: row.lang.clone(),
        }),
    };
    Some((term, value))
}

/// A resource-valued object, or `None` when the pointed-at resource is not
/// visible (upstream `ValueRepresentation::isHidden`).
fn resource_value(
    api: &Api<'_>,
    relations: &Relations,
    row: &ValueRow,
    property_label: String,
    annotation: Option<ValueMap>,
) -> Option<ValueResource> {
    let value_id = row.value_resource_id?;
    let target = relations.resource(value_id.0)?;
    if !api.sees(target.is_public, target.owner_id) {
        return None;
    }
    let name = resource_name(&target.resource_type);
    Some(ValueResource {
        data_type: resource_value_type(&row.r#type, &target.resource_type),
        property_id: row.property_id.0,
        property_label,
        is_public: row.is_public,
        annotation,
        id: api.base().api_resource(name, value_id.0),
        value_resource_id: value_id.0,
        value_resource_name: name.to_string(),
        url: None,
        display_title: Some(
            relations
                .title(target)
                .unwrap_or_else(|| UNTITLED.to_string()),
        ),
        thumbnail_url: None,
        thumbnail_title: None,
        thumbnail_type: None,
    })
}

fn annotation_map(
    api: &Api<'_>,
    relations: &Relations,
    annotation: AnnotationId,
    depth: u8,
) -> Option<ValueMap> {
    let rows = relations.values_of(annotation.as_resource().0);
    let map = build_map(api, relations, rows, depth);
    if map.is_empty() { None } else { Some(map) }
}

/// Put the template's property terms first, in template order.
fn order_by_template(
    api: &Api<'_>,
    relations: &Relations,
    map: &mut ValueMap,
    template: Option<TemplateId>,
) {
    let terms: Vec<String> = match template {
        Some(id) => relations
            .template_properties(id.0)
            .iter()
            .filter_map(|property| {
                api.catalog()
                    .property(*property)
                    .map(|info| info.term.clone())
            })
            .collect(),
        None => vec![
            "dcterms:title".to_string(),
            "dcterms:description".to_string(),
        ],
    };
    let mut ordered = ValueMap::new();
    for term in &terms {
        if let Some(values) = map.shift_remove(term) {
            ordered.insert(term.clone(), values);
        }
    }
    for (term, values) in std::mem::take(map) {
        ordered.entry(term).or_insert(values);
    }
    *map = ordered;
}

/// The `value_resource_name` a target resource reports
/// (`Entity\Resource::getResourceName`).
fn resource_name(stored_type: &str) -> &'static str {
    match stored_type {
        resource_type::ITEM_SET => "item_sets",
        resource_type::MEDIA => "media",
        resource_type::VALUE_ANNOTATION => "value_annotations",
        _ => "items",
    }
}

/// The `type` member of a resource value. The stored data type name wins when
/// it names a specific kind; a bare `resource` follows the target's kind.
fn resource_value_type(stored: &str, target_type: &str) -> ResourceValueType {
    if let Some(list) = term_list::list_id(stored) {
        return ResourceValueType::TermList(list);
    }
    match stored {
        "resource:item" => ResourceValueType::Item,
        "resource:itemset" => ResourceValueType::ItemSet,
        "resource:media" => ResourceValueType::Media,
        _ => match target_type {
            resource_type::ITEM_SET => ResourceValueType::ItemSet,
            resource_type::MEDIA => ResourceValueType::Media,
            _ => ResourceValueType::All,
        },
    }
}
