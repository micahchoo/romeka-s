//! `POST` and `PUT` for `sites`.
//!
//! `SiteAdapter::hydrate` sets the owner, theme, title, summary, slug, the
//! navigation and item pool, the visibility and `assign_new_items`, the site
//! permissions and item sets. On create it also grants the caller a site-admin
//! permission and builds a welcome page with three example blocks
//! (`SiteAdapter::getDefaultNavigation`, `getFirstPageContent`,
//! `getSecondPageContent`). `validateEntity` requires a title, a well-formed
//! unique slug and a theme.
//!
//! The navigation tree is stored as the caller sent it; the port does not yet
//! validate link data against the navigation-link registry.
//!
//! `o:theme_settings` is a port addition. Upstream stores the site's theme
//! settings in `site_setting` and edits them through a server-rendered form, so
//! no API member carries them. See [`crate::theme_settings`].

use omeka_db::ids::{ItemSetId, PageId, SiteId, UserId};
use omeka_db::repos::{ItemSetRepo, SiteRepo, UserRepo};
use omeka_db::rows::{NewSite, NewSiteBlock, NewSitePage, SiteRow};
use serde_json::{Value, json};

use crate::errors::{ApiError, ValidationErrors};
use crate::read::{Api, ApiResponse, db_error, not_found, parse_id};
use crate::write::access;
use crate::write::body::Payload;
use crate::write::resource::not_found as ref_not_found;

const SITE_CLASS: &str = "Omeka\\Entity\\Site";

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
    "o:owner",
    "o:thumbnail",
];

