//! `items`, `item_sets`, `media` and `value_annotations`.
//!
//! `ItemRepresentation::getResourceJsonLd`, `ItemSetRepresentation`,
//! `MediaRepresentation` and `ValueAnnotationRepresentation` build the
//! per-kind blocks; `AbstractResourceEntityRepresentation::getJsonLd` builds
//! the common block. `assets`, `jobs` and `modules` live in [`super::system`].

use std::cmp::Ordering;

use omeka_db::ids::{AssetId, ClassId, ItemId, ItemSetId, MediaId, ResourceId};
use omeka_db::repos::{
    ItemFilter, ItemRepo, ItemSetFilter, ItemSetRepo, MediaFilter, MediaRepo, Paging, ResourceRepo,
    compare_opt_text_ci,
};
use omeka_db::rows::{AssetRow, ItemRow, ItemSetRow, MediaRow, ValueAnnotationRow, resource_type};
use omeka_media::ThumbnailType;
use omeka_media::thumbnailer::thumbnail_path;

use super::paging::{Limit, Window, ordered_page, sorted_page};
use super::params::{Sort, SortField};
use super::query::{
    SearchQuery, fulltext_matches, id_filter, id_matches, media_visibility, relevance_page,
};
use super::relations::{Page, Relations};
use super::response::link_header;
use super::values::{reverse_map, value_map};
use super::window;
use super::{
    Api, THUMBNAIL_TYPES, db_error, empty_thumbnail_urls, jsonld_date, not_found, parse_id,
    resource_ref, resource_ref_opt, system,
};
use crate::errors::ApiError;
use crate::read::params::Params;
use crate::read::response::{ApiResponse, serialize};
use crate::wire::{JsonLdType, ResourceRef, ThumbnailDisplayUrls, ThumbnailUrls};

// ---------------------------------------------------------------- items

pub async fn search_items(api: &Api<'_>, params: &Params) -> Result<ApiResponse, ApiError> {
    let window = Window::from_params(params);
    let limit = Limit::from_params(params);
    let sort = Sort::from_params(params);
    let filter = ItemFilter {
        owner_id: params.id("owner_id"),
        resource_class_id: params.id("resource_class_id"),
        resource_template_id: params.id("resource_template_id"),
        is_public: params.bool("is_public"),
        ..ItemFilter::default()
    };
    if !params.rows("property").is_empty() {
        // A `property[i][property]` row names a term the page's own rows need
        // not name, so the term table is loaded before the query resolves it.
        api.fill_catalog().await?;
    }
    let search = SearchQuery::parse(params, api.catalog(), media_visibility(api.actor()));
    let (total, page) = if window::eligible(&filter, &search, &sort, &limit) {
        let read = window::read(api, &filter, &search, &limit).await?;
        (read.total, read.rows)
    } else {
        in_memory_page(api, &filter, &search, &sort, &limit, params).await?
    };
    let relations = Relations::load(
        api,
        Page::Items,
        page.iter()
            .map(|row| (row.resource.id.0, row.resource.resource_template_id)),
    )
    .await?;
    let mut documents = Vec::with_capacity(page.len());
    for row in &page {
        documents.push(item_doc(api, row, &relations));
    }
    let body = serialize(&documents)?;
    Ok(ApiResponse::listing(
        body,
        total,
        link_header(api.base(), "items", params, &window, total),
    ))
}

