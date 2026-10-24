//! Which items a run selects: the browse query, as the items list reads it.
//!
//! The query is the nested JSON object an item pool stores (`{"search": "x", "property":
//! [...]}`), turned into the list's parameters the way `site_items` turns a pool. A query that
//! names only `id` is a ticked list, which needs the lesser right; anything else selects every
//! item that matches. The window and the order (`page`, `per_page`, `sort_by`, `sort_order`)
//! choose no items, so they neither widen a ticked list nor narrow the selection.

use omeka_core::permissions::Action;
use omeka_db::ids::ItemId;
use serde_json::{Map, Value};

use crate::errors::ApiError;
use crate::read::params::Params;
use crate::read::{Api, matching_item_ids};
use crate::site_items::query_pairs;

/// The members of a list query that choose no items.
const WINDOW: [&str; 6] = [
    "page",
    "per_page",
    "sort_by",
    "sort_order",
    "limit",
    "offset",
];

/// The items a query selects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// The items a person ticked, by id.
    Ticked(Vec<ItemId>),
    /// Every item the query matches.
    Matching,
}

impl Scope {
    /// Read the scope of a browse query.
    ///
    /// # Errors
    /// A 400 for a query that is not an object, or whose `id` is not a list of ids: an empty
    /// `id` list would read as no filter and select every item.
    pub fn of(query: &Value) -> Result<Self, ApiError> {
        let Value::Object(members) = query else {
            return Err(refused("\"query\" must be an object"));
        };
        let ids = match members.get("id") {
            None => None,
            Some(ids) => Some(ids_of(ids)?),
        };
        let filters = members.keys().filter(|key| !WINDOW.contains(&key.as_str()));
        match ids {
            Some(ids) if filters.count() == 1 => Ok(Self::Ticked(ids)),
            _ => Ok(Self::Matching),
        }
    }

    /// The batch privilege the scope needs.
    #[must_use]
    pub fn right(&self) -> Action {
        match self {
            Self::Ticked(_) => Action::BatchUpdate,
            Self::Matching => Action::BatchUpdateAll,
        }
    }
}

/// The ids `query` selects for the caller, ascending: those `GET /api/items` lists for it.
pub(super) async fn select(api: &Api<'_>, query: &Value) -> Result<Vec<ItemId>, ApiError> {
    let filters: Map<String, Value> = match query {
        Value::Object(members) => members
            .iter()
            .filter(|(key, _)| !WINDOW.contains(&key.as_str()))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
        _ => Map::new(),
    };
    let params = Params::from_decoded(query_pairs(&Value::Object(filters)));
    matching_item_ids(api, &params, None).await
}

fn ids_of(value: &Value) -> Result<Vec<ItemId>, ApiError> {
    let list = value
        .as_array()
        .filter(|list| !list.is_empty())
        .ok_or_else(|| refused("\"query.id\" must be a list of item ids"))?;
    list.iter()
        .map(|id| {
            id.as_u64()
                .filter(|id| *id > 0)
                .map(ItemId)
                .ok_or_else(|| refused("\"query.id\" must be a list of item ids"))
        })
        .collect()
}

fn refused(reason: &str) -> ApiError {
    ApiError::bad_request(crate::messages::import_request_unreadable(reason))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use serde_json::json;

    use super::*;

    #[test]
    fn a_query_of_ids_alone_is_a_ticked_list_and_anything_more_matches() {
        assert_eq!(
            Scope::of(&json!({ "id": [3, 1], "sort_by": "title", "page": 2 })).unwrap(),
            Scope::Ticked(vec![ItemId(3), ItemId(1)])
        );
        for query in [
            json!({}),
            json!({ "search": "letter" }),
            json!({ "id": [3], "search": "letter" }),
        ] {
            assert_eq!(Scope::of(&query).unwrap(), Scope::Matching, "{query}");
        }
    }

    #[test]
    fn an_empty_or_malformed_id_list_is_refused_rather_than_read_as_everything() {
        for query in [
            json!({ "id": [] }),
            json!({ "id": ["3"] }),
            json!({ "id": 3 }),
            json!({ "id": [0] }),
            json!([1, 2]),
        ] {
            assert!(Scope::of(&query).is_err(), "{query}");
        }
    }
}
