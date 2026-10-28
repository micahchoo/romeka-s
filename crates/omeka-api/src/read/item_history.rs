//! `GET /api/romeka/items/{id}/history`: who changed what on one item, newest first. Upstream
//! has no such route.
//!
//! Only a caller who may edit the item reads its history: the item `PUT`'s checks. A missing item
//! and one the caller may not see are the same 404; an item the caller sees but may not edit is
//! the 403 the edit would answer. Inside an entry the caller sees only what the item page shows
//! them: a private value only under [`sees_private_values`], and an item set, site or linked
//! resource only when they may see it. A field that is then the same before and after is left
//! out, and an entry left with no field is too.
//!
//! Each value carries the members the item write reads (`type`, `property_id`, `@value`, `@id`,
//! `o:label`, `value_resource_id`, `@language`/`o:lang`, `is_public`), so the admin's "Put this
//! value back" sends them through the ordinary item write, which records its own entry.

use std::collections::{BTreeMap, BTreeSet};

use omeka_db::ids::{ClassId, ItemId, ItemSetId, ResourceId, SiteId, TemplateId, UserId};
use omeka_db::repos::{
    ClassRepo, ItemHistoryRepo, ItemRepo, RelationRepo, SiteRepo, TemplateRepo, UserRepo,
};
use omeka_db::rows::{FieldChange, HistoryEntryRow, HistorySource, ImageValue};
use omeka_db::with_db;
use serde::Serialize;
use serde_json::{Map, Value, json};

use super::response::{ApiResponse, serialize};
use super::{Api, db_error, not_found, parse_id};
use crate::auth::Actor;
use crate::errors::ApiError;
use crate::routes::Operation;
use crate::write::access::{self, SiteRole};

const ITEM_CLASS: &str = "Omeka\\Entity\\Item";

/// Upstream's `ValueVisibilityFilter` rule for one resource: a caller with `view-all` sees every
/// value, a signed-in caller sees the private values of a resource they own, everyone else sees
/// public values only. `owner` is the resource's owner now.
pub fn sees_private_values(actor: &Actor, owner: Option<UserId>) -> bool {
    actor.is_privileged() || matches!((actor.user(), owner), (Some(me), Some(owner)) if me == owner)
}

#[derive(Serialize)]
struct History {
    entries: Vec<Entry>,
}

#[derive(Serialize)]
struct Entry {
    id: u64,
    created: String,
    user: Option<Named>,
    source: Source,
    changes: Vec<Value>,
}

#[derive(Serialize)]
struct Source {
    /// `edit`, `bulk`, `import`, `bulk_undo` or `import_undo`.
    kind: &'static str,
    /// The bulk run or import, for every kind but `edit`.
    id: Option<u64>,
}

#[derive(Serialize, Clone)]
struct Named {
    id: u64,
    name: String,
}

pub async fn item_history(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = ItemId(parse_id(ITEM_CLASS, raw_id)?);
    let item = with_db!(api.pool(), |db| ItemRepo::new(db).get(id).await)
        .map_err(db_error)?
        .filter(|row| api.sees(row.resource.is_public, row.resource.owner_id))
        .ok_or_else(|| not_found(ITEM_CLASS, raw_id))?;
    api.allow_write("items", Operation::Update)?;
    api.authorize_resource(ITEM_CLASS, item.resource.owner_id)?;
    let rows =
        with_db!(api.pool(), |db| ItemHistoryRepo::new(db).list(id).await).map_err(db_error)?;
    let names = Names::load(
        api,
        &rows,
        sees_private_values(api.actor(), item.resource.owner_id),
    )
    .await?;
    let entries = rows
        .iter()
        .filter_map(|row| names.entry(api, row))
        .collect();
    Ok(ApiResponse::document(serialize(&History { entries })?))
}

/// The names and the visibility of everything the entries name, read once for all of them.
struct Names {
    private: bool,
    users: BTreeMap<UserId, String>,
    classes: BTreeMap<ClassId, String>,
    templates: BTreeMap<TemplateId, String>,
    /// The item sets and linked resources the caller may see, with their titles.
    resources: BTreeMap<u64, Option<String>>,
    sites: BTreeMap<SiteId, String>,
}

