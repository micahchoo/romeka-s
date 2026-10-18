//! Query-string parsing, the list window, and the sort order for read endpoints.
//!
//! `ApiController::getList` calls `setBrowseDefaults('id', 'asc')` before
//! `ApiManager::search`, so an API list defaults to `sort_by=id`,
//! `sort_order=asc`, `page=1`. `Stdlib\Paginator` defaults `per_page` to 25 and
//! clamps a page below 1 to 1 (`Paginator::CURRENT_PAGE`, `PER_PAGE`,
//! `setPerPage`). All of that lives here so the HTTP layer only decodes bytes.

use indexmap::IndexMap;

use crate::auth::decode;

/// Decoded query parameters for one request, in arrival order.
///
/// PHP `parse_str` keeps the last value for a repeated name, and so does this
/// map. The raw (still percent-encoded) pairs are kept beside it so the `Link`
/// header can echo the request query exactly, and the decoded pairs are kept so
/// a bracket array can read every value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Params {
    raw: Vec<(String, String)>,
    pairs: Vec<(String, String)>,
    decoded: IndexMap<String, String>,
}

impl Params {
    /// Parse a raw query string (`a=1&b=two`).
    pub fn parse(query: &str) -> Self {
        let raw: Vec<(String, String)> = query
            .split('&')
            .filter(|part| !part.is_empty())
            .map(|part| {
                let (name, value) = part.split_once('=').unwrap_or((part, ""));
                (name.to_string(), value.to_string())
            })
            .collect();
        let pairs: Vec<(String, String)> = raw
            .iter()
            .map(|(name, value)| (decode(name), decode(value)))
            .collect();
        let decoded = pairs.iter().cloned().collect();
        Self {
            raw,
            pairs,
            decoded,
        }
    }

    /// Parameters from pairs that are already decoded, in arrival order.
    ///
    /// The site layer keeps a stored or requested query as decoded pairs
    /// (`BlockLayout\BrowsePreview` stores `query` as a string, and the browse
    /// controller reads `fromQuery()`), so this is the entry point for a query
    /// that is not raw bytes.
    pub fn from_decoded(pairs: Vec<(String, String)>) -> Self {
        let decoded = pairs.iter().cloned().collect();
        Self {
            raw: pairs.clone(),
            pairs,
            decoded,
        }
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.decoded.get(name).map(String::as_str)
    }

    /// A parameter read as an integer, `None` when absent or not numeric.
    pub fn int(&self, name: &str) -> Option<i64> {
        self.get(name).and_then(|raw| raw.trim().parse().ok())
    }

    /// A parameter read as a table id.
    pub fn id<T: From<u64>>(&self, name: &str) -> Option<T> {
        self.int(name)
            .and_then(|raw| u64::try_from(raw).ok())
            .map(T::from)
    }

    /// A parameter read the way `is_numeric(...) || is_bool(...)` guards it
    /// upstream: `1`/`true` are true, `0`/`false` are false.
    pub fn bool(&self, name: &str) -> Option<bool> {
        match self.get(name)?.trim() {
            "1" | "true" => Some(true),
            "0" | "false" => Some(false),
            _ => None,
        }
    }

    /// The pairs as they arrived, still percent-encoded.
    pub fn raw_pairs(&self) -> &[(String, String)] {
        &self.raw
    }

    /// The query the `Link` header carries, as `setBrowseDefaults('id', 'asc')`
    /// leaves it (`Mvc\Controller\Plugin\SetBrowseDefaults`).
    ///
    /// `ApiController::getList` calls the plugin before it builds the links, and
    /// the plugin writes five parameters into the request query:
    /// `sort_by_default`, `sort_order_default`, `sort_by`, `sort_order` and
    /// `page`. Every generated link therefore echoes them, in the plugin's
    /// order, after the parameters the caller sent. The two `_default`
    /// parameters hold the empty string when the caller did not sort, and are
    /// left unset (`null`, which `http_build_query` drops) when it did. The
    /// value of an existing key never changes, so `sort_by=` stays empty.
    pub fn browse_pairs(&self, page: i64) -> Vec<(String, String)> {
        let mut pairs = self.raw.clone();
        let raw_sort_by = self.get("sort_by");
        let raw_sort_order = self.get("sort_order");
        let sort_by = raw_sort_by.unwrap_or("id");
        let sort_order = raw_sort_order.unwrap_or("asc");
        let sort_by_default = matches!(raw_sort_by, None | Some(""));
        if sort_by_default {
            set_or_append(&mut pairs, "sort_by_default", "");
        }
        if sort_by_default && matches!(raw_sort_order, None | Some("")) {
            set_or_append(&mut pairs, "sort_order_default", "");
        }
        set_or_append(&mut pairs, "sort_by", sort_by);
        set_or_append(&mut pairs, "sort_order", sort_order);
        set_or_append(&mut pairs, "page", &page.to_string());
        pairs
    }

