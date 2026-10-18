//! The list window read straight from SQL.
//!
//! `search_items` loaded every row the filter matched and sliced the page in
//! memory (`params::sorted_page`). The perf harness measured that as about half
//! the list endpoint's endpoint work at 200 items: 200 rows fetched and parsed
//! for a 25-row page. Upstream runs the same two reads:
//! `AbstractAdapter::search` counts the filter and then selects the page. This
//! module does both in one statement, so the page's query count does not grow.
//!
//! The window is read here only when the request's filter is one this module
//! renders: the [`ItemFilter`] fields, `item_set_id`, the caller's visibility,
//! the default `id` ascending order, and no fulltext. Everything else keeps the
//! in-memory path, which stays the documented fallback in
//! [`super::params::sorted_page`].
//!
//! The clause text for the fields both paths share is copied from
//! `omeka_db::repos` (`ItemRepo::list_filtered`, `QueryFilter::push_item_sets`,
//! `ItemRepo`'s `ITEM_COLUMNS`), because this wave does not own that crate.
//! `tests/list_window.rs` runs both paths over the same seeded database and
//! asserts they answer the same page and the same total for every filter shape
//! the fast path accepts, so a divergence in the copied text fails the test.

use omeka_db::repos::ItemFilter;
use omeka_db::rows::ItemRow;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use super::paging::Limit;
use super::params::{Sort, SortField};
use super::query::SearchQuery;
use super::{Api, db_error};
use crate::auth::Actor;
use crate::errors::ApiError;

/// The `item`/`resource` projection and join, copied from `ItemRepo`'s
/// `ITEM_COLUMNS`.
const ITEM_COLUMNS: &str = "resource.*, item.primary_media_id";
const ITEM_FROM: &str = " FROM item JOIN resource ON resource.id = item.id";

/// A page read from SQL: the total the filter matches and the window's rows.
pub(crate) struct ListPage {
    pub total: i64,
    pub rows: Vec<ItemRow>,
}

/// Whether this request's filter is one the SQL window renders.
///
/// A window that applies no `LIMIT` keeps the in-memory path, which reads every
/// row and slices it without a `LIMIT` of its own.
pub(crate) fn eligible(
    filter: &ItemFilter,
    search: &SearchQuery,
    sort: &Sort,
    limit: &Limit,
) -> bool {
    limit.max.is_some()
        && filter.item_set_id.is_none()
        && search.fulltext.is_none()
        && search.filter.value.is_empty()
        && search.filter.site_id.is_none()
        && search.filter.in_sites.is_none()
        && search.filter.has_media.is_none()
        && search.filter.not_item_set_ids.is_empty()
        && sort.field == SortField::Id
        && !sort.descending
}

/// Read the total and one window.
///
/// One query carries both: `COUNT(*) OVER ()` counts the rows the filter
/// matches before `LIMIT` applies, so a page of 25 rows runs one statement
/// rather than a count plus a page, and the page's query count stays what
/// `tests/relations_batch.rs` pins. A window past the last row returns no rows
/// to carry the total, so that case counts on its own.
pub(crate) async fn read(
    api: &Api<'_>,
    filter: &ItemFilter,
    search: &SearchQuery,
    limit: &Limit,
) -> Result<ListPage, ApiError> {
    let (where_sql, binds) = predicate(filter, search, api.actor());
    let rows = page(api.pool(), &where_sql, &binds, limit).await?;
    let total = match rows.first().map(|row| row.1) {
        Some(total) => total,
        // An empty first page means the filter matched nothing; an empty later
        // page means the offset ran past the end, which needs the count.
        None if limit.offset == 0 => 0,
        None => count(api.pool(), &where_sql, &binds).await?,
    };
    Ok(ListPage {
        total,
        rows: rows.into_iter().map(|(row, _)| row).collect(),
    })
}

