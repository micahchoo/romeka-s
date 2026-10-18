//! `setting`, `site_setting` and `user_setting`: get, set, list, delete and the key's scope.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::ids::{SiteId, UserId};
use omeka_db::repos::{SettingRepo, SiteRepo, SiteSettingRepo, UserRepo, UserSettingRepo};
use omeka_db::rows::{NewSite, NewUser};
use serde_json::json;
use sqlx::SqlitePool;

async fn a_site(pool: &SqlitePool, slug: &str) -> SiteId {
    SiteRepo::new(pool)
        .create(&NewSite {
            slug: slug.to_string(),
            theme: "default".to_string(),
            title: slug.to_string(),
            ..Default::default()
        })
        .await
        .unwrap()
        .id
}

async fn a_user(pool: &SqlitePool, email: &str) -> UserId {
    UserRepo::new(pool)
        .create(&NewUser {
            email: email.to_string(),
            name: email.to_string(),
            password_hash: None,
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn a_global_setting_round_trips_and_replaces() {
    let pool = common::memory().await;
    let settings = SettingRepo::new(&pool);

    assert_eq!(settings.get("administrator_email").await.unwrap(), None);
    settings
        .set("administrator_email", &json!("ada@example.org"))
        .await
        .unwrap();
    assert_eq!(
        settings.get("administrator_email").await.unwrap(),
        Some(json!("ada@example.org"))
    );

    settings
        .set("administrator_email", &json!("grace@example.org"))
        .await
        .unwrap();
    assert_eq!(
        settings.get("administrator_email").await.unwrap(),
        Some(json!("grace@example.org")),
        "a second set replaces the value"
    );

    settings
        .set("pagination_per_page", &json!(50))
        .await
        .unwrap();
    let listed = settings.list().await.unwrap();
    assert_eq!(
        listed.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
        vec!["administrator_email", "pagination_per_page"],
        "list is in key order"
    );

    assert!(settings.delete("administrator_email").await.unwrap());
    assert!(!settings.delete("administrator_email").await.unwrap());
    assert_eq!(settings.get("administrator_email").await.unwrap(), None);
}

#[tokio::test]
async fn a_setting_key_matches_ignoring_case() {
    let pool = common::memory().await;
    let settings = SettingRepo::new(&pool);
    settings
        .set("Administrator_Email", &json!("ada@example.org"))
        .await
        .unwrap();
    assert_eq!(
        settings.get("administrator_email").await.unwrap(),
        Some(json!("ada@example.org")),
        "the id column folds case, as upstream's unique key does"
    );
}

#[tokio::test]
async fn a_site_setting_is_scoped_to_its_site() {
    let pool = common::memory().await;
    let settings = SiteSettingRepo::new(&pool);
    let museum = a_site(&pool, "museum").await;
    let library = a_site(&pool, "library").await;

    settings
        .set(museum, "default_locale", &json!("en"))
        .await
        .unwrap();
    settings
        .set(library, "default_locale", &json!("fr"))
        .await
        .unwrap();
    settings.set(museum, "per_page", &json!(20)).await.unwrap();

    assert_eq!(
        settings.get(museum, "default_locale").await.unwrap(),
        Some(json!("en"))
    );
    assert_eq!(
        settings.get(library, "default_locale").await.unwrap(),
        Some(json!("fr"))
    );
    assert_eq!(settings.get(library, "per_page").await.unwrap(), None);
    assert_eq!(settings.list(museum).await.unwrap().len(), 2);
    assert_eq!(settings.list(library).await.unwrap().len(), 1);

    assert!(settings.delete(museum, "default_locale").await.unwrap());
    assert_eq!(settings.get(museum, "default_locale").await.unwrap(), None);
    assert_eq!(
        settings.get(library, "default_locale").await.unwrap(),
        Some(json!("fr")),
        "deleting one site's row leaves the other site's"
    );
}

#[tokio::test]
async fn a_user_setting_is_scoped_to_its_user() {
    let pool = common::memory().await;
    let settings = UserSettingRepo::new(&pool);
    let ada = a_user(&pool, "ada@example.org").await;
    let grace = a_user(&pool, "grace@example.org").await;

    settings
        .set(ada, "default_locale", &json!("en"))
        .await
        .unwrap();
    settings
        .set(grace, "default_locale", &json!("fr"))
        .await
        .unwrap();

    assert_eq!(
        settings.get(ada, "default_locale").await.unwrap(),
        Some(json!("en"))
    );
    assert_eq!(
        settings.get(grace, "default_locale").await.unwrap(),
        Some(json!("fr"))
    );
    assert_eq!(settings.list(ada).await.unwrap().len(), 1);
    assert!(settings.delete(ada, "default_locale").await.unwrap());
    assert_eq!(settings.get(ada, "default_locale").await.unwrap(), None);
    assert_eq!(
        settings.get(grace, "default_locale").await.unwrap(),
        Some(json!("fr"))
    );
}

#[tokio::test]
async fn a_setting_for_a_missing_parent_is_refused() {
    let pool = common::memory().await;
    let user_error = UserSettingRepo::new(&pool)
        .set(UserId(404), "default_locale", &json!("en"))
        .await
        .unwrap_err();
    assert!(
        user_error.is_foreign_key_violation(),
        "user_setting needs its user: {user_error}"
    );

    let site_error = SiteSettingRepo::new(&pool)
        .set(SiteId(404), "default_locale", &json!("en"))
        .await
        .unwrap_err();
    assert!(
        site_error.is_foreign_key_violation(),
        "site_setting needs its site: {site_error}"
    );
}

#[tokio::test]
async fn deleting_a_parent_removes_its_settings() {
    let pool = common::memory().await;
    let user = a_user(&pool, "ada@example.org").await;
    let site = a_site(&pool, "museum").await;
    UserSettingRepo::new(&pool)
        .set(user, "default_locale", &json!("en"))
        .await
        .unwrap();
    SiteSettingRepo::new(&pool)
        .set(site, "default_locale", &json!("en"))
        .await
        .unwrap();

    assert!(UserRepo::new(&pool).delete(user).await.unwrap());
    assert!(SiteRepo::new(&pool).delete(site).await.unwrap());

    assert_eq!(
        UserSettingRepo::new(&pool)
            .get(user, "default_locale")
            .await
            .unwrap(),
        None,
        "the user_setting cascaded"
    );
    assert_eq!(
        SiteSettingRepo::new(&pool)
            .get(site, "default_locale")
            .await
            .unwrap(),
        None,
        "the site_setting cascaded"
    );
}
