//! The items on a site: one place that changes a site's `item_site` rows in bulk.
//!
//! A site's membership is the set of its `item_site` rows. A change to it is a [`Command`]: an
//! [`Action`] over a [`Selector`]. [`plan`] computes the rows to attach and detach from the
//! current membership and the selected items, and [`apply`] reads both, plans, and writes. The
//! admin's "Items on this site" pane reaches it through `POST /api/sites/{id}/items` ([`handle`]),
//! and `romeka`'s `Omeka\Job\UpdateSiteItems` reaches it directly, so the two cannot drift.
//!
//! A selector resolves through the items list endpoint's own query ([`matching_item_ids`]), so a
//! stored `o:item_pool` selects the items `/api/items?<pool>` lists for the same caller.
//!
//! **Keep in sync.** A site whose `assign_new_items` is true takes each new item its `o:item_pool`
//! selects: an item create that names no `o:site` attaches the item to each such site
//! ([`attach_on_create`]). An edit never attaches, so an item someone removed stays removed, as
//! upstream's flag behaves. Upstream attaches every new item to such a site; here the pool narrows
//! it. The admin writes the pool from the item sets a curator picked (`item_set_id`), and the empty
//! pool selects every new item, so a site upstream made behaves the same.
//!
//! The route is Romeka's own. Upstream runs the same change as a job after its server-rendered
//! resources form, and has no API for it. It writes only `item_site`, the table upstream's job
//! writes, so a site changed here reads the same in upstream.

use std::collections::BTreeSet;

use omeka_db::ids::{ItemId, SiteId};
use omeka_db::repos::{ItemSiteRepo, Paging, SiteFilter, SiteRepo};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::errors::ApiError;
use crate::messages;
use crate::read::params::Params;
use crate::read::response::serialize;
use crate::read::{Api, ApiResponse, db_error, matching_item_ids, not_found, parse_id};

const SITE_CLASS: &str = "Omeka\\Entity\\Site";

/// What a command does with the items its selector names (`UpdateSiteItems::$actions`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Keep the site's items and attach the selected ones.
    Add,
    /// Detach the selected items.
    Remove,
    /// Make the selected items the site's whole membership.
    Replace,
}

/// The items a command acts on.
#[derive(Debug, Clone, PartialEq)]
pub enum Selector {
    /// An item query in the shape `o:item_pool` stores: the nested array PHP `parse_str` builds.
    /// Anything that is not an object or array selects every item, as `UpdateSiteItems` reads a
    /// query that is not an array.
    Query(Value),
    /// These items, together with the items in any of these item sets. Both empty names nothing.
    Picked {
        items: Vec<ItemId>,
        item_sets: Vec<u64>,
    },
}

/// One membership change.
#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    pub action: Action,
    pub selector: Selector,
}

/// Whether [`apply`] writes the change or only counts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Write,
    DryRun,
}

/// What a command did, or would do: the answer of `POST /api/sites/{id}/items`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Change {
    /// How many items the selector named.
    pub matched: usize,
    /// How many items the command attaches.
    pub added: usize,
    /// How many items the command detaches.
    pub removed: usize,
    /// How many items the site holds after the command.
    pub total: usize,
}

/// The rows one command attaches and detaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub attach: Vec<ItemId>,
    pub detach: Vec<ItemId>,
}

/// The rows `action` changes, given the site's items and the selected items.
pub fn plan(action: Action, current: &BTreeSet<ItemId>, selected: &BTreeSet<ItemId>) -> Plan {
    let missing = || selected.difference(current).copied().collect();
    match action {
        Action::Add => Plan {
            attach: missing(),
            detach: Vec::new(),
        },
        Action::Remove => Plan {
            attach: Vec::new(),
            detach: selected.intersection(current).copied().collect(),
        },
        Action::Replace => Plan {
            attach: missing(),
            detach: current.difference(selected).copied().collect(),
        },
    }
}