/// The list endpoint's in-memory window: every row the filter matches, the
/// caller's visibility, the ordering, then the page slice
/// ([`super::params::sorted_page`]).
async fn in_memory_page(
    api: &Api<'_>,
    filter: &ItemFilter,
    search: &SearchQuery,
    sort: &Sort,
    limit: &Limit,
    params: &Params,
) -> Result<(i64, Vec<ItemRow>), ApiError> {
    let rows = ItemRepo::new(api.pool())
        .list_filtered(filter, &search.filter, &Paging::default())
        .await
        .map_err(db_error)?;
    let matches = fulltext_matches(api, "items", search.fulltext.as_deref()).await?;
    let rows: Vec<ItemRow> = rows
        .into_iter()
        .filter(|row| api.sees(row.resource.is_public, row.resource.owner_id))
        .filter(|row| id_matches(&search.ids, row.id().0))
        .filter(|row| {
            matches
                .as_ref()
                .is_none_or(|hits| hits.contains_key(&row.id().0))
        })
        .collect();
    Ok(match (&matches, params.sort_by_is_default()) {
        (Some(hits), true) => relevance_page(
            rows,
            sort.descending,
            limit,
            |row| row.id().0,
            |row| hits.get(&row.id().0).copied().unwrap_or(0.0),
        ),
        _ => sorted_page(rows, sort, limit, cmp_item(*sort)),
    })
}

pub async fn read_item(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(ITEM_CLASS, raw_id)?;
    let Some(row) = ItemRepo::new(api.pool())
        .get(ItemId(id))
        .await
        .map_err(db_error)?
    else {
        return Err(not_found(ITEM_CLASS, raw_id));
    };
    if !api.sees(row.resource.is_public, row.resource.owner_id) {
        return Err(not_found(ITEM_CLASS, raw_id));
    }
    item_document(api, &row).await
}

/// The `Item` document for a row, as `read_item` answers it. A delete answers
/// with the row it removed (`Api\Manager::finalize` renders the representation),
/// and it has already passed the entity authorization a read applies.
pub(crate) async fn item_document(api: &Api<'_>, row: &ItemRow) -> Result<ApiResponse, ApiError> {
    let relations = Relations::load(
        api,
        Page::Items,
        [(row.resource.id.0, row.resource.resource_template_id)],
    )
    .await?;
    Ok(ApiResponse::document(serialize(&item_doc(
        api, row, &relations,
    ))?))
}

const ITEM_CLASS: &str = "Omeka\\Entity\\Item";

pub(crate) fn item_doc(
    api: &Api<'_>,
    row: &ItemRow,
    relations: &Relations,
) -> crate::wire::item::Item {
    let base = api.base();
    let resource = &row.resource;
    let media = visible_media(api, relations, row.id());
    let primary = primary_media_of(&media, row.primary_media_id);
    let media_refs: Vec<ResourceRef> = media
        .iter()
        .map(|row| resource_ref(base, "media", row.id().0))
        .collect();
    let item_sets: Vec<ResourceRef> = relations
        .item_sets_of(resource.id.0)
        .iter()
        .map(|id| resource_ref(base, "item_sets", id.0))
        .collect();
    // `ItemRepresentation::sites`: every site the item belongs to, whether or
    // not the site is public.
    let sites: Vec<ResourceRef> = relations
        .sites_of(resource.id.0)
        .iter()
        .map(|id| resource_ref(base, "sites", id.0))
        .collect();
    crate::wire::item::Item {
        context: base.api_context(),
        id: base.api_resource("items", resource.id.0),
        jsonld_type: jsonld_type(api, "o:Item", resource.resource_class_id),
        o_id: resource.id.0,
        is_public: resource.is_public,
        owner: resource_ref_opt(base, "users", resource.owner_id.map(|id| id.0)),
        resource_class: resource_ref_opt(
            base,
            "resource_classes",
            resource.resource_class_id.map(|id| id.0),
        ),
        resource_template: resource_ref_opt(
            base,
            "resource_templates",
            resource.resource_template_id.map(|id| id.0),
        ),
        thumbnail: resource_ref_opt(base, "assets", resource.thumbnail_id.map(|id| id.0)),
        title: resource.title.clone(),
        thumbnail_display_urls: thumbnails(api, relations, resource.thumbnail_id, primary),
        created: jsonld_date(resource.created),
        modified: resource.modified.map(jsonld_date),
        // `ItemRepresentation::primaryMedia`: the stored primary media when it
        // is visible, otherwise the first visible media.
        primary_media: primary.map(|media| resource_ref(base, "media", media.id().0)),
        media: media_refs,
        item_set: item_sets,
        site: sites,
        values: value_map(api, relations, resource.id, resource.resource_template_id),
        reverse: reverse_map(api, relations, resource.id),
    }
}

