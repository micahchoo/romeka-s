//! The value edits of a bulk change (`briefs/items-bulk-edit-design.md`, phase 3).
//!
//! Upstream's batch edit (`ResourceBatchUpdateForm::preprocessData`, `ValueHydrator::hydrate`)
//! clears every value of chosen properties (`clear_property_values`), sets the visibility of every
//! value of chosen properties (`set_value_visibility`), and appends new text, URI and link
//! values; the clear and the visibility run before the append (`Job/BatchUpdate.php`). This
//! module carries those, and one the port's `BatchUpdate` job adds: replace one exact text with
//! another. On the wire it is a list of operations:
//!
//! ```json
//! [ { "op": "add", "property": 3, "isPublic": true,
//!     "value": { "type": "literal", "text": "Maps", "lang": "en" } },
//!   { "op": "add", "property": 3, "value": { "type": "uri", "uri": "https://x.org", "label": "X" } },
//!   { "op": "add", "property": 13, "value": { "type": "item", "item": 7 } },
//!   { "op": "replace", "property": 4, "from": "Harbor", "to": "Harbour" },
//!   { "op": "clear", "property": 5 },
//!   { "op": "visibility", "property": 6, "isPublic": false } ]
//! ```
//!
//! Each field takes at most one of clear, replace and visibility, which runs first, and then any
//! number of different values to add. So the order of the list never matters, and a list that
//! names two of the first kind for one field is refused.

use std::collections::BTreeMap;

use omeka_core::value::{literal_is_valid, uri_is_valid};
use omeka_db::ids::{ItemId, PropertyId};
use omeka_db::repos::ResourceRepo;
use omeka_db::rows::{ValueContent, ValueWrites, resource_type};
use omeka_db::with_db;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::BulkError;
use super::change::Id;
use crate::errors::ApiError;
use crate::read::Api;
use crate::write::body::Payload;
use crate::write::resource::not_found;

/// The value edits of one change, by field. Never empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueEdits<A = NewValue> {
    fields: BTreeMap<PropertyId, FieldEdit<A>>,
}

/// What one field gets: a rewrite of the values it holds, then values added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldEdit<A> {
    pub rewrite: Option<Rewrite>,
    pub add: Vec<A>,
}

/// A change to the values a field already holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rewrite {
    /// Remove every value.
    Clear,
    /// Every text value that is exactly `from` becomes `to`.
    Replace { from: String, to: String },
    /// Every value becomes public, or private.
    Visibility(bool),
}

/// A value to add, as the change names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewValue {
    pub data: NewData,
    pub is_public: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewData {
    Literal { text: String, lang: Option<String> },
    Uri { uri: String, label: Option<String> },
    Item(ItemId),
}

impl<A> ValueEdits<A> {
    pub fn fields(&self) -> impl Iterator<Item = (PropertyId, &FieldEdit<A>)> {
        self.fields.iter().map(|(property, edit)| (*property, edit))
    }

    pub fn properties(&self) -> Vec<PropertyId> {
        self.fields.keys().copied().collect()
    }
}

impl ValueEdits {
    /// The edits a wire list names; `None` for an empty list.
    pub(super) fn from_wire(ops: Vec<OpWire>) -> Result<Option<Self>, String> {
        let mut fields: BTreeMap<PropertyId, FieldEdit<NewValue>> = BTreeMap::new();
        for op in ops {
            let (property, step) = op.into_step()?;
            let field = fields.entry(property).or_insert(FieldEdit {
                rewrite: None,
                add: Vec::new(),
            });
            match step {
                Step::Rewrite(rewrite) => {
                    if field.rewrite.replace(rewrite).is_some() {
                        return Err(format!(
                            "property {} takes one of clear, replace and visibility",
                            property.0
                        ));
                    }
                }
                Step::Add(value) => {
                    if field.add.contains(&value) {
                        return Err(format!("property {} adds the same value twice", property.0));
                    }
                    field.add.push(value);
                }
            }
        }
        Ok((!fields.is_empty()).then_some(Self { fields }))
    }

