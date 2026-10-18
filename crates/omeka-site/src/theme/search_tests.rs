//! The search views' own assertions: the document each view builds from typed
//! values, without a database.
//!
//! The server's `tests/site_search.rs` proves the routes end to end over a
//! seeded database; these tests name the upstream template each fragment comes
//! from, so a change to the markup fails here with the reason.

use crate::blocks::test_support::{context, site};
use crate::i18n::Locale;
use crate::search::{
    AdvancedSearchForm, CrossSiteBody, CrossSitePage, FormOption, SearchHit, SearchList,
    SearchResource, SearchResults, SearchSection, SearchType, SiteRef,
};
use crate::theme::Theme;
use crate::theme::default::DefaultTheme;
use crate::view::{NoResources, Thumbnail, View};

fn render(view: &View<'_>, ctx: &crate::view::RenderContext<'_>) -> String {
    DefaultTheme.render(view, ctx)
}

/// `index/search.phtml`: the heading, one section per resource, each hit linked,
/// and the section's `View all results` link carrying the needle.
#[test]
fn search_view_matches_index_search_phtml() {
    let site = site();
    let ctx = context(&site, &NoResources);
    let mut item = SearchHit::new("A picture of a cat", "/s/main/item/4");
    item.thumbnail = Some(Thumbnail::new("/files/square/4.jpg", "A picture of a cat"));
    let results = SearchResults {
        needle: "cat".to_string(),
        sections: vec![
            SearchSection::new(
                SearchResource::SitePages,
                vec![SearchHit::new("Welcome", "/s/main/page/welcome")],
                1,
            ),
            SearchSection::new(SearchResource::Items, vec![item], 7),
        ],
    };

    let document = render(&View::Search(&results), &ctx);
    assert!(
        document.contains("<h1>Search results for “cat”</h1>"),
        "{document}"
    );
    assert!(
        document.contains("<div class=\"site_pages results\"><h2>Site pages</h2><ul><li><a href=\"/s/main/page/welcome\">Welcome</a></li></ul>"),
        "{document}"
    );
    assert!(
        document.contains(
            "<li><a href=\"/s/main/item/4\"><img src=\"/files/square/4.jpg\" alt=\"A picture of a cat\">A picture of a cat</a></li>"
        ),
        "{document}"
    );
    assert!(
        document.contains(
            "<a href=\"/s/main/page?fulltext_search=cat\">View all results (1 total)</a>"
        ),
        "{document}"
    );
    assert!(
        document.contains(
            "<a href=\"/s/main/item?fulltext_search=cat\">View all results (7 total)</a>"
        ),
        "{document}"
    );

    // `empty($results)` prints the one line instead of any section.
    let empty = SearchResults::new("cat");
    assert!(
        render(&View::Search(&empty), &ctx)
            .contains("<h1>Search results for “cat”</h1><p>No result found</p>"),
        "no sections are no result"
    );
}

/// `cross-site-search/index.phtml` renders the fulltext form alone, and
/// `results.phtml` adds the sections.
#[test]
fn cross_site_view_matches_the_upstream_templates() {
    let site = site();
    let ctx = context(&site, &NoResources);
    let form = CrossSitePage {
        needle: "cat".to_string(),
        form_action: "/s/main/cross-site-search/results".to_string(),
        body: CrossSiteBody::Form,
    };
    let document = render(&View::CrossSiteSearch(&form), &ctx);
    assert!(
        document.contains("<form action=\"/s/main/cross-site-search/results\" class=\"sitewide-search-form\" role=\"search\">"),
        "{document}"
    );
    assert!(!document.contains("<h2>"), "{document}");

    let results = CrossSitePage {
        body: CrossSiteBody::Results(vec![SearchSection::new(
            SearchResource::Items,
            vec![SearchHit::new("A bronze plate", "/s/main/item/5")],
            1,
        )]),
        ..form.clone()
    };
    let document = render(&View::CrossSiteSearch(&results), &ctx);
    assert!(
        document.contains("<h2>Search results for “cat”</h2>"),
        "{document}"
    );
    assert!(
        document.contains("<div class=\"items results\"><h3>Items</h3>"),
        "{document}"
    );
    assert!(
        document.contains(
            "<a href=\"/s/main/cross-site-search/items?fulltext_search=cat\">View all results (1 total)</a>"
        ),
        "{document}"
    );
    assert!(
        !document.contains("No result found"),
        "a section is not an empty result: {document}"
    );
}

