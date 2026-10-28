//! Export: the JSON-LD documents the list endpoints serve, and the items as CSV.
//!
//! The read layer already builds and serializes one document per resource, so
//! this module does not add a second serializer. It opens the list window so
//! one call returns every row and reads the served body back: [`documents`]
//! hands the caller the documents `/api/{resource}` would answer, and
//! [`items_csv`] flattens the items `/api/items` would answer into CSV. The CLI
//! (`romeka export`) and the admin download (`GET /api/romeka/items.csv`) both
//! call them, so a file holds what the list holds for the same caller.
//!
//! Upstream has no bulk export. The nearest thing is
//! `Admin/ResourceTemplateController::exportAction`, which writes one resource
//! template as pretty JSON. So there is no upstream CSV column set to match;
//! this one is the port's:
//!
//! - One row per item, one header cell per value column.
//! - A cell names a property by its term, `prefix:local_name`.
//! - The default column set is every property that holds a value on any
//!   exported item: `dcterms:title` first, then the rest by term. A caller may
//!   name the terms instead, in its order.
//! - A property with several values on any item gets one column per value, and
//!   the header repeats the term, so a multi-valued item keeps every value.
//! - Values flatten to text: a literal is its `@value`, a URI its `@id`, a
//!   resource value its display title. A language tag is dropped.
//! - The file is [`crate::csv`]'s: a byte order mark, then CRLF records.
//!
//! The header depends on every row, so the file is built whole, not streamed.

use std::collections::{BTreeMap, BTreeSet};

use omeka_db::DbPool;
use serde_json::Value;

use crate::adapters::ADAPTERS;
use crate::auth::Actor;
use crate::csv::{BYTE_ORDER_MARK, push_line};
use crate::errors::ApiError;
use crate::jsonld::BaseUrl;
use crate::routes::Operation;
use crate::wire::Value as WireValue;
use crate::wire::item::Item as ItemDocument;

use super::Api;
use super::params::Params;

/// The resource names an export can list: every registered adapter that
/// implements `search`, in registry order.
///
/// The list comes from the adapter registry rather than a second table beside
/// the dispatch in [`Api::search`]. `resources` is here: the generic adapter
/// serves `search` now, and its list is every resource kind (`ResourceAdapter`).
/// An export of `resources` ignores `--item-set`, which is the adapter's own
/// `buildQuery` behavior (`AbstractResourceEntityAdapter` has no `item_set_id`
/// filter; `ItemAdapter` adds it).
pub fn resources() -> Vec<&'static str> {
    ADAPTERS
        .iter()
        .filter(|spec| spec.operations.contains(&Operation::Search))
        .map(|spec| spec.name)
        .collect()
}

/// What to export: one resource, optionally narrowed to one item set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    /// An [`resources`] entry.
    pub resource: String,
    /// An item set id. `ItemAdapter::buildQuery` reads `item_set_id`; the other
    /// adapters ignore it.
    pub item_set: Option<u64>,
}

impl Selection {
    pub fn new(resource: impl Into<String>) -> Self {
        Self {
            resource: resource.into(),
            item_set: None,
        }
    }

    /// Narrow the selection to one item set.
    pub fn in_item_set(mut self, id: u64) -> Self {
        self.item_set = Some(id);
        self
    }
}

/// Every document the list endpoint for `selection.resource` serves.
///
/// Order is the endpoint's default `sort_by=id&sort_order=asc`.
///
/// # Errors
/// The read layer's errors for the resource, plus an internal error when the
/// served body is not the JSON array a list endpoint answers.
pub async fn documents(
    pool: &DbPool,
    base: BaseUrl,
    actor: Actor,
    selection: &Selection,
) -> Result<Vec<Value>, ApiError> {
    let mut pairs = Vec::new();
    if let Some(id) = selection.item_set {
        pairs.push(("item_set_id".to_string(), id.to_string()));
    }
    let api = Api::load(pool, base, actor).await?;
    let response = api
        .search(
            &selection.resource,
            &every_row(&Params::from_decoded(pairs)),
        )
        .await?;
    serde_json::from_str(&response.body).map_err(ApiError::internal)
}

/// The query with its window opened to every row it matches.
///
/// `page`, `per_page`, `limit` and `offset` go, and one page as wide as the
/// list takes their place; the filters and the sort stay. `sorted_page` and its
/// siblings load every row the SQL filter matches and slice the page afterwards
/// (`read/params.rs`), so paging here would repeat the load instead of bounding
/// it: the widest page is both the simplest and the cheapest call.
fn every_row(query: &Params) -> Params {
    let mut pairs: Vec<(String, String)> = query
        .decoded_pairs()
        .into_iter()
        .filter(|(name, _)| !matches!(name.as_str(), "page" | "per_page" | "limit" | "offset"))
        .collect();
    pairs.push(("per_page".to_string(), i64::MAX.to_string()));
    Params::from_decoded(pairs)
}

