//! `user` and `api_key` behaviour: case-insensitive email identity, filters, and the delete
//! cascade onto keys.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use chrono::NaiveDate;
use omeka_db::repos::{ApiKeyRepo, Paging, SiteRepo, UserFilter, UserRepo};
use omeka_db::rows::{NewApiKey, NewSite, NewUser};

fn new_user(email: &str) -> NewUser {
    NewUser {
        email: email.to_string(),
        name: "Curator".to_string(),
        password_hash: Some("$2y$10$abcdefghijklmnopqrstuv".to_string()),
        role: "editor".to_string(),
        is_active: true,
        created: None,
    }
}

#[tokio::test]
async fn user_roundtrip_and_case_insensitive_email() {
    let pool = common::memory().await;
    let users = UserRepo::new(&pool);
    let user = users
        .create(&new_user("Curator@Example.org"))
        .await
        .unwrap();
    assert_eq!(users.get(user.id).await.unwrap(), Some(user.clone()));
    assert_eq!(
        users.get_by_email("curator@example.org").await.unwrap(),
        Some(user.clone()),
        "email is unique case-insensitively, as utf8mb4_unicode_ci is"
    );

    let mut edited = user.clone();
    edited.name = "Curator (renamed)".to_string();
    edited.is_active = false;
    assert!(users.update(&edited).await.unwrap());
    let after = users.get(user.id).await.unwrap().unwrap();
    assert_eq!(after.name, "Curator (renamed)");
    assert!(!after.is_active);
    assert_eq!(after.created, user.created, "update never moves created");
    assert!(after.modified.unwrap() >= user.created);
}

#[tokio::test]
async fn email_is_unique_even_when_the_case_differs() {
    let pool = common::memory().await;
    let users = UserRepo::new(&pool);
    users
        .create(&new_user("Curator@Example.org"))
        .await
        .unwrap();
    let error = users
        .create(&new_user("curator@example.org"))
        .await
        .unwrap_err();
    assert!(error.is_unique_violation(), "{error}");
}

#[tokio::test]
async fn user_filters_mirror_the_adapter_query_parameters() {
    let pool = common::memory().await;
    let users = UserRepo::new(&pool);
    let editor = users.create(&new_user("editor@example.org")).await.unwrap();
    let mut admin = new_user("admin@example.org");
    admin.role = "global_admin".to_string();
    admin.is_active = false;
    let admin = users.create(&admin).await.unwrap();

    let active = users
        .list(
            &UserFilter {
                is_active: Some(true),
                ..Default::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        active.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![editor.id]
    );

    let role = users
        .list(
            &UserFilter {
                role: Some("global_admin".to_string()),
                ..Default::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        role.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![admin.id]
    );

    let site = SiteRepo::new(&pool)
        .create(&NewSite::default())
        .await
        .unwrap();
    SiteRepo::new(&pool)
        .set_permission(site.id, admin.id, "editor")
        .await
        .unwrap();
    let with_permission = users
        .list(
            &UserFilter {
                site_id: Some(site.id),
                ..Default::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        with_permission.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![admin.id],
        "the site_id filter is user x site_permission"
    );
}

#[tokio::test]
async fn api_key_roundtrip_touch_and_delete() {
    let pool = common::memory().await;
    let user = UserRepo::new(&pool)
        .create(&new_user("keyholder@example.org"))
        .await
        .unwrap();
    let keys = ApiKeyRepo::new(&pool);
    let key = keys
        .create(&NewApiKey {
            id: "0123456789abcdef0123456789abcdef".to_string(),
            owner_id: user.id,
            label: "Script".to_string(),
            credential_hash: "$2y$10$abcdefghijklmnopqrstuv".to_string(),
            last_ip: None,
            last_accessed: None,
            created: None,
        })
        .await
        .unwrap();
    assert_eq!(keys.get(&key.id).await.unwrap(), Some(key.clone()));
    assert_eq!(
        keys.list_for_owner(user.id).await.unwrap(),
        vec![key.clone()]
    );

    let at = NaiveDate::from_ymd_opt(2024, 5, 6)
        .unwrap()
        .and_hms_opt(7, 8, 9)
        .unwrap();
    assert!(
        keys.touch(&key.id, Some(&[127, 0, 0, 1]), Some(at))
            .await
            .unwrap()
    );
    let touched = keys.get(&key.id).await.unwrap().unwrap();
    assert_eq!(touched.last_ip, Some(vec![127, 0, 0, 1]));
    assert_eq!(touched.last_accessed, Some(at));

    assert!(keys.touch(&key.id, None, None).await.unwrap());
    let touched = keys.get(&key.id).await.unwrap().unwrap();
    assert_eq!(touched.last_ip, None);
    assert!(touched.last_accessed.unwrap() >= key.created);

    assert!(keys.delete(&key.id).await.unwrap());
    assert!(keys.get(&key.id).await.unwrap().is_none());
    assert!(!keys.delete(&key.id).await.unwrap(), "already gone");
}

#[tokio::test]
async fn deleting_a_user_removes_their_api_keys() {
    let pool = common::memory().await;
    let user = UserRepo::new(&pool)
        .create(&new_user("keyholder@example.org"))
        .await
        .unwrap();
    let other = UserRepo::new(&pool)
        .create(&new_user("other@example.org"))
        .await
        .unwrap();
    let keys = ApiKeyRepo::new(&pool);
    let mut ids = Vec::new();
    for (owner, suffix) in [(user.id, "aa"), (other.id, "bb")] {
        ids.push(
            keys.create(&NewApiKey {
                id: format!("0123456789abcdef0123456789abc{suffix}"),
                owner_id: owner,
                label: "Script".to_string(),
                credential_hash: "$2y$10$abcdefghijklmnopqrstuv".to_string(),
                last_ip: None,
                last_accessed: None,
                created: None,
            })
            .await
            .unwrap(),
        );
    }

    assert!(UserRepo::new(&pool).delete(user.id).await.unwrap());

    assert!(keys.get(&ids[0].id).await.unwrap().is_none());
    assert!(keys.get(&ids[1].id).await.unwrap().is_some());
}
