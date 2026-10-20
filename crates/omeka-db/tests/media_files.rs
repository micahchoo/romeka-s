//! The file lookup behind `/files` access: which media owns a storage id, and
//! the visibility of that media and of its item.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::repos::{ItemRepo, MediaFileOwner, MediaRepo, UserRepo};
use omeka_db::rows::{NewItem, NewMedia, NewResource, NewUser};

#[tokio::test]
async fn a_storage_id_names_its_media_and_both_visibilities() {
    let pool = common::memory().await;
    let owner = UserRepo::new(&pool)
        .create(&NewUser {
            email: "owner@example.org".to_string(),
            name: "Owner".to_string(),
            password_hash: None,
            role: "author".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    let item = ItemRepo::new(&pool)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(owner.id),
                is_public: false,
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap();
    let media = MediaRepo::new(&pool)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: None,
                is_public: true,
                ..NewResource::default()
            },
            item_id: item.id(),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            storage_id: Some("abc123".to_string()),
            extension: Some("jpg".to_string()),
            ..NewMedia::default()
        })
        .await
        .unwrap();

    let repo = MediaRepo::new(&pool);
    assert_eq!(
        repo.file_owner("abc123").await.unwrap(),
        Some(MediaFileOwner {
            media_id: media.id(),
            extension: Some("jpg".to_string()),
            media_is_public: true,
            media_owner_id: None,
            item_is_public: false,
            item_owner_id: Some(owner.id),
        })
    );
    assert_eq!(repo.file_owner("missing").await.unwrap(), None);
}

/// The same lookup against a real server. `storage_id` is `utf8mb4_unicode_ci` there, as
/// upstream's unique key is, so a case variant still names the row.
#[cfg(feature = "mysql")]
#[tokio::test]
async fn a_storage_id_names_its_media_on_mysql() {
    use sqlx::MySql;

    let Some(db) = common::mysql().await else {
        return;
    };
    let item = ItemRepo::<MySql>::new(&db.pool)
        .create(&NewItem {
            resource: NewResource {
                is_public: true,
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .expect("create item");
    let media = MediaRepo::<MySql>::new(&db.pool)
        .create(&NewMedia {
            resource: NewResource {
                is_public: false,
                ..NewResource::default()
            },
            item_id: item.id(),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            storage_id: Some("abc123".to_string()),
            extension: None,
            ..NewMedia::default()
        })
        .await
        .expect("create media");

    let repo = MediaRepo::<MySql>::new(&db.pool);
    let expected = Some(MediaFileOwner {
        media_id: media.id(),
        extension: None,
        media_is_public: false,
        media_owner_id: None,
        item_is_public: true,
        item_owner_id: None,
    });
    assert_eq!(repo.file_owner("abc123").await.expect("lookup"), expected);
    assert_eq!(repo.file_owner("ABC123").await.expect("lookup"), expected);
    assert_eq!(repo.file_owner("missing").await.expect("lookup"), None);

    db.cleanup().await;
}