    /// The decoded name and value pairs, for key authentication.
    pub fn pairs(&self) -> impl Iterator<Item = (&str, &str)> {
        self.decoded
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    /// The decoded pairs in arrival order, duplicates kept.
    ///
    /// `decoded` keeps only the last value for a repeated name, the way PHP
    /// `parse_str` fills a scalar; a bracket array needs every pair, so this
    /// returns the list `parse` decoded.
    pub fn decoded_pairs(&self) -> Vec<(String, String)> {
        self.pairs.clone()
    }

    /// The rows of a PHP bracket array, in arrival order.
    ///
    /// `property[0][property]=1&property[0][type]=in&property[1][type]=ex`
    /// becomes `[{"property": "1", "type": "in"}, {"type": "ex"}]`, which is
    /// what `parse_str` hands `AbstractResourceEntityAdapter::buildPropertyQuery`.
    /// The index is kept as written and rows keep the order they arrived in,
    /// the order `parse_str` builds the array in.
    pub fn rows(&self, name: &str) -> Vec<IndexMap<String, String>> {
        let mut rows: IndexMap<String, IndexMap<String, String>> = IndexMap::new();
        for (key, value) in self.decoded_pairs() {
            let Some((index, field)) = bracket_index(&key, name) else {
                continue;
            };
            rows.entry(index).or_default().insert(field, value);
        }
        rows.into_values().collect()
    }

    /// A parameter repeated or written as a list: `name=a&name=b`,
    /// `name[]=a&name[]=b` or `name[0]=a&name[1]=b`.
    ///
    /// PHP `parse_str` turns all three into an array; the scalar form arrives
    /// here as a repeated name because this parser keeps the raw pairs.
    pub fn list(&self, name: &str) -> Vec<String> {
        let list_key = format!("{name}[]");
        let mut values = Vec::new();
        for (key, value) in self.decoded_pairs() {
            let bracketed =
                single_index(&key, name).is_some() || bracket_index(&key, name).is_some();
            if key == name || key == list_key || bracketed {
                values.push(value);
            }
        }
        values
    }

    /// `SetBrowseDefaults`: whether the caller left `sort_by` empty, so the
    /// browse default applied. `Module::searchFulltext` orders a fulltext
    /// search by relevance only in this case.
    pub fn sort_by_is_default(&self) -> bool {
        matches!(self.get("sort_by"), None | Some(""))
    }

    /// `SetBrowseDefaults`'s `sort_order_default`: set only when `sort_by` was
    /// default too.
    pub fn sort_order_is_default(&self) -> bool {
        self.sort_by_is_default() && matches!(self.get("sort_order"), None | Some(""))
    }
}

/// The index of a `name[index]` key.
fn single_index(key: &str, name: &str) -> Option<String> {
    let rest = key
        .strip_prefix(name)?
        .strip_prefix('[')?
        .strip_suffix(']')?;
    if rest.is_empty() || rest.contains(['[', ']']) {
        return None;
    }
    Some(rest.to_string())
}

/// The index and field of a `name[index][field]` key.
fn bracket_index(key: &str, name: &str) -> Option<(String, String)> {
    let rest = key.strip_prefix(name)?.strip_prefix('[')?;
    let (index, tail) = rest.split_once(']')?;
    let field = tail.strip_prefix('[')?.strip_suffix(']')?;
    if index.is_empty() || field.is_empty() || field.contains(['[', ']']) {
        return None;
    }
    Some((index.to_string(), field.to_string()))
}

/// Upstream `Paginator::PER_PAGE`.
pub const DEFAULT_PER_PAGE: i64 = 25;

/// Set a query parameter in place, or append it when the caller did not send
/// it. A repeated name keeps its last position, which is the value PHP
/// `parse_str` would have stored.
fn set_or_append(pairs: &mut Vec<(String, String)>, name: &str, value: &str) {
    if let Some((_, existing)) = pairs.iter_mut().rev().find(|(key, _)| key == name) {
        *existing = value.to_string();
    } else {
        pairs.push((name.to_string(), value.to_string()));
    }
}

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

/// Which column a list is ordered by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    Id,
    Title,
    Created,
    Modified,
    IsPublic,
}

