//! Export: the JSON-LD documents the list endpoints serve, for the CLI.
//!
//! The read layer already builds and serializes one document per resource, so
//! this module does not add a second serializer. It opens the list window so
//! one call returns every row, reads the served body back and hands the caller
//! the documents `/api/{resource}` would answer.
//!
//! Upstream has no bulk export endpoint. The nearest thing is
//! `Admin/ResourceTemplateController::exportAction`, which writes one resource
//! template as pretty JSON. Bulk export is a CLI-level need over the same
//! representation the API serves, so it belongs beside the read layer rather
//! than in a new HTTP route.

use serde_json::Value;
use sqlx::SqlitePool;

use crate::adapters::ADAPTERS;
use crate::auth::Actor;
use crate::errors::ApiError;
use crate::jsonld::BaseUrl;
use crate::routes::Operation;

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
/// The list window opens to one page. `sorted_page` and its siblings load every
/// row the SQL filter matches and slice the page afterwards
/// (`read/params.rs`), so paging here would repeat the load instead of bounding
/// it: the widest page is both the simplest and the cheapest call.
///
/// Order is the endpoint's default `sort_by=id&sort_order=asc`.
///
/// # Errors
/// The read layer's errors for the resource, plus an internal error when the
/// served body is not the JSON array a list endpoint answers.
pub async fn documents(
    pool: &SqlitePool,
    base: BaseUrl,
    actor: Actor,
    selection: &Selection,
) -> Result<Vec<Value>, ApiError> {
    let mut pairs = vec![("per_page".to_string(), i64::MAX.to_string())];
    if let Some(id) = selection.item_set {
        pairs.push(("item_set_id".to_string(), id.to_string()));
    }
    let api = Api::load(pool, base, actor).await?;
    let response = api
        .search(&selection.resource, &Params::from_decoded(pairs))
        .await?;
    serde_json::from_str(&response.body).map_err(ApiError::internal)
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