// ------------------------------------------------------------- item_sets

pub async fn search_item_sets(api: &Api<'_>, params: &Params) -> Result<ApiResponse, ApiError> {
    let window = Window::from_params(params);
    let limit = Limit::from_params(params);
    let sort = Sort::from_params(params);
    let filter = ItemSetFilter {
        owner_id: params.id("owner_id"),
        resource_class_id: params.id("resource_class_id"),
        resource_template_id: params.id("resource_template_id"),
        is_public: params.bool("is_public"),
        is_open: params.bool("is_open"),
    };
    if !params.rows("property").is_empty() {
        // A `property[i][property]` row names a term the page's own rows need
        // not name, so the term table is loaded before the query resolves it.
        api.fill_catalog().await?;
    }
    let search = SearchQuery::parse(params, api.catalog(), media_visibility(api.actor()));
    let rows = ItemSetRepo::new(api.pool())
        .list_filtered(&filter, &search.filter, &Paging::default())
        .await
        .map_err(db_error)?;
    let matches = fulltext_matches(api, "item_sets", search.fulltext.as_deref()).await?;
    let rows: Vec<ItemSetRow> = rows
        .into_iter()
        .filter(|row| api.sees(row.resource.is_public, row.resource.owner_id))
        .filter(|row| id_matches(&search.ids, row.id().0))
        .filter(|row| {
            matches
                .as_ref()
                .is_none_or(|hits| hits.contains_key(&row.id().0))
        })
        .collect();
    let (total, page) = match (&matches, params.sort_by_is_default()) {
        (Some(hits), true) => relevance_page(
            rows,
            sort.descending,
            &limit,
            |row| row.id().0,
            |row| hits.get(&row.id().0).copied().unwrap_or(0.0),
        ),
        _ => sorted_page(rows, &sort, &limit, cmp_item_set(sort)),
    };
    let relations = Relations::load(
        api,
        Page::ItemSets,
        page.iter()
            .map(|row| (row.resource.id.0, row.resource.resource_template_id)),
    )
    .await?;
    let mut documents = Vec::with_capacity(page.len());
    for row in &page {
        documents.push(item_set_doc(api, row, &relations));
    }
    let body = serialize(&documents)?;
    Ok(ApiResponse::listing(
        body,
        total,
        link_header(api.base(), "item_sets", params, &window, total),
    ))
}

pub async fn read_item_set(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(ITEM_SET_CLASS, raw_id)?;
    let Some(row) = ItemSetRepo::new(api.pool())
        .get(ItemSetId(id))
        .await
        .map_err(db_error)?
    else {
        return Err(not_found(ITEM_SET_CLASS, raw_id));
    };
    if !api.sees(row.resource.is_public, row.resource.owner_id) {
        return Err(not_found(ITEM_SET_CLASS, raw_id));
    }
    item_set_document(api, &row).await
}

/// The `ItemSet` document for a row, as `read_item_set` answers it.
pub(crate) async fn item_set_document(
    api: &Api<'_>,
    row: &ItemSetRow,
) -> Result<ApiResponse, ApiError> {
    let relations = Relations::load(
        api,
        Page::ItemSets,
        [(row.resource.id.0, row.resource.resource_template_id)],
    )
    .await?;
    Ok(ApiResponse::document(serialize(&item_set_doc(
        api, row, &relations,
    ))?))
}

const ITEM_SET_CLASS: &str = "Omeka\\Entity\\ItemSet";

