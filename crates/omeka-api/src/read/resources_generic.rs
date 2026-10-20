//! The generic `resources` resource: `ResourceAdapter`.
//!
//! `ResourceAdapter::getRepresentationClass` returns null, and every method it
//! inherits delegates: `search` runs on the abstract `Resource` entity, so one
//! query returns items, item sets, media and value annotations, and
//! `getRepresentation` proxies each row to the adapter named by
//! `Resource::getResourceName`. This module is that proxy. It gathers the rows
//! of every kind, applies the caller's visibility, orders by id, and renders
//! each row through the document builder of the kind it holds.
//!
//! `ResourceAdapter` declares an empty `sortFields`, so `sort_by` reaches
//! nothing and `AbstractEntityAdapter::search` orders by `id` alone.
//!
//! The filter set is the base adapter's `buildQuery` minus the parts this unit
//! does not carry: `property[i]` value filters, `fulltext_search` and the
//! `created_*` / `modified_*` dates are not rendered here. `owner_id`,
//! `resource_class_id`, `resource_template_id`, `is_public` and `search` are,
//! because the other list endpoints already model them.
//!
//! The four kinds come from the repositories that own their tables
//! (`ItemRepo::list`, `ItemSetRepo::list`, `MediaRepo::list` and the
//! `value_annotation` query in [`super::system`]), so this module holds no SQL
//! of its own.

use omeka_db::ids::ResourceId;
use omeka_db::repos::{
    ItemFilter, ItemRepo, ItemSetFilter, ItemSetRepo, MediaFilter, MediaRepo, Paging, ResourceRepo,
};
use omeka_db::rows::{
    ItemRow, ItemSetRow, MediaRow, ResourceRow, ValueAnnotationRow, resource_type,
};

use super::paging::{Limit, Window, ordered_page};
use super::params::Params;
use super::query::{id_filter, id_matches};
use super::relations::Relations;
use super::response::link_header;
use super::{Api, db_error, not_found, parse_id, resources, system};
use crate::errors::ApiError;
use crate::read::params::Sort;
use crate::read::response::{ApiResponse, serialize};
use crate::routes::Operation;

/// `Omeka\Entity\Resource`, the class the generic adapter's messages name.
const RESOURCE_CLASS: &str = "Omeka\\Entity\\Resource";

/// One resource row with the kind row that carries its extra columns.
///
/// The variants are boxed because a media row is several times the size of the
/// others, and a page holds one row per variant.
enum KindRow {
    Item(Box<ItemRow>),
    ItemSet(Box<ItemSetRow>),
    Media(Box<MediaRow>),
    Annotation(Box<ResourceRow>),
}

impl KindRow {
    /// The item that owns a media row.
    fn media_item(&self) -> Option<u64> {
        match self {
            Self::Media(row) => Some(row.item_id.0),
            _ => None,
        }
    }

    /// The shared `resource` row.
    fn resource(&self) -> &ResourceRow {
        match self {
            Self::Item(row) => &row.resource,
            Self::ItemSet(row) => &row.resource,
            Self::Media(row) => &row.resource,
            Self::Annotation(row) => row,
        }
    }

    fn id(&self) -> u64 {
        self.resource().id.0
    }

    /// The resource name of the adapter that renders this row
    /// (`Resource::getResourceName`).
    fn kind_resource_name(&self) -> &'static str {
        match self {
            Self::Item(_) => "items",
            Self::ItemSet(_) => "item_sets",
            Self::Media(_) => "media",
            Self::Annotation(_) => "value_annotations",
        }
    }
}

