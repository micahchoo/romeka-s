//! The Romeka carrier on a block's way to storage: the one-way rule at write.
//!
//! A block's Romeka settings live in `o:layout_data.romeka` (`docs/compat.md`,
//! "Site pages: the one-way rule"). On a write, what upstream reads is derived
//! from them here:
//!
//! - a Romeka block stored as `html` gets its plain rendering regenerated into
//!   `o:data.html`, with `plain_hash` recording it;
//! - an IIIF image whose source is a site media gets that media's Image API
//!   address as `o:data.image_url`, so upstream opens the same image.
//!
//! Any visitor reads the plain rendering and the stored address, so both
//! leave out a media that is private, or whose item is private. The write
//! does not refuse it: the reference stays in the settings, and the rich
//! render shows it to a visitor who may see it. Refusing would block every
//! later save of a page whose media became private after it was chosen.

use omeka_core::ids::MediaId;
use omeka_core::site::{RomkaCarrier, Slug};
use omeka_db::ids::{AssetId, ItemId, PageId, SiteId};
use omeka_db::repos::{
    AssetRepo, ItemRepo, MediaFilter, MediaRepo, Paging, SiteRepo, SiteSettingRepo, ValueRepo,
};
use omeka_db::rows::{AssetRow, ItemRow, MediaRow};
use omeka_db::with_db;
use omeka_site::blocks::BlockLayout;
use omeka_site::blocks::iiif_image::{IiifImage, media_info_path};
use omeka_site::blocks::romeka::{
    self, ItemSelection, PlainImage, PlainItem, PlainRefs, PlainValue,
};
use omeka_site::i18n::Locale;
use omeka_site::slug::{page_url, resource_show_url};
use serde_json::Value;

