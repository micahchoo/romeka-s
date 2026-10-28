//! The advanced search parameters, parsed once at the HTTP boundary.
//!
//! `AbstractResourceEntityAdapter::buildQuery` reads `search` and
//! `property[i][property|type|text|joiner]`; `ItemAdapter::buildQuery` reads
//! `item_set_id`, `not_item_set_id`, `site_id`, `in_sites` and `has_media`;
//! `ItemSetAdapter` and `MediaAdapter` read `site_id`; `Module::searchFulltext`
//! reads `fulltext_search`. This module turns those strings into the typed
//! [`QueryFilter`] the repositories render, plus the fulltext needle and the
//! relevance ordering the index queries need.
//!
//! Everything here is a pure transform from the parsed query string and the
//! loaded [`Catalog`], so the resource handlers only apply the result.

use std::collections::BTreeMap;

use chrono::{NaiveDate, NaiveDateTime};
use indexmap::IndexMap;
use omeka_db::ids::{ItemSetId, PropertyId, SiteId};
use omeka_db::repos::{
    FulltextRepo, Joiner, MatchKind, MediaVisibility, OthersChanges, QueryFilter, ValueQuery,
    WorkFilters,
};
use omeka_db::with_db;

use super::catalog::Catalog;
use super::paging::Limit;
use super::params::Params;
use super::php::php_int_cast;
use super::{Api, db_error};
use crate::auth::Actor;
use crate::errors::ApiError;

/// The media visibility one request's actor has, for the `has_media` test.
///
/// `ItemAdapter::buildQuery` counts only the media the SQL visibility filter
/// leaves in the join: a `view-all` caller sees every row, everyone else sees
/// public rows and their own.
pub fn media_visibility(actor: &Actor) -> MediaVisibility {
    if actor.is_privileged() {
        MediaVisibility::All
    } else {
        MediaVisibility::Actor(actor.user())
    }
}

/// One request's advanced search parameters.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchQuery {
    /// The filters every resource entity accepts.
    pub filter: QueryFilter,
    /// `fulltext_search`, trimmed. `None` when the parameter is absent or
    /// blank, which `Module::searchFulltext` treats as no fulltext filter.
    pub fulltext: Option<String>,
    /// `id`, the base filter `AbstractEntityAdapter::buildBaseQuery` applies to
    /// every resource search. `None` when the request sent no usable id, which
    /// `QueryFilter` cannot carry because it lives in `omeka-db`.
    pub ids: Option<Vec<i64>>,
}

impl SearchQuery {
    /// Parse the query string against the loaded catalog and the caller's
    /// media visibility.
    pub fn parse(params: &Params, catalog: &Catalog, visibility: MediaVisibility) -> Self {
        let mut filter = QueryFilter::default();
        for row in params.rows("property") {
            if let Some(query) = property_row(&row, catalog) {
                filter.value.push(query);
            }
        }
        // `buildQuery` appends a second property query for `search`: one row
        // with no property and type `in`, which matches any value text, URI or
        // referenced resource title.
        if let Some(text) = params.get("search").filter(|text| is_present(text)) {
            filter.value.push(ValueQuery {
                property: None,
                kind: MatchKind::In,
                negated: false,
                text: text.trim().to_string(),
                joiner: Joiner::And,
            });
        }
        filter.site_id = params.id::<SiteId>("site_id");
        filter.in_sites = params.bool("in_sites");
        filter.has_media = params.bool("has_media");
        filter.has_media_kind = kind_names(params.list("has_media_kind"));
        filter.site_attachments_only = params.bool("site_attachments_only").unwrap_or(false);
        filter.visibility = visibility;
        filter.item_set_ids = ids(&params.list("item_set_id"));
        filter.not_item_set_ids = ids(&params.list("not_item_set_id"));
        let fulltext = params
            .get("fulltext_search")
            .map(str::trim)
            .filter(|needle| !needle.is_empty())
            .map(str::to_string);
        Self {
            filter,
            fulltext,
            ids: id_filter(params),
        }
    }

    /// Whether the request asked for a fulltext search.
    pub fn has_fulltext(&self) -> bool {
        self.fulltext.is_some()
    }
}