/// `common/cross-site-search/site-page-results.phtml`: a page hit names its
/// site; `item-results.phtml` carries the advanced link.
#[test]
fn cross_site_list_matches_the_result_templates() {
    let site = site();
    let ctx = context(&site, &NoResources);
    let mut page = SearchHit::new("Welcome", "/s/main/page/welcome");
    page.site = Some(SiteRef::new("Main Site", "/s/main"));
    let list = CrossSitePage {
        needle: String::new(),
        form_action: "/s/main/cross-site-search/results".to_string(),
        body: CrossSiteBody::List(Box::new(SearchList {
            resource: SearchResource::SitePages,
            hits: vec![page],
            total: 1,
            page: 1,
            per_page: 10,
        })),
    };
    let document = render(&View::CrossSiteSearch(&list), &ctx);
    assert!(
        document.contains("<h2>Site page results for &quot;&quot;</h2>"),
        "{document}"
    );
    assert!(
        document.contains(
            "<li><span class=\"result-title\"><a href=\"/s/main/page/welcome\" class=\"result-title\">Welcome</a></span><span class=\"result-site\"><a href=\"/s/main\">Main Site</a></span></li>"
        ),
        "{document}"
    );
    assert!(
        !document.contains("advanced-search"),
        "the site-page list has no advanced form upstream: {document}"
    );

    let items = CrossSitePage {
        body: CrossSiteBody::List(Box::new(SearchList {
            resource: SearchResource::Items,
            hits: vec![SearchHit::new("A bronze plate", "/s/main/item/5")],
            total: 1,
            page: 1,
            per_page: 10,
        })),
        needle: "bronze".to_string(),
        ..list
    };
    let document = render(&View::CrossSiteSearch(&items), &ctx);
    assert!(
        document.contains("<h2>Item results for &quot;bronze&quot;</h2>"),
        "{document}"
    );
    assert!(
        document.contains(
            "<a href=\"/s/main/cross-site-search/items-advanced\" class=\"advanced-search\">Advanced item search</a>"
        ),
        "{document}"
    );
    assert!(
        document.contains("<div class=\"items results\">"),
        "{document}"
    );
}

/// `page/browse.phtml`: the `Pages` heading, an `h3` link per page, and the
/// empty line instead of the list.
#[test]
fn search_list_view_matches_page_browse_phtml() {
    let site = site();
    let ctx = context(&site, &NoResources);
    let list = SearchList {
        resource: SearchResource::SitePages,
        hits: vec![SearchHit::new("Welcome", "/s/main/page/welcome")],
        total: 1,
        page: 1,
        per_page: 10,
    };
    let document = render(&View::SearchList(&list), &ctx);
    assert!(
        document.contains(
            "<h2>Pages</h2><ul class=\"resource-list\"><li class=\"page resource\"><h3><a href=\"/s/main/page/welcome\">Welcome</a></h3></li></ul>"
        ),
        "{document}"
    );
    assert!(
        document.contains("<body class=\"minimal page browse\">"),
        "{document}"
    );

    let empty = SearchList {
        hits: Vec::new(),
        total: 0,
        ..list
    };
    let document = render(&View::SearchList(&empty), &ctx);
    assert!(document.contains("<p>No pages found</p>"), "{document}");
}

