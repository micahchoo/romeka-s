//! The public route table: path in, [`Outcome`](super::Outcome) out.
//!
//! `application/config/routes.config.php:16-116` fixes the paths; each arm here
//! mirrors the controller that route reaches (`src/Controller/Site/*`). The
//! search routes build their pages in [`search`].

mod search;

use axum::http::Uri;

use omeka_api::read::catalog::Catalog;
use omeka_core::ids::PageId;
use omeka_core::resource::{ResourceKind, Timestamp};
use omeka_core::site::{Site, SitePage};
use omeka_db::DbError;
use omeka_db::ids as db;
use omeka_db::repos::{ItemRepo, ItemSetRepo, MediaRepo, SiteRepo};
use omeka_db::rows::ResourceRow;
use omeka_site::blocks::browse_preview::parse_query;
use omeka_site::nav::{self, NavItem, PageRef};
use omeka_site::render::{render_index, render_page};
use omeka_site::search::SearchResource;
use omeka_site::settings::SiteSettings;
use omeka_site::slug::{PublicRoute, kind_for_controller, page_url, parse_public_path};
use omeka_site::theme::{DefaultTheme, Theme};
use omeka_site::view::{NoResources, RenderContext, ResourceProvider, View};
use sqlx::SqlitePool;

use super::load::{self, BrowseScope, Loader};
use super::provider;
use super::{Outcome, SiteError, route_slug};

use crate::AppState;

pub(crate) async fn dispatch(state: &AppState, uri: &Uri) -> Result<Outcome, SiteError> {
    let route = parse_public_path(uri.path()).map_err(|_| SiteError::NotFound)?;
    let slug = route_slug(&route);
    let row = SiteRepo::new(&state.pool)
        .get_by_slug(slug.as_str())
        .await?
        .ok_or(SiteError::NotFound)?;
    if !row.is_public {
        return Err(SiteError::Forbidden);
    }

    let site = load::site_from_row(&row)?;
    let catalog = load_catalog(&state.pool).await?;
    let loader = Loader::new(&state.pool, &state.base, &catalog, &site);
    let pages = loader.page_refs().await?;
    // A corrupt navigation blob renders an empty menu, not a 500;
    // `parse_navigation` is the storage boundary.
    let nav = nav::resolve(
        &nav::parse_navigation(&row.navigation).unwrap_or_default(),
        &site.slug,
        &pages,
    );
    let common = Common {
        site: &site,
        pages,
        nav,
        // There is no `site_setting` repository yet, so every setting keeps the
        // default its upstream caller passes (gap).
        settings: SiteSettings::default(),
        query: parse_query(uri.query().unwrap_or_default()),
        loader: &loader,
        current_path: uri.path().to_string(),
    };

    match route {
        PublicRoute::Home { .. } => home(&common),
        PublicRoute::Page { page_slug, .. } => show_page(&common, &page_slug).await,
        PublicRoute::PageBrowse { .. } => search::page_browse(&common).await,
        PublicRoute::ResourceBrowse { controller, .. } if controller == "search" => {
            search::site_search(&common).await
        }
        PublicRoute::ResourceBrowse { controller, .. } => browse(&common, &controller).await,
        PublicRoute::ResourceShow { controller, id, .. } => show(&common, &controller, id).await,
        PublicRoute::CrossSiteSearch { action, .. } => {
            search::cross_site_search(&common, action.as_deref()).await
        }
        // `Site\IndexController::searchAction` is `/s/:site/index/search`
        // (`site/resource` with controller `index`); the port keeps
        // `/s/:site/search` too.
        PublicRoute::ResourceAction {
            controller, action, ..
        } if action == "search" && controller == "index" => search::site_search(&common).await,
        PublicRoute::ResourceAction {
            controller, action, ..
        } if action == "search" => match SearchResource::from_controller(&controller) {
            // `Site\PageController` has no search action; a site page advanced
            // form does not exist upstream.
            Some(resource) if resource != SearchResource::SitePages => {
                let action = search::browse_action(&common, resource);
                search::advanced_form(&common, resource, action).await
            }
            _ => Err(SiteError::NotFound),
        },
        PublicRoute::ResourceShowAction {
            controller,
            id,
            action,
            ..
        } if action == "show" => show(&common, &controller, id).await,
        PublicRoute::ResourceAction { .. } | PublicRoute::ResourceShowAction { .. } => {
            Err(SiteError::NotFound)
        }
    }
}

/// The shared values one site render reads.
struct Common<'a> {
    site: &'a Site,
    pages: Vec<PageRef>,
    nav: Vec<NavItem>,
    settings: SiteSettings,
    query: Vec<(String, String)>,
    loader: &'a Loader<'a>,
    current_path: String,
}

impl<'a> Common<'a> {
    fn context<'b>(
        &'b self,
        page: Option<&'b SitePage>,
        resources: &'b dyn ResourceProvider,
    ) -> RenderContext<'b> {
        let mut ctx = RenderContext::new(self.site, resources);
        ctx.page = page;
        ctx.settings = self.settings.clone();
        ctx.nav = self.nav.clone();
        ctx.pages = self.pages.clone();
        ctx.query = self.query.clone();
        ctx.current_path = self.current_path.clone();
        ctx
    }
}

