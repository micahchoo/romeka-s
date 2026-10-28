//! The site-page search and the advanced form's options.
//!
//! `Site\PageController::browseAction` searches the `site_pages` resource, and
//! `CrossSiteSearchController::sitePagesAction` searches it across sites. The
//! `site_page` table is not an API adapter of its own in the port, so the read
//! lives here: the site's public pages intersected with the `site_pages`
//! fulltext index the API writes (`omeka-api/src/write/site_pages.rs:83`).
//!
//! The advanced form's selects read the vocabulary tables and the current site's
//! item sets, which is what `common/advanced-search/properties.phtml`,
//! `resource-class.phtml` and `item-sets.phtml` ask their view helpers for.

use std::collections::BTreeMap;

use omeka_api::read::catalog::Catalog;
use omeka_core::site::{Site, Slug};
use omeka_db::DbError;
use omeka_db::DbPool;
use omeka_db::ids as db;
use omeka_db::repos::{
    ClassFilter, ClassRepo, FulltextRepo, ItemSetRepo, Paging, PropertyFilter, PropertyRepo,
    SiteFilter, SiteRepo,
};
use omeka_db::rows::SitePageRow;
use omeka_db::with_db;
use omeka_site::i18n::Translator;
use omeka_site::search::{FormOption, SearchHit, SiteRef};
use omeka_site::slug::{page_url, site_url};

use super::load::{Access, BrowseScope, display_title, show_public_titles, site_from_row};

/// The select options the advanced search form offers.
pub(crate) struct FormOptions {
    /// The property select's options, `(id, label)`.
    pub(crate) properties: Vec<FormOption>,
    /// The class select's options, `(id, label)`.
    pub(crate) classes: Vec<FormOption>,
    /// The item-set select's options, the current site's item sets.
    pub(crate) item_sets: Vec<FormOption>,
}

/// The public pages a page search selects: the current site's for
/// `BrowseScope::Site` (`Site\PageController::browseAction`), every public
/// site's for `BrowseScope::AnySite`
/// (`CrossSiteSearchController::sitePagesAction`).
///
/// A needle ranks through the fulltext index; with no needle the pages keep the
/// repository's creation order.
pub(crate) async fn page_hits(
    pool: &DbPool,
    site: &Site,
    scope: BrowseScope,
    needle: &str,
    access: &Access<'_>,
) -> Result<Vec<SearchHit>, DbError> {
    let ranks = page_ranks(pool, needle).await?;
    let mut hits: Vec<(f64, SearchHit)> = Vec::new();
    match scope {
        BrowseScope::Site => {
            let page_site = PageSite::of(site);
            for row in with_db!(pool, |db| SiteRepo::new(db).list_pages(page_site.id).await)? {
                // `SitePageVisibilityFilter` reads with the actor, so an owner
                // sees their private page on their own site's browse.
                if !access.sees_site_page(row.is_public) {
                    continue;
                }
                let Some(rank) = page_rank(&ranks, &row) else {
                    continue;
                };
                if let Some(hit) = page_hit(&page_site, &row) {
                    hits.push((rank, hit));
                }
            }
        }
        BrowseScope::AnySite => {
            for page_site in public_page_sites(pool).await? {
                for row in with_db!(pool, |db| SiteRepo::new(db).list_pages(page_site.id).await)? {
                    if !row.is_public {
                        continue;
                    }
                    let Some(rank) = page_rank(&ranks, &row) else {
                        continue;
                    };
                    if let Some(hit) = page_hit(&page_site, &row) {
                        hits.push((rank, hit));
                    }
                }
            }
        }
    }
    // A fulltext search ranks; otherwise the rows keep the order the loop
    // walked them in.
    if !needle.trim().is_empty() {
        hits.sort_by(|(left, _), (right, _)| right.total_cmp(left));
    }
    Ok(hits.into_iter().map(|(_, hit)| hit).collect())
}