/// Romeka's work filters on an items search (`omeka_db::repos::WorkFilters`): `romeka_no_item_set`,
/// `romeka_no_resource_template` and `romeka_changed_by_others_since`, read for `actor`.
///
/// "Changed by others" reads the item history, which is for people who sign in: an anonymous
/// caller's filter keeps no item, as does a `since` that is not a time. `since` is an RFC 3339
/// time or a `YYYY-MM-DD` date, which reads as its midnight UTC.
pub fn work_filters(params: &Params, actor: &Actor) -> WorkFilters {
    WorkFilters {
        no_item_set: params.bool("romeka_no_item_set") == Some(true),
        no_resource_template: params.bool("romeka_no_resource_template") == Some(true),
        changed_by_others: params.get("romeka_changed_by_others_since").map(|raw| {
            match (since(raw), actor.user()) {
                (Some(since), Some(reader)) => OthersChanges::Since { since, reader },
                _ => OthersChanges::None,
            }
        }),
    }
}

fn since(raw: &str) -> Option<NaiveDateTime> {
    let raw = raw.trim();
    chrono::DateTime::parse_from_rfc3339(raw)
        .map(|at| at.naive_utc())
        .ok()
        .or_else(|| {
            NaiveDate::parse_from_str(raw, "%Y-%m-%d")
                .ok()
                .and_then(|day| day.and_hms_opt(0, 0, 0))
        })
}

/// A repeated kind parameter (`romeka_kind`, `has_media_kind`), trimmed, with
/// blank values dropped. `None` when the request named none, so no filter
/// applies; a name the server does not know stays in the list and matches no
/// row, the way an unknown `ingester` does.
pub fn kind_names(values: Vec<String>) -> Option<omeka_db::repos::MediaKindFilter> {
    let names: Vec<String> = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();
    (!names.is_empty()).then(|| omeka_db::repos::MediaKindFilter {
        names,
        local_image_types: omeka_media::kind::DISPLAY_IMAGE_MEDIA_TYPES
            .iter()
            .map(|media_type| (*media_type).to_string())
            .collect(),
        office_document_types: omeka_media::kind::OFFICE_DOCUMENT_MEDIA_TYPES
            .iter()
            .map(|media_type| (*media_type).to_string())
            .collect(),
        office_document_extensions: omeka_media::kind::OFFICE_DOCUMENT_EXTENSIONS
            .iter()
            .map(|extension| (*extension).to_string())
            .collect(),
    })
}

/// `AbstractEntityAdapter::buildBaseQuery`'s `id` filter: the request's id
/// values cast to the integers MySQL compares an `id` column against. `None`
/// when the request sent no id at all, so no narrowing applies.
pub fn id_filter(params: &Params) -> Option<Vec<i64>> {
    let values = params.id_values();
    if values.is_empty() {
        return None;
    }
    Some(values.iter().map(|value| php_int_cast(value)).collect())
}

/// Whether a row's id survives the request's `id` filter.
pub fn id_matches(ids: &Option<Vec<i64>>, id: u64) -> bool {
    match ids {
        None => true,
        Some(ids) => i64::try_from(id).is_ok_and(|id| ids.contains(&id)),
    }
}

/// The numeric ids of a repeated id parameter, dropping the rest the way
/// `array_filter($ids, 'is_numeric')` does.
fn ids(values: &[String]) -> Vec<ItemSetId> {
    values
        .iter()
        .filter_map(|value| value.trim().parse::<u64>().ok())
        .map(ItemSetId)
        .collect()
}

/// One `property[i][...]` row, or `None` when the row is incomplete or names a
/// comparison upstream does not implement.
fn property_row(row: &IndexMap<String, String>, catalog: &Catalog) -> Option<ValueQuery> {
    let property = row.get("property")?;
    let kind = row.get("type")?;
    let joiner = match row.get("joiner").map(String::as_str) {
        Some("or") => Joiner::Or,
        _ => Joiner::And,
    };
    let text = row
        .get("text")
        .map(|text| text.trim().to_string())
        .unwrap_or_default();
    let (kind, negated) = match_kind(kind)?;
    // `if (!$value && $queryType !== 'nex' && $queryType !== 'ex') continue;`
    if !is_present(&text) && !matches!(kind, MatchKind::Exists) {
        return None;
    }
    Some(ValueQuery {
        property: property_ref(property, catalog),
        kind,
        negated,
        text,
        joiner,
    })
}

/// PHP truthiness for a query row's text: `""` and `"0"` are absent.
fn is_present(text: &str) -> bool {
    let trimmed = text.trim();
    !trimmed.is_empty() && trimmed != "0"
}