/// The window's rows with the total the filter matches.
async fn page(
    pool: &SqlitePool,
    where_sql: &str,
    binds: &[i64],
    limit: &Limit,
) -> Result<Vec<(ItemRow, i64)>, ApiError> {
    let sql = format!(
        "SELECT {ITEM_COLUMNS}, COUNT(*) OVER () AS total{ITEM_FROM}{where_sql} \
         ORDER BY resource.id LIMIT ? OFFSET ?"
    );
    let mut query = sqlx::query(&sql);
    for bind in binds {
        query = query.bind(*bind);
    }
    let rows: Vec<SqliteRow> = query
        .bind(limit.max.unwrap_or(0))
        .bind(limit.offset)
        .fetch_all(pool)
        .await
        .map_err(ApiError::internal)?;
    rows.iter()
        .map(|row| {
            let total: i64 = row.try_get("total").map_err(ApiError::internal)?;
            Ok((ItemRow::from_row(row).map_err(db_error)?, total))
        })
        .collect()
}

/// The number of rows the filter matches.
async fn count(pool: &SqlitePool, where_sql: &str, binds: &[i64]) -> Result<i64, ApiError> {
    let sql = format!("SELECT COUNT(*){ITEM_FROM}{where_sql}");
    let mut query = sqlx::query_scalar::<_, i64>(&sql);
    for bind in binds {
        query = query.bind(*bind);
    }
    query.fetch_one(pool).await.map_err(ApiError::internal)
}

/// The filter's `WHERE`, and the values it binds in clause order.
fn predicate(filter: &ItemFilter, search: &SearchQuery, actor: &Actor) -> (String, Vec<i64>) {
    let mut sql = String::from(" WHERE 1 = 1");
    let mut binds: Vec<i64> = Vec::new();
    // `AbstractEntityAdapter::buildBaseQuery` runs before each adapter's own
    // `buildQuery`, so the `id` list is the first clause.
    if let Some(ids) = &search.ids {
        sql.push_str(" AND resource.id IN (");
        for (index, id) in ids.iter().enumerate() {
            if index > 0 {
                sql.push_str(", ");
            }
            sql.push('?');
            binds.push(*id);
        }
        sql.push(')');
    }
    if let Some(owner_id) = filter.owner_id {
        sql.push_str(" AND resource.owner_id = ?");
        binds.push(owner_id.0 as i64);
    }
    if let Some(class_id) = filter.resource_class_id {
        sql.push_str(" AND resource.resource_class_id = ?");
        binds.push(class_id.0 as i64);
    }
    if let Some(template_id) = filter.resource_template_id {
        sql.push_str(" AND resource.resource_template_id = ?");
        binds.push(template_id.0 as i64);
    }
    if let Some(is_public) = filter.is_public {
        sql.push_str(" AND resource.is_public = ?");
        binds.push(i64::from(is_public));
    }
    if !search.filter.item_set_ids.is_empty() {
        // `QueryFilter::push_item_sets`, which joins on `resource.id`; `item.id`
        // is the same value through this module's join.
        sql.push_str(
            " AND EXISTS (SELECT 1 FROM item_item_set WHERE item_item_set.item_id = resource.id \
             AND item_item_set.item_set_id IN (",
        );
        for (index, id) in search.filter.item_set_ids.iter().enumerate() {
            if index > 0 {
                sql.push_str(", ");
            }
            sql.push('?');
            binds.push(id.0 as i64);
        }
        sql.push_str("))");
    }
    push_visibility(&mut sql, &mut binds, actor);
    (sql, binds)
}

