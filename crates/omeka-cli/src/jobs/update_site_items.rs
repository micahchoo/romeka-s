//! Port of `application/src/Job/UpdateSiteItems.php`.
//!
//! Arguments: `action` (`add`, `replace`, `remove`, `remove_all`) and `sites`, an
//! object keyed by site id whose values are item queries (or anything else for
//! all items). Every site id is checked to exist before any change, as upstream
//! validates the whole argument before writing. The change itself is
//! `omeka_api::site_items`, the module the admin's `POST /api/sites/{id}/items`
//! runs, so the job and the API select and write the same rows. `remove_all` is
//! a `replace` with an empty selection.
//!
//! Upstream grants `view-all` so the search sees private items, because a job
//! may have no owner. The job reads as a global administrator for the same
//! reason.

use std::collections::BTreeMap;

use async_trait::async_trait;
use serde_json::Value;

use omeka_api::auth::Actor;
use omeka_api::jsonld::BaseUrl;
use omeka_api::read::Api;
use omeka_api::site_items::{self, Action, Command, Mode, Selector};
use omeka_db::ids::{SiteId, UserId};
use omeka_db::repos::SiteRepo;

use super::{Job, JobContext, JobError};

/// `Omeka\Job\UpdateSiteItems`.
pub struct UpdateSiteItems;

#[async_trait]
impl Job for UpdateSiteItems {
    fn class(&self) -> &'static str {
        "Omeka\\Job\\UpdateSiteItems"
    }

    async fn perform(&self, context: &JobContext<'_>) -> Result<(), JobError> {
        let raw_action = context.required_str("action")?.to_string();
        let (action, clears) = match raw_action.as_str() {
            "add" => (Action::Add, false),
            "replace" => (Action::Replace, false),
            "remove" => (Action::Remove, false),
            "remove_all" => (Action::Replace, true),
            _ => {
                return Err(JobError::InvalidArgument(format!(
                    "invalid \"action\" string \"{raw_action}\" passed to the UpdateSiteItems job"
                )));
            }
        };
        let sites_value = context.arg("sites").ok_or_else(|| {
            JobError::InvalidArgument(
                "no \"sites\" array passed to the UpdateSiteItems job".to_string(),
            )
        })?;
        let sites = parse_sites(context, sites_value).await?;
        let base =
            BaseUrl::parse(JOB_BASE_URL).map_err(|error| JobError::Runtime(error.to_string()))?;
        let api = Api::load(context.pool, base, Actor::new(UserId(0), "global_admin"))
            .await
            .map_err(api_error)?;
        for (site_id, query) in sites {
            let selector = if clears {
                Selector::Items(Vec::new())
            } else {
                Selector::Query(query)
            };
            let command = Command { action, selector };
            site_items::apply(&api, SiteId::from(site_id), &command, Mode::Write)
                .await
                .map_err(api_error)?;
        }
        Ok(())
    }
}

/// The read surface's base URL. The change writes rows and serves no document, so no `@id`
/// built from it leaves the job.
const JOB_BASE_URL: &str = "http://localhost";

fn api_error(error: omeka_api::errors::ApiError) -> JobError {
    JobError::Runtime(error.to_string())
}

async fn parse_sites(
    context: &JobContext<'_>,
    value: &Value,
) -> Result<BTreeMap<u64, Value>, JobError> {
    let sites_missing = || {
        JobError::InvalidArgument(
            "no \"sites\" array passed to the UpdateSiteItems job".to_string(),
        )
    };
    let object: BTreeMap<String, Value> =
        serde_json::from_value(value.clone()).map_err(|_| sites_missing())?;

    let mut sites = BTreeMap::new();
    for (key, query) in object {
        let invalid = || {
            JobError::InvalidArgument(format!(
                "invalid site ID \"{key}\" passed to the UpdateSiteItems job"
            ))
        };
        let site_id: u64 = key.parse().map_err(|_| invalid())?;
        if SiteRepo::new(context.pool)
            .get(SiteId::from(site_id))
            .await?
            .is_none()
        {
            return Err(invalid());
        }
        sites.insert(site_id, query);
    }
    Ok(sites)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::Runner;
    use crate::testing::{compose, seeded_db};
    use omeka_db::repos::ItemRepo;
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
        for (title, is_public) in [("one", true), ("two", true), ("three", false)] {
            items
                .create(&NewItem {
                    resource: NewResource {
                        title: Some(title.to_string()),
                        is_public,
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

        let public = serde_json::json!({ site.id.0.to_string(): {"is_public": "1"} });
        let replace_id = compose(
            &db.pool,
            &UpdateSiteItems,
            serde_json::json!({"action": "replace", "sites": public}),
        )
        .await;
        Runner::new(&db.pool).run(replace_id).await.expect("run");
        assert_eq!(
            count(site.id, &db.pool).await,
            2,
            "the query selects the items the items list would"
        );
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
