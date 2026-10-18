//! Port of `application/src/Job/UpdateSiteItems.php`.
//!
//! Arguments: `action` (`add`, `replace`, `remove`, `remove_all`) and `sites`, an
//! object keyed by site id whose values are item queries (or null for all
//! items). `replace` and `remove_all` clear the site's item assignments first;
//! `add` and `replace` then attach the matching items; `remove` detaches them.
//! Every site id is checked to exist before any change, as upstream validates
//! the whole argument before writing.

use std::collections::BTreeMap;

use async_trait::async_trait;
use serde_json::Value;

use omeka_db::ids::{ItemId, SiteId};
use omeka_db::repos::{ItemRepo, Paging, SiteRepo};

use super::query::BatchQuery;
use super::{Job, JobContext, JobError};
use crate::store;

/// The valid actions (`UpdateSiteItems::$actions`).
const ACTIONS: [&str; 4] = ["add", "replace", "remove", "remove_all"];

/// Upstream chunks item ids at 1000 (`UpdateSiteItems::updateSiteItems`).
const ITEM_CHUNK: usize = 1000;

/// `Omeka\Job\UpdateSiteItems`.
pub struct UpdateSiteItems;

#[async_trait]
impl Job for UpdateSiteItems {
    fn class(&self) -> &'static str {
        "Omeka\\Job\\UpdateSiteItems"
    }

    async fn perform(&self, context: &JobContext<'_>) -> Result<(), JobError> {
        let action = context.required_str("action")?.to_string();
        if !ACTIONS.contains(&action.as_str()) {
            return Err(JobError::InvalidArgument(format!(
                "invalid \"action\" string \"{action}\" passed to the UpdateSiteItems job"
            )));
        }
        let sites_value = context.arg("sites").ok_or_else(|| {
            JobError::InvalidArgument(
                "no \"sites\" array passed to the UpdateSiteItems job".to_string(),
            )
        })?;
        let sites = parse_sites(context, sites_value).await?;
        for (site_id, query) in &sites {
            update_site(context, &action, *site_id, query).await?;
        }
        Ok(())
    }
}

async fn parse_sites(
    context: &JobContext<'_>,
    value: &Value,
) -> Result<BTreeMap<u64, BatchQuery>, JobError> {
    let sites_missing = || {
        JobError::InvalidArgument(
            "no \"sites\" array passed to the UpdateSiteItems job".to_string(),
        )
    };
    let object: BTreeMap<String, Value> =
        serde_json::from_value(value.clone()).map_err(|_| sites_missing())?;

    let mut sites = BTreeMap::new();
    for (key, query) in object {
        let site_id: u64 = key.parse().map_err(|_| {
            JobError::InvalidArgument(format!(
                "invalid site ID \"{key}\" passed to the UpdateSiteItems job"
            ))
        })?;
        if SiteRepo::new(context.pool)
            .get(SiteId::from(site_id))
            .await?
            .is_none()
        {
            return Err(JobError::InvalidArgument(format!(
                "invalid site ID \"{key}\" passed to the UpdateSiteItems job"
            )));
        }
        let query = if query.is_object() {
            serde_json::from_value(query)
                .map_err(|error| JobError::InvalidArgument(format!("site \"{key}\": {error}")))?
        } else {
            BatchQuery::default()
        };
        sites.insert(site_id, query);
    }
    Ok(sites)
}

async fn update_site(
    context: &JobContext<'_>,
    action: &str,
    site_id: u64,
    query: &BatchQuery,
) -> Result<(), JobError> {
    let ids: Vec<ItemId> = ItemRepo::new(context.pool)
        .list(&query.item_filter(), &Paging::default())
        .await?
        .into_iter()
        .map(|row| row.id())
        .collect();
    let site = SiteId::from(site_id);

    if action == "replace" || action == "remove_all" {
        store::item_site_clear(context.pool, site).await?;
    }
    if action == "add" || action == "replace" {
        for chunk in ids.chunks(ITEM_CHUNK) {
            if context.should_stop().await? {
                return Ok(());
            }
            for id in chunk {
                store::item_site_add(context.pool, *id, site).await?;
            }
        }
    }
    if action == "remove" {
        for chunk in ids.chunks(ITEM_CHUNK) {
            if context.should_stop().await? {
                return Ok(());
            }
            for id in chunk {
                store::item_site_remove(context.pool, *id, site).await?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::Runner;
    use crate::testing::{compose, seeded_db};
    use omeka_db::repos::SiteRepo;
    use omeka_db::rows::{NewItem, NewResource, NewSite};

    async fn count(site: SiteId, pool: &sqlx::SqlitePool) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM item_site WHERE site_id = ?")
            .bind(site.0 as i64)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn update_site_items_adds_then_clears_assignments() {
        let db = seeded_db().await;
        let items = ItemRepo::new(&db.pool);
        for title in ["one", "two", "three"] {
            items
                .create(&NewItem {
                    resource: NewResource {
                        title: Some(title.to_string()),
                        ..NewResource::default()
                    },
                    primary_media_id: None,
                })
                .await
                .unwrap();
        }
        let site = SiteRepo::new(&db.pool)
            .create(&NewSite {
                slug: "gallery".to_string(),
                title: "Gallery".to_string(),
                ..NewSite::default()
            })
            .await
            .unwrap();

        let sites = serde_json::json!({ site.id.0.to_string(): null });
        let job_id = compose(
            &db.pool,
            &UpdateSiteItems,
            serde_json::json!({"action": "add", "sites": sites}),
        )
        .await;
        Runner::new(&db.pool).run(job_id).await.expect("run");
        assert_eq!(count(site.id, &db.pool).await, 3);
        assert_eq!(
            crate::store::job_status(&db.pool, job_id)
                .await
                .unwrap()
                .as_deref(),
            Some("completed")
        );

        let clear_id = compose(
            &db.pool,
            &UpdateSiteItems,
            serde_json::json!({"action": "remove_all", "sites": sites}),
        )
        .await;
        Runner::new(&db.pool).run(clear_id).await.expect("run");
        assert_eq!(count(site.id, &db.pool).await, 0);
    }

    #[tokio::test]
    async fn update_site_items_rejects_an_unknown_site() {
        let db = seeded_db().await;
        let job_id = compose(
            &db.pool,
            &UpdateSiteItems,
            serde_json::json!({"action": "add", "sites": {"424242": null}}),
        )
        .await;
        let error = Runner::new(&db.pool)
            .run(job_id)
            .await
            .expect_err("unknown site");
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
