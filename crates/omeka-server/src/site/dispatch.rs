//! The public route table: path in, [`Outcome`](super::Outcome) out.
//!
//! `application/config/routes.config.php:16-116` fixes the paths; each arm here
//! mirrors the controller that route reaches (`src/Controller/Site/*`). The
//! search routes build their pages in [`search`].

mod search;

use axum::http::Uri;

use omeka_api::auth::Actor;
use omeka_api::read::catalog::Catalog;
use omeka_api::write::site_pages::ResolvedAttachment;
use omeka_core::ids::{BlockId, PageId, SiteId, UserId};
use omeka_core::permissions::{Role, RoleSet, SitePermissionRole};
use omeka_core::resource::{ResourceKind, Timestamp};
use omeka_core::site::{Site, SitePage, Slug};
use omeka_db::DbError;
use omeka_db::DbPool;
use omeka_db::ids as db;
use omeka_db::repos::{
    AssetRepo, ItemRepo, ItemSetRepo, MediaRepo, SettingRepo, SiteRepo, UserRepo,
};
use omeka_db::rows::{MediaRow, ResourceRow, SiteRow};
use omeka_db::with_db;
use omeka_media::render::MediaStart;
use omeka_site::blocks::browse_preview::parse_query;
use omeka_site::nav::{self, NavItem, PageRef};
use omeka_site::render::{render_index, render_page};
use omeka_site::resource_page::ResourcePageBlocks;
use omeka_site::search::SearchResource;
use omeka_site::settings::SiteSettings;
use omeka_site::slug::{PublicRoute, kind_for_controller, page_url, parse_public_path};
use omeka_site::theme::tokens::ThemeSettings;
use omeka_site::theme::{DefaultTheme, Theme, theme_for};
use omeka_site::view::{
    ErrorView, NoResources, RenderContext, ResourceProvider, ResourceView, View, Viewer,
};
use serde_json::Value;

use super::load::{self, Access, BrowseScope, Loader, thumbnail_for_asset};
use super::{Outcome, SiteError, route_slug};
use super::{provider, settings};

use crate::AppState;

/// Resolve a public path and render it, filtering every read by the actor's
/// visibility the way upstream's `prepareSite` and the visibility filters do.
pub(crate) async fn dispatch(
    state: &AppState,
    uri: &Uri,
    actor: &Actor,
) -> Result<Outcome, SiteError> {
    let route = parse_public_path(uri.path()).map_err(|_| SiteError::NotFound)?;
    let slug = route_slug(&route);
    let Some(data) = SiteData::load(state, &slug).await? else {
        return Err(SiteError::NotFound);
    };
    // A site the caller cannot see is not found, matching a private site's API
    // read: `MvcListeners::prepareSite` reads the site through the same
    // visibility the API applies, so a hidden site never reaches a controller.
    let site_permission = site_permission(&state.pool, data.row.id, actor).await?;
    if !actor.sees_site(data.row.is_public, data.row.owner_id, site_permission) {
        return Err(SiteError::NotFound);
    }
    let access = Access {
        actor,
        site_is_public: data.row.is_public,
        site_owner: data.row.owner_id,
        site_permission,
    };
    let loader = Loader::new(
        &state.pool,
        &state.base,
        &state.files,
        &data.catalog,
        &data.site,
        &access,
        &data.settings,
        state.extents.as_ref(),
    );
    let common = Common::load(state, uri, &data, &loader, actor, &data.theme_settings).await?;

    // Past `prepareSite` the site locale is set, so upstream's
    // `ExceptionListener` renders `error/404` in the site's locale and layout.
    match route_to(&common, route).await {
        Err(SiteError::NotFound) => Ok(Outcome::NotFound(not_found_page(&common))),
        outcome => outcome,
    }
}