/// Resolve a command against a site, then write it unless `mode` is a dry run.
///
/// # Errors
/// A database error, or the read layer's error for the selector's query.
pub async fn apply(
    api: &Api<'_>,
    site: SiteId,
    command: &Command,
    mode: Mode,
) -> Result<Change, ApiError> {
    let selected: BTreeSet<ItemId> = select(api, &command.selector).await?.into_iter().collect();
    let repo = ItemSiteRepo::new(api.pool());
    let current: BTreeSet<ItemId> = repo
        .site_items(site)
        .await
        .map_err(db_error)?
        .into_iter()
        .collect();
    let plan = plan(command.action, &current, &selected);
    if mode == Mode::Write {
        repo.detach(site, &plan.detach).await.map_err(db_error)?;
        repo.attach(site, &plan.attach).await.map_err(db_error)?;
    }
    Ok(Change {
        matched: selected.len(),
        added: plan.attach.len(),
        removed: plan.detach.len(),
        total: current.len() + plan.attach.len() - plan.detach.len(),
    })
}

/// The items a selector names that this caller can see.
async fn select(api: &Api<'_>, selector: &Selector) -> Result<Vec<ItemId>, ApiError> {
    let (items, item_sets) = match selector {
        Selector::Query(query) => {
            return matching_item_ids(api, &Params::from_decoded(query_pairs(query)), None).await;
        }
        Selector::Picked { items, item_sets } => (items, item_sets),
    };
    let mut selected = BTreeSet::new();
    // An empty list names nothing; as a query it would add no filter and name everything.
    if !items.is_empty() {
        let pairs = list_pairs("id", items.iter().map(|id| id.0));
        selected.extend(matching_item_ids(api, &Params::from_decoded(pairs), None).await?);
    }
    if !item_sets.is_empty() {
        let pairs = list_pairs("item_set_id", item_sets.iter().copied());
        selected.extend(matching_item_ids(api, &Params::from_decoded(pairs), None).await?);
    }
    Ok(selected.into_iter().collect())
}

fn list_pairs(name: &str, ids: impl Iterator<Item = u64>) -> Vec<(String, String)> {
    ids.enumerate()
        .map(|(at, id)| (format!("{name}[{at}]"), id.to_string()))
        .collect()
}

/// `http_build_query` over a stored query, as decoded pairs: `{"property": [{"type": "eq"}]}`
/// is `property[0][type]=eq`. A boolean is `1` or `0`, and a null is dropped, as PHP writes them.
fn query_pairs(query: &Value) -> Vec<(String, String)> {
    fn push(key: String, value: &Value, pairs: &mut Vec<(String, String)>) {
        let child = |name: &str| {
            if key.is_empty() {
                name.to_string()
            } else {
                format!("{key}[{name}]")
            }
        };
        match value {
            Value::Object(members) => {
                for (name, member) in members {
                    push(child(name), member, pairs);
                }
            }
            Value::Array(entries) => {
                for (at, entry) in entries.iter().enumerate() {
                    push(child(&at.to_string()), entry, pairs);
                }
            }
            Value::String(text) => pairs.push((key, text.clone())),
            Value::Number(number) => pairs.push((key, number.to_string())),
            Value::Bool(flag) => pairs.push((key, if *flag { "1" } else { "0" }.to_string())),
            Value::Null => {}
        }
    }
    let mut pairs = Vec::new();
    if query.is_object() || query.is_array() {
        push(String::new(), query, &mut pairs);
    }
    pairs
}

/// The sites that keep in sync with their item pool and whose pool selects this item.
async fn synced_sites_matching(api: &Api<'_>, item: ItemId) -> Result<Vec<SiteId>, ApiError> {
    let sites = SiteRepo::new(api.pool())
        .list(
            &SiteFilter {
                assign_new_items: Some(true),
                ..SiteFilter::default()
            },
            &Paging::default(),
        )
        .await
        .map_err(db_error)?;
    let mut matching = Vec::new();
    for site in sites {
        let params = Params::from_decoded(query_pairs(&site.item_pool));
        if !matching_item_ids(api, &params, Some(&[item]))
            .await?
            .is_empty()
        {
            matching.push(site.id);
        }
    }
    Ok(matching)
}

/// Attach a newly created item to every synced site whose pool selects it.
///
/// Every path that creates an item calls this once the item, its values and its item sets are
/// written: the API create, `romeka import csv` and the archive import. No update path calls it.
///
/// # Errors
/// A database error, or the read layer's error for a site's pool.
pub async fn attach_on_create(api: &Api<'_>, item: ItemId) -> Result<(), ApiError> {
    let repo = ItemSiteRepo::new(api.pool());
    for site in synced_sites_matching(api, item).await? {
        repo.attach(site, &[item]).await.map_err(db_error)?;
    }
    Ok(())
}

