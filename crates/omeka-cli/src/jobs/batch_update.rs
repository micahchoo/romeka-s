//! Port of `application/src/Job/BatchUpdate.php`.
//!
//! Arguments: `resource`, an optional `query`, and up to three data maps:
//! `data` replaces the listed properties' values, `data_append` adds values,
//! `data_remove` deletes values that equal a listed literal. Each map keys a
//! `prefix:local_name` term to one string or an array of strings.
//!
//! Upstream passes `continueOnError` to `batchUpdate`, so one resource failing
//! does not stop the run; the port logs the failure on the job and continues.
//!
//! One resource's edit is atomic. The three maps apply in upstream's order
//! (replace, append, remove) to the values read before the edit, and the deletes
//! and inserts that result commit in one transaction, so a failed insert keeps
//! the values a replace would have deleted. An edited item then joins every site
//! that keeps in sync with its item pool and whose pool selects it now but did not
//! before the edit, as an API update does (`omeka_api::site_items`).

use std::collections::BTreeMap;

use async_trait::async_trait;
use serde::Deserialize;

use omeka_api::read::Api;
use omeka_api::site_items;
use omeka_db::ids::{ItemId, PropertyId, ResourceId, ValueId};
use omeka_db::repos::{ItemRepo, ItemSetRepo, MediaRepo, Paging, ValueRepo};
use omeka_db::rows::{ValueContent, ValueRow};

use super::query::{chunk_size, query_arg};
use super::{Job, JobContext, JobError};
use crate::{system_api, terms};

/// One map value: a single literal or a list of them.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum LiteralList {
    One(String),
    Many(Vec<String>),
}

impl LiteralList {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(text) => vec![text],
            Self::Many(list) => list,
        }
    }
}

/// One map: each property with its literals.
type Entries = Vec<(PropertyId, Vec<String>)>;

/// The job's three maps, the edit every selected resource receives.
struct Edit {
    replace: Entries,
    append: Entries,
    remove: Entries,
}

/// The rows one resource's edit deletes and inserts.
struct ValuePlan {
    delete: Vec<ValueId>,
    insert: Vec<ValueContent>,
}

impl Edit {
    fn is_empty(&self) -> bool {
        self.replace.is_empty() && self.append.is_empty() && self.remove.is_empty()
    }

    /// Apply replace, then append, then remove to `existing`. A remove also drops a literal an
    /// earlier map would have inserted, as upstream's third `batchUpdate` sees the second's values.
    fn plan(&self, existing: &[ValueRow]) -> ValuePlan {
        let mut kept: Vec<&ValueRow> = existing.iter().collect();
        let mut delete = Vec::new();
        let mut insert: Vec<(PropertyId, &str)> = Vec::new();
        for (property, literals) in &self.replace {
            kept.retain(|value| {
                let drops = value.property_id == *property;
                if drops {
                    delete.push(value.id);
                }
                !drops
            });
            insert.extend(literals.iter().map(|text| (*property, text.as_str())));
        }
        for (property, literals) in &self.append {
            insert.extend(literals.iter().map(|text| (*property, text.as_str())));
        }
        for (property, literals) in &self.remove {
            let listed = |text: &str| literals.iter().any(|item| item == text);
            kept.retain(|value| {
                let current = value.value.as_deref().or(value.uri.as_deref());
                let drops = value.property_id == *property && current.is_some_and(listed);
                if drops {
                    delete.push(value.id);
                }
                !drops
            });
            insert.retain(|(at, text)| !(at == property && listed(text)));
        }
        ValuePlan {
            delete,
            insert: insert
                .into_iter()
                .map(|(property, text)| ValueContent::literal(property, text))
                .collect(),
        }
    }
}

/// `Omeka\Job\BatchUpdate`.
pub struct BatchUpdate;

#[async_trait]
impl Job for BatchUpdate {
    fn class(&self) -> &'static str {
        "Omeka\\Job\\BatchUpdate"
    }

    async fn perform(&self, context: &JobContext<'_>) -> Result<(), JobError> {
        let resource = context.required_str("resource")?.to_string();
        let query = query_arg(context, "query")?;
        let edit = Edit {
            replace: update_entries(context, "data").await?,
            append: update_entries(context, "data_append").await?,
            remove: update_entries(context, "data_remove").await?,
        };
        if edit.is_empty() {
            return Ok(());
        }

        let ids: Vec<ResourceId> = match resource.as_str() {
            "items" => item_ids(context, &query).await?,
            "item_sets" => item_set_ids(context, &query).await?,
            "media" => media_ids(context, &query).await?,
            other => {
                return Err(JobError::InvalidArgument(format!(
                    "unsupported resource \"{other}\" for BatchUpdate"
                )));
            }
        };

        // Only items belong to sites, so only an item edit runs the sync.
        let sync = match resource.as_str() {
            "items" => Some(system_api::load(context.pool).await?),
            _ => None,
        };
        let chunk = chunk_size(context).await?;
        for ids in ids.chunks(chunk) {
            if context.should_stop().await? {
                return Ok(());
            }
            for id in ids {
                if let Err(error) = update_one(context, sync.as_ref(), *id, &edit).await {
                    context.log(&format!("resource {}: {error}", id.0)).await?;
                }
            }
        }
        Ok(())
    }
}

