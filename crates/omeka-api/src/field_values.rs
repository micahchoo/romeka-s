//! The values of one field across the archive, and moving one value to another field
//! (`briefs/admin-values-board-design.md`).
//!
//! A **field value** is one exact text a field holds on any item, item set or media: type
//! `literal`, text in `value`. The census counts, per text, the resources the caller can see that
//! carry it, by kind, and how many of them the caller may edit. A **move** takes one text off one
//! field and puts it on another, on every resource the caller may edit that carries it. A resource
//! that already holds the text on the target field loses it from the source and gains nothing.
//!
//! The answer to a move names the resources it moved, merged into a value already there, and
//! copied, so the admin can undo it exactly: the same move back, limited to the moved resources
//! (`only`), with the merged ones copied (`copy`) because they still hold the text on the target.
//! A move runs in one transaction (`ValueRewriteRepo::move_values`), records item history, and
//! takes a `dry_run` that answers the same and writes nothing. Visibility and edit rights are the
//! tag rewrites' own ([`tagging::seen`]). Upstream has neither route.

use std::collections::{BTreeMap, BTreeSet};

use omeka_db::ids::{PropertyId, ResourceId, TemplateId, ValueId};
use omeka_db::repos::{TemplateRepo, ValueRewriteRepo};
use omeka_db::rows::{MoveWrite, TextValueRow, resource_type};
use omeka_db::with_db;
use serde::{Deserialize, Serialize};

use crate::errors::ApiError;
use crate::read::response::serialize;
use crate::read::{Api, ApiResponse, db_error, not_found, parse_id};
use crate::tagging::{self, Spelling, parse, seen};

const PROPERTY_CLASS: &str = "Omeka\\Entity\\Property";

/// One text of a field and the resources the caller can see that carry it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldValue {
    pub value: String,
    pub items: usize,
    pub media: usize,
    pub item_sets: usize,
    /// Of all of those, the ones the caller may edit.
    pub editable: usize,
}

#[derive(Debug, Serialize)]
struct Census {
    property: u64,
    values: Vec<FieldValue>,
}

/// A property id from the path or a body, refused with a 404 when no property has it.
async fn property(api: &Api<'_>, id: PropertyId) -> Result<PropertyId, ApiError> {
    api.fill_catalog().await?;
    api.catalog()
        .property(id)
        .map(|_| id)
        .ok_or_else(|| not_found(PROPERTY_CLASS, &id.0.to_string()))
}

async fn text_values(api: &Api<'_>, property: PropertyId) -> Result<Vec<TextValueRow>, ApiError> {
    with_db!(api.pool(), |db| ValueRewriteRepo::new(db)
        .text_values(tagging::TAG_TYPE, Some(property))
        .await)
    .map_err(db_error)
}

/// `GET /api/romeka/field-values/{property}`: the field's texts, most used first.
pub async fn census(api: &Api<'_>, raw_property: &str) -> Result<ApiResponse, ApiError> {
    let property = property(api, PropertyId(parse_id(PROPERTY_CLASS, raw_property)?)).await?;
    let rows = text_values(api, property).await?;
    #[derive(Default)]
    struct Carriers<'r> {
        by_kind: BTreeMap<&'r str, BTreeSet<ResourceId>>,
        editable: BTreeSet<ResourceId>,
    }
    let mut texts: BTreeMap<&str, Carriers<'_>> = BTreeMap::new();
    for seen in seen(api, &rows) {
        let carriers = texts.entry(seen.row.text.as_str()).or_default();
        carriers
            .by_kind
            .entry(seen.row.resource_type.as_str())
            .or_default()
            .insert(seen.row.resource_id);
        if seen.editable {
            carriers.editable.insert(seen.row.resource_id);
        }
    }
    let count =
        |carriers: &Carriers<'_>, kind: &str| carriers.by_kind.get(kind).map_or(0, BTreeSet::len);
    let mut values: Vec<FieldValue> = texts
        .iter()
        .map(|(text, carriers)| FieldValue {
            value: (*text).to_string(),
            items: count(carriers, resource_type::ITEM),
            media: count(carriers, resource_type::MEDIA),
            item_sets: count(carriers, resource_type::ITEM_SET),
            editable: carriers.editable.len(),
        })
        .collect();
    let total = |value: &FieldValue| value.items + value.media + value.item_sets;
    values.sort_by(|a, b| total(b).cmp(&total(a)).then_with(|| a.value.cmp(&b.value)));
    Ok(ApiResponse::document(serialize(&Census {
        property: property.0,
        values,
    })?))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MoveBody {
    value: Spelling,
    from: u64,
    to: u64,
    #[serde(default)]
    dry_run: bool,
    /// Move only on these resources.
    #[serde(default)]
    only: Option<Vec<u64>>,
    /// On these resources, copy the text to `to` and keep it on `from`.
    #[serde(default)]
    copy: Vec<u64>,
}

/// An item type whose form does not show the target field, and how many changed resources use it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NotInForm {
    pub template: u64,
    pub label: String,
    pub resources: usize,
}