pub async fn create(api: &Api<'_>, payload: &Payload) -> Result<ApiResponse, ApiError> {
    // A site body carries no property terms; a term key is an unknown member.
    crate::write::values::collect(api, payload, CONTROL)?;
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
    validate(api, None, &title, &theme, &slug).await?;
    let navigation = payload
        .get("o:navigation")
        .map(crate::write::body::php_json)
        .unwrap_or_else(default_navigation);
    let owner = owner(api, payload).await?;
    let row = SiteRepo::new(api.pool())
        .create(&NewSite {
            owner_id: owner,
            slug,
            theme,
            title,
            summary,
            navigation,
            item_pool: payload
                .get("o:item_pool")
                .map(crate::write::body::php_json)
                .unwrap_or_else(|| json!([])),
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
    hydrate_permissions(api, row.id, payload).await?;
    hydrate_item_sets(api, row.id, payload, false).await?;
    hydrate_theme_settings(api, row.id, &row.theme, payload).await?;
    welcome_page(api, row.id).await?;
    api.read_member("sites", &row.id.0.to_string()).await
}

pub async fn update(
    api: &Api<'_>,
    raw_id: &str,
    payload: &Payload,
    partial: bool,
) -> Result<ApiResponse, ApiError> {
    crate::write::values::collect(api, payload, CONTROL)?;
    let id = parse_id(SITE_CLASS, raw_id)?;
    let repo = SiteRepo::new(api.pool());
    let Some(mut row) = repo.get(SiteId(id)).await.map_err(db_error)? else {
        return Err(not_found(SITE_CLASS, raw_id));
    };
    api.authorize_site_update(row.owner_id)?;
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
    if !partial || payload.has("o:homepage") {
        row.homepage_id = payload.ref_id("o:homepage").map(PageId);
    }
    validate(api, Some(&row), &row.title, &row.theme, &row.slug).await?;
    repo.update(&row).await.map_err(db_error)?;
    hydrate_permissions(api, row.id, payload).await?;
    hydrate_item_sets(api, row.id, payload, partial).await?;
    hydrate_theme_settings(api, row.id, &row.theme, payload).await?;
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
    let answer = crate::read::site_document(api, &row).await?;
    repo.delete(row.id).await.map_err(db_error)?;
    Ok(answer)
}

/// The site owner: the body's `o:owner`, or the caller on create.
async fn owner(api: &Api<'_>, payload: &Payload) -> Result<Option<UserId>, ApiError> {
    let Some(raw) = payload.ref_id("o:owner") else {
        return Ok(api.actor().user());
    };
    let id = UserId(raw);
    if omeka_db::repos::UserRepo::new(api.pool())
        .get(id)
        .await
        .map_err(db_error)?
        .is_none()
    {
        return Err(ref_not_found("Omeka\\Entity\\User", raw));
    }
    Ok(Some(id))
}

/// `SiteAdapter::hydrate` for `o:site_permission`.
///
/// Upstream acts only when the member is present and is an array, even on a
/// full `PUT` where every other member resets. Each entry with a
/// `o:user.o:id` and an `o:role` upserts that user's role; a permission the
/// body does not retain is removed. An unknown user is a 404, as upstream's
/// `findEntity` throws.
async fn hydrate_permissions(
    api: &Api<'_>,
    site: SiteId,
    payload: &Payload,
) -> Result<(), ApiError> {
    let Some(entries) = payload.get("o:site_permission").and_then(Value::as_array) else {
        return Ok(());
    };
    let repo = SiteRepo::new(api.pool());
    let mut retained: Vec<UserId> = Vec::with_capacity(entries.len());
    for entry in entries {
        let Some(user_id) = entry
            .get("o:user")
            .and_then(|user| user.get("o:id"))
            .and_then(Value::as_u64)
        else {
            continue;
        };
        let Some(role) = entry.get("o:role").and_then(Value::as_str) else {
            continue;
        };
        let user = UserId(user_id);
        if UserRepo::new(api.pool())
            .get(user)
            .await
            .map_err(db_error)?
            .is_none()
        {
            return Err(ref_not_found("Omeka\\Entity\\User", user_id));
        }
        repo.set_permission(site, user, role)
            .await
            .map_err(db_error)?;
        retained.push(user);
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

/// `SiteAdapter::hydrate` for `o:site_item_set`.
///
/// The body's order is the stored position, so the first entry gets position 1.
/// A full `PUT` that omits the member clears every attachment (upstream
/// defaults the value to `[]`); a `PATCH` leaves them. An unknown item set is a
/// 404.
async fn hydrate_item_sets(
    api: &Api<'_>,
    site: SiteId,
    payload: &Payload,
    partial: bool,
) -> Result<(), ApiError> {
    if partial && !payload.has("o:site_item_set") {
        return Ok(());
    }
    let empty = Vec::new();
    let entries = payload
        .get("o:site_item_set")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let repo = SiteRepo::new(api.pool());
    let mut retained: Vec<ItemSetId> = Vec::with_capacity(entries.len());
    let mut position = 1_i64;
    for entry in entries {
        let Some(item_set_id) = entry
            .get("o:item_set")
            .and_then(|item_set| item_set.get("o:id"))
            .and_then(Value::as_u64)
        else {
            continue;
        };
        let item_set = ItemSetId(item_set_id);
        if ItemSetRepo::new(api.pool())
            .get(item_set)
            .await
            .map_err(db_error)?
            .is_none()
        {
            return Err(ref_not_found("Omeka\\Entity\\ItemSet", item_set_id));
        }
        repo.add_item_set(site, item_set, Some(position))
            .await
            .map_err(db_error)?;
        retained.push(item_set);
        position += 1;
    }
    for attached in repo.list_item_sets(site).await.map_err(db_error)? {
        if !retained.contains(&attached.item_set_id) {
            repo.remove_item_set(site, attached.item_set_id)
                .await
                .map_err(db_error)?;
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

/// `SiteAdapter::validateEntity`.
async fn validate(
    api: &Api<'_>,
    existing: Option<&SiteRow>,
    title: &str,
    theme: &str,
    slug: &str,
) -> Result<(), ApiError> {
    let mut errors = ValidationErrors::new();
    if title.is_empty() {
        errors.insert(
            "o:title".to_string(),
            vec!["A site must have a title.".to_string()],
        );
    }
    if slug.is_empty() {
        errors.insert(
            "o:slug".to_string(),
            vec!["The slug cannot be empty.".to_string()],
        );
    } else if !slug_is_valid(slug) {
        errors.insert(
            "o:slug".to_string(),
            vec!["A slug can only contain letters, numbers, underscores, and hyphens.".to_string()],
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
            errors.insert(
                "o:slug".to_string(),
                vec![format!("The slug \"{slug}\" is already taken.")],
            );
        }
    }
    if theme.is_empty() {
        errors.insert(
            "o:theme".to_string(),
            vec!["A site must have a theme.".to_string()],
        );
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::Validation(errors))
    }
}

/// Build the welcome page `SiteAdapter::hydrate` adds on create.
async fn welcome_page(api: &Api<'_>, site: SiteId) -> Result<(), ApiError> {
    let repo = SiteRepo::new(api.pool());
    let page = repo
        .add_page(&NewSitePage {
            site_id: site,
            slug: "welcome".to_string(),
            title: "Welcome".to_string(),
            is_public: true,
            ..NewSitePage::default()
        })
        .await
        .map_err(db_error)?;
    for (position, layout, data) in [
        (1_i64, "html", json!({ "html": FIRST_PAGE_CONTENT })),
        (2, "lineBreak", json!({ "break_type": "opaque" })),
        (3, "html", json!({ "html": SECOND_PAGE_CONTENT })),
    ] {
        repo.add_block(&NewSiteBlock {
            page_id: page.id,
            layout: layout.to_string(),
            data,
            layout_data: None,
            position,
        })
        .await
        .map_err(db_error)?;
    }
    Ok(())
}

fn default_navigation() -> Value {
    json!([{
        "type": "browse",
        "data": { "label": "Browse", "query": "" },
        "links": [],
    }])
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

/// `SiteAdapter::getFirstPageContent`.
const FIRST_PAGE_CONTENT: &str = "<p>Welcome to your new site. This is an example page.</p>\n<ul>\n<li>This is a bullet list.</li>\n<li>Second entry.</li>\n</ul>\n<p>Back to normal again.</p>\n<ol>\n<li>This is an ordered list.</li>\n<li>Second entry.</li>\n</ol>\n<p>Back to normal again.</p>\n<blockquote>This is a blockquote.</blockquote>\n<p>Back to normal again.</p>\n<div><a href=\"#\">This text is a link, which currently points to nothing.</a></div>\n<p><strong>This text is bold, in a &lt;strong&gt; tag.</strong></p>\n<p><em>This text is italicized, in an &lt;em&gt; tag.</em></p>\n<p><u>This text is underlined, in a &lt;u&gt; tag.</u></p>\n<p><s>This text has a strikethrough, in a &lt;s&gt; tag.</s></p>\n<p>This text is <sub>subscript</sub> and <sup>superscript</sup> using &lt;sub&gt; and &lt;sup&gt;, which can be used for adding notes and citations.</p>\n<hr>\n<h1>This is an H1 title. It is bigger than the Page Title, which is rendered in H2.</h1>\n<h2>This is an H2 header, the same size as the Page Title.</h2>\n<h3>This is an H3 subheader.</h3>\n<h4>This is an H4 subheader.</h4>\n<h5>This is an H5 subheader.</h5>\n<h6>This is an H6 subheader.</h6>";

/// `SiteAdapter::getSecondPageContent`.
const SECOND_PAGE_CONTENT: &str = "<h2 style=\"font-style:italic;\">This is the &quot;Italic Title&quot; block style.</h2>\n<div style=\"background:#eeeeee;border:1px solid #cccccc;padding:5px 10px;\">This is the &quot;Special Container&quot; block style.</div>\n<p><span class=\"marker\">This is the &quot;Marker&quot; inline style. </span></p>\n<p><small>This text is inside a &quot;small&quot; inline style.</small> This is normal text.</p>\n<p><code>This is the &quot;Computer Code&quot; inline style.</code></p>";