pub(crate) fn item_set_doc(
    api: &Api<'_>,
    row: &ItemSetRow,
    relations: &Relations,
) -> crate::wire::item_set::ItemSet {
    let base = api.base();
    let resource = &row.resource;
    let primary = item_set_primary_media(api, relations, row.id());
    crate::wire::item_set::ItemSet {
        context: base.api_context(),
        id: base.api_resource("item_sets", resource.id.0),
        jsonld_type: jsonld_type(api, "o:ItemSet", resource.resource_class_id),
        o_id: resource.id.0,
        is_public: resource.is_public,
        owner: resource_ref_opt(base, "users", resource.owner_id.map(|id| id.0)),
        resource_class: resource_ref_opt(
            base,
            "resource_classes",
            resource.resource_class_id.map(|id| id.0),
        ),
        resource_template: resource_ref_opt(
            base,
            "resource_templates",
            resource.resource_template_id.map(|id| id.0),
        ),
        thumbnail: resource_ref_opt(base, "assets", resource.thumbnail_id.map(|id| id.0)),
        title: resource.title.clone(),
        thumbnail_display_urls: thumbnails(api, relations, resource.thumbnail_id, primary),
        created: jsonld_date(resource.created),
        modified: resource.modified.map(jsonld_date),
        is_open: row.is_open,
        items: base.items_for_item_set(resource.id.0),
        values: value_map(api, relations, resource.id, resource.resource_template_id),
        reverse: reverse_map(api, relations, resource.id),
    }
}

/// `ItemSetRepresentation::primaryMedia`: the first item's primary media.
fn item_set_primary_media<'r>(
    api: &Api<'_>,
    relations: &'r Relations,
    set: ItemSetId,
) -> Option<&'r MediaRow> {
    let (item, primary) = relations.first_item_of(set.0)?;
    let media = visible_media(api, relations, ItemId(item));
    primary_media_of(&media, primary)
}

// ---------------------------------------------------------------- media

pub async fn search_media(api: &Api<'_>, params: &Params) -> Result<ApiResponse, ApiError> {
    let window = Window::from_params(params);
    let limit = Limit::from_params(params);
    let sort = Sort::from_params(params);
    let filter = MediaFilter {
        item_id: params.id("item_id"),
        ingester: params.get("ingester").map(str::to_string),
        renderer: params.get("renderer").map(str::to_string),
        media_type: params.get("media_type").map(str::to_string),
        owner_id: params.id("owner_id"),
        resource_class_id: params.id("resource_class_id"),
        resource_template_id: params.id("resource_template_id"),
        is_public: params.bool("is_public"),
    };
    if !params.rows("property").is_empty() {
        // A `property[i][property]` row names a term the page's own rows need
        // not name, so the term table is loaded before the query resolves it.
        api.fill_catalog().await?;
    }
    let search = SearchQuery::parse(params, api.catalog(), media_visibility(api.actor()));
    let rows = MediaRepo::new(api.pool())
        .list_filtered(&filter, &search.filter, &Paging::default())
        .await
        .map_err(db_error)?;
    let matches = fulltext_matches(api, "media", search.fulltext.as_deref()).await?;
    let rows: Vec<MediaRow> = rows
        .into_iter()
        .filter(|row| api.sees(row.resource.is_public, row.resource.owner_id))
        .filter(|row| id_matches(&search.ids, row.id().0))
        .filter(|row| {
            matches
                .as_ref()
                .is_none_or(|hits| hits.contains_key(&row.id().0))
        })
        .collect();
    let (total, page) = match (&matches, params.sort_by_is_default()) {
        (Some(hits), true) => relevance_page(
            rows,
            sort.descending,
            &limit,
            |row| row.id().0,
            |row| hits.get(&row.id().0).copied().unwrap_or(0.0),
        ),
        _ => sorted_page(rows, &sort, &limit, cmp_media(sort)),
    };
    let relations = Relations::load(
        api,
        Page::Plain,
        page.iter()
            .map(|row| (row.resource.id.0, row.resource.resource_template_id)),
    )
    .await?;
    let mut documents = Vec::with_capacity(page.len());
    for row in &page {
        documents.push(media_doc(api, row, &relations));
    }
    let body = serialize(&documents)?;
    Ok(ApiResponse::listing(
        body,
        total,
        link_header(api.base(), "media", params, &window, total),
    ))
}

