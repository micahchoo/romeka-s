//! `POST` and `PUT` for `sites`.
//!
//! `SiteAdapter::hydrate` sets the owner, theme, title, summary, slug, the
//! navigation and item pool, the visibility and `assign_new_items`, the site
//! permissions, the item sets and the thumbnail. On create it also grants the
//! caller a site-admin permission and builds a welcome page with three example
//! blocks (`SiteAdapter::getDefaultNavigation`, `getFirstPageContent`,
//! `getSecondPageContent`). `validateEntity` requires a title, a well-formed
//! unique slug and a theme, and rejects a navigation tree, item pool or
//! homepage it cannot store (`SiteAdapter::validateNavigation`).
//!
//! `o:theme_settings` and `o:site_settings` are port additions. Upstream
//! stores the theme's own settings in `site_setting` under `theme_settings_
//! <theme>` and each site setting as its own row, and edits both through a
//! server-rendered form, so no API member carries them. See
//! [`crate::theme_settings`] and [`crate::site_settings`].

use omeka_db::ids::{ItemSetId, PageId, SiteId, UserId};
use omeka_db::repos::{ItemSetRepo, SiteRepo, UserRepo};
use omeka_db::rows::{NewSite, SiteRow};
use serde_json::{Value, json};

use crate::errors::{ApiError, ValidationErrors};
use crate::messages;
use crate::read::{Api, ApiResponse, db_error, not_found, parse_id};
use crate::write::access;
use crate::write::body::{self, Payload, ref_row};
use crate::write::resource::not_found as ref_not_found;
use crate::write::site_defaults;

const SITE_CLASS: &str = "Omeka\\Entity\\Site";
const PAGE_CLASS: &str = "Omeka\\Entity\\SitePage";
const USER_CLASS: &str = "Omeka\\Entity\\User";

const CONTROL: &[&str] = &[
    "o:theme",
    "o:title",
    "o:summary",
    "o:slug",
    "o:navigation",
    "o:homepage",
    "o:item_pool",
    "o:is_public",
    "o:assign_new_items",
    "o:page",
    "o:site_permission",
    "o:site_item_set",
    "o:theme_settings",
    "o:site_settings",
    "o:owner",
    "o:thumbnail",
];

pub async fn create(api: &Api<'_>, payload: &Payload) -> Result<ApiResponse, ApiError> {
    api.authorize_site_create()?;
    // A site body carries no property terms; a term key is an unknown member.
    crate::write::values::collect(api, payload, CONTROL).await?;
    let title = payload
        .str_member("o:title")
        .unwrap_or("")
        .trim()
        .to_string();
    let theme = payload
        .str_member("o:theme")
        .unwrap_or("")
        .trim()
        .to_string();
    let mut slug = payload
        .str_member("o:slug")
        .unwrap_or("")
        .trim()
        .to_string();
    if slug.is_empty() && !title.is_empty() {
        slug = slugify(&title);
    }
    let summary = trimmed(payload.str_member("o:summary"));
    let navigation = payload
        .get("o:navigation")
        .map(crate::write::body::php_json)
        .unwrap_or_else(site_defaults::default_navigation);
    let item_pool = payload
        .get("o:item_pool")
        .map(crate::write::body::php_json)
        .unwrap_or_else(|| json!([]));
    let thumbnail_id = crate::write::resource::asset(api, payload).await?;
    let homepage_id = homepage(api, payload).await?;
    validate(
        api,
        None,
        &title,
        &theme,
        &slug,
        &navigation,
        &item_pool,
        homepage_id,
        payload.get("o:theme_settings"),
    )
    .await?;
    let owner = owner(api, payload).await?;
    // Resolve every reference the body names before the first insert, so a
    // failure leaves nothing (`SiteAdapter::hydrate` runs before the flush).
    let permissions = permissions(api, payload).await?;
    let item_sets = site_item_sets(api, payload, false).await?;
    named_pages(api, payload).await?;
    let row = SiteRepo::new(api.pool())
        .create(&NewSite {
            owner_id: owner,
            slug,
            theme,
            title,
            summary,
            navigation,
            item_pool,
            thumbnail_id,
            homepage_id,
            is_public: payload.bool_member("o:is_public", true),
            assign_new_items: payload.bool_member("o:assign_new_items", false),
            ..NewSite::default()
        })
        .await
        .map_err(db_error)?;
    if let Some(user) = api.actor().user() {
        SiteRepo::new(api.pool())
            .set_permission(row.id, user, "admin")
            .await
            .map_err(db_error)?;
    }
    // Upstream hydrates the membership members on create as well as update
    // (`SiteAdapter::hydrate`): a create body that names permissions replaces
    // the caller's grant.
    apply_permissions(api, row.id, permissions.as_deref()).await?;
    apply_item_sets(api, row.id, item_sets.as_deref()).await?;
    if payload.has("o:theme_settings") {
        hydrate_theme_settings(api, row.id, &row.theme, payload).await?;
    } else {
        site_defaults::theme_settings(api, row.id, &row.theme).await?;
    }
    hydrate_site_settings(api, row.id, payload).await?;
    site_defaults::welcome_page(api, row.id).await?;
    api.read_member("sites", &row.id.0.to_string()).await
}

