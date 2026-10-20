//! `POST` and `PUT` for `site_pages`.
//!
//! `SitePageAdapter::hydrate` sets the site (on create), the title, slug,
//! visibility, layout and layout data, then replaces the page's blocks through
//! `hydrateBlocks`. A new page with no blocks gets one `pageTitle` block. Each
//! block carries a layout and its data; `JSON_FORCE_OBJECT` semantics mean an
//! empty data object is stored as an empty array, which
//! [`crate::write::body::php_json`] reproduces.

use omeka_db::ids::{BlockId, ItemId, MediaId, PageId, ResourceId, SiteId};
use omeka_db::repos::{
    AssetRepo, ItemRepo, MediaRepo, PageVersionRepo, SiteBlockAttachmentRepo, SiteRepo,
};
use omeka_db::rows::{
    AssetRow, BlockChange, NewPageAttachment, NewPageBlock, NewSiteBlock, NewSiteBlockAttachment,
    NewSitePage, PageSave, PageSaveOutcome, SitePageRow,
};
use serde_json::Value;

use omeka_core::site::{RomkaCarrier, Slug};
use omeka_site::blocks::romeka::{self, PlainRefs};
use omeka_site::slug::page_url;

use crate::errors::{ApiError, ValidationErrors};
use crate::messages;
use crate::read::{Api, ApiResponse, db_error, not_found, parse_id};
use crate::routes::Operation;
use crate::write::body::{self, Payload, php_json, ref_row};
use crate::write::fulltext;
use crate::write::resource::not_found as ref_not_found;

pub(crate) const PAGE_CLASS: &str = "Omeka\\Entity\\SitePage";

pub(crate) const CONTROL: &[&str] = &[
    "o:title",
    "o:slug",
    "o:is_public",
    "o:layout",
    "o:layout_data",
    "o:block",
    "o:site",
    // Read only by an update, as the conflict check's claim (`crate::page_history`).
    "o:modified",
];

/// One parsed block: layout, data, optional layout data, and its raw
/// attachments (item and media ids are resolved against the database when the
/// block is stored).
pub(crate) struct Block {
    pub(crate) layout: String,
    pub(crate) data: Value,
    pub(crate) layout_data: Option<Value>,
    pub(crate) attachments: Vec<RawAttachment>,
}

/// One `o:attachment` entry as it arrived: the ids are unresolved, because
/// `SitePageAdapter::hydrateAttachments` looks the item up and checks the media
/// belongs to it.
pub(crate) struct RawAttachment {
    pub(crate) item: Option<u64>,
    pub(crate) media: Option<u64>,
    pub(crate) caption: String,
}

/// One `o:attachment` entry after [`resolve_attachments`], the shape the write
/// stores and the preview hands to the render layer.
#[derive(Debug, Clone)]
pub struct ResolvedAttachment {
    /// The item the attachment names, absent when the id resolves to no row.
    pub item: Option<ItemId>,
    /// The media the attachment names, absent unless the item was found and the
    /// media belongs to it.
    pub media: Option<MediaId>,
    /// The purified caption.
    pub caption: String,
}

