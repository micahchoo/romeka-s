//! `POST /api/romeka/bulk {query, change}`: check, resolve the selection once, and write the run.
//!
//! The answer is what Apply will do: the frozen count, how many items already hold the change,
//! how many the caller may not update, and five items that change, before and after.

use omeka_db::DbPool;
use omeka_db::ids::PropertyId;
use omeka_db::repos::{
    BulkRepo, ClassRepo, ItemSetRepo, ResourceRepo, SiteRepo, TemplateRepo, UserRepo,
};
use omeka_db::rows::{BulkStatus, NewBulk, ValueContent};
use omeka_db::with_db;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::BulkError;
use super::access::{denied, holds};
use super::change::{Assign, BulkChange, BulkEdit};
use super::scope::{Scope, select};
use super::values::ValueEdits;
use super::view::Fields;
use crate::errors::ApiError;
use crate::read::Api;
use crate::write::resource::not_found;

/// How many items that change the preview shows.
const SAMPLES: usize = 5;

const ITEM_CLASS: &str = "Omeka\\Entity\\Item";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Body {
    query: Value,
    change: Value,
}

/// The preview's answer.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub id: u64,
    pub status: &'static str,
    /// The items the run will visit: the selection, frozen.
    pub total: usize,
    /// Items that already hold the change, which the run leaves untouched.
    pub unchanged: usize,
    /// Items the caller may not update, which the run records as failed.
    pub denied: usize,
    pub samples: Vec<Sample>,
}

#[derive(Debug, Serialize)]
pub struct Sample {
    pub id: u64,
    pub title: Option<String>,
    pub before: Fields,
    pub after: Fields,
    /// Each field whose values change, with its values before and after.
    pub values: Vec<FieldChange>,
}

#[derive(Debug, Serialize)]
pub struct FieldChange {
    pub property: u64,
    /// The property's `prefix:localName`, by which the admin finds its label in the reader's
    /// language.
    pub term: String,
    /// The property's label, which the admin shows as the field's name.
    pub label: String,
    pub before: Vec<ShownValue>,
    pub after: Vec<ShownValue>,
}

/// One value as a preview line shows it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShownValue {
    /// The text, a URI's label, or the title of a linked resource.
    pub text: Option<String>,
    pub lang: Option<String>,
    pub uri: Option<String>,
    /// The linked resource.
    pub resource: Option<u64>,
    pub is_public: bool,
}

/// The fields among `properties` whose values differ before and after, in words.
async fn field_changes(
    api: &Api<'_>,
    properties: &[PropertyId],
    before: &[ValueContent],
    after: &[ValueContent],
) -> Result<Vec<FieldChange>, BulkError> {
    let mut changes = Vec::new();
    for property in properties {
        let of = |values: &[ValueContent]| -> Vec<ValueContent> {
            values
                .iter()
                .filter(|value| value.property_id == *property)
                .cloned()
                .collect()
        };
        let (was, now) = (of(before), of(after));
        if was == now {
            continue;
        }
        let info = api.catalog().property(*property);
        changes.push(FieldChange {
            property: property.0,
            term: info.map(|info| info.term.clone()).unwrap_or_default(),
            label: info.map(|info| info.label.clone()).unwrap_or_default(),
            before: shown(api, &was).await?,
            after: shown(api, &now).await?,
        });
    }
    Ok(changes)
}

async fn shown(api: &Api<'_>, values: &[ValueContent]) -> Result<Vec<ShownValue>, BulkError> {
    let mut shown = Vec::with_capacity(values.len());
    for value in values {
        let text = match value.value_resource_id {
            Some(linked) => with_db!(api.pool(), |db| ResourceRepo::new(db).get(linked).await)?
                .filter(|row| api.sees(row.is_public, row.owner_id))
                .and_then(|row| row.title),
            None => value.value.clone(),
        };
        shown.push(ShownValue {
            text,
            lang: value.lang.clone(),
            uri: value.uri.clone(),
            resource: value.value_resource_id.map(|id| id.0),
            is_public: value.is_public,
        });
    }
    Ok(shown)
}