impl Names {
    async fn load(
        api: &Api<'_>,
        rows: &[HistoryEntryRow],
        private: bool,
    ) -> Result<Self, ApiError> {
        let mut users = BTreeSet::new();
        let mut classes = BTreeSet::new();
        let mut templates = BTreeSet::new();
        let mut resources = BTreeSet::new();
        let mut sites = BTreeSet::new();
        for row in rows {
            users.extend(row.user_id);
            for change in &row.changes {
                match change {
                    FieldChange::Values { before, after, .. } => resources.extend(
                        before
                            .iter()
                            .chain(after)
                            .filter_map(|value| value.content.value_resource_id),
                    ),
                    FieldChange::Class { before, after } => {
                        classes.extend(before.iter().chain(after))
                    }
                    FieldChange::Template { before, after } => {
                        templates.extend(before.iter().chain(after))
                    }
                    FieldChange::ItemSets { before, after } => {
                        resources.extend(before.iter().chain(after).map(|id| id.as_resource()))
                    }
                    FieldChange::Sites { before, after } => {
                        sites.extend(before.iter().chain(after))
                    }
                    FieldChange::Visibility { .. } => {}
                }
            }
        }
        let mut names = Self {
            private,
            users: BTreeMap::new(),
            classes: BTreeMap::new(),
            templates: BTreeMap::new(),
            resources: BTreeMap::new(),
            sites: BTreeMap::new(),
        };
        for user in users {
            if let Some(row) =
                with_db!(api.pool(), |db| UserRepo::new(db).get(user).await).map_err(db_error)?
            {
                names.users.insert(user, row.name);
            }
        }
        for class in classes {
            if let Some(row) =
                with_db!(api.pool(), |db| ClassRepo::new(db).get(class).await).map_err(db_error)?
            {
                names.classes.insert(class, row.label);
            }
        }
        for template in templates {
            if let Some(row) = with_db!(api.pool(), |db| TemplateRepo::new(db).get(template).await)
                .map_err(db_error)?
            {
                names.templates.insert(template, row.label);
            }
        }
        let ids: Vec<ResourceId> = resources.into_iter().collect();
        if !ids.is_empty() {
            for row in with_db!(api.pool(), |db| RelationRepo::new(db).resources(&ids).await)
                .map_err(db_error)?
            {
                if api.sees(row.is_public, row.owner_id) {
                    names.resources.insert(row.id.0, row.title);
                }
            }
        }
        for site in sites {
            if let Some(row) =
                with_db!(api.pool(), |db| SiteRepo::new(db).get(site).await).map_err(db_error)?
                && sees_site(api, &row).await?
            {
                names.sites.insert(site, row.title);
            }
        }
        api.fill_catalog().await?;
        Ok(names)
    }

    fn entry(&self, api: &Api<'_>, row: &HistoryEntryRow) -> Option<Entry> {
        let changes: Vec<Value> = row
            .changes
            .iter()
            .filter_map(|change| self.change(api, change))
            .collect();
        if changes.is_empty() {
            return None;
        }
        let (kind, id) = match row.source {
            HistorySource::Edit => ("edit", None),
            HistorySource::Bulk(run) => ("bulk", Some(run.0)),
            HistorySource::Import(import) => ("import", Some(import.0)),
            HistorySource::BulkUndo(run) => ("bulk_undo", Some(run.0)),
            HistorySource::ImportUndo(import) => ("import_undo", Some(import.0)),
        };
        Some(Entry {
            id: row.id,
            created: row.created.format("%Y-%m-%dT%H:%M:%S+00:00").to_string(),
            user: row.user_id.and_then(|user| {
                self.users.get(&user).map(|name| Named {
                    id: user.0,
                    name: name.clone(),
                })
            }),
            source: Source { kind, id },
            changes,
        })
    }