pub async fn update(
    api: &Api<'_>,
    raw_id: &str,
    payload: &Payload,
    partial: bool,
) -> Result<ApiResponse, ApiError> {
    crate::write::values::collect(api, payload, CONTROL).await?;
    let id = parse_id(SITE_CLASS, raw_id)?;
    let repo = SiteRepo::new(api.pool());
    let Some(mut row) = repo.get(SiteId(id)).await.map_err(db_error)? else {
        return Err(not_found(SITE_CLASS, raw_id));
    };
    api.authorize_site_update(row.id, row.owner_id).await?;
    // Resolve every reference the body names before the first write, so a
    // failure leaves the stored row unchanged (`SiteAdapter::hydrate` runs
    // before the flush).
    let permissions = permissions(api, payload).await?;
    let item_sets = site_item_sets(api, payload, partial).await?;
    let pages = if !partial || payload.has("o:page") {
        Some(named_pages(api, payload).await?)
    } else {
        None
    };
    // A full `PUT` resets every member `shouldHydrate` covers; a `PATCH` keeps
    // the members it omits.
    if !partial || payload.has("o:title") {
        row.title = trimmed(payload.str_member("o:title")).unwrap_or_default();
    }
    if !partial || payload.has("o:theme") {
        row.theme = payload
            .str_member("o:theme")
            .unwrap_or("")
            .trim()
            .to_string();
    }
    if !partial || payload.has("o:summary") {
        row.summary = trimmed(payload.str_member("o:summary"));
    }
    if !partial || payload.has("o:slug") {
        row.slug = payload
            .str_member("o:slug")
            .unwrap_or("")
            .trim()
            .to_string();
    }
    if !partial || payload.has("o:is_public") {
        row.is_public = payload.bool_member("o:is_public", true);
    }
    if !partial || payload.has("o:assign_new_items") {
        row.assign_new_items = payload.bool_member("o:assign_new_items", false);
    }
    if !partial || payload.has("o:navigation") {
        row.navigation = payload
            .get("o:navigation")
            .map(crate::write::body::php_json)
            .unwrap_or_else(|| json!([]));
    }
    if !partial || payload.has("o:item_pool") {
        row.item_pool = payload
            .get("o:item_pool")
            .map(crate::write::body::php_json)
            .unwrap_or_else(|| json!([]));
    }
    if !partial || payload.has("o:thumbnail") {
        row.thumbnail_id = crate::write::resource::asset(api, payload).await?;
    }
    if !partial || payload.has("o:homepage") {
        row.homepage_id = homepage(api, payload).await?;
    }
    validate(
        api,
        Some(&row),
        &row.title,
        &row.theme,
        &row.slug,
        &row.navigation,
        &row.item_pool,
        row.homepage_id,
        payload.get("o:theme_settings"),
    )
    .await?;
    repo.update(&row).await.map_err(db_error)?;
    apply_permissions(api, row.id, permissions.as_deref()).await?;
    apply_item_sets(api, row.id, item_sets.as_deref()).await?;
    if let Some(pages) = pages {
        retain_pages(api, row.id, &pages).await?;
    }
    hydrate_theme_settings(api, row.id, &row.theme, payload).await?;
    hydrate_site_settings(api, row.id, payload).await?;
    api.read_member("sites", raw_id).await
}

