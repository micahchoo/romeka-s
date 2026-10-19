//! `users`, `sites`, the `/api` resource listing, and `/api-context`.
//!
//! `UserRepresentation::getJsonLd` and `SiteRepresentation::getJsonLd` shape
//! the documents; `ApiController::getList` with no resource builds the root and
//! `ApiController::contextAction` builds the context document.

use std::collections::HashMap;

use omeka_db::ids::{PageId, SiteId, UserId};
use omeka_db::repos::{
    Paging, SiteBlockAttachmentRepo, SiteFilter, SiteRepo, UserFilter, UserRepo, compare_text_ci,
};
use omeka_db::rows::{SiteBlockRow, SitePageRow, SiteRow, UserRow};

use super::paging::{Limit, Window, ordered_page, sorted_page};
use super::params::{Sort, SortField};
use super::query::{fulltext_matches, id_filter, id_matches, relevance_page};
use super::response::link_header;
use super::{Api, db_error, iso8601, not_found, parse_id, resource_ref, resource_ref_opt, system};
use crate::errors::ApiError;
use crate::jsonld::{BaseUrl, ContextDocument};
use crate::read::params::Params;
use crate::read::response::{ApiResponse, serialize};
use crate::wire::api_resource::ApiResource;
use crate::wire::site::{SiteItemSet, SitePermission};
use crate::wire::{JsonLdType, NavLink};

// ---------------------------------------------------------------- users

pub async fn search_users(api: &Api<'_>, params: &Params) -> Result<ApiResponse, ApiError> {
    let window = Window::from_params(params);
    let limit = Limit::from_params(params);
    let sort = Sort::from_params(params);
    let filter = UserFilter {
        email: params.get("email").map(str::to_string),
        name: params.get("name").map(str::to_string),
        role: params.get("role").map(str::to_string),
        is_active: params.bool("is_active"),
        site_id: params.id("site_id"),
    };
    let ids = id_filter(params);
    let rows = UserRepo::new(api.pool())
        .list(&filter, &Paging::default())
        .await
        .map_err(db_error)?;
    let needle = search_needle(params);
    let rows: Vec<UserRow> = rows
        .into_iter()
        .filter(|row| id_matches(&ids, row.id.0))
        .filter(|row| {
            matches(&row.name, needle.as_deref()) || matches(&row.email, needle.as_deref())
        })
        .collect();
    let (total, page) = sorted_page(
        rows,
        &sort,
        &limit,
        cmp_user(params.get("sort_by").unwrap_or("id")),
    );
    let documents: Vec<_> = page.iter().map(|row| user_doc(api.base(), row)).collect();
    let body = serialize(&documents)?;
    Ok(ApiResponse::listing(
        body,
        total,
        link_header(api.base(), "users", params, &window, total),
    ))
}

pub async fn read_user(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(USER_CLASS, raw_id)?;
    let Some(row) = UserRepo::new(api.pool())
        .get(UserId(id))
        .await
        .map_err(db_error)?
    else {
        return Err(not_found(USER_CLASS, raw_id));
    };
    let mut document = user_doc(api.base(), &row);
    document.user_settings = crate::settings::load_user(api.pool(), row.id).await?;
    Ok(ApiResponse::document(serialize(&document)?))
}

/// The `User` document for a row, as `read_user` answers it. A delete answers
/// with the row it removed (`Api\Manager::finalize` renders the representation).
pub(crate) fn user_document(api: &Api<'_>, row: &UserRow) -> Result<ApiResponse, ApiError> {
    user_response(api.base(), row)
}

/// The `User` document for a row, for a route that has no [`Api`] request: a
/// login answers with the user it signed in, and does not load the catalog.
pub fn user_response(base: &BaseUrl, row: &UserRow) -> Result<ApiResponse, ApiError> {
    Ok(ApiResponse::document(serialize(&user_doc(base, row))?))
}