async fn item_ids(
    context: &JobContext<'_>,
    query: &super::query::BatchQuery,
) -> Result<Vec<ResourceId>, JobError> {
    Ok(ItemRepo::new(context.pool)
        .list(&query.item_filter(), &Paging::default())
        .await?
        .into_iter()
        .map(|row| row.resource.id)
        .collect())
}

async fn item_set_ids(
    context: &JobContext<'_>,
    query: &super::query::BatchQuery,
) -> Result<Vec<ResourceId>, JobError> {
    Ok(ItemSetRepo::new(context.pool)
        .list(&query.item_set_filter(), &Paging::default())
        .await?
        .into_iter()
        .map(|row| row.resource.id)
        .collect())
}

async fn media_ids(
    context: &JobContext<'_>,
    query: &super::query::BatchQuery,
) -> Result<Vec<ResourceId>, JobError> {
    Ok(MediaRepo::new(context.pool)
        .list(&query.media_filter(), &Paging::default())
        .await?
        .into_iter()
        .map(|row| row.resource.id)
        .collect())
}

async fn update_entries(context: &JobContext<'_>, name: &str) -> Result<Entries, JobError> {
    let Some(value) = context.arg(name) else {
        return Ok(Vec::new());
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let map: BTreeMap<String, LiteralList> = serde_json::from_value(value.clone())
        .map_err(|error| JobError::InvalidArgument(format!("argument \"{name}\": {error}")))?;

    let mut entries = Vec::with_capacity(map.len());
    for (term, literals) in map {
        let property = terms::resolve_property(context.pool, &term)
            .await
            .map_err(JobError::InvalidArgument)?;
        entries.push((property, literals.into_vec()));
    }
    Ok(entries)
}

/// Edit one resource in one transaction. `sync` is the read surface for an item, whose synced
/// sites are read before the write and attached after its commit.
async fn update_one(
    context: &JobContext<'_>,
    sync: Option<&Api<'_>>,
    resource: ResourceId,
    edit: &Edit,
) -> Result<(), JobError> {
    let values = ValueRepo::new(context.pool);
    let plan = edit.plan(&values.list_for_resource(resource).await?);
    if plan.delete.is_empty() && plan.insert.is_empty() {
        return Ok(());
    }
    let item = ItemId(resource.0);
    let synced_before = match sync {
        Some(api) => site_items::synced_sites_matching(api, item).await?,
        None => Vec::new(),
    };
    values.rewrite(resource, &plan.delete, &plan.insert).await?;
    if let Some(api) = sync {
        site_items::attach_new_matches(api, item, &synced_before).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::Runner;
    use crate::testing::{compose, refuse_value, seeded_db, site_items, synced_site};
    use omeka_db::repos::ValueRepo;
    use omeka_db::rows::{NewItem, NewResource};

    async fn item_with(pool: &sqlx::SqlitePool, title: &str, values: &[(&str, &str)]) -> ItemId {
        let item = ItemRepo::new(pool)
            .create(&NewItem {
                resource: NewResource {
                    title: Some(title.to_string()),
                    ..NewResource::default()
                },
                primary_media_id: None,
            })
            .await
            .unwrap();
        for (term, text) in values {
            let property = crate::terms::resolve_property(pool, term).await.unwrap();
            ValueRepo::new(pool)
                .create(&ValueContent::literal(property, *text).on(item.resource.id))
                .await
                .unwrap();
        }
        item.id()
    }

    async fn texts(pool: &sqlx::SqlitePool, item: ItemId) -> Vec<String> {
        ValueRepo::new(pool)
            .list_for_resource(ResourceId(item.0))
            .await
            .unwrap()
            .into_iter()
            .filter_map(|value| value.value)
            .collect()
    }

    async fn run(pool: &sqlx::SqlitePool, args: serde_json::Value) -> omeka_db::ids::JobId {
        let job_id = compose(pool, &BatchUpdate, args).await;
        Runner::new(pool).run(job_id).await.expect("run");
        job_id
    }

    #[tokio::test]
    async fn a_replace_whose_insert_fails_keeps_the_old_values() {
        let db = seeded_db().await;
        let item = item_with(&db.pool, "Doc", &[("dcterms:description", "old")]).await;
        refuse_value(&db.pool, "refused").await;

        let job_id = run(
            &db.pool,
            serde_json::json!({
                "resource": "items",
                "data": {"dcterms:description": ["new", "refused"]}
            }),
        )
        .await;

        assert_eq!(texts(&db.pool, item).await, ["old"]);
        let log = crate::store::job_get(&db.pool, job_id)
            .await
            .unwrap()
            .and_then(|job| job.log)
            .unwrap_or_default();
        assert!(
            log.contains(&format!("resource {}", item.0)),
            "the failure is logged on the job: {log}"
        );
    }

    #[tokio::test]
    async fn a_batch_update_attaches_only_items_it_makes_match_a_synced_pool() {
        let db = seeded_db().await;
        let featured = serde_json::json!({"property": [
            {"property": "dcterms:subject", "type": "eq", "text": "featured"}
        ]});
        let site = synced_site(&db.pool, "featured", featured).await;
        let newcomer = item_with(&db.pool, "Newcomer", &[]).await;
        let removed_by_hand =
            item_with(&db.pool, "Removed", &[("dcterms:subject", "featured")]).await;
        assert!(site_items(&db.pool, site).await.is_empty());

        run(
            &db.pool,
            serde_json::json!({
                "resource": "items",
                "data_append": {"dcterms:subject": "featured"}
            }),
        )
        .await;

        assert_eq!(
            site_items(&db.pool, site).await,
            [newcomer],
            "the item the edit made match joins; {removed_by_hand:?}, which matched before, stays out"
        );
    }

    #[tokio::test]
    async fn batch_update_replaces_and_appends_values() {
        let db = seeded_db().await;
        let item = ItemRepo::new(&db.pool)
            .create(&NewItem {
                resource: NewResource {
                    title: Some("Doc".to_string()),
                    ..NewResource::default()
                },
                primary_media_id: None,
            })
            .await
            .unwrap();
        let resource = item.resource.id;
        let values = ValueRepo::new(&db.pool);
        let description = crate::terms::resolve_property(&db.pool, "dcterms:description")
            .await
            .unwrap();
        values
            .create(&ValueContent::literal(description, "old").on(resource))
            .await
            .unwrap();

        let job_id = compose(
            &db.pool,
            &BatchUpdate,
            serde_json::json!({
                "resource": "items",
                "data": {"dcterms:description": ["new"]},
                "data_append": {"dcterms:subject": ["one", "two"]}
            }),
        )
        .await;
        Runner::new(&db.pool).run(job_id).await.expect("run");

        let stored = values.list_for_resource(resource).await.unwrap();
        let description_texts: Vec<&str> = stored
            .iter()
            .filter(|value| value.property_id == description)
            .filter_map(|value| value.value.as_deref())
            .collect();
        assert_eq!(description_texts, ["new"]);

        let subject = crate::terms::resolve_property(&db.pool, "dcterms:subject")
            .await
            .unwrap();
        let subject_texts: Vec<&str> = stored
            .iter()
            .filter(|value| value.property_id == subject)
            .filter_map(|value| value.value.as_deref())
            .collect();
        assert_eq!(subject_texts, ["one", "two"]);
        assert_eq!(
            crate::store::job_status(&db.pool, job_id)
                .await
                .unwrap()
                .as_deref(),
            Some("completed")
        );
    }

    #[tokio::test]
    async fn batch_update_removes_only_listed_literals() {
        let db = seeded_db().await;
        let item = ItemRepo::new(&db.pool)
            .create(&NewItem {
                resource: NewResource {
                    title: Some("Doc".to_string()),
                    ..NewResource::default()
                },
                primary_media_id: None,
            })
            .await
            .unwrap();
        let resource = item.resource.id;
        let values = ValueRepo::new(&db.pool);
        let subject = crate::terms::resolve_property(&db.pool, "dcterms:subject")
            .await
            .unwrap();
        values
            .create(&ValueContent::literal(subject, "keep").on(resource))
            .await
            .unwrap();
        values
            .create(&ValueContent::literal(subject, "drop").on(resource))
            .await
            .unwrap();

        let job_id = compose(
            &db.pool,
            &BatchUpdate,
            serde_json::json!({
                "resource": "items",
                "data_remove": {"dcterms:subject": "drop"}
            }),
        )
        .await;
        Runner::new(&db.pool).run(job_id).await.expect("run");

        let stored = values.list_for_resource(resource).await.unwrap();
        let remaining: Vec<&str> = stored
            .iter()
            .filter_map(|value| value.value.as_deref())
            .collect();
        assert_eq!(remaining, ["keep"]);
    }
}
