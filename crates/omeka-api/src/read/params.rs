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
        let mut values = Vec::new();
        for (key, value) in self.decoded_pairs() {
            if is_list_member(&key, name) {
                values.push(value);
            }
        }
        values
    }

    /// The `id` parameter's values as PHP `parse_str` builds them, with the
    /// scalar comma form split the way `AbstractEntityAdapter::buildBaseQuery`
    /// splits it.
    ///
    /// A bare `id=1,2` is one string that splits on commas. `id[]=1,2` is an
    /// array whose single element is the string `1,2`, and does not split. Every
    /// value is trimmed and an empty one dropped, which is the `array_filter` on
    /// `strlen` that guard ends with.
    pub fn id_values(&self) -> Vec<String> {
        let mut values = Vec::new();
        for (key, value) in self.decoded_pairs() {
            if key == "id" {
                values.extend(value.split(',').map(|part| part.trim().to_string()));
            } else if is_list_member(&key, "id") {
                values.push(value.trim().to_string());
            }
        }
        values.retain(|value| !value.is_empty());
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

/// Whether `key` is one of the three forms PHP `parse_str` turns into an
/// element of the array parameter `name`: `name`, `name[]` or `name[index]`.
fn is_list_member(key: &str, name: &str) -> bool {
    key == name
        || key == format!("{name}[]")
        || single_index(key, name).is_some()
        || bracket_index(key, name).is_some()
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

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

    #[test]
    fn the_id_list_splits_the_scalar_comma_form_only() {
        assert_eq!(
            Params::parse("id=1,2").id_values(),
            vec!["1".to_string(), "2".to_string()]
        );
        assert_eq!(
            Params::parse("id[]=1,2").id_values(),
            vec!["1,2".to_string()]
        );
        assert_eq!(
            Params::parse("id[]=1&id[]=2").id_values(),
            vec!["1".to_string(), "2".to_string()]
        );
        assert_eq!(
            Params::parse("id[0]=1&id[1]=2").id_values(),
            vec!["1".to_string(), "2".to_string()]
        );
        assert!(Params::parse("id[]=%20&other=1").id_values().is_empty());
        assert!(Params::parse("other=1").id_values().is_empty());
    }
}