const USER_CLASS: &str = "Omeka\\Entity\\User";

fn user_doc(base: &BaseUrl, row: &UserRow) -> crate::wire::user::User {
    crate::wire::user::User {
        context: base.api_context(),
        id: base.api_resource("users", row.id.0),
        jsonld_type: JsonLdType::single("o:User"),
        o_id: row.id.0,
        name: row.name.clone(),
        email: row.email.clone(),
        // `UserRepresentation::getDateTime` returns the plain formatted date.
        created: iso8601(row.created),
        role: row.role.clone(),
        is_active: row.is_active,
        user_settings: None,
        admin_locale: None,
    }
}

/// The `User` document a sign-in and `GET /api/me` answer: the user, plus the
/// admin locale they resolve to, so the client renders in it.
///
/// # Errors
/// [`ApiError::Internal`] when a setting cannot be read.
pub async fn session_user_response(
    base: &BaseUrl,
    pool: &sqlx::SqlitePool,
    row: &UserRow,
) -> Result<ApiResponse, ApiError> {
    let mut document = user_doc(base, row);
    document.admin_locale = crate::settings::admin_locale(pool, row.id).await?;
    Ok(ApiResponse::document(serialize(&document)?))
}

// ---------------------------------------------------------------- sites

pub async fn search_sites(api: &Api<'_>, params: &Params) -> Result<ApiResponse, ApiError> {
    let window = Window::from_params(params);
    let limit = Limit::from_params(params);
    let sort = Sort::from_params(params);
    let filter = SiteFilter {
        owner_id: params.id("owner_id"),
        slug: params.get("slug").map(str::to_string),
        exclude_id: params.id("exclude_id"),
        assign_new_items: params.bool("assign_new_items"),
    };
    let ids = id_filter(params);
    let rows = SiteRepo::new(api.pool())
        .list(&filter, &Paging::default())
        .await
        .map_err(db_error)?;
    let needle = search_needle(params);
    let candidates: Vec<SiteRow> = rows
        .into_iter()
        .filter(|row| id_matches(&ids, row.id.0))
        .filter(|row| matches(&row.title, needle.as_deref()))
        .collect();
    let mut permissions: HashMap<u64, bool> = HashMap::new();
    let mut rows: Vec<SiteRow> = Vec::with_capacity(candidates.len());
    for row in candidates {
        let permission = has_site_permission(api, row.id).await?;
        permissions.insert(row.id.0, permission);
        if api
            .actor()
            .sees_site(row.is_public, row.owner_id, permission)
        {
            rows.push(row);
        }
    }
    let (total, page) = sorted_page(rows, &sort, &limit, cmp_site(sort));
    let mut documents = Vec::with_capacity(page.len());
    for row in &page {
        let permission = permissions.get(&row.id.0).copied().unwrap_or(false);
        documents.push(site_doc(api, row, permission).await?);
    }
    let body = serialize(&documents)?;
    Ok(ApiResponse::listing(
        body,
        total,
        link_header(api.base(), "sites", params, &window, total),
    ))
}

pub async fn read_site(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(SITE_CLASS, raw_id)?;
    let Some(row) = SiteRepo::new(api.pool())
        .get(SiteId(id))
        .await
        .map_err(db_error)?
    else {
        return Err(not_found(SITE_CLASS, raw_id));
    };
    let permission = has_site_permission(api, row.id).await?;
    if !api
        .actor()
        .sees_site(row.is_public, row.owner_id, permission)
    {
        return Err(not_found(SITE_CLASS, raw_id));
    }
    site_document(api, &row, permission).await
}

/// The `Site` document for a row, as `read_site` answers it. A delete answers
/// with the row it removed, including its pages and permissions, which
/// `site_doc` reads before the rows are gone. `permission` is whether the actor
/// holds a site permission; the caller reads it once so a listing does not run
/// the permission query twice per row.
pub(crate) async fn site_document(
    api: &Api<'_>,
    row: &SiteRow,
    permission: bool,
) -> Result<ApiResponse, ApiError> {
    Ok(ApiResponse::document(serialize(
        &site_doc(api, row, permission).await?,
    )?))
}