/// Delete a site with its pages, blocks and attachments.
///
/// `Site::$pages`, `$sitePermissions` and `$siteItemSets` cascade remove
/// (`Entity/Site.php:88,99,109`), and each removed page takes its blocks. The
/// entity privilege is `OwnsEntityAssertion` for every non-admin role
/// (`AclFactory::addRulesForSites`), so only the owner or an admin may remove a
/// site.
pub async fn delete(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(SITE_CLASS, raw_id)?;
    let repo = SiteRepo::new(api.pool());
    let Some(row) = repo.get(SiteId(id)).await.map_err(db_error)? else {
        return Err(not_found(SITE_CLASS, raw_id));
    };
    access::owned_entity(api.actor(), SITE_CLASS, row.owner_id)?;
    // The document carries the permission-filtered page list; read the
    // permission once here so `site_document` does not query it again.
    let permission = match api.actor().user() {
        Some(user) => SiteRepo::new(api.pool())
            .has_permission(row.id, user)
            .await
            .map_err(db_error)?,
        None => false,
    };
    let answer = crate::read::site_document(api, &row, permission).await?;
    repo.delete(row.id).await.map_err(db_error)?;
    Ok(answer)
}

/// The site owner: the body's `o:owner`, or the caller on create.
///
/// `AbstractEntityAdapter::hydrateOwner` with a null stored owner: a numeric id
/// that is not zero names a user and must exist, anything else on a create
/// leaves the caller as the owner.
async fn owner(api: &Api<'_>, payload: &Payload) -> Result<Option<UserId>, ApiError> {
    let Some(raw) = payload.numeric_ref("o:owner") else {
        return Ok(api.actor().user());
    };
    let integer = crate::write::body::cast_int(raw).unwrap_or(0);
    if integer == 0 {
        return Ok(api.actor().user());
    }
    if integer < 0 {
        return Err(ref_not_found(USER_CLASS, &Value::from(integer)));
    }
    let id = UserId(integer as u64);
    if omeka_db::repos::UserRepo::new(api.pool())
        .get(id)
        .await
        .map_err(db_error)?
        .is_none()
    {
        return Err(ref_not_found(USER_CLASS, &Value::from(integer)));
    }
    Ok(Some(id))
}

/// `SiteAdapter::hydrate` for `o:site_permission`, the resolve half.
///
/// Upstream acts only when the member is present and is an array, even on a
/// full `PUT` where every other member resets, so `None` means "leave the
/// stored permissions alone" and `Some` carries the entries the body retains.
/// Each entry with a `o:user.o:id` and an `o:role` names that user's role; an
/// unknown user is a 404, as upstream's `findEntity` throws.
async fn permissions(
    api: &Api<'_>,
    payload: &Payload,
) -> Result<Option<Vec<(UserId, String)>>, ApiError> {
    let Some(entries) = payload.get("o:site_permission").and_then(Value::as_array) else {
        return Ok(None);
    };
    let mut wanted = Vec::with_capacity(entries.len());
    for entry in entries {
        let Some(raw_user) = entry.get("o:user").and_then(body::object_id) else {
            continue;
        };
        let Some(role) = entry.get("o:role").and_then(Value::as_str) else {
            continue;
        };
        let Some(user_id) = ref_row(raw_user) else {
            return Err(ref_not_found(USER_CLASS, raw_user));
        };
        let user = UserId(user_id);
        if UserRepo::new(api.pool())
            .get(user)
            .await
            .map_err(db_error)?
            .is_none()
        {
            return Err(ref_not_found(USER_CLASS, raw_user));
        }
        wanted.push((user, role.to_string()));
    }
    Ok(Some(wanted))
}

/// `SiteAdapter::hydrate` for `o:site_permission`, the write half: upsert each
/// retained entry and remove every permission the body does not retain.
async fn apply_permissions(
    api: &Api<'_>,
    site: SiteId,
    wanted: Option<&[(UserId, String)]>,
) -> Result<(), ApiError> {
    let Some(wanted) = wanted else {
        return Ok(());
    };
    let repo = SiteRepo::new(api.pool());
    let mut retained: Vec<UserId> = Vec::with_capacity(wanted.len());
    for (user, role) in wanted {
        repo.set_permission(site, *user, role)
            .await
            .map_err(db_error)?;
        retained.push(*user);
    }
    for permission in repo.list_permissions(site).await.map_err(db_error)? {
        if !retained.contains(&permission.user_id) {
            repo.remove_permission(site, permission.user_id)
                .await
                .map_err(db_error)?;
        }
    }
    Ok(())
}

