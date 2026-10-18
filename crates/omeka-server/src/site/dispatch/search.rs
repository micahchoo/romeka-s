//! The public search pages: the request query and the loaded rows composed into
//! the `omeka-site` search values.
//!
//! Ported from `Site\IndexController::searchAction`,
//! `Site\CrossSiteSearchController`, `Site\PageController::browseAction` and the
//! two empty `searchAction`s of `Site\ItemController` and
//! `Site\ItemSetController` (which exist only to render the advanced form).
//! [`super::dispatch`] reads the route; the functions here read rows and render.

use omeka_core::resource::ResourceKind;
use omeka_db::ids as db;
use omeka_site::search::{
    AdvancedSearchForm, CrossSiteBody, CrossSitePage, SearchList, SearchResource, SearchResults,
    SearchSection, cross_site_form_action,
};
use omeka_site::slug::{resource_action_url, resource_browse_url};
use omeka_site::theme::{DefaultTheme, Theme};
use omeka_site::view::{NoResources, View};

use super::super::pages;
use super::{BrowseScope, Common, Outcome, SiteError, query_usize};

/// The `per_page` a public list falls back to, the same default the site browse
/// uses.
const DEFAULT_PER_PAGE: usize = 10;
/// `IndexController::searchAction` and `CrossSiteSearchController::resultsAction`
/// search every section with `limit` 10.
const SECTION_LIMIT: usize = 10;
/// The resource names `SearchResource` maps onto `ResourceKind`.
fn kind_of(resource: SearchResource) -> Option<ResourceKind> {
    match resource {
        SearchResource::Items => Some(ResourceKind::Item),
        SearchResource::ItemSets => Some(ResourceKind::ItemSet),
        SearchResource::SitePages => None,
    }
}

/// `Site\IndexController::searchAction` (`/s/:site/index/search`, and the
/// port's `/s/:site/search`): one section per configured resource name.
pub(super) async fn site_search(common: &Common<'_>) -> Result<Outcome, SiteError> {
    let needle = needle(common);
    let names = common.settings.search_resource_names();
    // `count($resourceNames) === 1` redirects straight to the browse.
    if let [resource] = names[..] {
        return Ok(Outcome::Redirect(
            resource.site_all_url(&common.site.slug, &needle),
        ));
    }
    let mut sections = Vec::new();
    let query = section_query(common, &needle);
    for resource in names {
        if let Some(section) = section(common, resource, &needle, &query, BrowseScope::Site).await?
        {
            sections.push(section);
        }
    }
    let results = SearchResults { needle, sections };
    Ok(render(&View::Search(&results), common))
}

/// One section of a results page, `None` when the query matched nothing, which
/// is upstream's `if (!$totalResults) continue;`.
async fn section(
    common: &Common<'_>,
    resource: SearchResource,
    needle: &str,
    query: &[(String, String)],
    scope: BrowseScope,
) -> Result<Option<SearchSection>, SiteError> {
    let (hits, total) = match kind_of(resource) {
        Some(kind) => {
            common
                .loader
                .search_hits(kind, query, Some(SECTION_LIMIT), scope)
                .await?
        }
        None => {
            let hits = pages::page_hits(common.loader.pool(), common.site, scope, needle).await?;
            let total = hits.len();
            (truncate(hits, SECTION_LIMIT), total)
        }
    };
    if total == 0 {
        return Ok(None);
    }
    Ok(Some(SearchSection::new(resource, hits, total)))
}

/// The query one search section runs: the needle, and the attachment narrowing
/// `browse_attached_items` adds.
fn section_query(common: &Common<'_>, needle: &str) -> Vec<(String, String)> {
    let mut query = Vec::new();
    if !needle.is_empty() {
        query.push(("fulltext_search".to_string(), needle.to_string()));
    }
    if common.settings.browse_attached_items() {
        query.push(("site_attachments_only".to_string(), "true".to_string()));
    }
    query
}

/// `Site\CrossSiteSearchController` (`/s/:site/cross-site-search[/:action]`).
pub(super) async fn cross_site_search(
    common: &Common<'_>,
    action: Option<&str>,
) -> Result<Outcome, SiteError> {
    let needle = needle(common);
    let body = match action {
        None | Some("index") => CrossSiteBody::Form,
        Some("results") => {
            let names = common.settings.search_resource_names();
            // `count($resourceNames) === 1` redirects to that resource's action.
            if let [resource] = names[..] {
                return Ok(Outcome::Redirect(
                    resource.cross_all_url(&common.site.slug, &needle),
                ));
            }
            CrossSiteBody::Results(results_sections(common, &needle).await?)
        }
        Some("items") => CrossSiteBody::List(Box::new(
            cross_site_list(common, SearchResource::Items).await?,
        )),
        Some("item-sets") => CrossSiteBody::List(Box::new(
            cross_site_list(common, SearchResource::ItemSets).await?,
        )),
        Some("site-pages") => CrossSiteBody::List(Box::new(
            cross_site_list(common, SearchResource::SitePages).await?,
        )),
        Some("items-advanced") => {
            let action = cross_action_url(common, "items");
            return advanced_form(common, SearchResource::Items, action).await;
        }
        Some("item-sets-advanced") => {
            let action = cross_action_url(common, "item-sets");
            return advanced_form(common, SearchResource::ItemSets, action).await;
        }
        Some(_) => return Err(SiteError::NotFound),
    };
    let page = CrossSitePage {
        needle,
        form_action: cross_site_form_action(&common.site.slug),
        body,
    };
    Ok(render(&View::CrossSiteSearch(&page), common))
}