const SITE_CLASS: &str = "Omeka\\Entity\\Site";

/// Whether the actor holds any permission on the site
/// (`HasSitePermissionAssertion('viewer')` accepts every site role).
async fn has_site_permission(api: &Api<'_>, site: SiteId) -> Result<bool, ApiError> {
    let Some(user) = api.actor().user() else {
        return Ok(false);
    };
    SiteRepo::new(api.pool())
        .has_permission(site, user)
        .await
        .map_err(db_error)
}

/// The site facts a page-visibility test reads, loaded once per site so a page
/// listing does not repeat the site read and the permission query for every
/// row.
struct SiteAccess {
    is_public: bool,
    owner_id: Option<UserId>,
    permission: bool,
}

/// The site's visibility facts, `None` when no such site exists.
async fn site_access(api: &Api<'_>, site: SiteId) -> Result<Option<SiteAccess>, ApiError> {
    let Some(row) = SiteRepo::new(api.pool())
        .get(site)
        .await
        .map_err(db_error)?
    else {
        return Ok(None);
    };
    let permission = has_site_permission(api, site).await?;
    Ok(Some(SiteAccess {
        is_public: row.is_public,
        owner_id: row.owner_id,
        permission,
    }))
}

/// Whether the actor may read the page (`SitePageVisibilityFilter`: a public
/// page of a public site, the site's owner, or a site permission).
async fn site_page_visible(api: &Api<'_>, row: &SitePageRow) -> Result<bool, ApiError> {
    let Some(access) = site_access(api, row.site_id).await? else {
        return Ok(false);
    };
    Ok(api.actor().sees_site_page(
        row.is_public,
        access.is_public,
        access.owner_id,
        access.permission,
    ))
}

async fn site_doc(
    api: &Api<'_>,
    row: &SiteRow,
    permission: bool,
) -> Result<crate::wire::site::Site, ApiError> {
    let base = api.base();
    let repo = SiteRepo::new(api.pool());
    let pages: Vec<_> = repo
        .list_pages(row.id)
        .await
        .map_err(db_error)?
        .into_iter()
        .filter(|page| {
            api.actor()
                .sees_site_page(page.is_public, row.is_public, row.owner_id, permission)
        })
        .map(|page| resource_ref(base, "site_pages", page.id.0))
        .collect();
    let permissions: Vec<SitePermission> = repo
        .list_permissions(row.id)
        .await
        .map_err(db_error)?
        .into_iter()
        .map(|permission| SitePermission {
            user: resource_ref(base, "users", permission.user_id.0),
            role: permission.role,
        })
        .collect();
    let item_sets: Vec<SiteItemSet> = repo
        .list_item_sets(row.id)
        .await
        .map_err(db_error)?
        .into_iter()
        .map(|row| SiteItemSet {
            item_set: resource_ref(base, "item_sets", row.item_set_id.0),
        })
        .collect();
    let navigation: Vec<NavLink> =
        serde_json::from_value(row.navigation.clone()).unwrap_or_default();
    Ok(crate::wire::site::Site {
        context: base.api_context(),
        id: base.api_resource("sites", row.id.0),
        jsonld_type: JsonLdType::single("o:Site"),
        o_id: row.id.0,
        slug: row.slug.clone(),
        theme: row.theme.clone(),
        title: row.title.clone(),
        summary: row.summary.clone(),
        thumbnail: resource_ref_opt(base, "assets", row.thumbnail_id.map(|id| id.0)),
        navigation,
        homepage: resource_ref_opt(base, "site_pages", row.homepage_id.map(|id| id.0)),
        item_pool: row.item_pool.clone(),
        owner: resource_ref_opt(base, "users", row.owner_id.map(|id| id.0)),
        created: super::jsonld_date(row.created),
        modified: row.modified.map(super::jsonld_date),
        is_public: row.is_public,
        assign_new_items: row.assign_new_items,
        page: pages,
        site_permission: permissions,
        site_item_set: item_sets,
        theme_settings: crate::theme_settings::load(api.pool(), row.id, &row.theme).await?,
        site_settings: crate::site_settings::load(api.pool(), row.id).await?,
        site_item: base.items_for_site(row.id.0),
        extensions: Default::default(),
    })
}