/// The page or resource a route names under a resolved site.
async fn route_to(common: &Common<'_>, route: PublicRoute) -> Result<Outcome, SiteError> {
    match route {
        PublicRoute::Home { .. } => home(common),
        PublicRoute::Page { page_slug, .. } => show_page(common, &page_slug).await,
        PublicRoute::PageBrowse { .. } => search::page_browse(common).await,
        PublicRoute::ResourceBrowse { controller, .. } if controller == "search" => {
            search::site_search(common).await
        }
        PublicRoute::ResourceBrowse { controller, .. } => browse(common, &controller).await,
        PublicRoute::ResourceShow { controller, id, .. } => show(common, &controller, id).await,
        PublicRoute::CrossSiteSearch { action, .. } => {
            search::cross_site_search(common, action.as_deref()).await
        }
        // `Site\IndexController::searchAction` is `/s/:site/index/search`
        // (`site/resource` with controller `index`); the port keeps
        // `/s/:site/search` too.
        PublicRoute::ResourceAction {
            controller, action, ..
        } if action == "search" && controller == "index" => search::site_search(common).await,
        PublicRoute::ResourceAction {
            controller, action, ..
        } if action == "search" => match SearchResource::from_controller(&controller) {
            // `Site\PageController` has no search action; a site page advanced
            // form does not exist upstream.
            Some(resource) if resource != SearchResource::SitePages => {
                let action = search::browse_action(common, resource);
                search::advanced_form(common, resource, action).await
            }
            _ => Err(SiteError::NotFound),
        },
        PublicRoute::ResourceShowAction {
            controller,
            id,
            action,
            ..
        } if action == "show" => show(common, &controller, id).await,
        PublicRoute::ResourceAction { .. } | PublicRoute::ResourceShowAction { .. } => {
            Err(SiteError::NotFound)
        }
    }
}

/// `view/error/404.phtml` inside the resolved site's layout and locale.
fn not_found_page(common: &Common<'_>) -> String {
    let ctx = common.context(None, &NoResources);
    let error = ErrorView::not_found(ctx.translator().messages());
    common.theme.render(&View::Error(&error), &ctx)
}

/// The rows and settings one public render reads, owned for the request.
///
/// Kept apart from [`Common`] because a [`Loader`] borrows these values and a
/// struct cannot hold both itself and a borrow of itself.
struct SiteData {
    row: SiteRow,
    site: Site,
    catalog: Catalog,
    settings: SiteSettings,
    /// The global `pagination_per_page` setting, the fallback when the site does
    /// not set one (`PaginatorFactory.php:20`).
    global_per_page: usize,
    favicon_url: Option<String>,
    /// The site's stored `resource_page_blocks` theme setting, `None` when the
    /// theme's default applies.
    resource_page_blocks: Option<ResourcePageBlocks>,
    /// The theme's design tokens, read from the same stored map and validated.
    theme_settings: ThemeSettings,
}

impl SiteData {
    /// Resolve a site slug to everything a render reads, `None` for an unknown
    /// slug.
    async fn load(state: &AppState, slug: &Slug) -> Result<Option<Self>, SiteError> {
        let Some(row) = with_db!(&state.pool, |db| SiteRepo::new(db)
            .get_by_slug(slug.as_str())
            .await)?
        else {
            return Ok(None);
        };
        let site = load::site_from_row(&row)?;
        let catalog = load_catalog(&state.pool).await?;
        let settings = settings::load(&state.pool, row.id).await?;
        let global_per_page = global_per_page(&state.pool).await?;
        let favicon_url = favicon_url(&state.pool, &state.files, settings.favicon()).await?;
        // `Manager::getResourcePageBlocks`: the site administrator's stored
        // `resource_page_blocks` (the theme's own settings row) wins over the
        // theme's default. The same row carries the design tokens.
        let stored_theme = theme_settings_value(&state.pool, row.id, &row.theme).await?;
        let resource_page_blocks = stored_theme
            .as_ref()
            .filter(|value| value.get("resource_page_blocks").is_some())
            .map(ResourcePageBlocks::parse);
        let theme_settings =
            ThemeSettings::from_value(stored_theme.as_ref().unwrap_or(&Value::Null));
        Ok(Some(Self {
            row,
            site,
            catalog,
            settings,
            global_per_page,
            favicon_url,
            resource_page_blocks,
            theme_settings,
        }))
    }
}

/// The shared values one site render reads.
struct Common<'a> {
    site: &'a Site,
    pages: Vec<PageRef>,
    nav: Vec<NavItem>,
    settings: &'a SiteSettings,
    /// The site's stored `resource_page_blocks` theme setting, `None` when the
    /// theme's default applies.
    resource_page_blocks: Option<ResourcePageBlocks>,
    /// The global `pagination_per_page` setting, the fallback when the site does
    /// not set one (`PaginatorFactory.php:20`).
    global_per_page: usize,
    /// The theme `site.theme` names, or the bundled default when it names no
    /// bundled theme.
    theme: &'static dyn Theme,
    /// The theme's design tokens, validated.
    theme_settings: ThemeSettings,
    favicon_url: Option<String>,
    viewer: Option<Viewer>,
    query: Vec<(String, String)>,
    loader: &'a Loader<'a>,
    current_path: String,
    /// Whether this render is an admin preview, which hides the admin bar.
    preview: bool,
}

