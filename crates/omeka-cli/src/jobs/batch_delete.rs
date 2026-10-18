//! Port of `application/src/Job/BatchDelete.php`.
//!
//! Arguments: `resource` (`items`, `item_sets` or `media`), an optional `query`.
//! The job searches matching ids, then deletes them in `batch_chunk_size`
//! chunks, checking `shouldStop` between chunks.

use async_trait::async_trait;

use omeka_db::ids::{ItemId, ItemSetId, MediaId};
use omeka_db::repos::{ItemRepo, ItemSetRepo, MediaRepo, Paging};

use super::query::{chunk_size, query_arg};
use super::{Job, JobContext, JobError};

/// `Omeka\Job\BatchDelete`.
pub struct BatchDelete;

#[async_trait]
impl Job for BatchDelete {
    fn class(&self) -> &'static str {
        "Omeka\\Job\\BatchDelete"
    }

    async fn perform(&self, context: &JobContext<'_>) -> Result<(), JobError> {
        let resource = context.required_str("resource")?.to_string();
        let query = query_arg(context, "query")?;
        let chunk = chunk_size(context).await?;

        match resource.as_str() {
            "items" => {
                let ids: Vec<ItemId> = ItemRepo::new(context.pool)
                    .list(&query.item_filter(), &Paging::default())
                    .await?
                    .into_iter()
                    .map(|row| row.id())
                    .collect();
                for ids in ids.chunks(chunk) {
                    if context.should_stop().await? {
                        return Ok(());
                    }
                    for id in ids {
                        ItemRepo::new(context.pool).delete(*id).await?;
                    }
                }
                Ok(())
            }
            "item_sets" => {
                let ids: Vec<ItemSetId> = ItemSetRepo::new(context.pool)
                    .list(&query.item_set_filter(), &Paging::default())
                    .await?
                    .into_iter()
                    .map(|row| row.id())
                    .collect();
                for ids in ids.chunks(chunk) {
                    if context.should_stop().await? {
                        return Ok(());
                    }
                    for id in ids {
                        ItemSetRepo::new(context.pool).delete(*id).await?;
                    }
                }
                Ok(())
            }
            "media" => {
                let ids: Vec<MediaId> = MediaRepo::new(context.pool)
                    .list(&query.media_filter(), &Paging::default())
                    .await?
                    .into_iter()
                    .map(|row| row.id())
                    .collect();
                for ids in ids.chunks(chunk) {
                    if context.should_stop().await? {
                        return Ok(());
                    }
                    for id in ids {
                        MediaRepo::new(context.pool).delete(*id).await?;
                    }
                }
                Ok(())
            }
            other => Err(JobError::InvalidArgument(format!(
                "unsupported resource \"{other}\" for BatchDelete"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::Runner;
    use crate::testing::{compose, seeded_db};
    use omeka_db::repos::{ItemFilter, ItemRepo, Paging};
    use omeka_db::rows::{NewItem, NewResource};

    fn new_item(title: &str, is_public: bool) -> NewItem {
        NewItem {
            resource: NewResource {
                title: Some(title.to_string()),
                is_public,
                ..NewResource::default()
            },
            primary_media_id: None,
        }
    }

    #[tokio::test]
    async fn batch_delete_removes_only_the_matching_items() {
        let db = seeded_db().await;
        let items = ItemRepo::new(&db.pool);
        items.create(&new_item("public one", true)).await.unwrap();
        items.create(&new_item("public two", true)).await.unwrap();
        items.create(&new_item("private", false)).await.unwrap();

        let job_id = compose(
            &db.pool,
            &BatchDelete,
            serde_json::json!({"resource": "items", "query": {"is_public": false}}),
        )
        .await;
        Runner::new(&db.pool).run(job_id).await.expect("run");

        let remaining = items
            .list(&ItemFilter::default(), &Paging::default())
            .await
            .unwrap();
        assert_eq!(remaining.len(), 2);
        assert!(remaining.iter().all(|row| row.resource.is_public));
        assert_eq!(
            crate::store::job_status(&db.pool, job_id)
                .await
                .unwrap()
                .as_deref(),
            Some("completed")
        );
    }

    #[tokio::test]
    async fn batch_delete_rejects_an_unknown_resource() {
        let db = seeded_db().await;
        let job_id = compose(
            &db.pool,
            &BatchDelete,
            serde_json::json!({"resource": "widgets"}),
        )
        .await;
        let error = Runner::new(&db.pool)
            .run(job_id)
            .await
            .expect_err("unknown resource");
        assert!(
            matches!(error, JobError::InvalidArgument(_)),
            "got {error:?}"
        );
        assert_eq!(
            crate::store::job_status(&db.pool, job_id)
                .await
                .unwrap()
                .as_deref(),
            Some("error")
        );
    }
}