// ---------------------------------------------------------- site_pages

pub async fn read_site_page(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(PAGE_CLASS, raw_id)?;
    let Some(row) = SiteRepo::new(api.pool())
        .get_page(PageId(id))
        .await
        .map_err(db_error)?
    else {
        return Err(not_found(PAGE_CLASS, raw_id));
    };
    // `AclFactory::addRulesForAllRoles` grants `read` on `Omeka\Entity\SitePage`
    // through `SitePageIsPublicAssertion` (a public page of a public site), and
    // the per-role grants add site ownership and site permissions; the admin
    // roles hold `view-all` and bypass the test.
    if !site_page_visible(api, &row).await? {
        return Err(not_found(PAGE_CLASS, raw_id));
    }
    site_page_document(api, &row).await
}

/// The `SitePage` document for a row, as `read_site_page` answers it.
///
/// A delete answers with the row it removed and does not re-test the page's
/// public flag, because the delete already passed the entity authorization and
/// a non-public page owned by an author is not visible to a read.
pub(crate) async fn site_page_document(
    api: &Api<'_>,
    row: &SitePageRow,
) -> Result<ApiResponse, ApiError> {
    Ok(ApiResponse::document(serialize(
        &site_page_doc(api, row).await?,
    )?))
}

const PAGE_CLASS: &str = "Omeka\\Entity\\SitePage";

async fn site_page_doc(
    api: &Api<'_>,
    row: &SitePageRow,
) -> Result<crate::wire::site_page::SitePage, ApiError> {
    let base = api.base();
    let blocks = SiteRepo::new(api.pool())
        .list_blocks(row.id)
        .await
        .map_err(db_error)?;
    let mut documents = Vec::with_capacity(blocks.len());
    for block in &blocks {
        documents.push(block_doc(api, block).await?);
    }
    Ok(crate::wire::site_page::SitePage {
        context: base.api_context(),
        id: base.api_resource("site_pages", row.id.0),
        jsonld_type: JsonLdType::single("o:SitePage"),
        o_id: row.id.0,
        slug: row.slug.clone(),
        title: row.title.clone(),
        is_public: row.is_public,
        layout: row.layout.clone(),
        // `SitePageRepresentation::getJsonLd` writes `$this->layoutData() ?? []`.
        layout_data: row
            .layout_data
            .clone()
            .unwrap_or_else(|| serde_json::json!([])),
        block: documents,
        site: resource_ref(base, "sites", row.site_id.0),
        created: super::jsonld_date(row.created),
        modified: row.modified.map(super::jsonld_date),
    })
}

/// `SitePageBlockRepresentation::jsonSerialize`.
///
/// `o:attachment` is the block's `site_block_attachment` rows in `position`
/// order, each as `SiteBlockAttachmentRepresentation` writes it: `o:item` and
/// `o:media` are resource references or `null`, `o:caption` is the stored
/// string (never null upstream).
async fn block_doc(
    api: &Api<'_>,
    row: &SiteBlockRow,
) -> Result<crate::wire::site_page::SitePageBlock, ApiError> {
    let rows = SiteBlockAttachmentRepo::new(api.pool())
        .list_for_block(row.id)
        .await
        .map_err(db_error)?;
    let base = api.base();
    let attachment = rows
        .iter()
        .map(|row| crate::wire::site_page::SiteBlockAttachment {
            item: resource_ref_opt(base, "items", row.item_id.map(|id| id.0)),
            media: resource_ref_opt(base, "media", row.media_id.map(|id| id.0)),
            caption: row.caption.clone(),
        })
        .collect();
    Ok(crate::wire::site_page::SitePageBlock {
        layout: row.layout.clone(),
        data: row.data.clone(),
        layout_data: row.layout_data.clone(),
        attachment,
    })
}