impl<'a> Common<'a> {
    /// The values a render reads beside the page itself: the visible pages, the
    /// resolved navigation, the theme, the query and the caller's identity.
    async fn load(
        state: &'a AppState,
        uri: &Uri,
        data: &'a SiteData,
        loader: &'a Loader<'a>,
        actor: &Actor,
        theme_settings: &ThemeSettings,
    ) -> Result<Self, SiteError> {
        let pages = loader.page_refs().await?;
        // A corrupt navigation blob renders an empty menu, not a 500;
        // `parse_navigation` is the storage boundary.
        let nav = nav::resolve(
            &nav::parse_navigation(&data.row.navigation).unwrap_or_default(),
            &data.site.slug,
            &pages,
            data.settings.locale().translator().messages(),
        );
        let theme = theme_for(&data.site.theme).unwrap_or(&DefaultTheme);
        let viewer = viewer(&state.pool, actor, data.row.id).await?;
        Ok(Self {
            site: &data.site,
            pages,
            nav,
            settings: &data.settings,
            resource_page_blocks: data.resource_page_blocks.clone(),
            global_per_page: data.global_per_page,
            theme,
            theme_settings: theme_settings.clone(),
            favicon_url: data.favicon_url.clone(),
            viewer,
            query: parse_query(uri.query().unwrap_or_default()),
            loader,
            current_path: uri.path().to_string(),
            preview: false,
        })
    }

    /// The moment this request asks the player to open at, from `?t=`. A value
    /// that is not a finite, non-negative number is no start, so a broken link
    /// plays the file from the beginning rather than failing.
    fn media_start(&self) -> Option<MediaStart> {
        self.query
            .iter()
            .find(|(name, _)| name == "t")
            .and_then(|(_, value)| MediaStart::parse_seconds(value))
    }

    /// A media as its own page shows it. The request's query may cue the
    /// player; the loader holds the start inside the file's probed duration.
    /// `?notes_after=` pages the notes and `?note=` opens the page that holds a
    /// cited note.
    async fn media_resource_view(&self, row: &MediaRow) -> Result<ResourceView, SiteError> {
        let mut view = self
            .loader
            .media_resource_view(row, self.media_start())
            .await?;
        let after = query_i64(&self.query, "notes_after");
        let cited = query_i64(&self.query, "note");
        if let Some(notes) = self.loader.media_notes(row, after, cited).await? {
            self.loader.annotate_first(&mut view, &notes);
        }
        Ok(view)
    }

    /// The page size in force: the site setting, then the global setting, then
    /// 25 (`SitePaginatorDelegatorFactory.php:14`, `PaginatorFactory.php:20`).
    fn per_page(&self) -> usize {
        self.settings
            .pagination_per_page()
            .and_then(|value| usize::try_from(value).ok())
            .filter(|value| *value > 0)
            .unwrap_or(self.global_per_page)
    }

    fn context<'b>(
        &'b self,
        page: Option<&'b SitePage>,
        resources: &'b dyn ResourceProvider,
    ) -> RenderContext<'b> {
        let mut ctx = RenderContext::new(self.site, resources);
        ctx.page = page;
        ctx.settings = self.settings.clone();
        ctx.theme_settings = self.theme_settings.clone();
        ctx.nav = self.nav.clone();
        ctx.pages = self.pages.clone();
        ctx.resource_page_blocks = self.resource_page_blocks.clone();
        ctx.query = self.query.clone();
        ctx.current_path = self.current_path.clone();
        ctx.favicon_url = self.favicon_url.clone();
        ctx.viewer = self.viewer.clone();
        if self.preview {
            ctx.settings.show_user_bar = Some(-1);
        }
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
    Ok(Outcome::Html(render_index(common.theme, &ctx)))
}