/// `ResourceAdapter::search`: one page over every resource kind.
pub async fn search_resources(api: &Api<'_>, params: &Params) -> Result<ApiResponse, ApiError> {
    let window = Window::from_params(params);
    let limit = Limit::from_params(params);
    let needle = params
        .get("search")
        .filter(|needle| !needle.is_empty())
        .map(str::to_lowercase);
    let ids = id_filter(params);
    let rows: Vec<KindRow> = all_rows(api).await?;
    let rows: Vec<KindRow> = rows
        .into_iter()
        .filter(|row| {
            let resource = row.resource();
            api.sees(resource.is_public, resource.owner_id)
        })
        .filter(|row| id_matches(&ids, row.id()))
        .filter(|row| owner_matches(row.resource().owner_id, params))
        .filter(|row| class_matches(row.resource().resource_class_id, params))
        .filter(|row| template_matches(row.resource().resource_template_id, params))
        .filter(|row| {
            params
                .bool("is_public")
                .is_none_or(|want| row.resource().is_public == want)
        })
        .filter(|row| title_matches(row.resource().title.as_deref(), needle.as_deref()))
        .collect();
    let (total, page) = ordered_page(
        rows,
        Sort::from_params(params).descending,
        &limit,
        |a, b| a.id().cmp(&b.id()),
    );
    let mut relations = Relations::load_many(api, page.iter().map(KindRow::resource)).await?;
    relations
        .load_owners(api, page.iter().filter_map(KindRow::media_item))
        .await?;
    let mut documents = Vec::with_capacity(page.len());
    for row in &page {
        documents.push(render(api, row, &relations)?);
    }
    let body = serialize(&documents)?;
    Ok(ApiResponse::listing(
        body,
        total,
        link_header(api.base(), "resources", params, &window, total),
    ))
}

/// `ResourceAdapter::read`: the row's own kind answers, through the document
/// builder that kind's adapter uses.
///
/// The base adapter finds any `Resource` row, so a missing row is the
/// `Omeka\Entity\Resource` 404, not the kind's. The kind's adapter access check
/// then runs, which is what keeps a value annotation a privileged read here as
/// it is on `/api/value_annotations`.
pub async fn read_resource(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(RESOURCE_CLASS, raw_id)?;
    let Some(row) = ResourceRepo::new(api.pool())
        .get(ResourceId(id))
        .await
        .map_err(db_error)?
    else {
        return Err(not_found(RESOURCE_CLASS, raw_id));
    };
    if !api.sees(row.is_public, row.owner_id) {
        return Err(not_found(RESOURCE_CLASS, raw_id));
    }
    let kind = typed_row(api, row).await?;
    api.allow(kind.kind_resource_name(), Operation::Read)?;
    let mut relations = Relations::load_many(api, std::iter::once(kind.resource())).await?;
    relations.load_owners(api, kind.media_item()).await?;
    Ok(ApiResponse::document(serialize(&render(
        api, &kind, &relations,
    )?)?))
}

/// Every resource row, with the kind row that carries its extra columns.
async fn all_rows(api: &Api<'_>) -> Result<Vec<KindRow>, ApiError> {
    let mut rows: Vec<KindRow> = Vec::new();
    for row in ItemRepo::new(api.pool())
        .list(&ItemFilter::default(), &Paging::default())
        .await
        .map_err(db_error)?
    {
        rows.push(KindRow::Item(Box::new(row)));
    }
    for row in ItemSetRepo::new(api.pool())
        .list(&ItemSetFilter::default(), &Paging::default())
        .await
        .map_err(db_error)?
    {
        rows.push(KindRow::ItemSet(Box::new(row)));
    }
    for row in MediaRepo::new(api.pool())
        .list(&MediaFilter::default(), &Paging::default())
        .await
        .map_err(db_error)?
    {
        rows.push(KindRow::Media(Box::new(row)));
    }
    for row in system::annotation_rows(api.pool()).await? {
        rows.push(KindRow::Annotation(Box::new(row)));
    }
    Ok(rows)
}