    /// The wire list, which [`Self::from_wire`] reads back.
    pub(super) fn to_json(&self) -> Value {
        let mut ops = Vec::new();
        for (property, edit) in &self.fields {
            let property = property.0;
            match &edit.rewrite {
                None => {}
                Some(Rewrite::Clear) => ops.push(json!({ "op": "clear", "property": property })),
                Some(Rewrite::Replace { from, to }) => ops
                    .push(json!({ "op": "replace", "property": property, "from": from, "to": to })),
                Some(Rewrite::Visibility(is_public)) => ops.push(
                    json!({ "op": "visibility", "property": property, "isPublic": is_public }),
                ),
            }
            for value in &edit.add {
                let data = match &value.data {
                    NewData::Literal { text, lang } => {
                        json!({ "type": "literal", "text": text, "lang": lang })
                    }
                    NewData::Uri { uri, label } => {
                        json!({ "type": "uri", "uri": uri, "label": label })
                    }
                    NewData::Item(item) => json!({ "type": "item", "item": item.0 }),
                };
                ops.push(json!({
                    "op": "add", "property": property, "value": data, "isPublic": value.is_public,
                }));
            }
        }
        Value::Array(ops)
    }
}

/// Resolve the values to add through the item `PATCH`'s own value parser
/// (`write::values::collect`), so a bulk value has the data type, the columns and the checks an
/// edit by hand gives it. Each field must exist, and a linked item must be an item the caller
/// sees.
///
/// # Errors
/// 404 for a property or a linked item that does not exist or that the caller cannot see; the
/// value parser's own errors.
pub(super) async fn resolve(
    api: &Api<'_>,
    edits: &ValueEdits,
) -> Result<ValueEdits<ValueContent>, BulkError> {
    api.catalog().fill_all(api.pool()).await?;
    let mut fields = BTreeMap::new();
    for (property, edit) in edits.fields() {
        let Some(term) = api
            .catalog()
            .property(property)
            .map(|info| info.term.clone())
        else {
            return Err(not_found("Omeka\\Entity\\Property", &Value::from(property.0)).into());
        };
        let mut add = Vec::with_capacity(edit.add.len());
        for value in &edit.add {
            if let NewData::Item(item) = value.data {
                visible_item(api, item).await?;
            }
            add.push(content(api, &term, property, value).await?);
        }
        fields.insert(
            property,
            FieldEdit {
                rewrite: edit.rewrite.clone(),
                add,
            },
        );
    }
    Ok(ValueEdits { fields })
}

async fn visible_item(api: &Api<'_>, item: ItemId) -> Result<(), BulkError> {
    let row = with_db!(api.pool(), |db| ResourceRepo::new(db)
        .get(item.as_resource())
        .await)?;
    match row {
        Some(row)
            if row.resource_type == resource_type::ITEM
                && api.sees(row.is_public, row.owner_id) =>
        {
            Ok(())
        }
        _ => Err(not_found("Omeka\\Entity\\Item", &Value::from(item.0)).into()),
    }
}

/// One value as the item `PATCH` stores it.
async fn content(
    api: &Api<'_>,
    term: &str,
    property: PropertyId,
    value: &NewValue,
) -> Result<ValueContent, BulkError> {
    let mut object = match &value.data {
        NewData::Literal { text, lang } => {
            let mut object = json!({ "type": "literal", "@value": text });
            if let Some(lang) = lang {
                object["@language"] = json!(lang);
            }
            object
        }
        NewData::Uri { uri, label } => json!({ "type": "uri", "@id": uri, "o:label": label }),
        NewData::Item(item) => json!({ "type": "resource:item", "value_resource_id": item.0 }),
    };
    object["property_id"] = json!(property.0);
    object["is_public"] = json!(value.is_public);
    let mut body = Map::new();
    body.insert(term.to_string(), json!([object]));
    let parsed = crate::write::values::collect(api, &Payload::from_object(body), &[]).await?;
    let [input] = parsed.as_slice() else {
        return Err(ApiError::bad_request(format!("the value for {term} is not valid")).into());
    };
    Ok(crate::write::values::content(input))
}