pub async fn read_media(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(MEDIA_CLASS, raw_id)?;
    let Some(row) = MediaRepo::new(api.pool())
        .get(MediaId(id))
        .await
        .map_err(db_error)?
    else {
        return Err(not_found(MEDIA_CLASS, raw_id));
    };
    if !api.sees(row.resource.is_public, row.resource.owner_id) {
        return Err(not_found(MEDIA_CLASS, raw_id));
    }
    media_document(api, &row).await
}

/// The `Media` document for a row, as `read_media` answers it.
pub(crate) async fn media_document(api: &Api<'_>, row: &MediaRow) -> Result<ApiResponse, ApiError> {
    let relations = Relations::load(
        api,
        Page::Plain,
        [(row.resource.id.0, row.resource.resource_template_id)],
    )
    .await?;
    Ok(ApiResponse::document(serialize(&media_doc(
        api, row, &relations,
    ))?))
}

const MEDIA_CLASS: &str = "Omeka\\Entity\\Media";

pub(crate) fn media_doc(
    api: &Api<'_>,
    row: &MediaRow,
    relations: &Relations,
) -> crate::wire::media::Media {
    let base = api.base();
    let resource = &row.resource;
    let filename = media_filename(row);
    let thumbnail_urls = media_thumbnail_urls(api, row);
    let original_url = if row.has_original {
        filename
            .as_ref()
            .map(|name| base.file_url(&format!("original/{name}")))
    } else {
        None
    };
    let (jsonld_type, cnt_chars, cnt_character_encoding) = media_jsonld_type(api, row);
    crate::wire::media::Media {
        context: base.api_context(),
        id: base.api_resource("media", resource.id.0),
        jsonld_type,
        o_id: resource.id.0,
        is_public: resource.is_public,
        owner: resource_ref_opt(base, "users", resource.owner_id.map(|id| id.0)),
        resource_class: resource_ref_opt(
            base,
            "resource_classes",
            resource.resource_class_id.map(|id| id.0),
        ),
        resource_template: resource_ref_opt(
            base,
            "resource_templates",
            resource.resource_template_id.map(|id| id.0),
        ),
        thumbnail: resource_ref_opt(base, "assets", resource.thumbnail_id.map(|id| id.0)),
        title: resource.title.clone(),
        thumbnail_display_urls: thumbnails(api, relations, resource.thumbnail_id, Some(row)),
        created: jsonld_date(resource.created),
        modified: resource.modified.map(jsonld_date),
        ingester: row.ingester.clone(),
        renderer: Some(row.renderer.clone()),
        item: resource_ref(base, "items", row.item_id.0),
        source: row.source.clone(),
        media_type: row.media_type.clone(),
        sha256: row.sha256.clone(),
        size: row.size.and_then(|size| u64::try_from(size).ok()),
        filename,
        lang: row.lang.clone(),
        alt_text: row.alt_text.clone(),
        original_url,
        thumbnail_urls,
        data: row.data.clone(),
        values: value_map(api, relations, resource.id, resource.resource_template_id),
        reverse: reverse_map(api, relations, resource.id),
        cnt_chars,
        cnt_character_encoding,
    }
}