// ------------------------------------------------------- root and context
/// `GET /api`: every registered API resource, sorted by name.
pub fn api_root(api: &Api<'_>) -> Result<ApiResponse, ApiError> {
    let documents = api_resource_docs(api);
    Ok(ApiResponse::document(serialize(&documents)?))
}

/// `GET /api/api_resources` (`ApiResourceAdapter::search`).
///
/// Upstream builds the listing in adapter-registration order and ignores the
/// query, and its plain `Response` reports no total, so the answer carries no
/// `Omeka-S-Total-Results` and no page window. The port sorts by name, which is
/// what `GET /api` does for the same listing.
pub fn search_api_resources(api: &Api<'_>, params: &Params) -> ApiResponse {
    let window = Window::from_params(params);
    let rows = api_resource_docs(api);
    let body = serialize(&rows).unwrap_or_else(|_| "[]".to_string());
    ApiResponse::collection(
        body,
        link_header(api.base(), "api_resources", params, &window, 0),
    )
}

fn api_resource_docs(api: &Api<'_>) -> Vec<ApiResource> {
    let base = api.base();
    let mut resources: Vec<ApiResource> = crate::adapters::ADAPTERS
        .iter()
        .map(|spec| ApiResource {
            context: base.api_context(),
            id: base.api_resource("api_resources", spec.name),
            jsonld_type: JsonLdType::single("o:ApiResource"),
            o_id: spec.name.to_string(),
        })
        .collect();
    resources.sort_by(|a, b| a.o_id.cmp(&b.o_id));
    resources
}

/// `GET /api/api_resources/{id}` (`ApiResourceAdapter::read`).
///
/// Upstream returns `new Resource($request->getId())` without looking anything
/// up, so any id answers 200 with that id as `o:id`.
pub fn read_api_resource(api: &Api<'_>, id: &str) -> ApiResponse {
    let document = ApiResource {
        context: api.base().api_context(),
        id: api.base().api_resource("api_resources", id),
        jsonld_type: JsonLdType::single("o:ApiResource"),
        o_id: id.to_string(),
    };
    ApiResponse::document(serialize(&document).unwrap_or_else(|_| "{}".to_string()))
}

// ------------------------------------------------------------ data_types

/// The registered data types, in `application/config/module.config.php:563`
/// order, with the labels their classes return. `omeka-api` does not depend on
/// `omeka-core` (wave-1 brief), so the six entries are mirrored here beside the
/// wire type instead of read from `omeka_core::datatypes`.
const DATA_TYPES: [(&str, &str); 6] = [
    ("literal", "Text"),
    ("uri", "URI"),
    ("resource", "All"),
    ("resource:item", "Item"),
    ("resource:itemset", "Item Set"),
    ("resource:media", "Media"),
];

/// The class `DataTypeAdapter` hands the representation: `new Resource($id)`,
/// named in the port's 404 message.
const DATA_TYPE_CLASS: &str = "Omeka\\Api\\Resource";

/// `GET /api/data_types` (`DataTypeAdapter::search`).
///
/// Upstream returns the registered data types in registration order and ignores
/// the query, and its plain `Response` reports no total, so the answer carries
/// no `Omeka-S-Total-Results` and no page window.
pub fn search_data_types(api: &Api<'_>, params: &Params) -> ApiResponse {
    let window = Window::from_params(params);
    let rows: Vec<crate::wire::data_type::DataType> = DATA_TYPES
        .iter()
        .map(|(name, label)| data_type_doc(api, name, label))
        .collect();
    let body = serialize(&rows).unwrap_or_else(|_| "[]".to_string());
    ApiResponse::collection(
        body,
        link_header(api.base(), "data_types", params, &window, 0),
    )
}