/// `resultsAction`: one section per configured resource name, each searched
/// with `limit` 10 across sites.
async fn results_sections(
    common: &Common<'_>,
    needle: &str,
) -> Result<Vec<SearchSection>, SiteError> {
    let mut sections = Vec::new();
    let query = needle_query(needle);
    for resource in common.settings.search_resource_names() {
        if let Some(section) =
            section(common, resource, needle, &query, BrowseScope::AnySite).await?
        {
            sections.push(section);
        }
    }
    Ok(sections)
}

fn needle_query(needle: &str) -> Vec<(String, String)> {
    if needle.is_empty() {
        return Vec::new();
    }
    vec![("fulltext_search".to_string(), needle.to_string())]
}

/// `itemsAction`, `itemSetsAction` and `sitePagesAction`: one paginated list.
async fn cross_site_list(
    common: &Common<'_>,
    resource: SearchResource,
) -> Result<SearchList, SiteError> {
    let needle = needle(common);
    let (page, per_page) = window(common);
    let hits = match kind_of(resource) {
        Some(kind) => {
            common
                .loader
                .search_hits(kind, &needle_query(&needle), None, BrowseScope::AnySite)
                .await?
                .0
        }
        None => {
            pages::page_hits(
                common.loader.pool(),
                common.site,
                BrowseScope::AnySite,
                &needle,
            )
            .await?
        }
    };
    let total = hits.len();
    Ok(SearchList {
        resource,
        hits: slice(hits, page, per_page),
        total,
        page,
        per_page,
    })
}

/// `Site\PageController::browseAction` (`/s/:site/page`): the site's pages a
/// query selects.
pub(super) async fn page_browse(common: &Common<'_>) -> Result<Outcome, SiteError> {
    let needle = needle(common);
    let (page, per_page) = window(common);
    let hits = pages::page_hits(
        common.loader.pool(),
        common.site,
        BrowseScope::Site,
        &needle,
    )
    .await?;
    let total = hits.len();
    let list = SearchList {
        resource: SearchResource::SitePages,
        hits: slice(hits, page, per_page),
        total,
        page,
        per_page,
    };
    Ok(render(&View::SearchList(&list), common))
}

/// `Site\ItemController::searchAction` and `Site\ItemSetController::searchAction`
/// (`/s/:site/item/search`, `/s/:site/item-set/search`): the advanced form, which
/// posts to the browse action of the same controller.
pub(super) async fn advanced_form(
    common: &Common<'_>,
    resource: SearchResource,
    action: String,
) -> Result<Outcome, SiteError> {
    let options =
        pages::form_options(common.loader.pool(), db::SiteId::from(common.site.id.get())).await?;
    let form = AdvancedSearchForm {
        resource,
        action,
        query: common.query.clone(),
        properties: options.properties,
        classes: options.classes,
        // `item-sets.phtml` hides the field for a site with no item sets, and
        // only the item form carries it at all.
        item_sets: if resource == SearchResource::Items {
            options.item_sets
        } else {
            Vec::new()
        },
    };
    Ok(render(&View::AdvancedSearch(&form), common))
}

/// `/s/:site/item` and `/s/:site/item-set`: where a site advanced form posts.
pub(super) fn browse_action(common: &Common<'_>, resource: SearchResource) -> String {
    resource_browse_url(&common.site.slug, resource.controller(), None)
}

/// `/s/:site/cross-site-search/items`: where a cross-site advanced form posts.
fn cross_action_url(common: &Common<'_>, action: &str) -> String {
    resource_action_url(&common.site.slug, "cross-site-search", action)
}

/// The `fulltext_search` the request carried.
fn needle(common: &Common<'_>) -> String {
    common
        .query
        .iter()
        .find(|(key, _)| key == "fulltext_search")
        .map(|(_, value)| value.clone())
        .unwrap_or_default()
}

/// The one-based page and the page size a list request asks for.
fn window(common: &Common<'_>) -> (usize, usize) {
    let per_page = query_usize(&common.query, "per_page")
        .unwrap_or(DEFAULT_PER_PAGE)
        .max(1);
    let page = query_usize(&common.query, "page").unwrap_or(1).max(1);
    (page, per_page)
}

fn truncate(
    hits: Vec<omeka_site::search::SearchHit>,
    limit: usize,
) -> Vec<omeka_site::search::SearchHit> {
    hits.into_iter().take(limit).collect()
}

fn slice(
    hits: Vec<omeka_site::search::SearchHit>,
    page: usize,
    per_page: usize,
) -> Vec<omeka_site::search::SearchHit> {
    hits.into_iter()
        .skip(page.saturating_sub(1).saturating_mul(per_page))
        .take(per_page)
        .collect()
}

/// Render a search view through the bundled theme, with no page and no
/// provider: the search views read their values from the view itself.
fn render(view: &View<'_>, common: &Common<'_>) -> Outcome {
    let ctx = common.context(None, &NoResources);
    Outcome::Html(DefaultTheme.render(view, &ctx))
}