#[derive(Debug, Serialize)]
struct Moved {
    changed: usize,
    /// Resources carrying the text that the caller may not edit; they keep it where it is.
    skipped: usize,
    moved: Vec<u64>,
    merged: Vec<u64>,
    copied: Vec<u64>,
    not_in_form: Vec<NotInForm>,
}

/// What a move does to each value of the text on the source field.
#[derive(Default)]
struct Plan {
    retarget: Vec<ValueId>,
    delete: Vec<ValueId>,
    copy: Vec<ValueId>,
    moved: BTreeSet<ResourceId>,
    merged: BTreeSet<ResourceId>,
    copied: BTreeSet<ResourceId>,
    skipped: BTreeSet<ResourceId>,
    templates: BTreeMap<ResourceId, Option<TemplateId>>,
}

/// `POST /api/romeka/field-values/move`: `{value, from, to, dry_run?, only?, copy?}`.
pub async fn move_value(api: &Api<'_>, bytes: &[u8]) -> Result<ApiResponse, ApiError> {
    let body: MoveBody = parse(bytes)?;
    if body.from == body.to {
        return Err(ApiError::bad_request(
            "from and to must be different fields",
        ));
    }
    let from = property(api, PropertyId(body.from)).await?;
    let to = property(api, PropertyId(body.to)).await?;
    let text = body.value.0;
    let only: Option<BTreeSet<ResourceId>> = body
        .only
        .map(|ids| ids.into_iter().map(ResourceId).collect());
    let copy: BTreeSet<ResourceId> = body.copy.into_iter().map(ResourceId).collect();
    let in_scope = |resource: ResourceId| {
        copy.contains(&resource) || only.as_ref().is_none_or(|only| only.contains(&resource))
    };

    // Every resource that holds the text on the target, visible to the caller or not: a move
    // never adds a second copy there.
    let mut on_target: BTreeSet<ResourceId> = text_values(api, to)
        .await?
        .into_iter()
        .filter(|row| row.text == text)
        .map(|row| row.resource_id)
        .collect();
    let rows = text_values(api, from).await?;
    let mut plan = Plan::default();
    for seen in seen(api, &rows) {
        let row = seen.row;
        if row.text != text || !in_scope(row.resource_id) {
            continue;
        }
        if !seen.editable {
            plan.skipped.insert(row.resource_id);
            continue;
        }
        plan.templates.insert(row.resource_id, row.template_id);
        if copy.contains(&row.resource_id) {
            if on_target.insert(row.resource_id) {
                plan.copy.push(row.id);
                plan.copied.insert(row.resource_id);
            }
        } else if on_target.contains(&row.resource_id) && !row.has_annotation {
            plan.delete.push(row.id);
            if !plan.moved.contains(&row.resource_id) {
                plan.merged.insert(row.resource_id);
            }
        } else {
            on_target.insert(row.resource_id);
            plan.retarget.push(row.id);
            plan.moved.insert(row.resource_id);
        }
    }
    let resources: Vec<ResourceId> = plan.templates.keys().copied().collect();
    let answer = Moved {
        changed: resources.len(),
        skipped: plan.skipped.len(),
        moved: plan.moved.iter().map(|id| id.0).collect(),
        merged: plan.merged.iter().map(|id| id.0).collect(),
        copied: plan.copied.iter().map(|id| id.0).collect(),
        not_in_form: not_in_form(api, &plan.templates, to).await?,
    };
    if body.dry_run || resources.is_empty() {
        return Ok(ApiResponse::document(serialize(&answer)?));
    }
    with_db!(api.pool(), |db| ValueRewriteRepo::new(db)
        .move_values(
            &MoveWrite {
                from,
                to,
                text,
                retarget: plan.retarget.clone(),
                delete: plan.delete.clone(),
                copy: plan.copy.clone(),
                resources: resources.clone(),
                default_title_property: api.catalog().property_id("dcterms:title"),
            },
            &crate::write::items::edit_stamp(api)
        )
        .await)
    .map_err(db_error)?;
    tagging::reindex(api, &rows, &resources).await?;
    Ok(ApiResponse::document(serialize(&answer)?))
}

/// The item types of the changed resources whose fields do not include `to`, most used first. A
/// resource with no item type shows every value it holds, so it is never one.
async fn not_in_form(
    api: &Api<'_>,
    templates: &BTreeMap<ResourceId, Option<TemplateId>>,
    to: PropertyId,
) -> Result<Vec<NotInForm>, ApiError> {
    let mut uses: BTreeMap<TemplateId, usize> = BTreeMap::new();
    for template in templates.values().flatten() {
        *uses.entry(*template).or_default() += 1;
    }
    let mut missing = Vec::new();
    for (template, resources) in uses {
        let rows = with_db!(api.pool(), |db| TemplateRepo::new(db)
            .list_properties(template)
            .await)
        .map_err(db_error)?;
        if rows.iter().any(|row| row.property_id == to) {
            continue;
        }
        if let Some((label, _)) = tagging::template_label(api, template).await? {
            missing.push(NotInForm {
                template: template.0,
                label,
                resources,
            });
        }
    }
    missing.sort_by(|a, b| {
        b.resources
            .cmp(&a.resources)
            .then_with(|| a.label.cmp(&b.label))
    });
    Ok(missing)
}