pub async fn create(api: &Api<'_>, payload: &Payload) -> Result<ApiResponse, ApiError> {
    crate::write::values::collect(api, payload, CONTROL).await?;
    // `isset($data['o:site']['o:id'])`: a present value is looked up with no
    // `is_numeric` guard, so a non-numeric one is a 404 rather than a missing
    // site.
    let Some(raw_site) = payload.present_ref("o:site") else {
        return Err(validation("o:site", messages::PAGE_SITE_REQUIRED));
    };
    let Some(site_id) = ref_row(raw_site) else {
        return Err(ref_not_found("Omeka\\Entity\\Site", raw_site));
    };
    let Some(site) = SiteRepo::new(api.pool())
        .get(SiteId(site_id))
        .await
        .map_err(db_error)?
    else {
        return Err(ref_not_found("Omeka\\Entity\\Site", raw_site));
    };
    api.authorize_site_add_page(site.id, site.owner_id).await?;
    let mut blocks = parse_blocks(payload)?;
    if blocks.is_empty() {
        blocks.push(Block {
            layout: "pageTitle".to_string(),
            data: Value::Array(Vec::new()),
            layout_data: None,
            attachments: Vec::new(),
        });
    }
    carry_blocks(api, site.id, &mut blocks).await?;
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

/// `PUT` and `PATCH`: authorize, parse and check as upstream does, then save in one transaction
/// that also records the new version and runs the `o:modified` conflict check
/// ([`omeka_db::repos::PageVersionRepo::save`], [`crate::page_history`]).
pub async fn update(
    api: &Api<'_>,
    raw_id: &str,
    payload: &Payload,
    partial: bool,
) -> Result<ApiResponse, ApiError> {
    crate::write::values::collect(api, payload, CONTROL).await?;
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
    api.authorize_site_page(Operation::Update, page.site_id, site)
        .await?;

    let expected_modified = crate::page_history::loaded_modified(payload);
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
    let mut blocks = if !partial || payload.has("o:block") {
        parse_blocks(payload)?
    } else {
        Vec::new()
    };
    carry_blocks(api, page.site_id, &mut blocks).await?;
    validate(api, Some(&page), page.site_id, &page.title, &page.slug).await?;
    let stored = resolve_blocks(api, &blocks).await?;
    let change = match (partial, stored.is_empty()) {
        (false, _) => BlockChange::Replace(&stored),
        (true, true) => BlockChange::Keep,
        (true, false) => BlockChange::Append(&stored),
    };
    let snapshot = crate::read::page_snapshot(api.base());
    let outcome = PageVersionRepo::new(api.pool())
        .save(
            &PageSave {
                page: &page,
                blocks: change,
                user: api.actor().user(),
                expected_modified,
            },
            &snapshot,
        )
        .await
        .map_err(db_error)?;
    match outcome {
        PageSaveOutcome::Saved { .. } => {}
        PageSaveOutcome::Missing => return Err(not_found(PAGE_CLASS, raw_id)),
        PageSaveOutcome::Stale { modified, saved_by } => {
            return Err(crate::page_history::stale(api, modified, saved_by).await);
        }
    }
    fulltext::reindex(api, "site_pages", ResourceId(page.id.0)).await?;
    api.read_member("site_pages", raw_id).await
}

/// The blocks as a save stores them, each attachment resolved the way
/// `SitePageAdapter::hydrateAttachments` resolves it.
async fn resolve_blocks(api: &Api<'_>, blocks: &[Block]) -> Result<Vec<NewPageBlock>, ApiError> {
    let mut stored = Vec::with_capacity(blocks.len());
    for block in blocks {
        stored.push(NewPageBlock {
            layout: block.layout.clone(),
            data: block.data.clone(),
            layout_data: block.layout_data.clone(),
            attachments: resolve_attachments(api, &block.attachments)
                .await?
                .into_iter()
                .map(|attachment| NewPageAttachment {
                    item_id: attachment.item,
                    media_id: attachment.media,
                    caption: attachment.caption,
                })
                .collect(),
        });
    }
    Ok(stored)
}

/// Delete a page and its blocks.
///
/// `SitePage::$blocks` cascades remove (`Entity/SitePage.php:74`), and each
/// block's attachments go with it. The entity privilege is the site-editor
/// assertion (`AclFactory::addRulesForSites`): the site's owner, or a user the
/// site grants editor or admin.
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
    api.authorize_site_page(Operation::Delete, page.site_id, owner)
        .await?;
    let answer = crate::read::site_page_document(api, &page).await?;
    repo.delete_page(page.id).await.map_err(db_error)?;
    fulltext::remove(api, "site_pages", ResourceId(page.id.0)).await?;
    Ok(answer)
}

/// `SitePageAdapter::hydrateBlocks`: every block needs a layout, and an
/// `o:data` scalar that does not decode to an array is an error.
pub(crate) fn parse_blocks(payload: &Payload) -> Result<Vec<Block>, ApiError> {
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
            return Err(validation("o:block", messages::BLOCK_LAYOUT_REQUIRED));
        };
        let data = match object.get("o:data") {
            None => Value::Array(Vec::new()),
            Some(Value::String(raw)) => match serde_json::from_str::<Value>(raw) {
                Ok(Value::Object(map)) => php_json(&Value::Object(map)),
                Ok(Value::Array(items)) => Value::Array(items),
                _ => {
                    return Err(validation("o:block", messages::BLOCK_DATA_SCALAR));
                }
            },
            Some(value) => php_json(value),
        };
        blocks.push(Block {
            layout: layout.to_string(),
            data: purify_html_block(layout, data),
            layout_data: object.get("o:layout_data").map(php_json),
            attachments: parse_attachments(object),
        });
    }
    Ok(blocks)
}

/// `BlockLayout\Html::onHydrate` (`Site/BlockLayout/Html.php:31`): the `html`
/// layout's body passes through `Stdlib\HtmlPurifier` before it is stored, so
/// `block-layout/html.phtml` can echo it raw. No other layout purifies.
fn purify_html_block(layout: &str, data: Value) -> Value {
    if layout != "html" {
        return data;
    }
    let Value::Object(mut map) = data else {
        return data;
    };
    if let Some(Value::String(html)) = map.get("html") {
        let purified = omeka_core::purify::purify(html);
        map.insert("html".to_string(), Value::String(purified));
    }
    Value::Object(map)
}

