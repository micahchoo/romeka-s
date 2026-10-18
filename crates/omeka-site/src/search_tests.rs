//! The search model's own assertions: the resource table, the links it builds
//! and the advanced form's parsing of a query.

use super::*;

fn site() -> Slug {
    Slug::parse("main").unwrap()
}

fn query(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect()
}

/// The table names the three resources the way `SiteSettingsForm`,
/// `index/search.phtml` and `CrossSiteSearchController` do.
#[test]
fn the_resource_table_matches_the_upstream_names() {
    assert_eq!(SearchResource::SitePages.name(), "site_pages");
    assert_eq!(SearchResource::Items.name(), "items");
    assert_eq!(SearchResource::ItemSets.name(), "item_sets");
    assert_eq!(SearchResource::SitePages.controller(), "page");
    assert_eq!(SearchResource::ItemSets.controller(), "item-set");
    assert_eq!(SearchResource::SitePages.cross_action(), "site-pages");
    assert_eq!(SearchResource::ItemSets.cross_action(), "item-sets");
    assert_eq!(SearchResource::SitePages.body_class(), "page");
    assert_eq!(SearchResource::Items.body_class(), "items");
    assert_eq!(SearchResource::ItemSets.body_class(), "item-sets");
    for resource in SearchResource::ALL {
        assert_eq!(SearchResource::from_name(resource.name()), Some(resource));
        assert_eq!(
            SearchResource::from_controller(resource.controller()),
            Some(resource)
        );
    }
    assert_eq!(SearchResource::from_name("media"), None);
    assert_eq!(SearchResource::from_name(""), None);
    assert_eq!(SearchResource::from_controller("cross-site-search"), None);
}

/// `index/search.phtml` links a section to the site browse; an absent needle
/// drops the query, as `http_build_query` drops a null.
#[test]
fn section_links_match_the_upstream_urls() {
    assert_eq!(
        SearchResource::Items.site_all_url(&site(), "cat"),
        "/s/main/item?fulltext_search=cat"
    );
    assert_eq!(
        SearchResource::SitePages.site_all_url(&site(), "welcome"),
        "/s/main/page?fulltext_search=welcome"
    );
    assert_eq!(
        SearchResource::ItemSets.site_all_url(&site(), ""),
        "/s/main/item-set"
    );
    assert_eq!(
        SearchResource::Items.cross_all_url(&site(), "cat"),
        "/s/main/cross-site-search/items?fulltext_search=cat"
    );
    assert_eq!(
        SearchResource::ItemSets.cross_all_url(&site(), ""),
        "/s/main/cross-site-search/item-sets"
    );
}

/// A search with no matching resource has no sections, which is the `empty`
/// test the results pages make.
#[test]
fn results_are_empty_without_sections() {
    let mut results = SearchResults::new("cat");
    assert!(results.is_empty());
    results.sections.push(SearchSection::new(
        SearchResource::Items,
        vec![SearchHit::new("A cat", "/s/main/item/1")],
        1,
    ));
    assert!(!results.is_empty());
    assert_eq!(results.needle, "cat");
}

/// `search_resource_names` selects the sections in its own order.
#[test]
fn stored_resource_names_keep_their_order() {
    let names = ["item_sets", "site_pages"];
    let parsed: Vec<SearchResource> = names
        .iter()
        .filter_map(|name| SearchResource::from_name(name))
        .collect();
    assert_eq!(
        parsed,
        vec![SearchResource::ItemSets, SearchResource::SitePages]
    );
}

/// `properties.phtml` keeps the rows with text, numbers them from zero and
/// leaves one empty row when the query carries none.
#[test]
fn property_rows_follow_the_template() {
    let form = AdvancedSearchForm::new(
        SearchResource::Items,
        "/s/main/item",
        query(&[
            ("property[0][property]", "1"),
            ("property[0][type]", "in"),
            ("property[0][text]", "cat"),
            ("property[1][property]", "2"),
            ("property[1][text]", "  "),
        ]),
    );
    let rows = form.property_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].index, 0);
    assert_eq!(rows[0].property, "1");
    assert_eq!(rows[0].kind, "in");
    assert_eq!(rows[0].text, "cat");

    let empty = AdvancedSearchForm::new(SearchResource::Items, "/s/main/item", Vec::new());
    let rows = empty.property_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].index, 0);
    assert_eq!(rows[0].property, "");
    assert_eq!(rows[0].kind, "");
}

/// `resource_class_id[]`, `item_set_id[]` and `not_item_set_id[]` read every
/// form PHP's `parse_str` accepts.
#[test]
fn list_parameters_read_the_scalar_and_bracket_forms() {
    let rows = AdvancedSearchForm::new(
        SearchResource::Items,
        "/s/main/item",
        query(&[
            ("resource_class_id[]", "3"),
            ("resource_class_id[1]", "4"),
            ("item_set_id[]", "5"),
            ("not_item_set_id", "6"),
        ]),
    );
    assert_eq!(rows.class_ids(), vec!["3", "4"]);
    let sets = rows.item_set_rows();
    assert_eq!(sets.len(), 2);
    assert_eq!(sets[0].id, "5");
    assert!(!sets[0].not_in);
    assert_eq!(sets[1].id, "6");
    assert!(sets[1].not_in);
}

/// `ids.phtml` joins a repeated `id` with commas.
#[test]
fn the_id_field_joins_repeated_values() {
    let form = AdvancedSearchForm::new(
        SearchResource::Items,
        "/s/main/item",
        query(&[("id", "7"), ("id", "8")]),
    );
    assert_eq!(form.comma_joined("id"), "7,8");
    assert_eq!(form.comma_joined("absent"), "");
}

/// `sort.phtml` keeps a value only when the browse default did not set it.
#[test]
fn sort_values_keep_what_the_query_set() {
    let form = AdvancedSearchForm::new(
        SearchResource::Items,
        "/s/main/item",
        query(&[("sort_by", "title"), ("sort_order", "asc")]),
    );
    assert_eq!(form.value("sort_by"), Some("title"));
    assert_eq!(form.value("sort_order"), Some("asc"));
    assert_eq!(form.value("fulltext_search"), None);
}

/// `search-form.phtml`: `search_type` picks the form's action.
#[test]
fn search_type_picks_the_form_action() {
    assert_eq!(SearchType::parse("sitewide"), SearchType::Sitewide);
    assert_eq!(SearchType::parse("cross-site"), SearchType::CrossSite);
    assert_eq!(SearchType::parse("other"), SearchType::Sitewide);
    assert_eq!(
        SearchType::Sitewide.form_action(&site()),
        "/s/main/index/search"
    );
    assert_eq!(
        SearchType::CrossSite.form_action(&site()),
        "/s/main/cross-site-search/results"
    );
    assert_eq!(
        cross_site_form_action(&site()),
        "/s/main/cross-site-search/results"
    );
}

/// `keeps_row` drops a row whose text is blank and keeps one with no text.
#[test]
fn blank_property_text_drops_the_row() {
    assert!(keeps_row(""));
    assert!(keeps_row("cat"));
    assert!(!keeps_row("   "));
}
