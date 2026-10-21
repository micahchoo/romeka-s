//! The list window read straight from SQL.
//!
//! `search_items` loaded every row the filter matched and sliced the page in
//! memory (`params::sorted_page`). The perf harness measured that as about half
//! the list endpoint's endpoint work at 200 items: 200 rows fetched and parsed
//! for a 25-row page. Upstream runs the same two reads:
//! `AbstractAdapter::search` counts the filter and then selects the page.
//! `ItemRepo::window` does both in one statement, so the page's query count
//! does not grow.
//!
//! The window is read in SQL only when the request's filter is one
//! [`ItemWindow`] carries: the [`ItemFilter`] fields, `item_set_id`, the
//! caller's visibility, the default `id` ascending order, and no fulltext.
//! Everything else keeps the in-memory path, which stays the documented
//! fallback in [`super::params::sorted_page`]. `tests/list_window.rs` runs both
//! paths over the same seeded database and asserts they answer the same page
//! and the same total for every filter shape the fast path accepts.

use omeka_db::repos::{ItemFilter, ItemRepo, ItemWindow};
use omeka_db::rows::ItemRow;
use omeka_db::with_db;

use super::paging::Limit;
use super::params::{Sort, SortField};
use super::query::{SearchQuery, media_visibility};
use super::{Api, db_error};
use crate::errors::ApiError;

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
        && search.filter.has_media_kind.is_none()
        && search.filter.not_item_set_ids.is_empty()
        && search.filter.ids.is_none()
        && sort.field == SortField::Id
        && !sort.descending
}

/// Read the total and one window, as the caller may see them.
pub(crate) async fn read(
    api: &Api<'_>,
    filter: &ItemFilter,
    search: &SearchQuery,
    limit: &Limit,
) -> Result<ListPage, ApiError> {
    let window = ItemWindow {
        ids: search.ids.clone(),
        filter: filter.clone(),
        item_set_ids: search.filter.item_set_ids.clone(),
        visibility: media_visibility(api.actor()),
    };
    let page = with_db!(api.pool(), |db| ItemRepo::new(db)
        .window(&window, limit.max.unwrap_or(0), limit.offset)
        .await)
    .map_err(db_error)?;
    Ok(ListPage {
        total: page.total,
        rows: page.rows,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

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
}
