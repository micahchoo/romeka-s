//! Port of `application/src/Job/BatchUpdate.php`.
//!
//! Arguments: `resource`, an optional `query`, and up to three data maps:
//! `data` replaces the listed properties' values, `data_append` adds values,
//! `data_remove` deletes values that equal a listed literal. Each map keys a
//! `prefix:local_name` term to one string or an array of strings.
//!
//! Upstream passes `continueOnError` to `batchUpdate`, so one resource failing
//! does not stop the run; the port logs the failure on the job and continues.

use std::collections::BTreeMap;

use async_trait::async_trait;
use serde::Deserialize;

use omeka_db::ids::{PropertyId, ResourceId};
use omeka_db::repos::{ItemRepo, ItemSetRepo, MediaRepo, Paging, ValueRepo};
use omeka_db::rows::NewValue;

use super::query::{chunk_size, query_arg};
use super::{Job, JobContext, JobError};
use crate::terms;

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

/// How a data map changes a property's values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Replace,
    Append,
    Remove,
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
        let replace = update_entries(context, "data").await?;
        let append = update_entries(context, "data_append").await?;
        let remove = update_entries(context, "data_remove").await?;
        if replace.is_empty() && append.is_empty() && remove.is_empty() {
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

        let chunk = chunk_size(context).await?;
        for ids in ids.chunks(chunk) {
            if context.should_stop().await? {
                return Ok(());
            }
            for id in ids {
                let outcome = async {
                    apply(context, *id, &replace, Action::Replace).await?;
                    apply(context, *id, &append, Action::Append).await?;
                    apply(context, *id, &remove, Action::Remove).await
                }
                .await;
                if let Err(error) = outcome {
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

async fn update_entries(
    context: &JobContext<'_>,
    name: &str,
) -> Result<Vec<(PropertyId, Vec<String>)>, JobError> {
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

async fn apply(
    context: &JobContext<'_>,
    resource: ResourceId,
    entries: &[(PropertyId, Vec<String>)],
    action: Action,
) -> Result<(), JobError> {
    let values = ValueRepo::new(context.pool);
    for (property, literals) in entries {
        match action {
            Action::Replace => {
                for existing in values.list_for_property(resource, *property).await? {
                    values.delete(existing.id).await?;
                }
                for text in literals {
                    insert_literal(&values, resource, *property, text).await?;
                }
            }
            Action::Append => {
                for text in literals {
                    insert_literal(&values, resource, *property, text).await?;
                }
            }
            Action::Remove => {
                for existing in values.list_for_property(resource, *property).await? {
                    let current = existing.value.as_deref().or(existing.uri.as_deref());
                    if current.is_some_and(|text| literals.iter().any(|item| item == text)) {
                        values.delete(existing.id).await?;
                    }
                }
            }
        }
    }
    Ok(())
}

async fn insert_literal(
    values: &ValueRepo<'_>,
    resource: ResourceId,
    property: PropertyId,
    text: &str,
) -> Result<(), JobError> {
    values
        .create(&NewValue {
            resource_id: resource,
            property_id: property,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some(text.to_string()),
            uri: None,
            is_public: true,
        })
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::Runner;
    use crate::testing::{compose, seeded_db};
    use omeka_db::repos::ValueRepo;
    use omeka_db::rows::{NewItem, NewResource};

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
        insert_literal(&values, resource, description, "old")
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
        insert_literal(&values, resource, subject, "keep")
            .await
            .unwrap();
        insert_literal(&values, resource, subject, "drop")
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