/// `common/advanced-search.phtml` and its partials: the form posts every
/// parameter name the API's list endpoints read, and keeps the query's values.
#[test]
fn advanced_form_posts_the_api_parameter_names() {
    let site = site();
    let ctx = context(&site, &NoResources);
    let form = AdvancedSearchForm {
        resource: SearchResource::Items,
        action: "/s/main/item".to_string(),
        query: vec![
            ("fulltext_search".to_string(), "cat".to_string()),
            ("sort_by".to_string(), "title".to_string()),
            ("sort_order".to_string(), "desc".to_string()),
            ("property[0][property]".to_string(), "1".to_string()),
            ("property[0][type]".to_string(), "in".to_string()),
            ("property[0][text]".to_string(), "cat".to_string()),
            ("resource_class_id[]".to_string(), "3".to_string()),
            ("item_set_id[]".to_string(), "5".to_string()),
            ("has_media".to_string(), "1".to_string()),
            ("id".to_string(), "7".to_string()),
        ],
        properties: vec![FormOption::new("1", "Title")],
        classes: vec![FormOption::new("3", "Image")],
        item_sets: vec![FormOption::new("5", "Featured photographs")],
    };

    let document = render(&View::AdvancedSearch(&form), &ctx);
    assert!(
        document.contains("<h2>Advanced item search</h2>"),
        "{document}"
    );
    assert!(
        document.contains("<form id=\"advanced-search\" method=\"get\" action=\"/s/main/item\">"),
        "{document}"
    );
    for name in [
        "name=\"sort_by\"",
        "name=\"sort_order\"",
        "name=\"fulltext_search\"",
        "name=\"property[0][joiner]\"",
        "name=\"property[0][property]\"",
        "name=\"property[0][type]\"",
        "name=\"property[0][text]\"",
        "name=\"resource_class_id[]\"",
        "name=\"item_set_id[]\"",
        "name=\"has_media\"",
        "name=\"id\"",
    ] {
        assert!(document.contains(name), "{name}: {document}");
    }
    assert!(
        document.contains("<option value=\"title\" selected=\"selected\">Title</option>"),
        "{document}"
    );
    assert!(
        document.contains("<option value=\"desc\" selected=\"selected\">Descending</option>"),
        "{document}"
    );
    assert!(
        document.contains("<option value=\"1\" selected=\"selected\">Title</option>"),
        "the property select keeps the row's property: {document}"
    );
    assert!(
        document.contains("<option value=\"in\" selected=\"selected\">contains</option>"),
        "{document}"
    );
    assert!(
        document.contains("<input type=\"text\" name=\"id\" id=\"id\" value=\"7\">"),
        "{document}"
    );
    assert!(
        document.contains("<body class=\"minimal items search\">"),
        "{document}"
    );

    // `item-sets.phtml` returns early for a site with no item sets.
    let no_item_sets = AdvancedSearchForm {
        item_sets: Vec::new(),
        ..form.clone()
    };
    assert!(
        !render(&View::AdvancedSearch(&no_item_sets), &ctx).contains("item_set_id[]"),
        "the field is hidden without item sets"
    );

    let item_set_form = AdvancedSearchForm {
        resource: SearchResource::ItemSets,
        item_sets: Vec::new(),
        ..form
    };
    let document = render(&View::AdvancedSearch(&item_set_form), &ctx);
    assert!(
        document.contains("<h2>Advanced item set Search</h2>"),
        "the item-set form title keeps upstream's capital S: {document}"
    );
    assert!(
        document.contains("<body class=\"minimal item-sets search\">"),
        "{document}"
    );
    assert!(!document.contains("name=\"has_media\""), "{document}");
}

/// `sort.phtml` keeps the empty option when the browse default set the sort,
/// and `search-form.phtml` follows `search_type`.
#[test]
fn the_sort_and_search_forms_follow_the_query_and_the_setting() {
    let site = site();
    let ctx = context(&site, &NoResources);
    let defaulted = AdvancedSearchForm {
        resource: SearchResource::Items,
        action: "/s/main/item".to_string(),
        query: vec![
            ("sort_by".to_string(), "created".to_string()),
            ("sort_by_default".to_string(), String::new()),
        ],
        properties: Vec::new(),
        classes: Vec::new(),
        item_sets: Vec::new(),
    };
    let document = render(&View::AdvancedSearch(&defaulted), &ctx);
    assert!(
        document.contains("<option value=\"\" selected=\"selected\">Select sort by…</option>"),
        "a defaulted sort selects nothing: {document}"
    );

    let cross = crate::view::RenderContext {
        settings: crate::settings::SiteSettings {
            search_type: Some("cross-site".to_string()),
            ..Default::default()
        },
        ..context(&site, &NoResources)
    };
    let document = render(&View::Index, &cross);
    assert!(
        document.contains("<form action=\"/s/main/cross-site-search/results\" id=\"search-form\""),
        "{document}"
    );
    assert_eq!(cross.settings.search_type(), SearchType::CrossSite);
}

/// The frame's search form and the results page render in the site locale: the
/// Spanish catalogue supplies the labels.
#[test]
fn the_search_views_render_in_the_site_locale() {
    let site = site();
    let translated = crate::view::RenderContext {
        settings: crate::settings::SiteSettings {
            locale: Some("es".to_string()),
            ..Default::default()
        },
        ..context(&site, &NoResources)
    };
    let results = SearchResults {
        needle: "gato".to_string(),
        sections: vec![SearchSection::new(
            SearchResource::Items,
            vec![SearchHit::new("Una ficha", "/s/main/item/1")],
            1,
        )],
    };
    let document = render(&View::Search(&results), &translated);
    assert!(
        document.contains("<h1>Resultados de búsqueda para &quot;gato&quot;</h1>"),
        "{document}"
    );
    assert!(
        document.contains("<h2>Fichas</h2>"),
        "the section label follows the locale: {document}"
    );
    assert!(
        document.contains(">Ver todos los resultados (1 total)</a>"),
        "{document}"
    );
    assert!(
        document.contains("<span class=\"o-icon-search\""),
        "{document}"
    );
    assert_eq!(Locale::Es.translator().search_labels().search, "Búsqueda");
    assert!(
        render(&View::Index, &translated).contains("aria-label=\"Búsqueda\""),
        "the frame's form label follows the locale"
    );
}
