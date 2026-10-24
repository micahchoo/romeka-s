//! `value` behaviour: the object kinds, upstream's ordering, and the delete actions the schema
//! declares (`property` cascade, pointing resources cascade, annotation pointer set-null).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::ids::{PropertyId, ResourceId};
use omeka_db::repos::{ItemRepo, Paging, PropertyRepo, ResourceRepo, ValueRepo, VocabularyRepo};
use omeka_db::rows::{
    AnnotationTree, NewItem, NewProperty, NewResource, NewValue, NewVocabulary, ValueContent,
    ValueTree,
};
use sqlx::SqlitePool;

/// An item to hang values on, plus two properties from one vocabulary.
async fn setup(pool: &SqlitePool) -> (ResourceId, PropertyId, PropertyId) {
    let vocabulary = VocabularyRepo::new(pool)
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: "http://purl.org/dc/terms/".to_string(),
            prefix: "dcterms".to_string(),
            label: "Dublin Core".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    let properties = PropertyRepo::new(pool);
    let mut ids = Vec::new();
    for (local_name, label) in [("title", "Title"), ("description", "Description")] {
        ids.push(
            properties
                .create(&NewProperty {
                    owner_id: None,
                    vocabulary_id: vocabulary.id,
                    local_name: local_name.to_string(),
                    label: label.to_string(),
                    comment: None,
                })
                .await
                .unwrap(),
        );
    }
    let item = ItemRepo::new(pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    (item.resource.id, ids[0].id, ids[1].id)
}

#[tokio::test]
async fn value_roundtrip_keeps_the_object_kind_and_its_column() {
    let pool = common::memory().await;
    let (resource_id, title_id, _) = setup(&pool).await;
    let target = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    let values = ValueRepo::new(&pool);

    let literal = values
        .create(&NewValue {
            resource_id,
            property_id: title_id,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: Some("en".to_string()),
            value: Some("Bronze helmet".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
    assert_eq!(values.get(literal.id).await.unwrap(), Some(literal.clone()));

    let uri = values
        .create(&NewValue {
            resource_id,
            property_id: title_id,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "uri".to_string(),
            lang: None,
            value: None,
            uri: Some("http://example.org/term".to_string()),
            is_public: true,
        })
        .await
        .unwrap();
    let pointing = values
        .create(&NewValue {
            resource_id,
            property_id: title_id,
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
    assert_eq!(values.get(uri.id).await.unwrap(), Some(uri.clone()));
    assert_eq!(
        values.get(pointing.id).await.unwrap(),
        Some(pointing.clone())
    );

    let mut edited = literal.clone();
    edited.value = Some("Bronze helmet, restored".to_string());
    edited.is_public = false;
    assert!(values.update(&edited).await.unwrap());
    assert_eq!(values.get(literal.id).await.unwrap(), Some(edited));

    assert!(values.delete(uri.id).await.unwrap());
    assert!(values.get(uri.id).await.unwrap().is_none());
    assert!(!values.delete(uri.id).await.unwrap(), "already gone");
}

#[tokio::test]
async fn values_list_by_id_and_narrow_to_one_property() {
    let pool = common::memory().await;
    let (resource_id, title_id, description_id) = setup(&pool).await;
    let values = ValueRepo::new(&pool);

    let mut created = Vec::new();
    for (property_id, text) in [
        (title_id, "first"),
        (description_id, "second"),
        (title_id, "third"),
    ] {
        created.push(
            values
                .create(&NewValue {
                    resource_id,
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
                .unwrap(),
        );
    }

    let all = values.list_for_resource(resource_id).await.unwrap();
    assert_eq!(
        all.iter().map(|row| row.id).collect::<Vec<_>>(),
        created.iter().map(|row| row.id).collect::<Vec<_>>(),
        "upstream orders by id ascending"
    );
    let titles = values
        .list_for_property(resource_id, title_id)
        .await
        .unwrap();
    assert_eq!(titles.len(), 2);
    assert_eq!(titles[0].value.as_deref(), Some("first"));
    assert_eq!(titles[1].value.as_deref(), Some("third"));
}

#[tokio::test]
async fn pointing_values_are_found_with_paging() {
    let pool = common::memory().await;
    let (resource_id, title_id, _) = setup(&pool).await;
    let target = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    let values = ValueRepo::new(&pool);
    for _ in 0..2 {
        values
            .create(&NewValue {
                resource_id,
                property_id: title_id,
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
    }
    // A value pointing elsewhere is not returned.
    let other = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    values
        .create(&NewValue {
            resource_id,
            property_id: title_id,
            value_resource_id: Some(other.resource.id),
            value_annotation_id: None,
            r#type: "resource:item".to_string(),
            lang: None,
            value: None,
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();

    let found = values
        .list_pointing_at(target.resource.id, &Paging::default())
        .await
        .unwrap();
    assert_eq!(found.len(), 2);
    let windowed = values
        .list_pointing_at(target.resource.id, &Paging::new(1, 1))
        .await
        .unwrap();
    assert_eq!(windowed.len(), 1);
    assert_eq!(windowed[0].id, found[1].id);
}

#[tokio::test]
async fn deleting_a_property_removes_its_values_only() {
    let pool = common::memory().await;
    let (resource_id, title_id, description_id) = setup(&pool).await;
    let values = ValueRepo::new(&pool);
    let title = values
        .create(&NewValue {
            resource_id,
            property_id: title_id,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some("titled".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
    values
        .create(&NewValue {
            resource_id,
            property_id: description_id,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some("described".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();

    assert!(PropertyRepo::new(&pool).delete(title_id).await.unwrap());

    assert!(values.get(title.id).await.unwrap().is_none());
    let remaining = values.list_for_resource(resource_id).await.unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].value.as_deref(), Some("described"));
}

#[tokio::test]
async fn an_annotation_target_clears_when_the_annotation_is_deleted() {
    let pool = common::memory().await;
    let (resource_id, title_id, _) = setup(&pool).await;
    let annotation = ResourceRepo::new(&pool)
        .create_annotation(&NewResource {
            title: Some("a note".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    let values = ValueRepo::new(&pool);
    let value = values
        .create(&NewValue {
            resource_id,
            property_id: title_id,
            value_resource_id: None,
            value_annotation_id: Some(annotation.id()),
            r#type: "resource".to_string(),
            lang: None,
            value: None,
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
    assert_eq!(
        values
            .get(value.id)
            .await
            .unwrap()
            .unwrap()
            .value_annotation_id,
        Some(annotation.id())
    );

    assert!(
        ResourceRepo::new(&pool)
            .delete(annotation.resource.id)
            .await
            .unwrap()
    );

    let after = values.get(value.id).await.unwrap().unwrap();
    assert_eq!(after.value_annotation_id, None, "ON DELETE SET NULL");
    assert_eq!(after.r#type, "resource", "the value row itself survives");
}

fn literal(property_id: PropertyId, text: &str) -> ValueTree {
    ValueTree {
        content: ValueContent::literal(property_id, text),
        annotation: None,
    }
}

#[tokio::test]
async fn replace_all_swaps_the_values_and_drops_their_annotations_at_any_depth() {
    let pool = common::memory().await;
    let (resource_id, title_id, description_id) = setup(&pool).await;
    let values = ValueRepo::new(&pool);
    let annotated = ValueTree {
        content: ValueContent::literal(title_id, "Bronze helmet"),
        annotation: Some(AnnotationTree {
            resource: NewResource::default(),
            values: vec![ValueTree {
                content: ValueContent::literal(description_id, "Said of the helmet"),
                annotation: Some(AnnotationTree {
                    resource: NewResource::default(),
                    values: vec![literal(description_id, "Said of the remark")],
                }),
            }],
        }),
    };
    values.replace_all(resource_id, &[annotated]).await.unwrap();
    let stored = values.list_for_resource(resource_id).await.unwrap();
    assert_eq!(stored.len(), 1);
    let outer = stored[0].value_annotation_id.expect("an annotation");
    let inner_values = values.list_for_resource(outer.as_resource()).await.unwrap();
    assert_eq!(inner_values[0].value.as_deref(), Some("Said of the helmet"));
    let inner = inner_values[0]
        .value_annotation_id
        .expect("a nested annotation");

    values
        .replace_all(resource_id, &[literal(title_id, "Iron helmet")])
        .await
        .unwrap();
    let texts: Vec<_> = values
        .list_for_resource(resource_id)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.value)
        .collect();
    assert_eq!(texts, vec![Some("Iron helmet".to_string())]);
    let resources = ResourceRepo::new(&pool);
    assert!(resources.get(outer.as_resource()).await.unwrap().is_none());
    assert!(resources.get(inner.as_resource()).await.unwrap().is_none());
}

#[tokio::test]
async fn concurrent_replaces_leave_one_set_never_the_union() {
    let (pool, path) = common::file("values_concurrent_replace").await;
    let (resource_id, title_id, _) = setup(&pool).await;
    let sets: Vec<Vec<ValueTree>> = (0..8)
        .map(|writer| {
            (0..5)
                .map(|n| literal(title_id, &format!("writer {writer} value {n}")))
                .collect()
        })
        .collect();
    let mut writes = Vec::new();
    for set in sets {
        let pool = pool.clone();
        writes.push(tokio::spawn(async move {
            ValueRepo::new(&pool).replace_all(resource_id, &set).await
        }));
    }
    for write in writes {
        write.await.unwrap().unwrap();
    }
    let stored = ValueRepo::new(&pool)
        .list_for_resource(resource_id)
        .await
        .unwrap();
    assert_eq!(stored.len(), 5, "one writer's set, whole");
    let writer = stored[0]
        .value
        .as_deref()
        .unwrap()
        .split(" value")
        .next()
        .unwrap()
        .to_string();
    assert!(stored.iter().all(|row| {
        row.value
            .as_deref()
            .unwrap()
            .starts_with(&format!("{writer} value"))
    }));
    pool.close().await;
    common::remove_db(&path);
}
