//! Resource, item, item set and media behaviour, including the delete cascades upstream relies on
//! in Doctrine.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use chrono::NaiveDate;
use omeka_db::ids::PropertyId;
use omeka_db::repos::{
    ItemFilter, ItemRepo, ItemSetFilter, ItemSetRepo, MediaFilter, MediaRepo, Paging, ResourceRepo,
    ValueRepo,
};
use omeka_db::rows::{NewItem, NewItemSet, NewMedia, NewResource, NewValue, resource_type};
use sqlx::SqlitePool;

/// A vocabulary with one property, so values have somewhere to point.
async fn title_property(pool: &SqlitePool) -> PropertyId {
    let repo = omeka_db::repos::VocabularyRepo::new(pool);
    let vocabulary = repo
        .create(&omeka_db::rows::NewVocabulary {
            owner_id: None,
            namespace_uri: "http://purl.org/dc/terms/".to_string(),
            prefix: "dcterms".to_string(),
            label: "Dublin Core".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    omeka_db::repos::PropertyRepo::new(pool)
        .create(&omeka_db::rows::NewProperty {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: "title".to_string(),
            label: "Title".to_string(),
            comment: None,
        })
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn item_roundtrip_writes_upstream_datetime_text_and_the_item_discriminator() {
    let pool = common::memory().await;
    let created = NaiveDate::from_ymd_opt(2024, 1, 2)
        .unwrap()
        .and_hms_opt(3, 4, 5)
        .unwrap();
    let new = NewItem {
        resource: NewResource {
            title: Some("Bronze helmet".to_string()),
            created: Some(created),
            ..Default::default()
        },
        primary_media_id: None,
    };
    let item = ItemRepo::new(&pool).create(&new).await.unwrap();

    assert_eq!(item.resource.created, created);
    assert_eq!(item.resource.modified, Some(created));
    assert_eq!(item.resource.resource_type, resource_type::ITEM);

    // The stored bytes are the upstream format, which MySQL reads back unchanged.
    let stored: String = sqlx::query_scalar("SELECT created FROM resource WHERE id = ?")
        .bind(item.id().0 as i64)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, "2024-01-02 03:04:05");

    let read_back = ItemRepo::new(&pool).get(item.id()).await.unwrap().unwrap();
    assert_eq!(read_back, item);

    let mut edited = read_back;
    edited.resource.title = Some("Bronze helmet, restored".to_string());
    edited.resource.is_public = false;
    assert!(ItemRepo::new(&pool).update(&edited).await.unwrap());
    let read_back = ItemRepo::new(&pool).get(item.id()).await.unwrap().unwrap();
    assert_eq!(
        read_back.resource.title,
        Some("Bronze helmet, restored".to_string())
    );
    assert!(!read_back.resource.is_public);
    assert!(read_back.resource.modified.unwrap() > created);
    assert_eq!(
        read_back.resource.created, created,
        "update never moves created"
    );
    assert_eq!(read_back.resource.resource_type, resource_type::ITEM);
}

#[tokio::test]
async fn item_filters_and_paging_match_the_upstream_query_parameters() {
    let pool = common::memory().await;
    let owner = omeka_db::repos::UserRepo::new(&pool)
        .create(&omeka_db::rows::NewUser {
            email: "owner@example.org".to_string(),
            name: "Owner".to_string(),
            password_hash: None,
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    let items = ItemRepo::new(&pool);
    let in_set = items
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(owner.id),
                title: Some("first".to_string()),
                ..Default::default()
            },
            ..Default::default()
        })
        .await
        .unwrap();
    let private = items
        .create(&NewItem {
            resource: NewResource {
                is_public: false,
                title: Some("second".to_string()),
                ..Default::default()
            },
            ..Default::default()
        })
        .await
        .unwrap();
    let item_set = ItemSetRepo::new(&pool)
        .create(&NewItemSet::default())
        .await
        .unwrap();
    items
        .add_to_item_set(in_set.id(), item_set.id())
        .await
        .unwrap();

    let by_owner = items
        .list(
            &ItemFilter {
                owner_id: Some(owner.id),
                ..Default::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        by_owner.iter().map(|row| row.id()).collect::<Vec<_>>(),
        vec![in_set.id()]
    );

    let public = items
        .list(
            &ItemFilter {
                is_public: Some(true),
                ..Default::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        public.iter().map(|row| row.id()).collect::<Vec<_>>(),
        vec![in_set.id()]
    );

    let in_the_set = items
        .list(
            &ItemFilter {
                item_set_id: Some(item_set.id()),
                ..Default::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        in_the_set.iter().map(|row| row.id()).collect::<Vec<_>>(),
        vec![in_set.id()]
    );

    let windowed = items
        .list(&ItemFilter::default(), &Paging::new(1, 1))
        .await
        .unwrap();
    assert_eq!(windowed.len(), 1);
    assert_eq!(windowed[0].id(), private.id());

    assert_eq!(
        items.list_item_sets(in_set.id()).await.unwrap(),
        vec![item_set.id()]
    );
    assert!(
        items
            .remove_from_item_set(in_set.id(), item_set.id())
            .await
            .unwrap()
    );
    assert!(items.list_item_sets(in_set.id()).await.unwrap().is_empty());
}

#[tokio::test]
async fn deleting_a_resource_removes_its_values_and_memberships_as_upstream_does() {
    let pool = common::memory().await;
    let property_id = title_property(&pool).await;
    let items = ItemRepo::new(&pool);
    let item = items.create(&NewItem::default()).await.unwrap();
    let item_set = ItemSetRepo::new(&pool)
        .create(&NewItemSet::default())
        .await
        .unwrap();
    items
        .add_to_item_set(item.id(), item_set.id())
        .await
        .unwrap();
    let values = ValueRepo::new(&pool);
    for text in ["one", "two"] {
        values
            .create(&NewValue {
                resource_id: item.resource.id,
                property_id,
                value_resource_id: None,
                value_annotation_id: None,
                r#type: "literal".to_string(),
                lang: None,
                value: Some(text.to_string()),
                uri: None,
                is_public: true,
            })
            .await
            .unwrap();
    }

    assert!(items.delete(item.id()).await.unwrap());

    assert!(items.get(item.id()).await.unwrap().is_none());
    assert!(
        values
            .list_for_resource(item.resource.id)
            .await
            .unwrap()
            .is_empty()
    );
    let membership: i64 = sqlx::query_scalar("SELECT count(*) FROM item_item_set")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(membership, 0);
    let resource: i64 = sqlx::query_scalar("SELECT count(*) FROM resource WHERE id = ?")
        .bind(item.resource.id.0 as i64)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(resource, 0, "the item's resource row is gone");
    assert!(
        ItemSetRepo::new(&pool)
            .get(item_set.id())
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn a_value_pointing_at_a_deleted_resource_is_removed() {
    let pool = common::memory().await;
    let property_id = title_property(&pool).await;
    let items = ItemRepo::new(&pool);
    let pointing = items.create(&NewItem::default()).await.unwrap();
    let target = items.create(&NewItem::default()).await.unwrap();
    let value = ValueRepo::new(&pool)
        .create(&NewValue {
            resource_id: pointing.resource.id,
            property_id,
            value_resource_id: Some(target.resource.id),
            value_annotation_id: None,
            r#type: "resource:item".to_string(),
            lang: None,
            value: None,
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();

    assert!(items.delete(target.id()).await.unwrap());

    assert!(ValueRepo::new(&pool).get(value.id).await.unwrap().is_none());
}

#[tokio::test]
async fn deleting_an_item_set_removes_its_memberships() {
    let pool = common::memory().await;
    let items = ItemRepo::new(&pool);
    let item = items.create(&NewItem::default()).await.unwrap();
    let item_sets = ItemSetRepo::new(&pool);
    let item_set = item_sets.create(&NewItemSet::default()).await.unwrap();
    items
        .add_to_item_set(item.id(), item_set.id())
        .await
        .unwrap();

    let stamp = omeka_db::rows::HistoryStamp {
        user: None,
        source: omeka_db::rows::HistorySource::Edit,
    };
    assert!(item_sets.delete(item_set.id(), &stamp).await.unwrap());

    assert!(item_sets.get(item_set.id()).await.unwrap().is_none());
    assert!(items.list_item_sets(item.id()).await.unwrap().is_empty());
    assert!(items.get(item.id()).await.unwrap().is_some());
}

#[tokio::test]
async fn item_set_filters_narrow_by_open_and_public() {
    let pool = common::memory().await;
    let item_sets = ItemSetRepo::new(&pool);
    let open = item_sets
        .create(&NewItemSet {
            is_open: true,
            ..Default::default()
        })
        .await
        .unwrap();
    item_sets.create(&NewItemSet::default()).await.unwrap();

    let listed = item_sets
        .list(
            &ItemSetFilter {
                is_open: Some(true),
                ..Default::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        listed.iter().map(|row| row.id()).collect::<Vec<_>>(),
        vec![open.id()]
    );
    let read_back = item_sets.get(open.id()).await.unwrap().unwrap();
    assert!(read_back.is_open);
    assert_eq!(read_back.resource.resource_type, resource_type::ITEM_SET);
}

#[tokio::test]
async fn media_roundtrip_orders_by_position_and_clears_the_primary_pointer() {
    let pool = common::memory().await;
    let item = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    let media = MediaRepo::new(&pool);
    let second = media
        .create(&NewMedia {
            item_id: item.id(),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            data: Some(serde_json::json!({"original_filename": "b.jpg"})),
            media_type: Some("image/jpeg".to_string()),
            storage_id: Some("b".to_string()),
            has_original: true,
            has_thumbnails: true,
            position: Some(2),
            ..Default::default()
        })
        .await
        .unwrap();
    let first = media
        .create(&NewMedia {
            item_id: item.id(),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            media_type: Some("image/jpeg".to_string()),
            storage_id: Some("a".to_string()),
            has_original: true,
            position: Some(1),
            ..Default::default()
        })
        .await
        .unwrap();
    let unpositioned = media
        .create(&NewMedia {
            item_id: item.id(),
            ingester: "url".to_string(),
            renderer: "file".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(media.get(second.id()).await.unwrap(), Some(second.clone()));
    assert_eq!(
        second.data,
        Some(serde_json::json!({"original_filename": "b.jpg"}))
    );

    let listed = media
        .list(
            &MediaFilter {
                item_id: Some(item.id()),
                ..Default::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        listed.iter().map(|row| row.id()).collect::<Vec<_>>(),
        vec![unpositioned.id(), first.id(), second.id()],
        "upstream orders by position ASC and MySQL sorts NULL first"
    );

    let mut item = ItemRepo::new(&pool).get(item.id()).await.unwrap().unwrap();
    item.primary_media_id = Some(second.id());
    assert!(ItemRepo::new(&pool).update(&item).await.unwrap());

    assert!(media.delete(second.id()).await.unwrap());
    let after = ItemRepo::new(&pool).get(item.id()).await.unwrap().unwrap();
    assert_eq!(after.primary_media_id, None, "ON DELETE SET NULL");

    let by_ingester = media
        .list(
            &MediaFilter {
                ingester: Some("url".to_string()),
                ..Default::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        by_ingester.iter().map(|row| row.id()).collect::<Vec<_>>(),
        vec![unpositioned.id()]
    );
}

#[tokio::test]
async fn storage_id_is_unique_across_media() {
    let pool = common::memory().await;
    let item = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    let media = MediaRepo::new(&pool);
    let new = NewMedia {
        item_id: item.id(),
        ingester: "upload".to_string(),
        renderer: "file".to_string(),
        storage_id: Some("same".to_string()),
        ..Default::default()
    };
    media.create(&new).await.unwrap();
    let error = media.create(&new).await.unwrap_err();
    assert!(error.is_unique_violation(), "{error}");
}

#[tokio::test]
async fn resource_repo_reads_any_kind_and_annotations_are_resources() {
    let pool = common::memory().await;
    let item = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    let annotation = ResourceRepo::new(&pool)
        .create_annotation(&NewResource {
            title: Some("annotation".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(
        annotation.resource.resource_type,
        resource_type::VALUE_ANNOTATION
    );
    let resources = ResourceRepo::new(&pool);
    assert_eq!(
        resources.get(annotation.resource.id).await.unwrap(),
        Some(annotation.resource.clone())
    );
    assert_eq!(
        resources
            .get(item.resource.id)
            .await
            .unwrap()
            .unwrap()
            .resource_type,
        resource_type::ITEM
    );
    assert!(resources.delete(annotation.resource.id).await.unwrap());
    assert!(
        resources
            .get(annotation.resource.id)
            .await
            .unwrap()
            .is_none()
    );
    let annotation_rows: i64 = sqlx::query_scalar("SELECT count(*) FROM value_annotation")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(annotation_rows, 0);
}

#[tokio::test]
async fn deleting_a_user_clears_resource_ownership() {
    let pool = common::memory().await;
    let users = omeka_db::repos::UserRepo::new(&pool);
    let user = users
        .create(&omeka_db::rows::NewUser {
            email: "owner@example.org".to_string(),
            name: "Owner".to_string(),
            password_hash: None,
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    let item = ItemRepo::new(&pool)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(user.id),
                ..Default::default()
            },
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(users.delete(user.id).await.unwrap());

    let after = ItemRepo::new(&pool).get(item.id()).await.unwrap().unwrap();
    assert_eq!(after.resource.owner_id, None, "ON DELETE SET NULL");
}