/// `SiteAdapter::hydrate` for `o:site_item_set`, the resolve half.
///
/// A full `PUT` that omits the member clears every attachment (upstream
/// defaults the value to `[]`); a `PATCH` that omits it leaves them, which is
/// `None` here. An unknown item set is a 404.
async fn site_item_sets(
    api: &Api<'_>,
    payload: &Payload,
    partial: bool,
) -> Result<Option<Vec<ItemSetId>>, ApiError> {
    if partial && !payload.has("o:site_item_set") {
        return Ok(None);
    }
    let empty = Vec::new();
    let entries = payload
        .get("o:site_item_set")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let mut wanted = Vec::with_capacity(entries.len());
    for entry in entries {
        let Some(raw) = entry.get("o:item_set").and_then(body::object_id) else {
            continue;
        };
        let Some(item_set_id) = ref_row(raw) else {
            return Err(ref_not_found("Omeka\\Entity\\ItemSet", raw));
        };
        let item_set = ItemSetId(item_set_id);
        if ItemSetRepo::new(api.pool())
            .get(item_set)
            .await
            .map_err(db_error)?
            .is_none()
        {
            return Err(ref_not_found("Omeka\\Entity\\ItemSet", raw));
        }
        wanted.push(item_set);
    }
    Ok(Some(wanted))
}

/// `SiteAdapter::hydrate` for `o:site_item_set`, the write half: the body's
/// order is the stored position, so the first entry gets position 1, and an
/// attachment the body does not name is removed. `None` leaves them all.
async fn apply_item_sets(
    api: &Api<'_>,
    site: SiteId,
    wanted: Option<&[ItemSetId]>,
) -> Result<(), ApiError> {
    let Some(wanted) = wanted else {
        return Ok(());
    };
    let repo = SiteRepo::new(api.pool());
    for (index, item_set) in wanted.iter().enumerate() {
        repo.add_item_set(site, *item_set, Some(index as i64 + 1))
            .await
            .map_err(db_error)?;
    }
    for attached in repo.list_item_sets(site).await.map_err(db_error)? {
        if !wanted.contains(&attached.item_set_id) {
            repo.remove_item_set(site, attached.item_set_id)
                .await
                .map_err(db_error)?;
        }
    }
    Ok(())
}

/// `SiteAdapter::hydrate` for `o:page`, the resolve half: the pages the body
/// names by `o:id`. An unknown page is the 404 `findEntity` throws.
async fn named_pages(api: &Api<'_>, payload: &Payload) -> Result<Vec<PageId>, ApiError> {
    let mut wanted = Vec::new();
    for entry in payload.entries("o:page") {
        let Some(raw) = entry.id() else {
            continue;
        };
        let Some(id) = ref_row(raw) else {
            return Err(ref_not_found(PAGE_CLASS, raw));
        };
        let page = PageId(id);
        if SiteRepo::new(api.pool())
            .get_page(page)
            .await
            .map_err(db_error)?
            .is_none()
        {
            return Err(ref_not_found(PAGE_CLASS, raw));
        }
        wanted.push(page);
    }
    Ok(wanted)
}

/// `SiteAdapter::hydrate` for `o:page`, the write half: a full `PUT` (or a
/// `PATCH` that carries the member) removes every page the body does not name.
/// Upstream adds nothing here; on create the welcome page is the only one.
async fn retain_pages(api: &Api<'_>, site: SiteId, wanted: &[PageId]) -> Result<(), ApiError> {
    let repo = SiteRepo::new(api.pool());
    for page in repo.list_pages(site).await.map_err(db_error)? {
        if !wanted.contains(&page.id) {
            repo.delete_page(page.id).await.map_err(db_error)?;
        }
    }
    Ok(())
}

/// The theme settings member, a port addition.
///
/// Upstream edits the map through a server-rendered form and stores it in
/// `site_setting`. The site write accepts the member when present: an object
/// writes it, `null` removes the row, an absent member leaves the stored one
/// alone, so a full `PUT` from the site form does not clear settings it never
/// shows.
async fn hydrate_theme_settings(
    api: &Api<'_>,
    site: SiteId,
    theme: &str,
    payload: &Payload,
) -> Result<(), ApiError> {
    let Some(value) = payload.get("o:theme_settings") else {
        return Ok(());
    };
    let value = (!value.is_null()).then_some(value);
    crate::theme_settings::store(api.pool(), site, theme, value).await
}

