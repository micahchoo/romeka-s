//! The one-way upgrade that moves a site's settings out of the theme blob.
//!
//! Before this change the port stored the 13 site settings inside the
//! `theme_settings_<theme>` JSON blob, so an upstream `site_setting` row was
//! never read and upstream could not read the port's. `romeka install` runs
//! this step: for every site, it moves the keys `SITE_SETTINGS` names out of
//! the blob and into their own rows, leaves the theme's own keys in the blob,
//! and drops an emptied blob. It is idempotent: a second run finds only
//! theme-own keys to keep.

use omeka_db::DbError;
use omeka_db::DbPool;
use omeka_db::repos::{Paging, SiteFilter, SiteRepo, SiteSettingRepo};
use omeka_db::with_db;
use omeka_site::settings::SITE_SETTINGS;
use serde_json::{Map, Value};

/// Move every site setting a site hides in its `theme_settings_<theme>` blob
/// into its own `site_setting` row. Returns how many rows were written.
///
/// # Errors
/// [`DbError`] for a storage failure.
pub async fn run(pool: &DbPool) -> Result<usize, DbError> {
    let mut moved = 0;
    for site in with_db!(pool, |db| SiteRepo::new(db)
        .list(&SiteFilter::default(), &Paging::default())
        .await)?
    {
        let blob_key = format!("theme_settings_{}", site.theme);
        let Some(Value::Object(blob)) = with_db!(pool, |db| SiteSettingRepo::new(db)
            .get(site.id, &blob_key)
            .await)?
        else {
            continue;
        };
        let mut kept = Map::new();
        for (name, value) in blob {
            if !is_site_setting(&name) {
                kept.insert(name, value);
                continue;
            }
            // A row already holding the key was written after the blob, so it
            // wins; the blob's copy is dropped without a write.
            if with_db!(pool, |db| SiteSettingRepo::new(db)
                .get(site.id, &name)
                .await)?
            .is_none()
            {
                with_db!(pool, |db| SiteSettingRepo::new(db)
                    .set(site.id, &name, &value)
                    .await)?;
                moved += 1;
            }
        }
        if kept.is_empty() {
            with_db!(pool, |db| SiteSettingRepo::new(db)
                .delete(site.id, &blob_key)
                .await)?;
        } else {
            with_db!(pool, |db| SiteSettingRepo::new(db)
                .set(site.id, &blob_key, &Value::Object(kept))
                .await)?;
        }
    }
    Ok(moved)
}

/// Whether a key is a site setting rather than a theme's own setting.
fn is_site_setting(key: &str) -> bool {
    SITE_SETTINGS.iter().any(|spec| spec.key == key)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use omeka_db::rows::NewSite;
    use omeka_db::{migrate, open_memory};
    use serde_json::json;

    async fn temp_db() -> sqlx::SqlitePool {
        let pool = open_memory().await.expect("open memory database");
        migrate(&pool).await.expect("run migrations");
        pool
    }

    async fn site(pool: &sqlx::SqlitePool) -> omeka_db::ids::SiteId {
        SiteRepo::new(pool)
            .create(&NewSite {
                owner_id: None,
                slug: "main".to_string(),
                theme: "default".to_string(),
                title: "Main".to_string(),
                ..NewSite::default()
            })
            .await
            .expect("the site row is writable")
            .id
    }

    /// A site's blob splits: site settings become rows, the theme's own keys
    /// stay in the blob, and a second run moves nothing more.
    #[tokio::test]
    async fn blobs_split_into_rows_once() {
        let db = temp_db().await;
        let id = site(&db).await;
        let repo = SiteSettingRepo::new(&db);
        repo.set(
            id,
            "theme_settings_default",
            &json!({
                "pagination_per_page": 50,
                "browse_defaults_public_items": {"sort_by": "title", "sort_order": "asc"},
                "logo": 7
            }),
        )
        .await
        .expect("the blob is writable");

        let moved = run(&DbPool::from(db.clone()))
            .await
            .expect("the upgrade runs");
        assert_eq!(moved, 2);
        assert_eq!(
            repo.get(id, "pagination_per_page").await.unwrap(),
            Some(json!(50))
        );
        // The theme's own key stays, under the blob.
        assert_eq!(
            repo.get(id, "theme_settings_default").await.unwrap(),
            Some(json!({"logo": 7}))
        );

        // Idempotent: nothing left to move, and no key is rewritten.
        assert_eq!(
            run(&DbPool::from(db.clone()))
                .await
                .expect("the upgrade reruns"),
            0
        );
    }

    /// A blob that is only site settings is removed once they are rows.
    #[tokio::test]
    async fn an_all_site_settings_blob_is_removed() {
        let db = temp_db().await;
        let id = site(&db).await;
        let repo = SiteSettingRepo::new(&db);
        repo.set(
            id,
            "theme_settings_default",
            &json!({"locale": "es", "show_page_pagination": false}),
        )
        .await
        .unwrap();

        assert_eq!(run(&DbPool::from(db.clone())).await.unwrap(), 2);
        assert_eq!(repo.get(id, "theme_settings_default").await.unwrap(), None);
        assert_eq!(repo.get(id, "locale").await.unwrap(), Some(json!("es")));
    }
}
