//! `password_creation`: the key lookup, the one-key-per-user rule and the user cascade.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::ids::UserId;
use omeka_db::repos::{PasswordCreationRepo, UserRepo};
use omeka_db::rows::{NewPasswordCreation, NewUser};
use sqlx::SqlitePool;

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
async fn a_password_creation_round_trips_by_key_and_user() {
    let pool = common::memory().await;
    let user = a_user(&pool, "ada@example.org").await;
    let keys = PasswordCreationRepo::new(&pool);

    assert_eq!(keys.get("key-1").await.unwrap(), None);
    let created = keys
        .create(&NewPasswordCreation {
            id: "key-1".to_string(),
            user_id: user,
            activate: true,
            created: None,
        })
        .await
        .unwrap();
    assert!(created.activate);

    assert_eq!(keys.get("key-1").await.unwrap(), Some(created.clone()));
    assert_eq!(keys.get_for_user(user).await.unwrap(), Some(created));

    assert!(keys.delete("key-1").await.unwrap());
    assert_eq!(keys.get("key-1").await.unwrap(), None);
    assert_eq!(keys.get_for_user(user).await.unwrap(), None);
    assert!(!keys.delete("key-1").await.unwrap());
}

#[tokio::test]
async fn a_user_holds_one_key_at_a_time() {
    let pool = common::memory().await;
    let user = a_user(&pool, "ada@example.org").await;
    let keys = PasswordCreationRepo::new(&pool);
    keys.create(&NewPasswordCreation {
        id: "key-1".to_string(),
        user_id: user,
        activate: false,
        created: None,
    })
    .await
    .unwrap();

    let error = keys
        .create(&NewPasswordCreation {
            id: "key-2".to_string(),
            user_id: user,
            activate: true,
            created: None,
        })
        .await
        .unwrap_err();
    assert!(error.is_unique_violation(), "{error}");

    assert!(keys.delete_for_user(user).await.unwrap());
    keys.create(&NewPasswordCreation {
        id: "key-2".to_string(),
        user_id: user,
        activate: true,
        created: None,
    })
    .await
    .unwrap();
    assert_eq!(
        keys.get_for_user(user).await.unwrap().unwrap().id,
        "key-2",
        "the freed slot takes the new key"
    );
}

#[tokio::test]
async fn deleting_a_user_removes_their_key() {
    let pool = common::memory().await;
    let user = a_user(&pool, "ada@example.org").await;
    PasswordCreationRepo::new(&pool)
        .create(&NewPasswordCreation {
            id: "key-1".to_string(),
            user_id: user,
            activate: true,
            created: None,
        })
        .await
        .unwrap();

    assert!(UserRepo::new(&pool).delete(user).await.unwrap());

    assert_eq!(
        PasswordCreationRepo::new(&pool).get("key-1").await.unwrap(),
        None,
        "the key cascaded with the user"
    );
}

/// Using a key sets the password once, and leaves alone a column an admin changed meanwhile.
#[tokio::test]
async fn a_key_sets_one_password_and_keeps_the_other_columns() {
    let pool = common::memory().await;
    let user = a_user(&pool, "ada@example.org").await;
    let keys = PasswordCreationRepo::new(&pool);
    keys.create(&NewPasswordCreation {
        id: "key-1".to_string(),
        user_id: user,
        activate: true,
        created: None,
    })
    .await
    .unwrap();
    let users = UserRepo::new(&pool);
    let mut demoted = users.get(user).await.unwrap().unwrap();
    demoted.role = "researcher".to_string();
    demoted.is_active = false;
    users.update(&demoted).await.unwrap();

    assert!(
        keys.consume("key-1", user, Some("hash-1"), None)
            .await
            .unwrap()
    );
    assert!(
        !keys
            .consume("key-1", user, Some("hash-2"), Some(true))
            .await
            .unwrap()
    );

    let stored = users.get(user).await.unwrap().unwrap();
    assert_eq!(stored.password_hash.as_deref(), Some("hash-1"));
    assert_eq!(stored.role, "researcher");
    assert!(!stored.is_active);
    assert_eq!(keys.get("key-1").await.unwrap(), None);
}