/// `Site\PageController::showAction`: the page of this site with this slug. A
/// page the caller cannot see is a 404 (`SitePageVisibilityFilter`), not a 403.
async fn show_page(common: &Common<'_>, page_slug: &str) -> Result<Outcome, SiteError> {
    let Some(row) = common.loader.page_row(page_slug).await? else {
        return Err(SiteError::NotFound);
    };
    if !common.loader.access().sees_site_page(row.is_public) {
        return Err(SiteError::NotFound);
    }
    let page = common.loader.page_from_row(&row).await?;
    // The snapshot runs before the context exists, so no `RenderContext` lives
    // across an `await`; the page then renders against the snapshot, so a
    // block's synchronous search finds its answer.
    let loaded = provider::load_blocks(
        common.loader,
        &page,
        common.site.id,
        common.settings,
        &common.query,
        &common.theme_settings,
    )
    .await?;
    let mut ctx = common.context(Some(&page), &loaded);
    ctx.page_created = Some(Timestamp::from_naive(row.created));
    ctx.page_modified = row.modified.map(Timestamp::from_naive);
    Ok(Outcome::Html(render_page(common.theme, &page, &mut ctx)))
}

/// The values a preview render reads: the unsaved page and its attachments, the
/// page's timestamps, and the body's theme tokens when it carries any.
pub(crate) struct PreviewPage<'a> {
    /// The page the body describes.
    pub(crate) page: &'a SitePage,
    /// Each block's resolved attachments, keyed by render block id.
    pub(crate) attachments: &'a [(BlockId, Vec<ResolvedAttachment>)],
    /// The edited row's `created`, `None` for a new page.
    pub(crate) created: Option<Timestamp>,
    /// The edited row's `modified`, `None` when unknown.
    pub(crate) modified: Option<Timestamp>,
    /// `o:theme_settings` from the body, which overrides the stored tokens for
    /// this render only.
    pub(crate) theme_settings: Option<&'a ThemeSettings>,
}

/// Render a page assembled from a preview body.
///
/// The frame is `show_page`'s: the same site, theme, settings, navigation and
/// caller visibility, and the same block rendering. The site is the one the
/// request path names, and the page and its attachments arrive already built, so
/// nothing is read for the page itself.
pub(crate) async fn render_preview(
    state: &AppState,
    uri: &Uri,
    actor: &Actor,
    preview: &PreviewPage<'_>,
) -> Result<Outcome, SiteError> {
    let route = parse_public_path(uri.path()).map_err(|_| SiteError::NotFound)?;
    let slug = route_slug(&route);
    let Some(data) = SiteData::load(state, &slug).await? else {
        return Err(SiteError::NotFound);
    };
    let site_permission = site_permission(&state.pool, data.row.id, actor).await?;
    let access = Access {
        actor,
        site_is_public: data.row.is_public,
        site_owner: data.row.owner_id,
        site_permission,
    };
    let loader = Loader::new(
        &state.pool,
        &state.base,
        &state.files,
        &data.catalog,
        &data.site,
        &access,
        &data.settings,
        state.extents.as_ref(),
    );
    // The body's tokens win over the stored ones for this render only; nothing
    // is written.
    let theme_settings = preview.theme_settings.unwrap_or(&data.theme_settings);
    // A draft not yet titled renders under the site locale's word for a
    // resource without a title, the one upstream shows for an untitled item.
    let titled;
    let page = if preview.page.title.is_empty() {
        titled = SitePage {
            title: data
                .settings
                .locale()
                .translator()
                .messages()
                .untitled
                .to_string(),
            ..preview.page.clone()
        };
        &titled
    } else {
        preview.page
    };
    let common = Common::load(state, uri, &data, &loader, actor, theme_settings).await?;
    let loaded = provider::load_preview_blocks(
        common.loader,
        page,
        common.site.id,
        common.settings,
        // The saved page is fetched with no query, so the preview resolves the
        // same block searches without the credentials this request carries.
        &[],
        preview.attachments,
        theme_settings,
    )
    .await?;
    // The editor maps each block wrapper back to its draft block through these
    // markers. This is the only render that turns them on.
    #[allow(clippy::disallowed_methods)]
    let mut ctx = common.context(Some(page), &loaded).with_preview_markers();
    // The document is the one the saved page will serve, so the path is the
    // page's own and the request's query is dropped: a preview request carries
    // credentials there, not a page query, and a block that builds a link from
    // it must not copy them.
    ctx.current_path = page_url(&common.site.slug, &page.slug);
    ctx.query = Vec::new();
    ctx.page_created = preview.created;
    ctx.page_modified = preview.modified;
    // The editor shows the page as a visitor will see it. The caller is always
    // a signed-in editor, so `show_user_bar` would otherwise put the admin bar
    // in every preview.
    ctx.settings.show_user_bar = Some(-1);
    Ok(Outcome::Html(render_page(common.theme, page, &mut ctx)))
}

