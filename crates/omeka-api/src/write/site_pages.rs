//! `POST` and `PUT` for `site_pages`.
//!
//! `SitePageAdapter::hydrate` sets the site (on create), the title, slug,
//! visibility, layout and layout data, then replaces the page's blocks through
//! `hydrateBlocks`. A new page with no blocks gets one `pageTitle` block. Each
//! block carries a layout and its data; `JSON_FORCE_OBJECT` semantics mean an
//! empty data object is stored as an empty array, which
//! [`crate::write::body::php_json`] reproduces.

use omeka_db::ids::{PageId, ResourceId, SiteId};
use omeka_db::repos::SiteRepo;
use omeka_db::rows::{NewSiteBlock, NewSitePage, SitePageRow};
use serde_json::Value;

use crate::errors::{ApiError, ValidationErrors};
use crate::read::{Api, ApiResponse, db_error, not_found, parse_id};
use crate::write::access;
use crate::write::body::{Payload, php_json};
use crate::write::fulltext;
use crate::write::resource::not_found as ref_not_found;

const PAGE_CLASS: &str = "Omeka\\Entity\\SitePage";

const CONTROL: &[&str] = &[
    "o:title",
    "o:slug",
    "o:is_public",
    "o:layout",
    "o:layout_data",
    "o:block",
    "o:site",
];

/// One parsed block: layout, data, optional layout data.
struct Block {
    layout: String,
    data: Value,
    layout_data: Option<Value>,
}

pub async fn create(api: &Api<'_>, payload: &Payload) -> Result<ApiResponse, ApiError> {
    crate::write::values::collect(api, payload, CONTROL)?;
    let Some(raw_site) = payload.ref_id("o:site") else {
        return Err(validation("o:site", "A page must belong to a site."));
    };
    let Some(site) = SiteRepo::new(api.pool())
        .get(SiteId(raw_site))
        .await
        .map_err(db_error)?
    else {
        return Err(ref_not_found("Omeka\\Entity\\Site", raw_site));
    };
    api.authorize_site_update(site.owner_id)?;
    let mut blocks = parse_blocks(payload)?;
    if blocks.is_empty() {
        blocks.push(Block {
            layout: "pageTitle".to_string(),
            data: Value::Array(Vec::new()),
            layout_data: None,
        });
    }
    let title = payload
        .str_member("o:title")
        .unwrap_or("")
        .trim()
        .to_string();
    let slug = slug_or_auto(payload, &title);
    validate(api, None, site.id, &title, &slug).await?;
    let repo = SiteRepo::new(api.pool());
    let page = repo
        .add_page(&NewSitePage {
            site_id: site.id,
            slug,
            title,
            is_public: payload.bool_member("o:is_public", true),
            layout: payload.str_member("o:layout").map(str::to_string),
            layout_data: payload.get("o:layout_data").map(php_json),
            ..NewSitePage::default()
        })
        .await
        .map_err(db_error)?;
    store_blocks(api, page.id, &blocks).await?;
    fulltext::reindex(api, "site_pages", ResourceId(page.id.0)).await?;
    api.read_member("site_pages", &page.id.0.to_string()).await
}

pub async fn update(
    api: &Api<'_>,
    raw_id: &str,
    payload: &Payload,
    partial: bool,
) -> Result<ApiResponse, ApiError> {
    crate::write::values::collect(api, payload, CONTROL)?;
    let id = parse_id(PAGE_CLASS, raw_id)?;
    let repo = SiteRepo::new(api.pool());
    let Some(mut page) = repo.get_page(PageId(id)).await.map_err(db_error)? else {
        return Err(not_found(PAGE_CLASS, raw_id));
    };
    let site = repo
        .get(page.site_id)
        .await
        .map_err(db_error)?
        .and_then(|row| row.owner_id);
    api.authorize_site_update(site)?;

    if !partial || payload.has("o:title") {
        page.title = payload
            .str_member("o:title")
            .unwrap_or("")
            .trim()
            .to_string();
    }
    if !partial || payload.has("o:slug") {
        page.slug = slug_or_auto(payload, &page.title);
    }
    if !partial || payload.has("o:is_public") {
        page.is_public = payload.bool_member("o:is_public", true);
    }
    if !partial || payload.has("o:layout") {
        page.layout = payload.str_member("o:layout").map(str::to_string);
    }
    if !partial || payload.has("o:layout_data") {
        page.layout_data = payload.get("o:layout_data").map(php_json);
    }
    let append = partial;
    let blocks = if !partial || payload.has("o:block") {
        parse_blocks(payload)?
    } else {
        Vec::new()
    };
    validate(api, Some(&page), page.site_id, &page.title, &page.slug).await?;
    repo.update_page(&page).await.map_err(db_error)?;
    if !append || !blocks.is_empty() {
        if !append {
            repo_clear_blocks(api, page.id).await?;
        }
        store_blocks(api, page.id, &blocks).await?;
    }
    fulltext::reindex(api, "site_pages", ResourceId(page.id.0)).await?;
    api.read_member("site_pages", raw_id).await
}