    /// One change as the caller may see it, or `None` when what they may see did not change.
    fn change(&self, api: &Api<'_>, change: &FieldChange) -> Option<Value> {
        let answer = match change {
            FieldChange::Values {
                property,
                before,
                after,
            } => {
                let before = self.values(before);
                let after = self.values(after);
                if before == after {
                    return None;
                }
                let info = api.catalog().property(*property)?;
                json!({
                    "field": "values",
                    "property_id": property.0,
                    "term": info.term,
                    "label": info.label,
                    "before": before,
                    "after": after,
                })
            }
            FieldChange::Visibility { before, after } => {
                json!({ "field": "visibility", "before": before, "after": after })
            }
            FieldChange::Class { before, after } => json!({
                "field": "class",
                "before": before.map(|id| named(id.0, self.classes.get(&id))),
                "after": after.map(|id| named(id.0, self.classes.get(&id))),
            }),
            FieldChange::Template { before, after } => json!({
                "field": "template",
                "before": before.map(|id| named(id.0, self.templates.get(&id))),
                "after": after.map(|id| named(id.0, self.templates.get(&id))),
            }),
            FieldChange::ItemSets { before, after } => {
                let seen = |ids: &[ItemSetId]| -> Vec<Value> {
                    ids.iter()
                        .filter_map(|id| {
                            self.resources
                                .get(&id.0)
                                .map(|title| json!({ "id": id.0, "title": title }))
                        })
                        .collect()
                };
                let (before, after) = (seen(before), seen(after));
                if before == after {
                    return None;
                }
                json!({ "field": "item_sets", "before": before, "after": after })
            }
            FieldChange::Sites { before, after } => {
                let seen = |ids: &[SiteId]| -> Vec<Value> {
                    ids.iter()
                        .filter_map(|id| {
                            self.sites
                                .get(id)
                                .map(|title| json!({ "id": id.0, "title": title }))
                        })
                        .collect()
                };
                let (before, after) = (seen(before), seen(after));
                if before == after {
                    return None;
                }
                json!({ "field": "sites", "before": before, "after": after })
            }
        };
        Some(answer)
    }

    /// The values the caller may see, each in the shape the item write reads plus `display`.
    fn values(&self, values: &[ImageValue]) -> Vec<Value> {
        values
            .iter()
            .filter(|value| self.private || value.content.is_public)
            .filter_map(|value| self.value(value))
            .collect()
    }

    fn value(&self, value: &ImageValue) -> Option<Value> {
        let content = &value.content;
        let mut out = Map::new();
        out.insert("type".into(), Value::from(content.r#type.clone()));
        out.insert("property_id".into(), Value::from(content.property_id.0));
        out.insert("is_public".into(), Value::from(content.is_public));
        let display = if let Some(target) = content.value_resource_id {
            let title = self.resources.get(&target.0)?;
            out.insert("value_resource_id".into(), Value::from(target.0));
            title.clone().unwrap_or_default()
        } else if let Some(uri) = &content.uri {
            out.insert("@id".into(), Value::from(uri.clone()));
            if let Some(label) = &content.value {
                out.insert("o:label".into(), Value::from(label.clone()));
            }
            if let Some(lang) = &content.lang {
                out.insert("o:lang".into(), Value::from(lang.clone()));
            }
            content.value.clone().unwrap_or_else(|| uri.clone())
        } else {
            let text = content.value.clone().unwrap_or_default();
            out.insert("@value".into(), Value::from(text.clone()));
            if let Some(lang) = &content.lang {
                out.insert("@language".into(), Value::from(lang.clone()));
            }
            text
        };
        out.insert("display".into(), Value::from(display));
        Some(Value::Object(out))
    }
}

fn named(id: u64, label: Option<&String>) -> Value {
    json!({ "id": id, "label": label })
}

/// Whether the caller may see a site: the admin roles, a public site, its owner, or a holder of
/// any site permission (`Actor::sees_site`).
async fn sees_site(api: &Api<'_>, site: &omeka_db::rows::SiteRow) -> Result<bool, ApiError> {
    let has_permission = match api.actor().user() {
        Some(user) => {
            let permissions = with_db!(api.pool(), |db| SiteRepo::new(db)
                .list_permissions(site.id)
                .await)
            .map_err(db_error)?;
            access::has_site_permission(&permissions, user, SiteRole::Viewer)
        }
        None => false,
    };
    Ok(api
        .actor()
        .sees_site(site.is_public, site.owner_id, has_permission))
}