/// The select options the advanced form offers.
///
/// Upstream narrows the property and class selects to the members the site uses
/// when `vocabulary_scope` is `sitewide`; the port lists every row, which is the
/// `cross-site` scope. Laminas `formSelect` translates each option label, so a
/// built-in term's label is in the site's locale.
pub(crate) async fn form_options(
    pool: &DbPool,
    catalog: &Catalog,
    site: db::SiteId,
    translator: Translator,
) -> Result<FormOptions, DbError> {
    let messages = translator.messages();
    let label = |vocabulary: db::VocabularyId, local_name: &str, stored: &str| {
        let term = format!("{}:{local_name}", catalog.prefix(vocabulary));
        translator.term_label(&term, stored).to_string()
    };
    let properties = with_db!(pool, |db| PropertyRepo::new(db)
        .list(&PropertyFilter::default(), &Paging::default())
        .await)?
    .into_iter()
    .map(|row| {
        let text = label(row.vocabulary_id, &row.local_name, &row.label);
        FormOption::new(row.id.0.to_string(), text)
    })
    .collect();
    let classes = with_db!(pool, |db| ClassRepo::new(db)
        .list(&ClassFilter::default(), &Paging::default())
        .await)?
    .into_iter()
    .filter(|row| !row.label.is_empty())
    .map(|row| {
        let text = label(row.vocabulary_id, &row.local_name, &row.label);
        FormOption::new(row.id.0.to_string(), text)
    })
    .collect();
    let mut rows = Vec::new();
    for entry in with_db!(pool, |db| SiteRepo::new(db).list_item_sets(site).await)? {
        if let Some(row) = with_db!(pool, |db| ItemSetRepo::new(db).get(entry.item_set_id).await)? {
            rows.push(row);
        }
    }
    show_public_titles(pool, rows.iter_mut().map(|row| &mut row.resource)).await?;
    let item_sets = rows
        .iter()
        .map(|row| {
            FormOption::new(
                row.id().0.to_string(),
                display_title(&row.resource, messages),
            )
        })
        .collect();
    Ok(FormOptions {
        properties,
        classes,
        item_sets,
    })
}

/// The ranks the fulltext index gives the `site_pages` rows matching a needle,
/// `None` when the needle is blank. A page the index does not hold is not a hit,
/// which is what the index join does upstream.
async fn page_ranks(pool: &DbPool, needle: &str) -> Result<Option<BTreeMap<u64, f64>>, DbError> {
    if needle.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(
        with_db!(pool, |db| FulltextRepo::new(db)
            .search("site_pages", needle)
            .await)?
        .into_iter()
        .map(|hit| (hit.id.0, hit.score))
        .collect(),
    ))
}

/// Every public site whose slug parses, for a cross-site page search.
async fn public_page_sites(pool: &DbPool) -> Result<Vec<PageSite>, DbError> {
    let rows = with_db!(pool, |db| SiteRepo::new(db)
        .list(&SiteFilter::default(), &Paging::default())
        .await)?;
    Ok(rows
        .iter()
        .filter(|row| row.is_public)
        .filter_map(|row| site_from_row(row).ok())
        .map(|site| PageSite::of(&site))
        .collect())
}

/// One public site as a page search walks it.
struct PageSite {
    id: db::SiteId,
    slug: Slug,
    title: String,
}

impl PageSite {
    fn of(site: &Site) -> Self {
        Self {
            id: db::SiteId::from(site.id.get()),
            slug: site.slug.clone(),
            title: site.title.clone(),
        }
    }
}

/// One page result: the page title, its site URL and the site it belongs to.
fn page_hit(site: &PageSite, row: &SitePageRow) -> Option<SearchHit> {
    let slug = Slug::parse(&row.slug).ok()?;
    Some(SearchHit {
        title: row.title.clone(),
        url: page_url(&site.slug, &slug),
        thumbnail: None,
        site: Some(SiteRef::new(site.title.clone(), site_url(&site.slug))),
    })
}

/// The rank a page row's id carries, `None` when the index does not hold it.
/// Without a needle every page is a hit at rank zero.
fn page_rank(ranks: &Option<BTreeMap<u64, f64>>, row: &SitePageRow) -> Option<f64> {
    match ranks {
        Some(ranks) => ranks.get(&row.id.0).copied(),
        None => Some(0.0),
    }
}