/// Delete a page and its blocks.
///
/// `SitePage::$blocks` cascades remove (`Entity/SitePage.php:74`), and each
/// block's attachments go with it. The entity privilege is the site-editor
/// assertion (`AclFactory::addRulesForSites`): the site's owner, or a user the
/// site grants editor. The port's actor carries no site role, so ownership is
/// the whole test, the same approximation `update` makes.
pub async fn delete(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(PAGE_CLASS, raw_id)?;
    let repo = SiteRepo::new(api.pool());
    let Some(page) = repo.get_page(PageId(id)).await.map_err(db_error)? else {
        return Err(not_found(PAGE_CLASS, raw_id));
    };
    let owner = repo
        .get(page.site_id)
        .await
        .map_err(db_error)?
        .and_then(|row| row.owner_id);
    access::owned_entity(api.actor(), PAGE_CLASS, owner)?;
    let answer = crate::read::site_page_document(api, &page).await?;
    repo.delete_page(page.id).await.map_err(db_error)?;
    fulltext::remove(api, "site_pages", ResourceId(page.id.0)).await?;
    Ok(answer)
}

/// `SitePageAdapter::hydrateBlocks`: every block needs a layout, and an
/// `o:data` scalar that does not decode to an array is an error.
fn parse_blocks(payload: &Payload) -> Result<Vec<Block>, ApiError> {
    let Some(array) = payload.get("o:block").and_then(|value| value.as_array()) else {
        return Ok(Vec::new());
    };
    let mut blocks = Vec::with_capacity(array.len());
    for item in array {
        let Some(object) = item.as_object() else {
            continue;
        };
        let layout = object
            .get("o:layout")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty());
        let Some(layout) = layout else {
            return Err(validation("o:block", "All blocks must have a layout."));
        };
        let data = match object.get("o:data") {
            None => Value::Array(Vec::new()),
            Some(Value::String(raw)) => match serde_json::from_str::<Value>(raw) {
                Ok(Value::Object(map)) => php_json(&Value::Object(map)),
                Ok(Value::Array(items)) => Value::Array(items),
                _ => {
                    return Err(validation(
                        "o:block",
                        "Block data must not be a scalar value.",
                    ));
                }
            },
            Some(value) => php_json(value),
        };
        blocks.push(Block {
            layout: layout.to_string(),
            data,
            layout_data: object.get("o:layout_data").map(php_json),
        });
    }
    Ok(blocks)
}

async fn store_blocks(api: &Api<'_>, page: PageId, blocks: &[Block]) -> Result<(), ApiError> {
    let repo = SiteRepo::new(api.pool());
    for (index, block) in blocks.iter().enumerate() {
        repo.add_block(&NewSiteBlock {
            page_id: page,
            layout: block.layout.clone(),
            data: block.data.clone(),
            layout_data: block.layout_data.clone(),
            position: index as i64 + 1,
        })
        .await
        .map_err(db_error)?;
    }
    Ok(())
}

async fn repo_clear_blocks(api: &Api<'_>, page: PageId) -> Result<(), ApiError> {
    let repo = SiteRepo::new(api.pool());
    for block in repo.list_blocks(page).await.map_err(db_error)? {
        repo.delete_block(block.id).await.map_err(db_error)?;
    }
    Ok(())
}

/// `SitePageAdapter::validateEntity`.
async fn validate(
    api: &Api<'_>,
    existing: Option<&SitePageRow>,
    site: SiteId,
    title: &str,
    slug: &str,
) -> Result<(), ApiError> {
    let mut errors = ValidationErrors::new();
    if title.is_empty() {
        errors.insert(
            "o:title".to_string(),
            vec!["A page must have a title.".to_string()],
        );
    }
    if slug.is_empty() {
        errors.insert(
            "o:slug".to_string(),
            vec!["The slug cannot be empty.".to_string()],
        );
    } else if !slug
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        errors.insert(
            "o:slug".to_string(),
            vec!["A slug can only contain letters, numbers, underscores, and hyphens.".to_string()],
        );
    } else {
        let taken = SiteRepo::new(api.pool())
            .list_pages(site)
            .await
            .map_err(db_error)?
            .into_iter()
            .any(|page| page.slug == slug && existing.map(|row| row.id) != Some(page.id));
        if taken {
            errors.insert(
                "o:slug".to_string(),
                vec![format!("The slug \"{slug}\" is already taken.")],
            );
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::Validation(errors))
    }
}

fn slug_or_auto(payload: &Payload, title: &str) -> String {
    let slug = payload.str_member("o:slug").unwrap_or("").trim();
    if !slug.is_empty() {
        return slug.to_string();
    }
    slugify(title)
}

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

fn validation(key: &str, message: &str) -> ApiError {
    let mut errors = ValidationErrors::new();
    errors.insert(key.to_string(), vec![message.to_string()]);
    ApiError::Validation(errors)
}