impl ValueEdits<ValueContent> {
    /// The value rows the edits change on an item that holds `values`, in id order. Empty when
    /// the item already holds the result: a value to add that the field holds already (the same
    /// type, text or URI or link, and language) is not added again.
    #[must_use]
    pub fn plan(&self, values: &[ValueContent]) -> ValueWrites {
        let mut writes = ValueWrites::default();
        for (property, edit) in &self.fields {
            let mut held = Vec::new();
            for (at, value) in values.iter().enumerate() {
                if value.property_id != *property {
                    continue;
                }
                match rewritten(edit.rewrite.as_ref(), value) {
                    None => {
                        writes.delete.insert(at);
                    }
                    Some(after) if after != *value => {
                        writes.update.insert(at, after.clone());
                        held.push(after);
                    }
                    Some(_) => held.push(value.clone()),
                }
            }
            for value in &edit.add {
                if !held.iter().any(|other| same_value(other, value)) {
                    writes.insert.push(value.clone());
                    held.push(value.clone());
                }
            }
        }
        writes
    }
}

/// A value after a rewrite; `None` when it goes.
fn rewritten(rewrite: Option<&Rewrite>, value: &ValueContent) -> Option<ValueContent> {
    let mut after = value.clone();
    match rewrite {
        None => {}
        Some(Rewrite::Clear) => return None,
        Some(Rewrite::Replace { from, to }) => {
            let text_only = value.uri.is_none() && value.value_resource_id.is_none();
            if text_only && value.value.as_deref() == Some(from.as_str()) {
                after.value = Some(to.clone());
            }
        }
        Some(Rewrite::Visibility(is_public)) => after.is_public = *is_public,
    }
    Some(after)
}

/// The import's `append` sameness: the term, the type, the text and the language; the text of a
/// URI is the URI and of a link the linked resource.
fn same_value(a: &ValueContent, b: &ValueContent) -> bool {
    let keyed = |value: &ValueContent| value.uri.is_some() || value.value_resource_id.is_some();
    a.property_id == b.property_id
        && a.r#type == b.r#type
        && a.lang == b.lang
        && a.uri == b.uri
        && a.value_resource_id == b.value_resource_id
        && (keyed(a) || keyed(b) || a.value == b.value)
}

/// One operation as the wire names it.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum OpWire {
    Add {
        property: Id,
        value: DataWire,
        #[serde(default = "public", rename = "isPublic")]
        is_public: bool,
    },
    Replace {
        property: Id,
        from: String,
        to: String,
    },
    Clear {
        property: Id,
    },
    Visibility {
        property: Id,
        #[serde(rename = "isPublic")]
        is_public: bool,
    },
}

fn public() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum DataWire {
    Literal {
        text: String,
        #[serde(default)]
        lang: Option<String>,
    },
    Uri {
        uri: String,
        #[serde(default)]
        label: Option<String>,
    },
    Item {
        item: Id,
    },
}

enum Step {
    Rewrite(Rewrite),
    Add(NewValue),
}

impl OpWire {
    fn into_step(self) -> Result<(PropertyId, Step), String> {
        Ok(match self {
            Self::Add {
                property,
                value,
                is_public,
            } => (
                PropertyId(property.0),
                Step::Add(NewValue {
                    data: value.into_data()?,
                    is_public,
                }),
            ),
            Self::Replace { property, from, to } => {
                if from.is_empty() || !literal_is_valid(&to) {
                    return Err("a replace needs the text to find and a text to put".to_string());
                }
                if from == to {
                    return Err("a replace puts the text it finds".to_string());
                }
                (
                    PropertyId(property.0),
                    Step::Rewrite(Rewrite::Replace { from, to }),
                )
            }
            Self::Clear { property } => (PropertyId(property.0), Step::Rewrite(Rewrite::Clear)),
            Self::Visibility {
                property,
                is_public,
            } => (
                PropertyId(property.0),
                Step::Rewrite(Rewrite::Visibility(is_public)),
            ),
        })
    }
}