/// The body of `POST /api/sites/{id}/items`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Body {
    action: Action,
    #[serde(default)]
    dry_run: bool,
    query: Option<Value>,
    items: Option<Vec<u64>>,
    item_sets: Option<Vec<u64>>,
}

/// Parse a request body into a command and its mode. The body names a query, or items, item sets
/// or both, which select their union.
///
/// # Errors
/// A 400 naming what is wrong with the body.
pub fn parse_body(bytes: &[u8]) -> Result<(Command, Mode), ApiError> {
    let body: Body =
        serde_json::from_slice(bytes).map_err(|error| ApiError::bad_request(error.to_string()))?;
    let selector = match (body.query, body.items, body.item_sets) {
        (Some(query), None, None) => Selector::Query(query),
        (None, None, None) | (Some(_), _, _) => {
            return Err(ApiError::bad_request(messages::SITE_ITEMS_ONE_SELECTOR));
        }
        (None, items, item_sets) => Selector::Picked {
            items: items.unwrap_or_default().into_iter().map(ItemId).collect(),
            item_sets: item_sets.unwrap_or_default(),
        },
    };
    let mode = if body.dry_run {
        Mode::DryRun
    } else {
        Mode::Write
    };
    Ok((
        Command {
            action: body.action,
            selector,
        },
        mode,
    ))
}

/// `POST /api/sites/{id}/items`: check that the caller may edit the site's resources, then apply
/// the body's command and answer the [`Change`].
///
/// The permission is the site update upstream's resources form saves through: the site's owner,
/// a site admin, or a global or site administrator.
///
/// # Errors
/// 404 for an unknown site, 403 for a caller who may not edit it, 400 for a malformed body.
pub async fn handle(
    api: &Api<'_>,
    raw_site_id: &str,
    bytes: &[u8],
) -> Result<ApiResponse, ApiError> {
    let id = parse_id(SITE_CLASS, raw_site_id)?;
    let Some(site) = SiteRepo::new(api.pool())
        .get(SiteId(id))
        .await
        .map_err(db_error)?
    else {
        return Err(not_found(SITE_CLASS, raw_site_id));
    };
    api.authorize_site_update(site.id, site.owner_id).await?;
    let (command, mode) = parse_body(bytes)?;
    let change = apply(api, site.id, &command, mode).await?;
    Ok(ApiResponse::document(serialize(&change)?))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn ids(raw: &[u64]) -> BTreeSet<ItemId> {
        raw.iter().copied().map(ItemId).collect()
    }

    #[test]
    fn add_attaches_only_the_items_the_site_lacks() {
        let plan = plan(Action::Add, &ids(&[1, 2]), &ids(&[2, 3]));
        assert_eq!(plan.attach, vec![ItemId(3)]);
        assert!(plan.detach.is_empty());
    }

    #[test]
    fn remove_detaches_only_the_items_the_site_holds() {
        let plan = plan(Action::Remove, &ids(&[1, 2]), &ids(&[2, 3]));
        assert!(plan.attach.is_empty());
        assert_eq!(plan.detach, vec![ItemId(2)]);
    }

    #[test]
    fn replace_makes_the_selection_the_whole_membership() {
        let plan = plan(Action::Replace, &ids(&[1, 2]), &ids(&[2, 3]));
        assert_eq!(plan.attach, vec![ItemId(3)]);
        assert_eq!(plan.detach, vec![ItemId(1)]);
    }

    #[test]
    fn a_body_names_a_query_or_items_and_item_sets() {
        let (command, mode) =
            parse_body(br#"{"action":"add","item_sets":[4],"dry_run":true}"#).expect("parses");
        assert_eq!(
            command.selector,
            Selector::Picked {
                items: Vec::new(),
                item_sets: vec![4]
            }
        );
        assert_eq!(mode, Mode::DryRun);
        let (both, _) = parse_body(br#"{"action":"add","items":[1],"item_sets":[2]}"#).unwrap();
        assert_eq!(
            both.selector,
            Selector::Picked {
                items: vec![ItemId(1)],
                item_sets: vec![2]
            }
        );
        assert!(parse_body(br#"{"action":"add"}"#).is_err());
        assert!(parse_body(br#"{"action":"add","query":{},"items":[1]}"#).is_err());
        assert!(parse_body(br#"{"action":"swap","items":[1]}"#).is_err());
    }
}
