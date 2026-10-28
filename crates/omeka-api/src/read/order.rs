//! The order of an items, item sets or media list.
//!
//! `AbstractResourceEntityAdapter::sortQuery` orders these lists by a resource
//! column, by the values of a property term, or by the label of the resource's
//! class, template or owner. Every text order here is shelf order
//! ([`compare_natural`]): `AF_2` before `AF_10`, case folded. A row with no
//! text sorts last in both directions. A tie falls back to the id in the
//! requested direction, as upstream's trailing `ORDER BY id <sort_order>`
//! does, so the order is total and a walk through the list is stable.

use std::cmp::Ordering;
use std::collections::HashMap;

use omeka_db::ids::{PropertyId, ResourceId, UserId};
use omeka_db::repos::{RelationRepo, ResourceRepo, SortLabel, ValueRepo, compare_natural};
use omeka_db::rows::ResourceRow;
use omeka_db::with_db;

use super::paging::{Limit, ordered_page};
use super::params::{Params, Sort};
use super::query::media_visibility;
use super::{Api, db_error};
use crate::errors::ApiError;

/// A resource column that orders as itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Column {
    Id,
    Created,
    Modified,
    IsPublic,
}

/// Text a row is shelved by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shelved {
    Title,
    Label(SortLabel),
    /// The values of one property the caller may see, in value id order.
    Property(PropertyId),
}

/// What `sort_by` names: a built-in column, or text in shelf order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SortKey {
    Column(Column),
    Text(Shelved),
}

/// The requested order of a resource list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResourceSort {
    pub key: SortKey,
    pub descending: bool,
}

impl ResourceSort {
    pub(crate) const ID_ASCENDING: Self = Self {
        key: SortKey::Column(Column::Id),
        descending: false,
    };

    /// `sort_by` and `sort_order`, read once. A term the catalog does not
    /// know orders by id, as upstream's fallback to `parent::sortQuery` does.
    pub(crate) async fn read(api: &Api<'_>, params: &Params) -> Result<Self, ApiError> {
        let key = match params.get("sort_by").unwrap_or("id") {
            "title" => SortKey::Text(Shelved::Title),
            "created" => SortKey::Column(Column::Created),
            "modified" => SortKey::Column(Column::Modified),
            "is_public" => SortKey::Column(Column::IsPublic),
            "resource_class_label" => SortKey::Text(Shelved::Label(SortLabel::Class)),
            "resource_template_label" => SortKey::Text(Shelved::Label(SortLabel::Template)),
            "owner_name" => SortKey::Text(Shelved::Label(SortLabel::Owner)),
            term if term.contains(':') => {
                api.fill_catalog().await?;
                api.catalog()
                    .property_id(term)
                    .map_or(SortKey::Column(Column::Id), |property| {
                        SortKey::Text(Shelved::Property(property))
                    })
            }
            _ => SortKey::Column(Column::Id),
        };
        Ok(Self {
            key,
            descending: Sort::from_params(params).descending,
        })
    }
}

/// Order `rows` and slice the requested window out of them: the total and the page.
pub(crate) async fn order_page<T>(
    api: &Api<'_>,
    rows: Vec<T>,
    sort: &ResourceSort,
    limit: &Limit,
    resource: impl Fn(&T) -> &ResourceRow,
) -> Result<(i64, Vec<T>), ApiError> {
    let shelved = match sort.key {
        SortKey::Column(column) => {
            return Ok(ordered_page(rows, sort.descending, limit, |a, b| {
                compare_column(column, resource(a), resource(b))
            }));
        }
        SortKey::Text(shelved) => shelved,
    };
    let mut texts = shelf_texts(api, shelved, rows.iter().map(&resource)).await?;
    let keyed: Vec<(Vec<String>, T)> = rows
        .into_iter()
        .map(|row| (texts.remove(&resource(&row).id.0).unwrap_or_default(), row))
        .collect();
    let (total, page) = ordered_page(keyed, false, limit, |(left, a), (right, b)| {
        shelf_order(
            (left, resource(a).id),
            (right, resource(b).id),
            sort.descending,
        )
    });
    Ok((total, page.into_iter().map(|(_, row)| row).collect()))
}

fn compare_column(column: Column, a: &ResourceRow, b: &ResourceRow) -> Ordering {
    match column {
        Column::Id => a.id.cmp(&b.id),
        Column::Created => a.created.cmp(&b.created),
        Column::Modified => a.modified.cmp(&b.modified),
        Column::IsPublic => a.is_public.cmp(&b.is_public),
    }
}