/// The comparison a `type` names, with the leading `n` of a negative type
/// stripped the way `buildPropertyQuery` strips it.
fn match_kind(raw: &str) -> Option<(MatchKind, bool)> {
    let (negated, name) = match raw.strip_prefix('n') {
        Some(rest) if matches!(rest, "eq" | "in" | "sw" | "ew" | "res" | "ex" | "dt") => {
            (true, rest)
        }
        _ => (false, raw),
    };
    let kind = match name {
        "eq" => MatchKind::Eq,
        "in" => MatchKind::In,
        "sw" => MatchKind::StartsWith,
        "ew" => MatchKind::EndsWith,
        "res" => MatchKind::Resource,
        "ex" => MatchKind::Exists,
        "dt" => MatchKind::DataType,
        _ => return None,
    };
    Some((kind, negated))
}

/// The property a row narrows to.
///
/// `buildPropertyQuery` keeps the `if ($propertyId)` guard before it resolves
/// the value: an empty or `0` property means no narrowing. A number is the id.
/// Anything else is a `prefix:localName` term, and an unknown term becomes id
/// `0`, which matches no value.
fn property_ref(raw: &str, catalog: &Catalog) -> Option<PropertyId> {
    if !is_present(raw) {
        return None;
    }
    if let Ok(number) = raw.trim().parse::<f64>() {
        // PHP `(int)` truncates toward zero; a negative id matches no row.
        return Some(PropertyId(if number >= 0.0 { number as u64 } else { 0 }));
    }
    Some(catalog.property_id(raw).unwrap_or(PropertyId(0)))
}

/// The ids one fulltext query matched, with their scores.
///
/// `None` when the request carries no fulltext query, so every row is a
/// candidate and the caller keeps its plain ordering.
pub async fn fulltext_matches(
    api: &Api<'_>,
    resource: &str,
    needle: Option<&str>,
) -> Result<Option<BTreeMap<u64, f64>>, ApiError> {
    let Some(needle) = needle else {
        return Ok(None);
    };
    let matches = with_db!(api.pool(), |db| FulltextRepo::new(db)
        .search(resource, needle)
        .await)
    .map_err(db_error)?;
    Ok(Some(
        matches
            .into_iter()
            .map(|hit| (hit.id.0, hit.score))
            .collect(),
    ))
}