/// Preview a change and write the run, `previewed`.
///
/// # Errors
/// 400 for a body that does not parse, 403 without the batch privilege the query needs or the
/// right to assign items to a named site, 404 for a named row that does not exist.
pub async fn preview(api: &Api<'_>, body: &[u8]) -> Result<Preview, BulkError> {
    let body: Body =
        serde_json::from_slice(body).map_err(|error| unreadable(&error.to_string()))?;
    let scope = Scope::of(&body.query)?;
    if !holds(api.actor(), scope.right()) {
        return Err(denied(scope.right()));
    }
    let change = BulkChange::parse(&body.change).map_err(|reason| unreadable(&reason))?;
    let values = check(api, &change).await?;
    let BulkChange::Edit(edit) = &change;
    let items = select(api, &body.query).await?;
    let described = with_db!(api.pool(), |db| BulkRepo::new(db).described(&items).await)?;
    let properties = values
        .as_ref()
        .map(ValueEdits::properties)
        .unwrap_or_default();
    let mut held = with_db!(api.pool(), |db| BulkRepo::new(db)
        .values(&items, &properties)
        .await)?;
    let mut preview = Preview {
        id: 0,
        status: BulkStatus::Previewed.as_str(),
        total: items.len(),
        unchanged: 0,
        denied: 0,
        samples: Vec::new(),
    };
    for item in described {
        let after = edit.apply(&item.fields);
        let before_values = held.remove(&item.item).unwrap_or_default();
        let after_values = match &values {
            Some(values) => values.plan(&before_values).applied(&before_values),
            None => before_values.clone(),
        };
        if api
            .authorize_resource(ITEM_CLASS, item.fields.owner)
            .is_err()
        {
            preview.denied += 1;
        } else if after == item.fields && after_values == before_values {
            preview.unchanged += 1;
        } else if preview.samples.len() < SAMPLES {
            preview.samples.push(Sample {
                id: item.item.0,
                title: item.title,
                before: Fields::of(&item.fields),
                after: Fields::of(&after),
                values: field_changes(api, &properties, &before_values, &after_values).await?,
            });
        }
    }
    let row = with_db!(api.pool(), |db| BulkRepo::new(db)
        .create(&NewBulk {
            owner_id: api.actor().user(),
            kind: change.kind(),
            query: body.query.clone(),
            change: change.to_json(),
            items: items.clone(),
        })
        .await)?;
    preview.id = row.id.0;
    Ok(preview)
}

/// Every row the change names exists and the caller sees it, the caller may change owners when
/// the change sets one, and may assign items to each site it names (`can-assign-items`, which
/// the item write checks too). Answers the change's value edits with each value to add as the
/// item `PATCH` stores it.
///
/// Upstream changes an owner only for a caller with `change-owner` on the entity
/// (`AbstractEntityAdapter`), which only the administrator roles hold. The permissions table has
/// no such action yet, so the administrator test stands for it.
///
/// # Errors
/// 404 for a missing row or an item set the caller cannot see, 403 for an owner change by a
/// caller who is not an administrator or a site the caller may not assign items to.
pub(super) async fn check(
    api: &Api<'_>,
    change: &BulkChange,
) -> Result<Option<ValueEdits<ValueContent>>, BulkError> {
    let BulkChange::Edit(edit) = change;
    let pool = api.pool();
    if let Some(owner) = edit.owner {
        if !api.actor().is_admin() {
            return Err(BulkError::Api(ApiError::PermissionDenied {
                message: crate::messages::permission_denied("change-owner", ITEM_CLASS),
            }));
        }
        if with_db!(pool, |db| UserRepo::new(db).get(owner).await)?.is_none() {
            return Err(missing("Omeka\\Entity\\User", owner.0));
        }
    }
    if let Some(Assign::Set(class)) = edit.class
        && with_db!(pool, |db| ClassRepo::new(db).get(class).await)?.is_none()
    {
        return Err(missing("Omeka\\Entity\\ResourceClass", class.0));
    }
    if let Some(Assign::Set(template)) = edit.template
        && with_db!(pool, |db| TemplateRepo::new(db).get(template).await)?.is_none()
    {
        return Err(missing("Omeka\\Entity\\ResourceTemplate", template.0));
    }
    for set in edit.item_sets.add().iter().chain(edit.item_sets.remove()) {
        let row = with_db!(pool, |db| ItemSetRepo::new(db).get(*set).await)?;
        if !row.is_some_and(|row| api.sees(row.resource.is_public, row.resource.owner_id)) {
            return Err(missing("Omeka\\Entity\\ItemSet", set.0));
        }
    }
    check_sites(api, pool, edit).await?;
    match &edit.values {
        Some(values) => Ok(Some(super::values::resolve(api, values).await?)),
        None => Ok(None),
    }
}

async fn check_sites(api: &Api<'_>, pool: &DbPool, edit: &BulkEdit) -> Result<(), BulkError> {
    for site in edit.sites.add().iter().chain(edit.sites.remove()) {
        let Some(row) = with_db!(pool, |db| SiteRepo::new(db).get(*site).await)? else {
            return Err(missing("Omeka\\Entity\\Site", site.0));
        };
        if !crate::write::items::can_assign(api, &row).await? {
            return Err(BulkError::Api(ApiError::PermissionDenied {
                message: crate::messages::permission_denied(
                    "can-assign-items",
                    "Omeka\\Entity\\Site",
                ),
            }));
        }
    }
    Ok(())
}

fn missing(class: &str, id: u64) -> BulkError {
    BulkError::Api(not_found(class, &Value::from(id)))
}

fn unreadable(reason: &str) -> BulkError {
    BulkError::Api(ApiError::bad_request(
        crate::messages::import_request_unreadable(reason),
    ))
}