/// `SiteAdapter::hydrate` for `o:homepage`: the named page, checked to exist.
///
/// A member without a numeric `o:id` clears the homepage. An id upstream's
/// `site_pages` adapter cannot find is the 404 its `findEntity` throws.
async fn homepage(api: &Api<'_>, payload: &Payload) -> Result<Option<PageId>, ApiError> {
    let Some(raw) = payload.numeric_ref("o:homepage") else {
        return Ok(None);
    };
    let Some(id) = ref_row(raw) else {
        return Err(ref_not_found(PAGE_CLASS, raw));
    };
    let page = PageId(id);
    if SiteRepo::new(api.pool())
        .get_page(page)
        .await
        .map_err(db_error)?
        .is_none()
    {
        return Err(ref_not_found(PAGE_CLASS, raw));
    }
    Ok(Some(page))
}

/// The site settings member, a port addition.
///
/// Upstream saves each field of `SiteSettingsForm` as its own `site_setting`
/// row and edits them through a server-rendered form. The site write accepts
/// `o:site_settings` when present: an object upserts each key (a `null` value
/// removes that key), `null` removes them all, and an absent member leaves the
/// stored rows alone, so a `PATCH` from one admin form does not clear the rest.
async fn hydrate_site_settings(
    api: &Api<'_>,
    site: SiteId,
    payload: &Payload,
) -> Result<(), ApiError> {
    let Some(value) = payload.get("o:site_settings") else {
        return Ok(());
    };
    let value = (!value.is_null()).then_some(value);
    crate::site_settings::store(api.pool(), site, value).await
}

/// `SiteAdapter::validateEntity`.
#[allow(clippy::too_many_arguments)]
async fn validate(
    api: &Api<'_>,
    existing: Option<&SiteRow>,
    title: &str,
    theme: &str,
    slug: &str,
    navigation: &Value,
    item_pool: &Value,
    homepage_id: Option<PageId>,
    theme_settings: Option<&Value>,
) -> Result<(), ApiError> {
    let mut errors = ValidationErrors::new();
    if title.is_empty() {
        errors.insert(
            "o:title".to_string(),
            vec![messages::SITE_TITLE_REQUIRED.to_string()],
        );
    }
    if slug.is_empty() {
        errors.insert("o:slug".to_string(), vec![messages::SLUG_EMPTY.to_string()]);
    } else if !slug_is_valid(slug) {
        errors.insert(
            "o:slug".to_string(),
            vec![messages::SLUG_INVALID.to_string()],
        );
    } else {
        let taken = match existing {
            Some(row) => row.slug != slug,
            None => true,
        };
        if taken
            && SiteRepo::new(api.pool())
                .get_by_slug(slug)
                .await
                .map_err(db_error)?
                .is_some()
        {
            errors.insert("o:slug".to_string(), vec![messages::slug_taken(slug)]);
        }
    }
    if theme.is_empty() {
        errors.insert(
            "o:theme".to_string(),
            vec![messages::SITE_THEME_REQUIRED.to_string()],
        );
    }
    crate::write::navigation::validate(navigation, &mut errors);
    // PHP's `is_array` is true for a JSON object too: a saved search is a keyed array.
    if !(item_pool.is_array() || item_pool.is_object()) {
        errors.insert(
            "o:item_pool".to_string(),
            vec![messages::SITE_ITEM_POOL_REQUIRED.to_string()],
        );
    }
    if let Some(page) = homepage_id {
        let owner = SiteRepo::new(api.pool())
            .get_page(page)
            .await
            .map_err(db_error)?
            .map(|row| row.site_id);
        if owner != existing.map(|row| row.id) {
            errors.insert(
                "o:homepage".to_string(),
                vec![messages::HOMEPAGE_NOT_IN_SITE.to_string()],
            );
        }
    }
    if let Some(messages) = theme_settings.and_then(crate::theme_settings::custom_css_errors) {
        errors.insert("o:theme_settings".to_string(), messages);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::Validation(errors))
    }
}

fn trimmed(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

/// `SiteSlugTrait::slugify`.
fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut pending = false;
    for ch in input.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            if pending {
                out.push('-');
            }
            pending = false;
            out.push(lower);
        } else {
            pending = true;
        }
    }
    out
}

/// `preg_match('/[^a-zA-Z0-9_-]/u', $slug)`.
fn slug_is_valid(slug: &str) -> bool {
    slug.bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