use crate::errors::ApiError;
use crate::read::{Api, db_error};

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
    data: Value,
    layout_data: Option<Value>,
) -> Result<(Value, Option<Value>), ApiError> {
    let Some(layout_data) = layout_data else {
        return Ok((data, None));
    };
    let Some(carrier) = layout_data.get("romeka").and_then(RomkaCarrier::parse) else {
        return Ok((data, Some(layout_data)));
    };
    let media = public_media(api, layout, &carrier).await?;
    if layout == IiifImage.name() {
        let names_media = !romeka::media_refs(layout, &carrier).is_empty();
        return Ok((
            with_image_url(api, data, names_media, &media),
            Some(layout_data),
        ));
    }
    if layout != "html" {
        return Ok((data, Some(layout_data)));
    }
    let stored = data.get("html").and_then(Value::as_str).unwrap_or("");
    if !romeka::needs_plain(&carrier, stored) {
        return Ok((data, Some(layout_data)));
    }
    let refs = plain_refs(api, site, &carrier, &media).await?;
    let Some(plain) = romeka::plain_for(&carrier, &refs) else {
        return Ok((data, Some(layout_data)));
    };
    let hash = romeka::plain_hash(&plain);
    let mut data_object = match data {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    data_object.insert("html".to_string(), Value::String(plain));
    let mut layout_object = match layout_data {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    if let Some(Value::Object(romeka_object)) = layout_object.get_mut("romeka") {
        romeka_object.insert("plain_hash".to_string(), Value::String(hash));
        romeka_object.insert("v".to_string(), Value::from(romeka::CARRIER_VERSION));
    }
    Ok((
        Value::Object(data_object),
        Some(Value::Object(layout_object)),
    ))
}

/// The media the carrier's block renders that any visitor may see, read from
/// the database. A media that no longer exists, or that is private or on a
/// private item, is skipped, so the plain rendering leaves it out.
async fn public_media(
    api: &Api<'_>,
    layout: &str,
    carrier: &RomkaCarrier,
) -> Result<Vec<(MediaId, MediaRow)>, ApiError> {
    let mut found = Vec::new();
    for id in romeka::media_refs(layout, carrier) {
        let row = with_db!(api.pool(), |db| MediaRepo::new(db)
            .get(omeka_db::ids::MediaId(id.get()))
            .await)
        .map_err(db_error)?;
        let Some(row) = row else {
            continue;
        };
        let item_public = with_db!(api.pool(), |db| ItemRepo::new(db).get(row.item_id).await)
            .map_err(db_error)?
            .is_some_and(|item| item.resource.is_public);
        if row.resource.is_public && item_public {
            found.push((id, row));
        }
    }
    Ok(found)
}

/// The IIIF image's `o:data` with `image_url` set to the chosen site media's
/// Image API address on this server, unchanged for an image URL source.
///
/// A chosen media no visitor may see stores the empty address, on which
/// upstream renders nothing.
fn with_image_url(
    api: &Api<'_>,
    data: Value,
    names_media: bool,
    media: &[(MediaId, MediaRow)],
) -> Value {
    if !names_media {
        return data;
    }
    let address = media.first().map_or_else(String::new, |(id, _)| {
        format!("{}{}", api.base().as_str(), media_info_path(*id))
    });
    let mut object = match data {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    object.insert("image_url".to_string(), Value::String(address));
    Value::Object(object)
}

/// The file path under `/files/` of the image a Romeka setting shows for a
/// media: its large rendition, or its original when it is an image with no
/// renditions. `None` for a media with no image to show.
#[must_use]
pub fn media_image_path(row: &MediaRow) -> Option<String> {
    let storage = row.storage_id.as_deref().filter(|id| !id.is_empty())?;
    if row.has_thumbnails {
        return Some(format!("large/{storage}.jpg"));
    }
    let is_image = row
        .media_type
        .as_deref()
        .is_some_and(|kind| kind.starts_with("image/"));
    if !row.has_original || !is_image {
        return None;
    }
    Some(match row.extension.as_deref() {
        Some(extension) if !extension.is_empty() => format!("original/{storage}.{extension}"),
        _ => format!("original/{storage}"),
    })
}

/// The references a carrier's plain rendering reads: its background asset or
/// media, its call-to-action pages and the items it selects.
///
/// The file URLs are relative to the origin (`/files/...`) so the stored plain
/// html keeps working when the page is copied to an upstream installation at
/// another origin.
async fn plain_refs(
    api: &Api<'_>,
    site: Option<SiteId>,
    carrier: &RomkaCarrier,
    media: &[(MediaId, MediaRow)],
) -> Result<PlainRefs, ApiError> {
    let mut refs = PlainRefs::new();
    for id in romeka::asset_refs(carrier) {
        if let Some(row) = with_db!(api.pool(), |db| AssetRepo::new(db)
            .get(AssetId::from(id.get()))
            .await)
        .map_err(db_error)?
        {
            refs = refs.asset(
                id,
                format!("/files/{}", asset_file_path(&row)),
                row.alt_text.clone().unwrap_or_default(),
            );
        }
    }
    for (id, row) in media {
        if let Some(path) = media_image_path(row) {
            refs = refs.media(
                *id,
                format!("/files/{path}"),
                row.alt_text.clone().unwrap_or_default(),
            );
        }
    }
    let Some(site) = site else {
        return Ok(refs);
    };
    let Some(site_row) =
        with_db!(api.pool(), |db| SiteRepo::new(db).get(site).await).map_err(db_error)?
    else {
        return Ok(refs);
    };
    let Ok(site_slug) = Slug::parse(&site_row.slug) else {
        return Ok(refs);
    };
    if let Some(Value::String(tag)) = with_db!(api.pool(), |db| SiteSettingRepo::new(db)
        .get(site, "locale")
        .await)
    .map_err(db_error)?
    {
        refs = refs.locale(Locale::parse(&tag));
    }
    for id in romeka::page_refs(carrier) {
        let Some(row) = with_db!(api.pool(), |db| SiteRepo::new(db)
            .get_page(PageId(id.get()))
            .await)
        .map_err(db_error)?
        else {
            continue;
        };
        if row.site_id != site {
            continue;
        }
        if let Ok(slug) = Slug::parse(&row.slug) {
            refs = refs.page(id, page_url(&site_slug, &slug));
        }
    }
    if let Some(selection) = romeka::items(carrier) {
        for item in plain_items(api, site, &site_slug, selection).await? {
            refs = refs.item(item);
        }
    }
    Ok(refs)
}

/// The selected items a plain rendering lists, each with its public values,
/// the image of its main media and its values in the site's location field.
///
/// The plain html is public, so only public items, public values and public
/// media are listed, whoever saves the page.
async fn plain_items(
    api: &Api<'_>,
    site: SiteId,
    site_slug: &Slug,
    selection: ItemSelection,
) -> Result<Vec<PlainItem>, ApiError> {
    let term = with_db!(api.pool(), |db| SiteSettingRepo::new(db)
        .get(site, "romeka_location_property")
        .await)
    .map_err(db_error)?;
    api.fill_catalog().await?;
    let location = term
        .as_ref()
        .and_then(Value::as_str)
        .and_then(|term| api.catalog().property_id(term));
    let rows: Vec<ItemRow> = match selection {
        ItemSelection::Picked(ids) => {
            let mut rows = Vec::with_capacity(ids.len());
            for id in ids {
                if let Some(row) = with_db!(api.pool(), |db| ItemRepo::new(db)
                    .get(ItemId(id.get()))
                    .await)
                .map_err(db_error)?
                {
                    rows.push(row);
                }
            }
            rows
        }
        ItemSelection::Query(query) => {
            let search = ItemSelection::browse_query(
                &query,
                omeka_core::ids::SiteId::new(site.0),
                ItemSelection::LIMIT,
            );
            let params = crate::read::params::Params::from_decoded(search.query);
            crate::read::query_rows(api, &params).await?
        }
    };
    let mut items = Vec::new();
    for row in rows
        .iter()
        .filter(|row| row.resource.is_public)
        .take(ItemSelection::LIMIT)
    {
        items.push(plain_item(api, site_slug, row, location).await?);
    }
    Ok(items)
}

/// One public item as a plain rendering reads it.
async fn plain_item(
    api: &Api<'_>,
    site_slug: &Slug,
    row: &ItemRow,
    location: Option<omeka_db::ids::PropertyId>,
) -> Result<PlainItem, ApiError> {
    let id = omeka_core::ids::ItemId::new(row.id().0);
    let mut item = PlainItem::new(
        id,
        row.resource
            .title
            .clone()
            .unwrap_or_else(|| omeka_site::i18n::EN.untitled.to_string()),
        resource_show_url(site_slug, "item", row.id().0),
    );
    for value in with_db!(api.pool(), |db| ValueRepo::new(db)
        .list_for_resource(row.resource.id)
        .await)
    .map_err(db_error)?
    .into_iter()
    .filter(|value| value.is_public)
    {
        let Some(text) = value.value.clone().or_else(|| value.uri.clone()) else {
            continue;
        };
        if Some(value.property_id) == location {
            item.places.push(text.clone());
        }
        if let Some(property) = api.catalog().property(value.property_id) {
            item.values.push(PlainValue {
                term: property.term.clone(),
                label: property.label.clone(),
                text,
            });
        }
    }
    item.image = main_image(api, row).await?;
    Ok(item)
}

/// The image of an item's main media, its primary media or else its first
/// public one, `None` when that media has no image to show.
async fn main_image(api: &Api<'_>, row: &ItemRow) -> Result<Option<PlainImage>, ApiError> {
    let primary = match row.primary_media_id {
        Some(id) => {
            with_db!(api.pool(), |db| MediaRepo::new(db).get(id).await).map_err(db_error)?
        }
        None => None,
    };
    let media = match primary.filter(|media| media.resource.is_public) {
        Some(media) => Some(media),
        None => with_db!(api.pool(), |db| MediaRepo::new(db)
            .list(
                &MediaFilter {
                    item_id: Some(row.id()),
                    ..MediaFilter::default()
                },
                &Paging::default()
            )
            .await)
        .map_err(db_error)?
        .into_iter()
        .find(|media| media.resource.is_public),
    };
    Ok(media.and_then(|media| {
        media_image_path(&media).map(|path| PlainImage {
            url: format!("/files/{path}"),
            alt: media.alt_text.clone().unwrap_or_default(),
        })
    }))
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