impl SortField {
    /// The `sortFields` maps of `ItemAdapter` and `ItemSetAdapter`. Anything
    /// else falls back to `id`, which `AbstractEntityAdapter::sortQuery` does
    /// for an unmapped field on these adapters.
    pub fn parse(name: &str) -> Self {
        match name {
            "title" => Self::Title,
            "created" => Self::Created,
            "modified" => Self::Modified,
            "is_public" => Self::IsPublic,
            _ => Self::Id,
        }
    }
}

/// The requested sort: a field and a direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sort {
    pub field: SortField,
    pub descending: bool,
}

impl Sort {
    pub fn from_params(params: &Params) -> Self {
        let field = SortField::parse(params.get("sort_by").unwrap_or("id"));
        let descending = params
            .get("sort_order")
            .map(|order| order.eq_ignore_ascii_case("desc"))
            .unwrap_or(false);
        Self { field, descending }
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
    window: &Window,
    cmp: impl Fn(&T, &T) -> std::cmp::Ordering,
) -> (i64, Vec<T>) {
    ordered_page(rows, sort.descending, window, cmp)
}

/// Order `rows` by `cmp`, then slice the requested window out of them.
///
/// The adapters whose sort fields are not the resource fields (`assets`,
/// `jobs`) pick their comparator themselves, so they need the direction
/// without a [`Sort`].
pub fn ordered_page<T>(
    mut rows: Vec<T>,
    descending: bool,
    window: &Window,
    cmp: impl Fn(&T, &T) -> std::cmp::Ordering,
) -> (i64, Vec<T>) {
    rows.sort_by(&cmp);
    if descending {
        rows.reverse();
    }
    let total = rows.len() as i64;
    let start = window.offset.max(0) as usize;
    let page = rows
        .into_iter()
        .skip(start)
        .take(window.per_page as usize)
        .collect();
    (total, page)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn defaults_match_the_paginator() {
        let params = Params::parse("");
        let window = Window::from_params(&params);
        assert_eq!(window.page, 1);
        assert_eq!(window.per_page, DEFAULT_PER_PAGE);
        assert_eq!(window.offset, 0);
        let sort = Sort::from_params(&params);
        assert_eq!(sort.field, SortField::Id);
        assert!(!sort.descending);
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
    fn sort_order_only_accepts_desc() {
        assert!(Sort::from_params(&Params::parse("sort_order=DESC")).descending);
        assert!(!Sort::from_params(&Params::parse("sort_order=sideways")).descending);
    }

    #[test]
    fn repeated_names_keep_the_last_value() {
        let params = Params::parse("search=first&search=second");
        assert_eq!(params.get("search"), Some("second"));
    }

    #[test]
    fn bracket_rows_keep_the_arrival_order_and_the_written_index() {
        let params = Params::parse("property[2][type]=ex&property[1][type]=in&property[1][text]=x");
        let rows = params.rows("property");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get("type").map(String::as_str), Some("ex"));
        assert_eq!(rows[1].get("text").map(String::as_str), Some("x"));

        let named = Params::parse("property[first][type]=ex");
        assert_eq!(named.rows("property").len(), 1);
    }

    #[test]
    fn list_reads_the_scalar_repeated_and_bracket_forms() {
        assert_eq!(Params::parse("id=1").list("id"), vec!["1".to_string()]);
        assert_eq!(
            Params::parse("id[]=1&id[]=2").list("id"),
            vec!["1".to_string(), "2".to_string()]
        );
        assert_eq!(
            Params::parse("id[0]=1&id[1]=2").list("id"),
            vec!["1".to_string(), "2".to_string()]
        );
    }

    #[test]
    fn browse_defaults_follow_the_plugin() {
        assert!(Params::parse("").sort_by_is_default());
        assert!(Params::parse("sort_by=").sort_by_is_default());
        assert!(!Params::parse("sort_by=title").sort_by_is_default());
        assert!(Params::parse("sort_by=").sort_order_is_default());
        assert!(!Params::parse("sort_by=title").sort_order_is_default());
    }
}