/// `Site\IndexController::indexAction`: redirect to the homepage, else to the
/// first page linked in the navigation, else render the index.
fn home(common: &Common<'_>) -> Result<Outcome, SiteError> {
    if let Some(id) = common.site.homepage
        && let Some(page) = common.pages.iter().find(|page| page.id == id)
    {
        return Ok(Outcome::Redirect(page_url(&common.site.slug, &page.slug)));
    }
    if let Some(id) = first_linked_page(&common.nav)
        && let Some(page) = common.pages.iter().find(|page| page.id == id)
    {
        return Ok(Outcome::Redirect(page_url(&common.site.slug, &page.slug)));
    }
    let ctx = common.context(None, &NoResources);
    Ok(Outcome::Html(render_index(&ctx)))
}

/// `Site\PageController::showAction`: the page of this site with this slug.
async fn show_page(common: &Common<'_>, page_slug: &str) -> Result<Outcome, SiteError> {
    let Some(row) = common.loader.page_row(page_slug).await? else {
        return Err(SiteError::NotFound);
    };
    if !row.is_public {
        return Err(SiteError::Forbidden);
    }
    let page = common.loader.page_from_row(&row).await?;
    // The snapshot runs before the context exists, so no `RenderContext` lives
    // across an `await`; the page then renders against the snapshot, so a
    // block's synchronous search finds its answer.
    let loaded = provider::load_blocks(
        common.loader,
        &page,
        common.site.id,
        &common.settings,
        &common.query,
    )
    .await?;
    let mut ctx = common.context(Some(&page), &loaded);
    ctx.page_created = Some(Timestamp::from_naive(row.created));
    ctx.page_modified = row.modified.map(Timestamp::from_naive);
    Ok(Outcome::Html(render_page(&page, &mut ctx)))
}

/// `Site\ItemController::browseAction` and `Site\ItemSetController::browseAction`.
async fn browse(common: &Common<'_>, controller: &str) -> Result<Outcome, SiteError> {
    browse_scoped(common, controller, BrowseScope::Site).await
}

/// The public browse view for one kind, with the scope the caller's controller
/// puts on the query.
async fn browse_scoped(
    common: &Common<'_>,
    controller: &str,
    scope: BrowseScope,
) -> Result<Outcome, SiteError> {
    let Some(kind) = kind_for_controller(controller) else {
        return Err(SiteError::NotFound);
    };
    let per_page = query_usize(&common.query, "per_page").unwrap_or(10).max(1);
    let page = query_usize(&common.query, "page").unwrap_or(1).max(1);
    let browse = common
        .loader
        .browse(kind, per_page, page, &common.query, scope)
        .await?;
    let ctx = common.context(None, &NoResources);
    Ok(Outcome::Html(
        DefaultTheme.render(&View::Browse(&browse), &ctx),
    ))
}

/// `Site\ItemController::showAction`, `Site\ItemSetController` and
/// `Site\MediaController::showAction`: read the resource by id.
async fn show(common: &Common<'_>, controller: &str, id: u64) -> Result<Outcome, SiteError> {
    let Some(kind) = kind_for_controller(controller) else {
        return Err(SiteError::NotFound);
    };
    let resource = match kind {
        ResourceKind::Item => {
            let row = ItemRepo::new(common.loader.pool())
                .get(db::ItemId::from(id))
                .await?
                .ok_or(SiteError::NotFound)?;
            require_public(&row.resource)?;
            common.loader.item_resource_view(&row).await?
        }
        ResourceKind::ItemSet => {
            let row = ItemSetRepo::new(common.loader.pool())
                .get(db::ItemSetId::from(id))
                .await?
                .ok_or(SiteError::NotFound)?;
            require_public(&row.resource)?;
            common.loader.item_set_resource_view(&row).await?
        }
        ResourceKind::Media => {
            let row = MediaRepo::new(common.loader.pool())
                .get(db::MediaId::from(id))
                .await?
                .ok_or(SiteError::NotFound)?;
            require_public(&row.resource)?;
            common.loader.media_resource_view(&row).await?
        }
    };
    let ctx = common.context(None, &NoResources);
    let view = match kind {
        ResourceKind::Item => View::Item(&resource),
        ResourceKind::ItemSet => View::ItemSet(&resource),
        ResourceKind::Media => View::Media(&resource),
    };
    Ok(Outcome::Html(DefaultTheme.render(&view, &ctx)))
}

fn require_public(row: &ResourceRow) -> Result<(), SiteError> {
    if row.is_public {
        Ok(())
    } else {
        Err(SiteError::Forbidden)
    }
}

/// The first page id the navigation links, depth-first.
fn first_linked_page(items: &[NavItem]) -> Option<PageId> {
    for item in items {
        if let Some(page) = item.page {
            return Some(page);
        }
        if let Some(found) = first_linked_page(&item.children) {
            return Some(found);
        }
    }
    None
}

fn query_usize(query: &[(String, String)], key: &str) -> Option<usize> {
    query
        .iter()
        .find(|(name, _)| name == key)
        .and_then(|(_, value)| value.parse().ok())
}

/// `Catalog::load` reports a database failure as `ApiError`; the site layer's
/// internal variant carries a `DbError`, so the message is wrapped here.
async fn load_catalog(pool: &SqlitePool) -> Result<Catalog, SiteError> {
    Catalog::load(pool).await.map_err(|error| {
        SiteError::Internal(DbError::Column {
            column: "catalog",
            reason: error.to_string(),
        })
    })
}
