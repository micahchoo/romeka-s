//! Migration behaviour: idempotency on one database, and durability across a restart.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::ids::PropertyId;
use omeka_db::migrate;
use omeka_db::repos::{ItemRepo, MediaRepo, PropertyRepo, ValueRepo, VocabularyRepo};
use omeka_db::rows::{NewItem, NewMedia, NewProperty, NewValue, NewVocabulary};
use sqlx::{Row, SqlitePool};

/// The schema as a comparable value: every table, index and trigger definition, sorted.
async fn schema(pool: &SqlitePool) -> Vec<(String, String, Option<String>)> {
    let rows = sqlx::query(
        "SELECT type, name, sql FROM sqlite_master
         WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    rows.iter()
        .map(|row| (row.get("type"), row.get("name"), row.get("sql")))
        .collect()
}

async fn migration_rows(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn migrating_twice_leaves_the_schema_unchanged() {
    let pool = common::memory().await;
    let first = schema(&pool).await;
    let applied = migration_rows(&pool).await;
    assert_eq!(applied, 1, "one migration file");

    migrate(&pool).await.unwrap();

    assert_eq!(schema(&pool).await, first);
    assert_eq!(migration_rows(&pool).await, applied);
}

#[tokio::test]
async fn a_file_database_reopens_with_the_same_rows() {
    let (pool, path) = common::file("reopen").await;

    let vocabulary = VocabularyRepo::new(&pool)
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: "http://purl.org/dc/terms/".to_string(),
            prefix: "dcterms".to_string(),
            label: "Dublin Core".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    let property = PropertyRepo::new(&pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: "title".to_string(),
            label: "Title".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    let item = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    let value = ValueRepo::new(&pool)
        .create(&NewValue {
            resource_id: item.resource.id,
            property_id: property.id,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some("Bronze helmet".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
    let media = MediaRepo::new(&pool)
        .create(&NewMedia {
            item_id: item.id(),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            media_type: Some("image/jpeg".to_string()),
            storage_id: Some("abc123".to_string()),
            has_original: true,
            ..Default::default()
        })
        .await
        .unwrap();

    pool.close().await;

    let pool = omeka_db::open_file(&path).await.unwrap();
    migrate(&pool).await.unwrap();

    assert_eq!(
        VocabularyRepo::new(&pool).get(vocabulary.id).await.unwrap(),
        Some(vocabulary)
    );
    assert_eq!(
        PropertyRepo::new(&pool).get(property.id).await.unwrap(),
        Some(property)
    );
    assert_eq!(
        ItemRepo::new(&pool).get(item.id()).await.unwrap(),
        Some(item)
    );
    assert_eq!(
        ValueRepo::new(&pool).get(value.id).await.unwrap(),
        Some(value)
    );
    assert_eq!(
        MediaRepo::new(&pool).get(media.id()).await.unwrap(),
        Some(media)
    );

    pool.close().await;
    common::remove_db(&path);
}

#[tokio::test]
async fn a_file_database_is_in_wal_mode_with_foreign_keys_on() {
    let (pool, path) = common::file("pragmas").await;
    let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(journal, "wal");
    let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(foreign_keys, 1);
    pool.close().await;
    common::remove_db(&path);
}

#[tokio::test]
async fn a_value_is_never_orphaned_from_its_property() {
    // The foreign key is ON for every connection the pool opens, not just the first.
    let pool = common::memory().await;
    let item = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    let error = ValueRepo::new(&pool)
        .create(&NewValue {
            resource_id: item.resource.id,
            property_id: PropertyId::from(4242),
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some("no such property".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap_err();
    assert!(error.is_foreign_key_violation(), "{error}");
}