/// The caller's visibility, matching [`Actor::sees_resource`]: public, owned by
/// the caller, or the caller holds `view-all`.
fn push_visibility(sql: &mut String, binds: &mut Vec<i64>, actor: &Actor) {
    if actor.is_privileged() {
        return;
    }
    match actor.user() {
        Some(user) => {
            sql.push_str(" AND (resource.is_public = 1 OR resource.owner_id = ?)");
            binds.push(user.0 as i64);
        }
        None => sql.push_str(" AND resource.is_public = 1"),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use omeka_db::ids::{ClassId, ItemSetId, TemplateId, UserId};

    fn no_sort() -> Sort {
        Sort {
            field: SortField::Id,
            descending: false,
        }
    }

    /// A limit that applies a `LIMIT`, which the SQL window needs.
    fn bounded() -> Limit {
        Limit {
            max: Some(25),
            offset: 0,
        }
    }

    #[test]
    fn an_unfiltered_default_request_is_eligible() {
        assert!(eligible(
            &ItemFilter::default(),
            &SearchQuery::default(),
            &no_sort(),
            &bounded()
        ));
    }

    #[test]
    fn a_fulltext_other_sort_or_unbounded_request_is_not() {
        let fulltext = SearchQuery {
            fulltext: Some("cat".to_string()),
            ..SearchQuery::default()
        };
        assert!(!eligible(
            &ItemFilter::default(),
            &fulltext,
            &no_sort(),
            &bounded()
        ));
        let title = Sort {
            field: SortField::Title,
            descending: false,
        };
        assert!(!eligible(
            &ItemFilter::default(),
            &SearchQuery::default(),
            &title,
            &bounded()
        ));
        let descending = Sort {
            field: SortField::Id,
            descending: true,
        };
        assert!(!eligible(
            &ItemFilter::default(),
            &SearchQuery::default(),
            &descending,
            &bounded()
        ));
        // A non-numeric `page` keeps no `LIMIT`, so the window is read in
        // memory (`params::ordered_page`).
        assert!(!eligible(
            &ItemFilter::default(),
            &SearchQuery::default(),
            &no_sort(),
            &Limit::ALL
        ));
    }

    #[test]
    fn a_predicate_renders_the_fields_it_carries_in_order() {
        let filter = ItemFilter {
            owner_id: Some(UserId(3)),
            resource_class_id: Some(ClassId(4)),
            resource_template_id: Some(TemplateId(5)),
            item_set_id: None,
            is_public: Some(true),
        };
        let search = SearchQuery {
            filter: omeka_db::repos::QueryFilter {
                item_set_ids: vec![ItemSetId(6), ItemSetId(7)],
                ..Default::default()
            },
            ..SearchQuery::default()
        };
        let (sql, binds) = predicate(&filter, &search, &Actor::anonymous());
        assert_eq!(
            sql,
            " WHERE 1 = 1 AND resource.owner_id = ? AND resource.resource_class_id = ? \
             AND resource.resource_template_id = ? AND resource.is_public = ? \
             AND EXISTS (SELECT 1 FROM item_item_set WHERE item_item_set.item_id = resource.id \
             AND item_item_set.item_set_id IN (?, ?)) AND resource.is_public = 1"
        );
        assert_eq!(binds, vec![3, 4, 5, 1, 6, 7]);
    }

    /// `buildBaseQuery` renders the `id` list first, before the adapter's own
    /// filters.
    #[test]
    fn a_predicate_renders_the_id_list_first() {
        let search = SearchQuery {
            ids: Some(vec![149, 150]),
            ..SearchQuery::default()
        };
        let (sql, binds) = predicate(&ItemFilter::default(), &search, &Actor::anonymous());
        assert_eq!(
            sql,
            " WHERE 1 = 1 AND resource.id IN (?, ?) AND resource.is_public = 1"
        );
        assert_eq!(binds, vec![149, 150]);
    }

    #[test]
    fn a_privileged_caller_adds_no_visibility_clause() {
        let (sql, binds) = predicate(
            &ItemFilter::default(),
            &SearchQuery::default(),
            &Actor::new(UserId(1), "global_admin"),
        );
        assert_eq!(sql, " WHERE 1 = 1");
        assert!(binds.is_empty());
    }

    #[test]
    fn a_signed_in_caller_sees_public_rows_and_their_own() {
        let (sql, binds) = predicate(
            &ItemFilter::default(),
            &SearchQuery::default(),
            &Actor::new(UserId(9), "author"),
        );
        assert_eq!(
            sql,
            " WHERE 1 = 1 AND (resource.is_public = 1 OR resource.owner_id = ?)"
        );
        assert_eq!(binds, vec![9]);
    }

    #[test]
    fn an_anonymous_caller_sees_public_rows() {
        let (sql, binds) = predicate(
            &ItemFilter::default(),
            &SearchQuery::default(),
            &Actor::anonymous(),
        );
        assert_eq!(sql, " WHERE 1 = 1 AND resource.is_public = 1");
        assert!(binds.is_empty());
    }
}
