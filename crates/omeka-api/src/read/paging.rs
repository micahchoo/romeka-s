//! The list window and the `LIMIT`/`OFFSET` one request applies.
//!
//! `ApiController::getList` calls `setBrowseDefaults('id', 'asc')` before
//! `ApiManager::search`, and `Stdlib\Paginator` defaults `per_page` to 25 and
//! clamps a page below 1 to 1 (`Paginator::CURRENT_PAGE`, `PER_PAGE`,
//! `setPerPage`). [`Window`] is that paginator, which the `Link` header reads.
//!
//! [`Limit`] is `AbstractEntityAdapter::limitQuery`, which is a different
//! reading of the same parameters: the page window applies only when `page` is
//! numeric, and a non-numeric `page` falls through to `limit` and `offset`.

use super::params::{DEFAULT_PER_PAGE, Params, Sort};
use super::php::numeric_int;

/// The window one list request reads: `page` and `per_page` turned into a
/// `LIMIT`/`OFFSET` pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub page: i64,
    pub per_page: i64,
    pub offset: i64,
}

impl Window {
    pub fn from_params(params: &Params) -> Self {
        let page = params.int("page").unwrap_or(1).max(1);
        let per_page = params.int("per_page").unwrap_or(DEFAULT_PER_PAGE).max(1);
        Self {
            page,
            per_page,
            // A page and per_page this large reach the arithmetic from the
            // query string alone. Upstream `Paginator::getOffset` multiplies
            // PHP ints, and PHP turns that overflow into a float instead of
            // failing. Saturating keeps the same no-crash behaviour and leaves
            // every pair whose product fits unchanged.
            offset: per_page.saturating_mul(page - 1),
        }
    }

    /// `Paginator::getPageCount`, `(int) ceil($totalCount / $perPage)`.
    pub fn page_count(&self, total: i64) -> i64 {
        if total <= 0 {
            0
        } else {
            // Ceiling division over u64, where `div_ceil` is exact and the
            // usual `(total + per_page - 1) / per_page` overflows for a total
            // or a per_page near `i64::MAX`. Both values are positive here.
            (total as u64).div_ceil(self.per_page as u64) as i64
        }
    }
}

/// The `LIMIT`/`OFFSET` one list request applies.
///
/// `AbstractEntityAdapter::limitQuery` reads the window from the paginator only
/// when `page` is numeric (`is_numeric`). `SetBrowseDefaults` writes `page=1`
/// when the caller sent none, so an absent page is numeric; a page the caller
/// did send is numeric only when it looks like a number, and anything else
/// (including the empty string) falls through to numeric `limit` and `offset`.
/// `max` is `None` when no `LIMIT` applies at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limit {
    pub max: Option<i64>,
    pub offset: i64,
}

impl Limit {
    /// Every row the request's filter selects.
    pub const ALL: Self = Self {
        max: None,
        offset: 0,
    };

    pub fn from_params(params: &Params) -> Self {
        let page = match params.get("page") {
            None => Some(1),
            Some(raw) => numeric_int(raw),
        };
        if let Some(page) = page {
            let page = page.max(1);
            let per_page = params
                .get("per_page")
                .and_then(numeric_int)
                .map_or(DEFAULT_PER_PAGE, |per_page| per_page.max(1));
            return Self {
                max: Some(per_page),
                offset: per_page.saturating_mul(page - 1),
            };
        }
        Self {
            max: params.get("limit").and_then(numeric_int),
            offset: params.get("offset").and_then(numeric_int).unwrap_or(0),
        }
    }
}

/// Order `rows`, then slice the requested window out of them.
///
/// `cmp` is the `id`-ascending comparison; the field and direction from [`Sort`]
/// are applied on top. `total` is the count before slicing, which is what
/// `Omeka-S-Total-Results` carries.
///
/// This is the list endpoints' in-memory fallback. The handlers load every row
/// the SQL filter matches, drop the rows the actor may not see, order the rest
/// and slice the page. Moving the window into SQL needs a repository call that
/// renders the same `WHERE` (the advanced-search rows plus the ACL visibility
/// filter) with an `ORDER BY`, a `LIMIT`/`OFFSET` and a `COUNT` for the total;
/// `omeka-db` has no such call and this wave does not own that crate. The cost
/// grows with the item set rather than the page: the perf harness measured
/// about 2.2 us per item in the set at 1000 items, against a page of 25.
pub fn sorted_page<T>(
    rows: Vec<T>,
    sort: &Sort,
    limit: &Limit,
    cmp: impl Fn(&T, &T) -> std::cmp::Ordering,
) -> (i64, Vec<T>) {
    ordered_page(rows, sort.descending, limit, cmp)
}