/// Render one resource page under a draft theme map: [`show`]'s page, with the
/// body's tokens and, when the map carries one, its `resource_page_blocks`
/// order. The site frame and the caller's visibility are the public page's;
/// nothing is written.
pub(crate) async fn render_resource_preview(
    state: &AppState,
    uri: &Uri,
    actor: &Actor,
    kind: ResourceKind,
    id: u64,
    theme: Option<&Value>,
) -> Result<Outcome, SiteError> {
    let route = parse_public_path(uri.path()).map_err(|_| SiteError::NotFound)?;
    let slug = route_slug(&route);
    let Some(mut data) = SiteData::load(state, &slug).await? else {
        return Err(SiteError::NotFound);
    };
    if let Some(theme) = theme {
        data.theme_settings = ThemeSettings::from_value(theme);
        if theme.get("resource_page_blocks").is_some() {
            data.resource_page_blocks = Some(ResourcePageBlocks::parse(theme));
        }
    }
    let site_permission = site_permission(&state.pool, data.row.id, actor).await?;
    let access = Access {
        actor,
        site_is_public: data.row.is_public,
        site_owner: data.row.owner_id,
        site_permission,
    };
    let loader = Loader::new(
        &state.pool,
        &state.base,
        &state.files,
        &data.catalog,
        &data.site,
        &access,
        &data.settings,
        state.extents.as_ref(),
    );
    let mut common = Common::load(state, uri, &data, &loader, actor, &data.theme_settings).await?;
    // The request's query carries the caller's credentials, not a page query,
    // and the frame shows the page as a visitor sees it, without the admin bar.
    common.query = Vec::new();
    common.preview = true;
    show_kind(&common, kind, id).await
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
    let per_page = query_usize(&common.query, "per_page")
        .unwrap_or(common.per_page())
        .max(1);
    let page = query_usize(&common.query, "page").unwrap_or(1).max(1);
    let browse = common
        .loader
        .browse(kind, per_page, page, &common.query, scope)
        .await?;
    let ctx = common.context(None, &NoResources);
    Ok(Outcome::Html(
        common.theme.render(&View::Browse(&browse), &ctx),
    ))
}

/// `Site\ItemController::showAction`, `Site\ItemSetController` and
/// `Site\MediaController::showAction`: read the resource by id. A resource the
/// caller cannot see is a 404 (`Db\Filter\ResourceVisibilityFilter`), not a 403.
async fn show(common: &Common<'_>, controller: &str, id: u64) -> Result<Outcome, SiteError> {
    let Some(kind) = kind_for_controller(controller) else {
        return Err(SiteError::NotFound);
    };
    show_kind(common, kind, id).await
}

async fn show_kind(common: &Common<'_>, kind: ResourceKind, id: u64) -> Result<Outcome, SiteError> {
    let (resource, noindex) = match kind {
        ResourceKind::Item => {
            let row = with_db!(common.loader.pool(), |dbp| ItemRepo::new(dbp)
                .get(db::ItemId::from(id))
                .await)?
            .ok_or(SiteError::NotFound)?;
            require_visible(common, &row.resource)?;
            let in_site = common.loader.item_in_site(row.id()).await?;
            (common.loader.item_resource_view(&row).await?, !in_site)
        }
        ResourceKind::ItemSet => {
            let row = with_db!(common.loader.pool(), |dbp| ItemSetRepo::new(dbp)
                .get(db::ItemSetId::from(id))
                .await)?
            .ok_or(SiteError::NotFound)?;
            require_visible(common, &row.resource)?;
            let in_site = common.loader.item_set_in_site(row.id()).await?;
            (common.loader.item_set_resource_view(&row).await?, !in_site)
        }
        ResourceKind::Media => {
            let row = with_db!(common.loader.pool(), |dbp| MediaRepo::new(dbp)
                .get(db::MediaId::from(id))
                .await)?
            .ok_or(SiteError::NotFound)?;
            require_visible(common, &row.resource)?;
            // `noindexMedia` reads the media's item (`Module.php:813-817`).
            let in_site = common.loader.item_in_site(row.item_id).await?;
            (common.media_resource_view(&row).await?, !in_site)
        }
    };
    let mut ctx = common.context(None, &NoResources);
    ctx.noindex = noindex;
    let view = match kind {
        ResourceKind::Item => View::Item(&resource),
        ResourceKind::ItemSet => View::ItemSet(&resource),
        ResourceKind::Media => View::Media(&resource),
    };
    Ok(Outcome::Html(common.theme.render(&view, &ctx)))
}