/// `Module::filterHtmlMediaJsonLd` (`application/Module.php:397-408`): the
/// `rep.resource.json` listener replaces `@type` with `o-cnt:ContentAsText` and
/// adds `o-cnt:chars` and `o-cnt:characterEncoding` for html media. It runs
/// after the class-derived type is set, so the html type is always the plain
/// string; a resource class cannot add a second term.
fn media_jsonld_type(
    api: &Api<'_>,
    row: &MediaRow,
) -> (JsonLdType, Option<String>, Option<String>) {
    if row.ingester != "html" {
        return (
            jsonld_type(api, "o:Media", row.resource.resource_class_id),
            None,
            None,
        );
    }
    let chars = row
        .data
        .as_ref()
        .and_then(|data| data.get("html"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    (
        JsonLdType::single("o-cnt:ContentAsText"),
        chars,
        Some("UTF-8".to_string()),
    )
}

// --------------------------------------------------- value_annotations

pub async fn search_value_annotations(
    api: &Api<'_>,
    params: &Params,
) -> Result<ApiResponse, ApiError> {
    let window = Window::from_params(params);
    let limit = Limit::from_params(params);
    let rows = system::annotation_rows(api.pool()).await?;
    let needle = search_needle(params);
    let ids = id_filter(params);
    let rows: Vec<ValueAnnotationRow> = rows
        .into_iter()
        .filter(|row| api.sees(row.is_public, row.owner_id))
        .filter(|row| id_matches(&ids, row.id.0))
        .filter(|row| {
            params
                .bool("is_public")
                .is_none_or(|want| row.is_public == want)
        })
        .filter(|row| {
            params
                .id("owner_id")
                .is_none_or(|want: omeka_db::ids::UserId| row.owner_id == Some(want))
        })
        .filter(|row| matches_search(row.title.as_deref(), needle.as_deref()))
        .map(|resource| ValueAnnotationRow { resource })
        .collect();
    // `ValueAnnotationAdapter` declares no `sortFields`, so
    // `AbstractEntityAdapter::sortQuery` orders by `id` for every `sort_by`.
    let (total, page) = ordered_page(
        rows,
        Sort::from_params(params).descending,
        &limit,
        |a, b| a.resource.id.cmp(&b.resource.id),
    );
    let relations = Relations::load(
        api,
        Page::Plain,
        page.iter()
            .map(|row| (row.resource.id.0, row.resource.resource_template_id)),
    )
    .await?;
    let mut documents = Vec::with_capacity(page.len());
    for row in &page {
        documents.push(value_annotation_doc(api, row, &relations));
    }
    let body = serialize(&documents)?;
    Ok(ApiResponse::listing(
        body,
        total,
        link_header(api.base(), "value_annotations", params, &window, total),
    ))
}

pub async fn read_value_annotation(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(ANNOTATION_CLASS, raw_id)?;
    let Some(resource) = ResourceRepo::new(api.pool())
        .get(ResourceId(id))
        .await
        .map_err(db_error)?
    else {
        return Err(not_found(ANNOTATION_CLASS, raw_id));
    };
    if resource.resource_type != resource_type::VALUE_ANNOTATION {
        return Err(not_found(ANNOTATION_CLASS, raw_id));
    }
    if !api.sees(resource.is_public, resource.owner_id) {
        return Err(not_found(ANNOTATION_CLASS, raw_id));
    }
    let row = ValueAnnotationRow {
        resource: resource.clone(),
    };
    value_annotation_document(api, &row).await
}

const ANNOTATION_CLASS: &str = "Omeka\\Entity\\ValueAnnotation";

/// The `ValueAnnotation` document for a resource row, as `read_value_annotation`
/// answers it. The generic `resources` reader dispatches on the row's kind and
/// reaches this for an annotation.
pub(crate) async fn value_annotation_document(
    api: &Api<'_>,
    row: &ValueAnnotationRow,
) -> Result<ApiResponse, ApiError> {
    let relations = Relations::load(
        api,
        Page::Plain,
        [(row.resource.id.0, row.resource.resource_template_id)],
    )
    .await?;
    Ok(ApiResponse::document(serialize(&value_annotation_doc(
        api, row, &relations,
    ))?))
}

pub(crate) fn value_annotation_doc(
    api: &Api<'_>,
    row: &ValueAnnotationRow,
    relations: &Relations,
) -> crate::wire::value_annotation::ValueAnnotation {
    let base = api.base();
    let resource = &row.resource;
    crate::wire::value_annotation::ValueAnnotation {
        context: base.api_context(),
        id: base.api_resource("value_annotations", resource.id.0),
        jsonld_type: jsonld_type(api, "o:ValueAnnotation", resource.resource_class_id),
        o_id: resource.id.0,
        is_public: resource.is_public,
        owner: resource_ref_opt(base, "users", resource.owner_id.map(|id| id.0)),
        resource_class: resource_ref_opt(
            base,
            "resource_classes",
            resource.resource_class_id.map(|id| id.0),
        ),
        resource_template: resource_ref_opt(
            base,
            "resource_templates",
            resource.resource_template_id.map(|id| id.0),
        ),
        thumbnail: resource_ref_opt(base, "assets", resource.thumbnail_id.map(|id| id.0)),
        title: resource.title.clone(),
        // `AbstractRepresentation::thumbnailDisplayUrl`: a custom asset when one
        // is set, and a value annotation has no primary media to fall back to.
        thumbnail_display_urls: thumbnails(api, relations, resource.thumbnail_id, None),
        created: jsonld_date(resource.created),
        modified: resource.modified.map(jsonld_date),
        values: value_map(api, relations, resource.id, resource.resource_template_id),
        reverse: reverse_map(api, relations, resource.id),
    }
}

// ------------------------------------------------------------- helpers

/// Every visible media of an item, in `position` order
/// (`Entity/Item.php` `@OrderBy({"position" = "ASC"})`).
fn visible_media<'r>(api: &Api<'_>, relations: &'r Relations, item: ItemId) -> Vec<&'r MediaRow> {
    relations
        .media_of(item.0)
        .iter()
        .filter(|row| api.sees(row.resource.is_public, row.resource.owner_id))
        .collect()
}

/// `ItemRepresentation::primaryMedia`: the stored primary media when one is
/// set and visible, otherwise the first visible media.
fn primary_media_of<'r>(media: &[&'r MediaRow], primary: Option<MediaId>) -> Option<&'r MediaRow> {
    match primary {
        Some(id) => media.iter().find(|row| row.id() == id).copied(),
        None => media.first().copied(),
    }
}