/// Apply the Romeka carrier rule to every parsed block.
async fn carry_blocks(api: &Api<'_>, site: SiteId, blocks: &mut [Block]) -> Result<(), ApiError> {
    for block in blocks.iter_mut() {
        let (data, layout_data) = carry_block(
            api,
            Some(site),
            &block.layout,
            std::mem::take(&mut block.data),
            block.layout_data.take(),
        )
        .await?;
        block.data = data;
        block.layout_data = layout_data;
    }
    Ok(())
}

/// The Romeka carrier rule for one block on its way to storage.
///
/// An `html` block whose `o:layout_data.romeka` names a registered Romeka block
/// has `o:data.html` regenerated from that block's plain renderer, `plain_hash`
/// set to the hash of what was stored and `v` set to
/// [`romeka::CARRIER_VERSION`], which migrates a version 1 hash on its first
/// save. A stored hash that does not vouch for `o:data.html` is an edit made
/// outside this product: the write keeps that edit and leaves the hash alone.
///
/// The preview route reuses this function, so a preview renders the same plain
/// rendering a save would store. `site` is `None` for the welcome page of a
/// site not yet stored: no block can name a page of that site.
pub async fn carry_block(
    api: &Api<'_>,
    site: Option<SiteId>,
    layout: &str,
    mut data: Value,
    layout_data: Option<Value>,
) -> Result<(Value, Option<Value>), ApiError> {
    if layout != "html" {
        return Ok((data, layout_data));
    }
    let Some(layout_data) = layout_data else {
        return Ok((data, None));
    };
    let Some(carrier) = layout_data.get("romeka").and_then(RomkaCarrier::parse) else {
        return Ok((data, Some(layout_data)));
    };
    let stored = data.get("html").and_then(Value::as_str).unwrap_or("");
    if !romeka::needs_plain(&carrier, stored) {
        return Ok((data, Some(layout_data)));
    }
    let refs = plain_refs(api, site, &carrier).await?;
    let Some(plain) = romeka::plain_for(&carrier, &refs) else {
        return Ok((data, Some(layout_data)));
    };
    let hash = romeka::plain_hash(&plain);
    let mut data_object = match data {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    data_object.insert("html".to_string(), Value::String(plain));
    data = Value::Object(data_object);
    let mut layout_object = match layout_data {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    if let Some(Value::Object(romeka_object)) = layout_object.get_mut("romeka") {
        romeka_object.insert("plain_hash".to_string(), Value::String(hash));
        romeka_object.insert("v".to_string(), Value::from(romeka::CARRIER_VERSION));
    }
    Ok((data, Some(Value::Object(layout_object))))
}

/// The references a carrier's plain rendering reads: its background asset and
/// its call-to-action page.
///
/// The asset URL is relative to the origin (`/files/...`) so the stored plain
/// html keeps working when the page is copied to an upstream installation at
/// another origin.
async fn plain_refs(
    api: &Api<'_>,
    site: Option<SiteId>,
    carrier: &RomkaCarrier,
) -> Result<PlainRefs, ApiError> {
    let mut refs = PlainRefs::new();
    let assets = AssetRepo::new(api.pool());
    for id in romeka::asset_refs(carrier) {
        if let Some(row) = assets
            .get(omeka_db::ids::AssetId::from(id.get()))
            .await
            .map_err(db_error)?
        {
            refs = refs.asset(
                id,
                format!("/files/{}", asset_file_path(&row)),
                row.alt_text.clone().unwrap_or_default(),
            );
        }
    }
    let Some(site) = site else {
        return Ok(refs);
    };
    let Some(site_row) = SiteRepo::new(api.pool())
        .get(site)
        .await
        .map_err(db_error)?
    else {
        return Ok(refs);
    };
    let Ok(site_slug) = Slug::parse(&site_row.slug) else {
        return Ok(refs);
    };
    let pages = SiteRepo::new(api.pool());
    for id in romeka::page_refs(carrier) {
        let Some(row) = pages.get_page(PageId(id.get())).await.map_err(db_error)? else {
            continue;
        };
        if row.site_id != site {
            continue;
        }
        if let Ok(slug) = Slug::parse(&row.slug) {
            refs = refs.page(id, page_url(&site_slug, &slug));
        }
    }
    Ok(refs)
}

/// `File\Store\Local::getUri`'s storage path for an asset: `asset/<name>` with
/// the extension when the row has one.
fn asset_file_path(row: &AssetRow) -> String {
    match row.extension.as_deref() {
        Some(extension) if !extension.is_empty() => {
            format!("asset/{}.{extension}", row.storage_id)
        }
        _ => format!("asset/{}", row.storage_id),
    }
}

/// `SitePageAdapter::hydrateAttachments` input: every object entry of
/// `o:attachment`, in order, with the item and media ids unresolved. A
/// non-object entry is skipped, as `foreach` with `is_array` does.
fn parse_attachments(object: &serde_json::Map<String, Value>) -> Vec<RawAttachment> {
    let values: Vec<&Value> = match object.get("o:attachment") {
        Some(Value::Array(items)) => items.iter().collect(),
        Some(Value::Object(map)) => map.values().collect(),
        _ => Vec::new(),
    };
    values
        .into_iter()
        .filter_map(|value| {
            let entry = value.as_object()?;
            Some(RawAttachment {
                // `findEntity` on the item id, with its `NotFoundException`
                // caught and read as "no item".
                item: entry
                    .get("o:item")
                    .and_then(body::object_id)
                    .and_then(ref_row),
                // `$item->getMedia()->get($id)`, the same numeric reading.
                media: entry
                    .get("o:media")
                    .and_then(body::object_id)
                    .and_then(ref_row),
                // `SitePageAdapter::hydrateAttachments` (`Api/Adapter/SitePageAdapter.php:326`)
                // purifies every caption.
                caption: omeka_core::purify::purify(
                    entry.get("o:caption").and_then(Value::as_str).unwrap_or(""),
                ),
            })
        })
        .collect()
}

async fn store_blocks(api: &Api<'_>, page: PageId, blocks: &[Block]) -> Result<(), ApiError> {
    let repo = SiteRepo::new(api.pool());
    for (index, block) in blocks.iter().enumerate() {
        let row = repo
            .add_block(&NewSiteBlock {
                page_id: page,
                layout: block.layout.clone(),
                data: block.data.clone(),
                layout_data: block.layout_data.clone(),
                position: index as i64 + 1,
            })
            .await
            .map_err(db_error)?;
        store_attachments(api, row.id, &block.attachments).await?;
    }
    Ok(())
}

/// `SitePageAdapter::hydrateAttachments`: resolve each attachment's item and
/// media and write the block's rows in input order.
async fn store_attachments(
    api: &Api<'_>,
    block: BlockId,
    attachments: &[RawAttachment],
) -> Result<(), ApiError> {
    let repo = SiteBlockAttachmentRepo::new(api.pool());
    for (index, attachment) in resolve_attachments(api, attachments)
        .await?
        .into_iter()
        .enumerate()
    {
        repo.create(&NewSiteBlockAttachment {
            block_id: block,
            item_id: attachment.item,
            media_id: attachment.media,
            caption: attachment.caption,
            position: index as i64 + 1,
        })
        .await
        .map_err(db_error)?;
    }
    Ok(())
}

/// Resolve each attachment's item and media the way `store_attachments` stores
/// them.
///
/// A missing item is stored as a null reference (upstream catches
/// `NotFoundException` and leaves `$item` null). A media is stored only when the
/// item was found and the media belongs to it (`$item->getMedia()->get($id)`);
/// otherwise it is null. Neither case raises an error.
pub(crate) async fn resolve_attachments(
    api: &Api<'_>,
    attachments: &[RawAttachment],
) -> Result<Vec<ResolvedAttachment>, ApiError> {
    let items = ItemRepo::new(api.pool());
    let media = MediaRepo::new(api.pool());
    let mut resolved = Vec::with_capacity(attachments.len());
    for attachment in attachments {
        let item_id = match attachment.item {
            Some(raw) => items
                .get(ItemId(raw))
                .await
                .map_err(db_error)?
                .map(|_| ItemId(raw)),
            None => None,
        };
        let media_id = match (item_id, attachment.media) {
            (Some(item), Some(raw)) => media
                .get(MediaId(raw))
                .await
                .map_err(db_error)?
                .filter(|row| row.item_id == item)
                .map(|_| MediaId(raw)),
            _ => None,
        };
        resolved.push(ResolvedAttachment {
            item: item_id,
            media: media_id,
            caption: attachment.caption.clone(),
        });
    }
    Ok(resolved)
}

/// `SitePageAdapter::validateEntity`.
pub(crate) async fn validate(
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
            vec![messages::PAGE_TITLE_REQUIRED.to_string()],
        );
    }
    if slug.is_empty() {
        errors.insert("o:slug".to_string(), vec![messages::SLUG_EMPTY.to_string()]);
    } else if !slug
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        errors.insert(
            "o:slug".to_string(),
            vec![messages::SLUG_INVALID.to_string()],
        );
    } else {
        let taken = SiteRepo::new(api.pool())
            .list_pages(site)
            .await
            .map_err(db_error)?
            .into_iter()
            .any(|page| page.slug == slug && existing.map(|row| row.id) != Some(page.id));
        if taken {
            errors.insert("o:slug".to_string(), vec![messages::slug_taken(slug)]);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::Validation(errors))
    }
}

pub(crate) fn slug_or_auto(payload: &Payload, title: &str) -> String {
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
