//! `asset`: the round trip, the owner filter, the unique storage id and the thumbnail cascade.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::ids::UserId;
use omeka_db::repos::{AssetFilter, AssetOwner, AssetRepo, Paging, SiteRepo, UserRepo};
use omeka_db::rows::{NewAsset, NewSite, NewUser};
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

async fn an_asset(
    pool: &SqlitePool,
    owner: Option<UserId>,
    storage_id: &str,
) -> omeka_db::rows::AssetRow {
    AssetRepo::new(pool)
        .create(&NewAsset {
            owner_id: owner,
            name: "helmet.jpg".to_string(),
            media_type: "image/jpeg".to_string(),
            storage_id: storage_id.to_string(),
            extension: Some("jpg".to_string()),
            alt_text: Some("A bronze helmet".to_string()),
        })
        .await
        .unwrap()
}

async fn list(pool: &SqlitePool, filter: AssetFilter) -> Vec<omeka_db::rows::AssetRow> {
    AssetRepo::new(pool)
        .list(&filter, &Paging::default())
        .await
        .unwrap()
}

#[tokio::test]
async fn an_asset_round_trips_and_updates_its_name() {
    let pool = common::memory().await;
    let owner = a_user(&pool, "ada@example.org").await;
    let assets = AssetRepo::new(&pool);
    let asset = an_asset(&pool, Some(owner), "abc123").await;

    assert_eq!(assets.get(asset.id).await.unwrap(), Some(asset.clone()));
    assert_eq!(
        assets.get_by_storage_id("abc123").await.unwrap(),
        Some(asset.clone())
    );

    let mut renamed = asset.clone();
    renamed.name = "helmet, conserved.jpg".to_string();
    renamed.alt_text = None;
    assert!(assets.update(&renamed).await.unwrap());
    let after = assets.get(asset.id).await.unwrap().unwrap();
    assert_eq!(after.name, "helmet, conserved.jpg");
    assert_eq!(after.alt_text, None);
    assert_eq!(after.storage_id, "abc123", "the stored file id is fixed");

    assert!(assets.delete(asset.id).await.unwrap());
    assert_eq!(assets.get(asset.id).await.unwrap(), None);
    assert!(!assets.delete(asset.id).await.unwrap());
}

#[tokio::test]
async fn assets_filter_by_owner_including_the_unowned() {
    let pool = common::memory().await;
    let owner = a_user(&pool, "ada@example.org").await;
    let mine = an_asset(&pool, Some(owner), "mine").await;
    let orphan = an_asset(&pool, None, "orphan").await;

    let all = list(&pool, AssetFilter::default()).await;
    assert_eq!(
        all.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![mine.id, orphan.id]
    );

    let owned = list(
        &pool,
        AssetFilter {
            owner: Some(AssetOwner::Owned(owner)),
        },
    )
    .await;
    assert_eq!(
        owned.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![mine.id]
    );

    let unowned = list(
        &pool,
        AssetFilter {
            owner: Some(AssetOwner::Unowned),
        },
    )
    .await;
    assert_eq!(
        unowned.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![orphan.id]
    );
}

#[tokio::test]
async fn a_storage_id_is_unique_ignoring_case() {
    let pool = common::memory().await;
    an_asset(&pool, None, "abc123").await;
    let error = AssetRepo::new(&pool)
        .create(&NewAsset {
            owner_id: None,
            name: "again.jpg".to_string(),
            media_type: "image/jpeg".to_string(),
            storage_id: "ABC123".to_string(),
            extension: None,
            alt_text: None,
        })
        .await
        .unwrap_err();
    assert!(error.is_unique_violation(), "{error}");
}

#[tokio::test]
async fn deleting_an_asset_clears_a_site_thumbnail() {
    let pool = common::memory().await;
    let asset = an_asset(&pool, None, "abc123").await;
    let site = SiteRepo::new(&pool)
        .create(&NewSite {
            slug: "museum".to_string(),
            theme: "default".to_string(),
            title: "Museum".to_string(),
            thumbnail_id: Some(asset.id),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(site.thumbnail_id, Some(asset.id));

    assert!(AssetRepo::new(&pool).delete(asset.id).await.unwrap());

    assert_eq!(
        SiteRepo::new(&pool)
            .get(site.id)
            .await
            .unwrap()
            .unwrap()
            .thumbnail_id,
        None,
        "the site thumbnail is set null, as the schema declares"
    );
}