fn jsonld_type(api: &Api<'_>, primary: &str, class: Option<ClassId>) -> JsonLdType {
    let term = class.and_then(|id| api.catalog().class_term(id).map(str::to_string));
    JsonLdType::with_resource_class(primary, term)
}

/// `AbstractRepresentation::thumbnailDisplayUrl` for a resource entity: the
/// custom thumbnail asset when one is set, otherwise the primary media's
/// display URL for each type, otherwise nothing.
fn thumbnails(
    api: &Api<'_>,
    relations: &Relations,
    custom: Option<AssetId>,
    primary: Option<&MediaRow>,
) -> ThumbnailDisplayUrls {
    if let Some(id) = custom {
        // `AssetRepresentation::thumbnail` returns the asset itself, so every
        // type shows its `assetUrl`.
        let Some(asset) = relations.asset(id.0) else {
            return empty_thumbnail_urls();
        };
        let url = api
            .base()
            .file_url(&format!("asset/{}", asset_filename(asset)));
        return ThumbnailDisplayUrls {
            large: Some(url.clone()),
            medium: Some(url.clone()),
            square: Some(url),
        };
    }
    match primary {
        Some(media) => media_display_urls(api, media),
        None => empty_thumbnail_urls(),
    }
}

/// `Entity\Asset::getFilename`: the storage id plus the extension when there is
/// one.
fn asset_filename(row: &AssetRow) -> String {
    match row.extension.as_deref() {
        Some(extension) if !extension.is_empty() => format!("{}.{extension}", row.storage_id),
        _ => row.storage_id.clone(),
    }
}

/// `MediaRepresentation::thumbnailUrl` for the three configured types
/// (`Module::addTermDefinitionsToContext`'s `thumbnails.types`). A media with
/// no thumbnail file falls back to the asset for its media type, so a media
/// (and any resource whose primary media it is) always has three URLs.
fn media_display_urls(api: &Api<'_>, media: &MediaRow) -> ThumbnailDisplayUrls {
    ThumbnailDisplayUrls {
        large: Some(media_display_url(api, media, "large")),
        medium: Some(media_display_url(api, media, "medium")),
        square: Some(media_display_url(api, media, "square")),
    }
}

fn media_display_url(api: &Api<'_>, media: &MediaRow, kind: &str) -> String {
    if has_thumbnails(api, media)
        && let Some(storage) = &media.storage_id
    {
        return api.base().file_url(&format!("{kind}/{storage}.jpg"));
    }
    api.base()
        .fallback_thumbnail_url(media.media_type.as_deref())
}