/// `Module::searchFulltext`'s default ordering: relevance descending, then the
/// id tiebreak in the requested direction.
///
/// Upstream replaces the sort clause with `MATCH(...) DESC` and then appends
/// `omeka_root.id` with the requested `sort_order`. The `DESC` is always the
/// relevance direction: `searchFulltext` compares the normalized `sort_order`
/// (`ASC` or `DESC`) against the lowercase literal `asc`, which never matches.
pub fn relevance_page<T>(
    rows: Vec<T>,
    descending: bool,
    limit: &Limit,
    id: impl Fn(&T) -> u64,
    rank: impl Fn(&T) -> f64,
) -> (i64, Vec<T>) {
    let mut rows = rows;
    rows.sort_by(|a, b| {
        rank(b).total_cmp(&rank(a)).then_with(|| {
            if descending {
                id(b).cmp(&id(a))
            } else {
                id(a).cmp(&id(b))
            }
        })
    });
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

    fn catalog() -> Catalog {
        // The term table is empty, so every term is unknown, which is the
        // `PropertyId(0)` branch.
        Catalog::default()
    }

    #[test]
    fn search_appends_a_propertyless_contains_row() {
        let query = SearchQuery::parse(
            &Params::parse("search=cat"),
            &catalog(),
            MediaVisibility::default(),
        );
        assert_eq!(query.filter.value.len(), 1);
        let row = &query.filter.value[0];
        assert_eq!(row.property, None);
        assert_eq!(row.kind, MatchKind::In);
        assert!(!row.negated);
        assert_eq!(row.text, "cat");
    }

    #[test]
    fn blank_search_adds_no_row() {
        for raw in ["search=", "search=0", "search=%20"] {
            let query =
                SearchQuery::parse(&Params::parse(raw), &catalog(), MediaVisibility::default());
            assert!(query.filter.value.is_empty(), "raw was {raw}");
        }
    }

    #[test]
    fn property_rows_keep_their_order_and_joiner() {
        let query = SearchQuery::parse(
            &Params::parse(
                "property[0][property]=1&property[0][type]=in&property[0][text]=bronze\
                 &property[1][property]=&property[1][type]=ex&property[1][joiner]=or",
            ),
            &catalog(),
            MediaVisibility::default(),
        );
        assert_eq!(query.filter.value.len(), 2);
        assert_eq!(query.filter.value[0].property, Some(PropertyId(1)));
        assert_eq!(query.filter.value[0].kind, MatchKind::In);
        assert_eq!(query.filter.value[1].kind, MatchKind::Exists);
        assert_eq!(query.filter.value[1].joiner, Joiner::Or);
        assert_eq!(query.filter.value[1].text, "");
    }

    #[test]
    fn negative_types_strip_the_leading_n() {
        for (raw, kind) in [
            ("neq", MatchKind::Eq),
            ("nin", MatchKind::In),
            ("nsw", MatchKind::StartsWith),
            ("new", MatchKind::EndsWith),
            ("nres", MatchKind::Resource),
            ("nex", MatchKind::Exists),
            ("ndt", MatchKind::DataType),
        ] {
            let (parsed, negated) = match_kind(raw).unwrap();
            assert_eq!(parsed, kind, "type was {raw}");
            assert!(negated, "type was {raw}");
        }
        assert_eq!(match_kind("sideways"), None);
    }

    #[test]
    fn a_row_without_a_value_is_dropped_unless_it_tests_existence() {
        let dropped = SearchQuery::parse(
            &Params::parse("property[0][property]=1&property[0][type]=in"),
            &catalog(),
            MediaVisibility::default(),
        );
        assert!(dropped.filter.value.is_empty());
        let kept = SearchQuery::parse(
            &Params::parse("property[0][property]=1&property[0][type]=nex"),
            &catalog(),
            MediaVisibility::default(),
        );
        assert_eq!(kept.filter.value.len(), 1);
    }

    #[test]
    fn an_unknown_term_becomes_property_zero() {
        let query = SearchQuery::parse(
            &Params::parse("property[0][property]=dcterms:absent&property[0][type]=ex"),
            &catalog(),
            MediaVisibility::default(),
        );
        assert_eq!(query.filter.value[0].property, Some(PropertyId(0)));
    }

    #[test]
    fn a_zero_property_means_no_narrowing() {
        let query = SearchQuery::parse(
            &Params::parse("property[0][property]=0&property[0][type]=ex"),
            &catalog(),
            MediaVisibility::default(),
        );
        assert_eq!(query.filter.value[0].property, None);
    }

    #[test]
    fn id_lists_accept_the_scalar_array_and_indexed_forms() {
        let query = SearchQuery::parse(
            &Params::parse("item_set_id=3&not_item_set_id[]=4&not_item_set_id[]=five"),
            &catalog(),
            MediaVisibility::default(),
        );
        assert_eq!(query.filter.item_set_ids, vec![ItemSetId(3)]);
        assert_eq!(query.filter.not_item_set_ids, vec![ItemSetId(4)]);
    }

    #[test]
    fn fulltext_search_trims_and_drops_a_blank() {
        let query = SearchQuery::parse(
            &Params::parse("fulltext_search=%20bronze%20"),
            &catalog(),
            MediaVisibility::default(),
        );
        assert_eq!(query.fulltext.as_deref(), Some("bronze"));
        assert!(query.has_fulltext());
        let blank = SearchQuery::parse(
            &Params::parse("fulltext_search="),
            &catalog(),
            MediaVisibility::default(),
        );
        assert!(!blank.has_fulltext());
    }

    #[test]
    fn relevance_page_orders_by_score_then_id() {
        let limit = Limit {
            max: Some(25),
            offset: 0,
        };
        let rows = vec![(1u64, 0.5f64), (2, 2.0), (3, 0.5)];
        let (total, page) = relevance_page(rows, false, &limit, |row| row.0, |row| row.1);
        assert_eq!(total, 3);
        assert_eq!(
            page.iter().map(|row| row.0).collect::<Vec<_>>(),
            vec![2, 1, 3]
        );

        let rows = vec![(1u64, 0.5f64), (2, 2.0), (3, 0.5)];
        let (_, page) = relevance_page(rows, true, &limit, |row| row.0, |row| row.1);
        assert_eq!(
            page.iter().map(|row| row.0).collect::<Vec<_>>(),
            vec![2, 3, 1]
        );
    }
}