/// The items of one search as a CSV file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemsCsv {
    /// The data rows: the list's total for the same query and caller.
    pub items: usize,
    /// The header, with a term repeated once per value column.
    pub columns: Vec<String>,
    pub text: String,
}

/// Every item `GET /api/items?<query>` lists for `api`'s caller, whatever page
/// the query names, as CSV in the list's order. `columns` names the property
/// terms to write; empty derives them from the items.
///
/// The served JSON body, the parsed items, and the flattened rows with the file
/// are each held whole in turn; each is dropped before the next is built.
///
/// # Errors
/// The read layer's errors for an items search, plus an internal error when the
/// served body is not a list of items.
pub async fn items_csv(
    api: &Api<'_>,
    query: &Params,
    columns: &[String],
) -> Result<ItemsCsv, ApiError> {
    let body = api.search("items", &every_row(query)).await?.body;
    let items: Vec<ItemDocument> = serde_json::from_str(&body).map_err(ApiError::internal)?;
    drop(body);
    let rows: Vec<ItemRow> = items.iter().map(item_row).collect();
    drop(items);
    let header = header(&rows, columns);
    let mut text = String::from(BYTE_ORDER_MARK);
    let terms: Vec<String> = header.iter().map(|column| column.term.clone()).collect();
    push_line(&mut text, &terms);
    for row in &rows {
        let cells: Vec<String> = header
            .iter()
            .map(|column| {
                row.get(&column.term)
                    .and_then(|values| values.get(column.index))
                    .cloned()
                    .unwrap_or_default()
            })
            .collect();
        push_line(&mut text, &cells);
    }
    Ok(ItemsCsv {
        items: rows.len(),
        columns: terms,
        text,
    })
}

/// One item's values as text, keyed by property term.
type ItemRow = BTreeMap<String, Vec<String>>;

fn item_row(item: &ItemDocument) -> ItemRow {
    item.values
        .iter()
        .map(|(term, values)| (term.clone(), values.iter().map(flatten).collect()))
        .collect()
}

/// One value as text: a literal is its `@value`, a URI its `@id`, a resource
/// value its display title, or its `@id` when it has no title.
fn flatten(value: &WireValue) -> String {
    match value {
        WireValue::Literal(literal) => literal.value.clone(),
        WireValue::Uri(uri) => uri.id.clone(),
        WireValue::Resource(resource) => resource
            .display_title
            .clone()
            .unwrap_or_else(|| resource.id.clone()),
    }
}

/// One header cell: a property term and which of its values it carries.
struct Column {
    term: String,
    index: usize,
}

/// The header: each term once per value the fullest item holds, and at least
/// once.
fn header(rows: &[ItemRow], requested: &[String]) -> Vec<Column> {
    let mut header = Vec::new();
    for term in header_terms(rows, requested) {
        let count = rows
            .iter()
            .map(|row| row.get(&term).map_or(0, Vec::len))
            .max()
            .unwrap_or(0)
            .max(1);
        for index in 0..count {
            header.push(Column {
                term: term.clone(),
                index,
            });
        }
    }
    header
}

/// The terms in file order: the requested list, or every term that holds a
/// value, title first. `dcterms:title` leads because it is the item's
/// denormalised title, the first non-empty title value.
fn header_terms(rows: &[ItemRow], requested: &[String]) -> Vec<String> {
    if !requested.is_empty() {
        return requested.to_vec();
    }
    let mut terms: BTreeSet<String> = rows.iter().flat_map(|row| row.keys().cloned()).collect();
    let mut ordered = Vec::with_capacity(terms.len());
    if terms.remove("dcterms:title") {
        ordered.push("dcterms:title".to_string());
    }
    ordered.extend(terms);
    ordered
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// Every searchable adapter is one the dispatch answers. `resources` serves
    /// `search` through `ResourceAdapter`, so it joins the list.
    #[test]
    fn the_export_list_is_the_searchable_adapters() {
        let names = resources();
        assert!(names.contains(&"items"));
        assert!(names.contains(&"data_types"));
        assert!(names.contains(&"resources"));
        assert!(names.contains(&"custom_vocabs"));
        assert_eq!(names.len(), 18);
    }
}