/// `GET /api/data_types/{id}` (`DataTypeAdapter::read`).
///
/// Upstream builds a `Resource` for any id and the representation looks the id
/// up in the data type manager, which throws for an unregistered name and
/// becomes a 500. The port answers 404 instead.
pub fn read_data_type(api: &Api<'_>, id: &str) -> Result<ApiResponse, ApiError> {
    let Some((name, label)) = DATA_TYPES.iter().find(|(name, _)| *name == id) else {
        return Err(not_found(DATA_TYPE_CLASS, id));
    };
    Ok(ApiResponse::document(serialize(&data_type_doc(
        api, name, label,
    ))?))
}

fn data_type_doc(api: &Api<'_>, name: &str, label: &str) -> crate::wire::data_type::DataType {
    crate::wire::data_type::DataType {
        context: api.base().api_context(),
        id: api.base().api_resource("data_types", name),
        jsonld_type: JsonLdType::single("o:DataType"),
        o_id: name.to_string(),
        label: label.to_string(),
    }
}

// ------------------------------------------------------------ site_pages

/// `GET /api/site_pages` (`SitePageAdapter::search`).
///
/// `SitePageAdapter::$sortFields` is `id`, `created`, `modified`, `title`,
/// `slug`; the shared [`SortField`] has no `slug`, so the comparator is local.
/// The `item_id` filter needs `site_block_attachment`, which no repository
/// walks, so it is ignored; reported as a gap.
pub async fn search_site_pages(api: &Api<'_>, params: &Params) -> Result<ApiResponse, ApiError> {
    let window = Window::from_params(params);
    let limit = Limit::from_params(params);
    let rows = system::page_rows(api.pool()).await?;
    let ids = id_filter(params);
    let site_id = match (params.id::<SiteId>("site_id"), params.get("site_slug")) {
        (Some(id), _) => Some(id),
        (None, Some(slug)) => SiteRepo::new(api.pool())
            .get_by_slug(slug)
            .await
            .map_err(db_error)?
            .map(|site| site.id),
        (None, None) => None,
    };
    // `SitePageAdapter::buildQuery` joins the site by slug, so a slug that
    // matches no site selects no rows.
    let unknown_site = params.get("site_slug").is_some() && site_id.is_none();
    // `Module::searchFulltext` joins the index for site pages too
    // (`SitePageAdapter` implements `FulltextSearchableInterface`).
    let fulltext = params
        .get("fulltext_search")
        .map(str::trim)
        .filter(|needle| !needle.is_empty());
    let matches = fulltext_matches(api, "site_pages", fulltext).await?;
    let candidates: Vec<SitePageRow> = rows
        .into_iter()
        .filter(|row| !unknown_site && site_id.is_none_or(|id| row.site_id == id))
        .filter(|row| id_matches(&ids, row.id.0))
        .filter(|row| params.get("slug").is_none_or(|slug| row.slug == slug))
        .filter(|row| {
            params
                .bool("is_public")
                .is_none_or(|want| row.is_public == want)
        })
        .filter(|row| {
            matches
                .as_ref()
                .is_none_or(|hits| hits.contains_key(&row.id.0))
        })
        .collect();
    // `SitePageVisibilityFilter`: a page is visible when it is public on a
    // public site, the caller owns its site, or the caller holds a site
    // permission. The test is async (it reads the site and its permissions), so
    // it runs after the cheap filters.
    let mut rows: Vec<SitePageRow> = Vec::with_capacity(candidates.len());
    let mut distinct: Vec<u64> = candidates.iter().map(|row| row.site_id.0).collect();
    distinct.sort_unstable();
    distinct.dedup();
    let mut cache: HashMap<u64, Option<SiteAccess>> = HashMap::new();
    for site in distinct {
        cache.insert(site, site_access(api, SiteId::from(site)).await?);
    }
    for row in candidates {
        let Some(access) = cache.get(&row.site_id.0).and_then(Option::as_ref) else {
            continue;
        };
        if api.actor().sees_site_page(
            row.is_public,
            access.is_public,
            access.owner_id,
            access.permission,
        ) {
            rows.push(row);
        }
    }
    let sort_by = params.get("sort_by").unwrap_or("id");
    let (total, page) = match (&matches, params.sort_by_is_default()) {
        (Some(hits), true) => relevance_page(
            rows,
            Sort::from_params(params).descending,
            &limit,
            |row| row.id.0,
            |row| hits.get(&row.id.0).copied().unwrap_or(0.0),
        ),
        _ => ordered_page(
            rows,
            Sort::from_params(params).descending,
            &limit,
            cmp_site_page(sort_by),
        ),
    };
    let mut documents = Vec::with_capacity(page.len());
    for row in &page {
        documents.push(site_page_doc(api, row).await?);
    }
    let body = serialize(&documents)?;
    Ok(ApiResponse::listing(
        body,
        total,
        link_header(api.base(), "site_pages", params, &window, total),
    ))
}