/// Order `rows` by `cmp`, then slice the requested window out of them.
///
/// The adapters whose sort fields are not the resource fields (`assets`,
/// `jobs`) pick their comparator themselves, so they need the direction
/// without a [`Sort`].
pub fn ordered_page<T>(
    mut rows: Vec<T>,
    descending: bool,
    limit: &Limit,
    cmp: impl Fn(&T, &T) -> std::cmp::Ordering,
) -> (i64, Vec<T>) {
    rows.sort_by(&cmp);
    if descending {
        rows.reverse();
    }
    let total = rows.len() as i64;
    let start = limit.offset.max(0) as usize;
    let taken = rows.into_iter().skip(start);
    let page = match limit.max {
        Some(max) => taken.take(max.max(0) as usize).collect(),
        None => taken.collect(),
    };
    (total, page)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn defaults_match_the_paginator() {
        let window = Window::from_params(&Params::parse(""));
        assert_eq!(window.page, 1);
        assert_eq!(window.per_page, DEFAULT_PER_PAGE);
        assert_eq!(window.offset, 0);
    }

    #[test]
    fn page_and_per_page_clamp_to_one() {
        let window = Window::from_params(&Params::parse("page=0&per_page=-3"));
        assert_eq!(window.page, 1);
        assert_eq!(window.per_page, 1);
        assert_eq!(window.offset, 0);
    }

    #[test]
    fn offset_is_per_page_times_page_minus_one() {
        let window = Window::from_params(&Params::parse("page=3&per_page=10"));
        assert_eq!(window.offset, 20);
        assert_eq!(window.page_count(25), 3);
    }

    #[test]
    fn a_non_numeric_page_leaves_the_window_to_limit_and_offset() {
        assert_eq!(
            Limit::from_params(&Params::parse("page=abc&per_page=2")),
            Limit {
                max: None,
                offset: 0
            }
        );
        assert_eq!(
            Limit::from_params(&Params::parse("page=abc&limit=2")),
            Limit {
                max: Some(2),
                offset: 0
            }
        );
        assert_eq!(
            Limit::from_params(&Params::parse("page=abc&offset=3")),
            Limit {
                max: None,
                offset: 3
            }
        );
        assert_eq!(
            Limit::from_params(&Params::parse("page=")),
            Limit {
                max: None,
                offset: 0
            }
        );
    }

    #[test]
    fn a_numeric_page_opens_the_window_and_ignores_limit() {
        assert_eq!(
            Limit::from_params(&Params::parse("page=2&per_page=10")),
            Limit {
                max: Some(10),
                offset: 10
            }
        );
        assert_eq!(
            Limit::from_params(&Params::parse("page=0&per_page=2")),
            Limit {
                max: Some(2),
                offset: 0
            }
        );
        assert_eq!(
            Limit::from_params(&Params::parse("per_page=2")),
            Limit {
                max: Some(2),
                offset: 0
            }
        );
        assert_eq!(
            Limit::from_params(&Params::parse("page=1.9&per_page=2")),
            Limit {
                max: Some(2),
                offset: 0
            }
        );
        assert_eq!(
            Limit::from_params(&Params::parse("page=1&limit=2&offset=5")),
            Limit {
                max: Some(DEFAULT_PER_PAGE),
                offset: 0
            }
        );
        assert_eq!(
            Limit::from_params(&Params::parse("")),
            Limit {
                max: Some(DEFAULT_PER_PAGE),
                offset: 0
            }
        );
    }

    #[test]
    fn an_unbounded_limit_keeps_every_row_past_the_offset() {
        let rows = vec![1, 2, 3, 4];
        let (total, page) = ordered_page(
            rows.clone(),
            false,
            &Limit {
                max: None,
                offset: 1,
            },
            |a, b| a.cmp(b),
        );
        assert_eq!(total, 4);
        assert_eq!(page, vec![2, 3, 4]);

        let (_, all) = ordered_page(rows, false, &Limit::ALL, |a, b| a.cmp(b));
        assert_eq!(all.len(), 4);

        let (_, none) = ordered_page(
            vec![1, 2],
            false,
            &Limit {
                max: Some(0),
                offset: 0,
            },
            |a, b| a.cmp(b),
        );
        assert!(none.is_empty());
    }
}