fn require_visible(common: &Common<'_>, row: &ResourceRow) -> Result<(), SiteError> {
    if common
        .loader
        .access()
        .sees_resource(row.is_public, row.owner_id)
    {
        Ok(())
    } else {
        Err(SiteError::NotFound)
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

/// A non-negative integer member of the query, `None` when it is absent or not
/// one. An id is a row id, so a negative or broken value names nothing.
fn query_i64(query: &[(String, String)], key: &str) -> Option<i64> {
    query
        .iter()
        .find(|(name, _)| name == key)
        .and_then(|(_, value)| value.parse::<i64>().ok())
        .filter(|value| *value >= 0)
}

/// `Catalog::load` reports a database failure as `ApiError`; the site layer's
/// internal variant carries a `DbError`, so the message is wrapped here.
async fn load_catalog(pool: &DbPool) -> Result<Catalog, SiteError> {
    Catalog::load(pool).await.map_err(|error| {
        SiteError::Internal(DbError::Column {
            column: "catalog",
            reason: error.to_string(),
        })
    })
}

/// The site's stored theme settings map, from its `theme_settings_<theme>` row.
/// `None` when no row stores one, so the theme's defaults apply.
async fn theme_settings_value(
    pool: &DbPool,
    site: db::SiteId,
    theme: &str,
) -> Result<Option<serde_json::Value>, SiteError> {
    omeka_api::theme_settings::load(pool, site, theme)
        .await
        .map_err(|error| {
            SiteError::Internal(DbError::Column {
                column: "theme_settings",
                reason: error.to_string(),
            })
        })
}

/// The signed-in caller with the roles the user bar's edit links read: the
/// global role and the caller's own `site_permission` row on this site.
async fn viewer(
    pool: &DbPool,
    actor: &Actor,
    site: db::SiteId,
) -> Result<Option<Viewer>, SiteError> {
    let Some(user) = actor.user() else {
        return Ok(None);
    };
    let Some(row) = with_db!(pool, |db| UserRepo::new(db).get(user).await)? else {
        return Ok(None);
    };
    let mut roles = RoleSet {
        user: Some(UserId::new(user.0)),
        global: Role::from_wire_name(&row.role),
        ..RoleSet::default()
    };
    let permission = with_db!(pool, |db| SiteRepo::new(db).list_permissions(site).await)?
        .into_iter()
        .find(|permission| permission.user_id == user)
        .and_then(|permission| SitePermissionRole::from_wire_name(&permission.role));
    if let Some(role) = permission {
        roles = roles.with_site(SiteId::new(site.0), role);
    }
    Ok(Some(Viewer {
        name: row.name,
        roles,
    }))
}

/// Whether the actor holds a permission on the site
/// (`HasSitePermissionAssertion('viewer')` accepts every site role).
async fn site_permission(
    pool: &DbPool,
    site: db::SiteId,
    actor: &Actor,
) -> Result<bool, SiteError> {
    let Some(user) = actor.user() else {
        return Ok(false);
    };
    Ok(with_db!(pool, |db| SiteRepo::new(db)
        .has_permission(site, user)
        .await)?)
}

/// The global `pagination_per_page` setting, the fallback when a site sets no
/// page size (`PaginatorFactory.php:20`), or 25 when even that is unset.
async fn global_per_page(pool: &DbPool) -> Result<usize, SiteError> {
    let value = with_db!(pool, |db| SettingRepo::new(db)
        .get("pagination_per_page")
        .await)?;
    Ok(value
        .and_then(|value| match value {
            serde_json::Value::Number(number) => number.as_u64(),
            serde_json::Value::String(text) => text.trim().parse().ok(),
            _ => None,
        })
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value > 0)
        .unwrap_or(25))
}

/// The `favicon` setting's asset id resolved to its file URL
/// (`Module.php:195-201`), `None` when the setting is unset or names no asset.
async fn favicon_url(
    pool: &DbPool,
    files: &omeka_media::store::FileUrls,
    value: Option<&str>,
) -> Result<Option<String>, SiteError> {
    let Some(id) = value.and_then(|raw| raw.trim().parse::<u64>().ok()) else {
        return Ok(None);
    };
    let Some(row) = with_db!(pool, |dbp| AssetRepo::new(dbp)
        .get(db::AssetId::from(id))
        .await)?
    else {
        return Ok(None);
    };
    Ok(Some(thumbnail_for_asset(files, &row).url))
}