/// The `SitePageAdapter::$sortFields` map, with the `id` tiebreak
/// `AbstractEntityAdapter::search` appends.
fn cmp_site_page(field: &str) -> impl Fn(&SitePageRow, &SitePageRow) -> std::cmp::Ordering + '_ {
    move |a, b| match field {
        "title" => compare_text_ci(&a.title, &b.title).then(a.id.cmp(&b.id)),
        "created" => a.created.cmp(&b.created).then(a.id.cmp(&b.id)),
        "modified" => a.modified.cmp(&b.modified).then(a.id.cmp(&b.id)),
        "slug" => compare_text_ci(&a.slug, &b.slug).then(a.id.cmp(&b.id)),
        _ => a.id.cmp(&b.id),
    }
}

/// `GET /api-context`: the term definitions from every vocabulary.
pub async fn context(api: &Api<'_>) -> Result<ApiResponse, ApiError> {
    let rows = omeka_db::repos::VocabularyRepo::new(api.pool())
        .list(&Default::default(), &Paging::default())
        .await
        .map_err(db_error)?;
    let document =
        ContextDocument::new(rows.into_iter().map(|row| (row.prefix, row.namespace_uri)));
    Ok(ApiResponse::document(serialize(&document)?))
}

// --------------------------------------------------------------- helpers

fn search_needle(params: &Params) -> Option<String> {
    params
        .get("search")
        .filter(|needle| !needle.is_empty())
        .map(str::to_lowercase)
}

fn matches(text: &str, needle: Option<&str>) -> bool {
    match needle {
        Some(needle) => text.to_lowercase().contains(needle),
        None => true,
    }
}

/// `UserAdapter::$sortFields`, whose text field is not a shared [`SortField`]
/// name: `name` orders by name. `ordered_page`'s stable sort keeps the `id`
/// tiebreak `AbstractEntityAdapter::search` appends.
fn cmp_user(field: &str) -> impl Fn(&UserRow, &UserRow) -> std::cmp::Ordering + '_ {
    move |a, b| match field {
        "name" => compare_text_ci(&a.name, &b.name),
        _ => a.id.cmp(&b.id),
    }
}

fn cmp_site(sort: Sort) -> impl Fn(&SiteRow, &SiteRow) -> std::cmp::Ordering {
    move |a, b| match sort.field {
        SortField::Title => compare_text_ci(&a.title, &b.title),
        SortField::Created => a.created.cmp(&b.created),
        SortField::Modified => a.modified.cmp(&b.modified),
        SortField::IsPublic => a.is_public.cmp(&b.is_public),
        SortField::Id => a.id.cmp(&b.id),
    }
}