impl DataWire {
    fn into_data(self) -> Result<NewData, String> {
        let some = |text: Option<String>| {
            text.map(|text| text.trim().to_string())
                .filter(|text| !text.is_empty())
        };
        match self {
            Self::Literal { text, lang } => {
                if !literal_is_valid(&text) {
                    return Err("a text value is empty".to_string());
                }
                Ok(NewData::Literal {
                    text,
                    lang: some(lang),
                })
            }
            Self::Uri { uri, label } => {
                if !uri_is_valid(&uri) {
                    return Err(format!("\"{uri}\" is not a URI"));
                }
                Ok(NewData::Uri {
                    uri: uri.trim().to_string(),
                    label: some(label),
                })
            }
            Self::Item { item } => Ok(NewData::Item(ItemId(item.0))),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use omeka_db::ids::ResourceId;

    use super::*;

    fn parse(ops: Value) -> Result<Option<ValueEdits>, String> {
        let ops: Vec<OpWire> = serde_json::from_value(ops).map_err(|error| error.to_string())?;
        ValueEdits::from_wire(ops)
    }

    fn text(property: u64, text: &str) -> ValueContent {
        ValueContent::literal(PropertyId(property), text)
    }

    /// The edits with each value to add resolved as the `PATCH` parser would store a plain text.
    fn resolved(edits: &ValueEdits) -> ValueEdits<ValueContent> {
        ValueEdits {
            fields: edits
                .fields()
                .map(|(property, edit)| {
                    let add = edit
                        .add
                        .iter()
                        .map(|value| match &value.data {
                            NewData::Literal { text, lang } => ValueContent {
                                lang: lang.clone(),
                                is_public: value.is_public,
                                ..ValueContent::literal(property, text)
                            },
                            NewData::Uri { uri, label } => ValueContent {
                                r#type: "uri".to_string(),
                                uri: Some(uri.clone()),
                                value: label.clone(),
                                ..ValueContent::literal(property, "")
                            },
                            NewData::Item(item) => ValueContent {
                                r#type: "resource:item".to_string(),
                                value: None,
                                value_resource_id: Some(ResourceId(item.0)),
                                ..ValueContent::literal(property, "")
                            },
                        })
                        .collect();
                    (
                        property,
                        FieldEdit {
                            rewrite: edit.rewrite.clone(),
                            add,
                        },
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn the_wire_list_reads_back_whatever_its_order() {
        let edits = parse(json!([
            { "op": "add", "property": 3, "value": { "type": "literal", "text": "Maps", "lang": " en " } },
            { "op": "clear", "property": 3 },
            { "op": "add", "property": 4, "isPublic": false,
              "value": { "type": "uri", "uri": "https://example.org/x", "label": "" } },
            { "op": "replace", "property": 5, "from": "Harbor", "to": "Harbour" },
            { "op": "visibility", "property": 6, "isPublic": false },
            { "op": "add", "property": 7, "value": { "type": "item", "item": 9 } },
        ]))
        .unwrap()
        .unwrap();
        let (_, subject) = edits.fields().next().unwrap();
        assert_eq!(subject.rewrite, Some(Rewrite::Clear), "clear runs first");
        assert_eq!(
            subject.add[0].data,
            NewData::Literal {
                text: "Maps".to_string(),
                lang: Some("en".to_string())
            }
        );
        let uri = &edits.fields().nth(1).unwrap().1.add[0];
        assert_eq!(
            uri.data,
            NewData::Uri {
                uri: "https://example.org/x".to_string(),
                label: None
            },
            "an empty label is no label"
        );
        assert!(!uri.is_public);
        let ops: Vec<OpWire> = serde_json::from_value(edits.to_json()).unwrap();
        assert_eq!(ValueEdits::from_wire(ops).unwrap().unwrap(), edits);
        assert_eq!(parse(json!([])).unwrap(), None);
    }

    #[test]
    fn a_list_that_contradicts_itself_or_names_nothing_to_write_is_refused() {
        for ops in [
            json!([{ "op": "clear", "property": 3 }, { "op": "visibility", "property": 3, "isPublic": true }]),
            json!([{ "op": "replace", "property": 3, "from": "a", "to": "b" }, { "op": "clear", "property": 3 }]),
            json!([{ "op": "add", "property": 3, "value": { "type": "literal", "text": "a" } },
                   { "op": "add", "property": 3, "value": { "type": "literal", "text": "a" } }]),
            json!([{ "op": "add", "property": 3, "value": { "type": "literal", "text": "  " } }]),
            json!([{ "op": "add", "property": 3, "value": { "type": "uri", "uri": "javascript:alert(1)" } }]),
            json!([{ "op": "add", "property": 3, "value": { "type": "item", "item": 0 } }]),
            json!([{ "op": "add", "property": 3, "value": { "type": "literal", "text": "a", "uri": "x" } }]),
            json!([{ "op": "replace", "property": 3, "from": "", "to": "b" }]),
            json!([{ "op": "replace", "property": 3, "from": "a", "to": " " }]),
            json!([{ "op": "replace", "property": 3, "from": "a", "to": "a" }]),
            json!([{ "op": "visibility", "property": 3 }]),
            json!([{ "op": "set", "property": 3 }]),
            json!([{ "op": "clear", "property": 0 }]),
        ] {
            assert!(parse(ops.clone()).is_err(), "{ops}");
        }
    }

    #[test]
    fn a_plan_rewrites_in_place_deletes_and_adds_only_what_changes() {
        let edits = parse(json!([
            { "op": "replace", "property": 1, "from": "Harbor", "to": "Harbour" },
            { "op": "clear", "property": 2 },
            { "op": "visibility", "property": 3, "isPublic": false },
            { "op": "add", "property": 3, "value": { "type": "literal", "text": "Maps" } },
            { "op": "add", "property": 4, "value": { "type": "literal", "text": "Rivers" } },
        ]))
        .unwrap()
        .unwrap();
        let values = vec![
            text(1, "Harbor"),
            text(2, "old"),
            text(3, "Maps"),
            text(1, "Harbor view"),
            text(2, "older"),
        ];
        let plan = resolved(&edits).plan(&values);
        assert_eq!(plan.delete, [1, 4].into());
        assert_eq!(
            plan.update,
            [
                (0, text(1, "Harbour")),
                (
                    2,
                    ValueContent {
                        is_public: false,
                        ..text(3, "Maps")
                    }
                ),
            ]
            .into(),
            "only the exact text is replaced"
        );
        assert_eq!(
            plan.insert,
            vec![text(4, "Rivers")],
            "Maps is there already: its visibility changes, and it is not added again"
        );
        assert_eq!(
            plan.applied(&values),
            vec![
                text(1, "Harbour"),
                ValueContent {
                    is_public: false,
                    ..text(3, "Maps")
                },
                text(1, "Harbor view"),
                text(4, "Rivers"),
            ]
        );
    }

    #[test]
    fn an_item_that_holds_the_result_has_an_empty_plan() {
        let edits = parse(json!([
            { "op": "replace", "property": 1, "from": "Harbor", "to": "Harbour" },
            { "op": "visibility", "property": 3, "isPublic": true },
            { "op": "add", "property": 3, "value": { "type": "literal", "text": "Maps" } },
            { "op": "clear", "property": 2 },
        ]))
        .unwrap()
        .unwrap();
        let values = vec![text(1, "Harbour"), text(3, "Maps")];
        assert!(resolved(&edits).plan(&values).is_empty());
        let other_language = ValueContent {
            lang: Some("fr".to_string()),
            ..text(3, "Maps")
        };
        assert_eq!(
            resolved(&edits).plan(&[other_language]).insert,
            vec![text(3, "Maps")],
            "a value in another language is another value"
        );
    }
}