/// The texts each row is shelved by, keyed by resource id. A row with no text is absent.
async fn shelf_texts<'r>(
    api: &Api<'_>,
    shelved: Shelved,
    rows: impl Iterator<Item = &'r ResourceRow>,
) -> Result<HashMap<u64, Vec<String>>, ApiError> {
    let mut texts: HashMap<u64, Vec<String>> = HashMap::new();
    match shelved {
        Shelved::Title => {
            // A stored title drawn from a value the caller may not see is replaced by the one
            // they are shown, so a private title never decides a row's place.
            let rows: Vec<&ResourceRow> = rows.collect();
            let ids: Vec<ResourceId> = rows.iter().map(|row| row.id).collect();
            let shown: HashMap<u64, Option<String>> =
                with_db!(api.pool(), |db| RelationRepo::new(db)
                    .hidden_titles(&ids, media_visibility(api.actor()))
                    .await)
                .map_err(db_error)?
                .into_iter()
                .map(|(id, title)| (id.0, title))
                .collect();
            for row in rows {
                let title = match shown.get(&row.id.0) {
                    Some(title) => title.as_ref(),
                    None => row.title.as_ref(),
                };
                if let Some(title) = title.filter(|title| !title.is_empty()) {
                    texts.insert(row.id.0, vec![title.clone()]);
                }
            }
        }
        Shelved::Label(label) => {
            let wanted: std::collections::HashSet<u64> = rows.map(|row| row.id.0).collect();
            let labels = with_db!(api.pool(), |db| ResourceRepo::new(db)
                .sort_labels(label)
                .await)
            .map_err(db_error)?;
            for (id, text) in labels {
                if wanted.contains(&id.0) && !text.is_empty() {
                    texts.insert(id.0, vec![text]);
                }
            }
        }
        Shelved::Property(property) => {
            let owners: HashMap<u64, Option<UserId>> =
                rows.map(|row| (row.id.0, row.owner_id)).collect();
            let values = with_db!(api.pool(), |db| ValueRepo::new(db)
                .sort_values(property)
                .await)
            .map_err(db_error)?;
            for value in values {
                let Some(owner) = owners.get(&value.resource_id.0) else {
                    continue;
                };
                if sees_value(api, value.is_public, *owner) {
                    texts
                        .entry(value.resource_id.0)
                        .or_default()
                        .push(value.text);
                }
            }
        }
    }
    Ok(texts)
}

/// `ValueVisibilityFilter`: a caller with `view-all` sees every value, a
/// signed-in caller sees the private values of resources they own, everyone
/// else sees public values. That is the resource rule applied to the value's
/// own flag and its resource's owner.
fn sees_value(api: &Api<'_>, value_is_public: bool, resource_owner: Option<UserId>) -> bool {
    api.sees(value_is_public, resource_owner)
}

/// Two rows in shelf order: their texts, then their ids, reversed when
/// descending. A row with no text sorts after every row with text, in either
/// direction.
fn shelf_order(
    (left, left_id): (&[String], ResourceId),
    (right, right_id): (&[String], ResourceId),
    descending: bool,
) -> Ordering {
    let order = match (left.is_empty(), right.is_empty()) {
        (true, false) => return Ordering::Greater,
        (false, true) => return Ordering::Less,
        (true, true) => Ordering::Equal,
        (false, false) => compare_texts(left, right),
    }
    .then_with(|| left_id.cmp(&right_id));
    if descending { order.reverse() } else { order }
}

/// Text lists compared value by value, so a second value breaks a tie on the first.
fn compare_texts(left: &[String], right: &[String]) -> Ordering {
    left.iter()
        .zip(right)
        .map(|(l, r)| compare_natural(l, r))
        .find(|order| order.is_ne())
        .unwrap_or_else(|| left.len().cmp(&right.len()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// Rows `(texts, id)` in the order [`shelf_order`] gives them, as ids.
    fn shelved(rows: &[(&[&str], u64)], descending: bool) -> Vec<u64> {
        let mut rows: Vec<(Vec<String>, ResourceId)> = rows
            .iter()
            .map(|(texts, id)| {
                (
                    texts.iter().map(|t| (*t).to_string()).collect(),
                    ResourceId(*id),
                )
            })
            .collect();
        rows.sort_by(|(left, a), (right, b)| shelf_order((left, *a), (right, *b), descending));
        rows.into_iter().map(|(_, id)| id.0).collect()
    }

    #[test]
    fn digit_runs_order_as_numbers() {
        assert_eq!(shelved(&[(&["AF_10"], 1), (&["AF_2"], 2)], false), [2, 1]);
    }

    #[test]
    fn case_folds_and_a_tie_falls_back_to_the_id_in_the_direction() {
        let rows: &[(&[&str], u64)] = &[(&["af_2"], 7), (&["AF_2"], 3)];
        assert_eq!(shelved(rows, false), [3, 7]);
        assert_eq!(shelved(rows, true), [7, 3]);
    }

    #[test]
    fn no_text_sorts_last_in_both_directions() {
        let rows: &[(&[&str], u64)] = &[(&[], 1), (&["B"], 2), (&[], 3), (&["A"], 4)];
        assert_eq!(shelved(rows, false), [4, 2, 1, 3]);
        assert_eq!(shelved(rows, true), [2, 4, 3, 1]);
    }

    #[test]
    fn a_second_value_breaks_a_tie_on_the_first() {
        let rows: &[(&[&str], u64)] = &[(&["B", "x 10"], 1), (&["B", "x 9"], 2), (&["B"], 3)];
        assert_eq!(shelved(rows, false), [3, 2, 1]);
    }
}