/// `MediaRepresentation::thumbnailUrls`: the PHP empty array when the media has
/// no thumbnails, otherwise one file URL per type.
fn media_thumbnail_urls(api: &Api<'_>, media: &MediaRow) -> ThumbnailUrls {
    let mut urls = ThumbnailUrls::empty();
    if !has_thumbnails(api, media) {
        return urls;
    }
    let Some(storage) = &media.storage_id else {
        return urls;
    };
    for kind in THUMBNAIL_TYPES {
        urls.insert(kind, api.base().file_url(&format!("{kind}/{storage}.jpg")));
    }
    urls
}

/// Whether a media row has thumbnails to show.
///
/// Upstream reads the stored `has_thumbnails` column
/// (`MediaRepresentation::thumbnailUrls`, `Entity\Media::$hasThumbnails`). The
/// port also treats the three files in the store as proof, so a row whose
/// thumbnails were generated without the flag still serves the derivatives
/// instead of the full-size original.
fn has_thumbnails(api: &Api<'_>, media: &MediaRow) -> bool {
    if media.has_thumbnails {
        return true;
    }
    let (Some(store), Some(storage_id)) = (api.store(), media.storage_id.as_deref()) else {
        return false;
    };
    ThumbnailType::ALL
        .iter()
        .all(|kind| store.exists(&thumbnail_path(storage_id, *kind)))
}

/// `Entity\Media::getFilename`: the storage id plus the extension.
fn media_filename(media: &MediaRow) -> Option<String> {
    let storage = media.storage_id.as_ref()?;
    Some(match media.extension.as_deref() {
        Some(extension) if !extension.is_empty() => format!("{storage}.{extension}"),
        _ => storage.clone(),
    })
}

fn search_needle(params: &Params) -> Option<String> {
    params
        .get("search")
        .filter(|needle| !needle.is_empty())
        .map(str::to_lowercase)
}

fn matches_search(title: Option<&str>, needle: Option<&str>) -> bool {
    let Some(needle) = needle else {
        return true;
    };
    title
        .map(|title| title.to_lowercase().contains(needle))
        .unwrap_or(false)
}

fn cmp_item(sort: Sort) -> impl Fn(&ItemRow, &ItemRow) -> Ordering {
    move |a, b| match sort.field {
        SortField::Title => {
            compare_opt_text_ci(a.resource.title.as_deref(), b.resource.title.as_deref())
        }
        SortField::Created => a.resource.created.cmp(&b.resource.created),
        SortField::Modified => a.resource.modified.cmp(&b.resource.modified),
        SortField::IsPublic => a.resource.is_public.cmp(&b.resource.is_public),
        SortField::Id => a.resource.id.cmp(&b.resource.id),
    }
}

fn cmp_item_set(sort: Sort) -> impl Fn(&ItemSetRow, &ItemSetRow) -> Ordering {
    move |a, b| match sort.field {
        SortField::Title => {
            compare_opt_text_ci(a.resource.title.as_deref(), b.resource.title.as_deref())
        }
        SortField::Created => a.resource.created.cmp(&b.resource.created),
        SortField::Modified => a.resource.modified.cmp(&b.resource.modified),
        SortField::IsPublic => a.resource.is_public.cmp(&b.resource.is_public),
        SortField::Id => a.resource.id.cmp(&b.resource.id),
    }
}

fn cmp_media(sort: Sort) -> impl Fn(&MediaRow, &MediaRow) -> Ordering {
    move |a, b| match sort.field {
        SortField::Title => {
            compare_opt_text_ci(a.resource.title.as_deref(), b.resource.title.as_deref())
        }
        SortField::Created => a.resource.created.cmp(&b.resource.created),
        SortField::Modified => a.resource.modified.cmp(&b.resource.modified),
        SortField::IsPublic => a.resource.is_public.cmp(&b.resource.is_public),
        SortField::Id => a.resource.id.cmp(&b.resource.id),
    }
}