/// The kind row a shared `resource` row names (`Resource::getResourceName`).
async fn typed_row(api: &Api<'_>, row: ResourceRow) -> Result<KindRow, ApiError> {
    Ok(match row.resource_type.as_str() {
        resource_type::ITEM => {
            let Some(typed) = ItemRepo::new(api.pool())
                .get(omeka_db::ids::ItemId(row.id.0))
                .await
                .map_err(db_error)?
            else {
                return Err(not_found(RESOURCE_CLASS, &row.id.0.to_string()));
            };
            KindRow::Item(Box::new(typed))
        }
        resource_type::ITEM_SET => {
            let Some(typed) = ItemSetRepo::new(api.pool())
                .get(omeka_db::ids::ItemSetId(row.id.0))
                .await
                .map_err(db_error)?
            else {
                return Err(not_found(RESOURCE_CLASS, &row.id.0.to_string()));
            };
            KindRow::ItemSet(Box::new(typed))
        }
        resource_type::MEDIA => {
            let Some(typed) = MediaRepo::new(api.pool())
                .get(omeka_db::ids::MediaId(row.id.0))
                .await
                .map_err(db_error)?
            else {
                return Err(not_found(RESOURCE_CLASS, &row.id.0.to_string()));
            };
            KindRow::Media(Box::new(typed))
        }
        resource_type::VALUE_ANNOTATION => KindRow::Annotation(Box::new(row)),
        // A discriminator the port does not know has no representation.
        other => {
            return Err(ApiError::internal(format!(
                "resource {} has unknown resource_type {other}",
                row.id.0
            )));
        }
    })
}

/// One page row's representation, through the builder its kind owns.
///
/// The value is the JSON-LD document itself: one page carries documents of
/// different kinds, so the array is assembled from `serde_json` values whose
/// key order the wire structs fixed (the crate enables `preserve_order`).
fn render(
    api: &Api<'_>,
    row: &KindRow,
    relations: &Relations,
) -> Result<serde_json::Value, ApiError> {
    let document = match row {
        KindRow::Item(row) => serde_json::to_value(resources::item_doc(api, row, relations))
            .map_err(ApiError::internal)?,
        KindRow::ItemSet(row) => serde_json::to_value(resources::item_set_doc(api, row, relations))
            .map_err(ApiError::internal)?,
        KindRow::Media(row) => serde_json::to_value(resources::media_doc(api, row, relations))
            .map_err(ApiError::internal)?,
        KindRow::Annotation(row) => serde_json::to_value(resources::value_annotation_doc(
            api,
            &ValueAnnotationRow {
                resource: row.as_ref().clone(),
            },
            relations,
        ))
        .map_err(ApiError::internal)?,
    };
    Ok(document)
}

/// `AbstractResourceEntityAdapter::buildQuery`'s `owner_id`: an equality on the
/// owner's id, ignored when the caller sends no number.
fn owner_matches(owner: Option<omeka_db::ids::UserId>, params: &Params) -> bool {
    params
        .id("owner_id")
        .is_none_or(|want: omeka_db::ids::UserId| owner == Some(want))
}

/// `AbstractResourceEntityAdapter::buildQuery`'s `resource_class_id`.
fn class_matches(class: Option<omeka_db::ids::ClassId>, params: &Params) -> bool {
    params
        .id("resource_class_id")
        .is_none_or(|want: omeka_db::ids::ClassId| class == Some(want))
}

/// `AbstractResourceEntityAdapter::buildQuery`'s `resource_template_id`.
fn template_matches(template: Option<omeka_db::ids::TemplateId>, params: &Params) -> bool {
    params
        .id("resource_template_id")
        .is_none_or(|want: omeka_db::ids::TemplateId| template == Some(want))
}

/// `AbstractResourceEntityAdapter::buildQuery`'s `search` branch is a contains
/// test over the resource's values. The port's other list endpoints match the
/// title, so this one does too.
fn title_matches(title: Option<&str>, needle: Option<&str>) -> bool {
    match needle {
        Some(needle) => title.is_some_and(|title| title.to_lowercase().contains(needle)),
        None => true,
    }
}
