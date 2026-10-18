//! `resource_template` and `resource_template_property` behaviour: label uniqueness, property
//! order, the JSON data-type list, and the delete actions the schema declares.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::ids::PropertyId;
use omeka_db::repos::{ItemRepo, PropertyRepo, TemplateRepo, VocabularyRepo};
use omeka_db::rows::{
    NewItem, NewProperty, NewResource, NewTemplate, NewTemplateProperty, NewVocabulary,
};
use sqlx::SqlitePool;

/// One vocabulary with `count` properties and their ids.
async fn properties(pool: &SqlitePool, count: usize) -> Vec<PropertyId> {
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
    let repo = PropertyRepo::new(pool);
    let mut ids = Vec::new();
    for index in 0..count {
        ids.push(
            repo.create(&NewProperty {
                owner_id: None,
                vocabulary_id: vocabulary.id,
                local_name: format!("property{index}"),
                label: format!("Property {index}"),
                comment: None,
            })
            .await
            .unwrap()
            .id,
        );
    }
    ids
}

fn new_template(label: &str) -> NewTemplate {
    NewTemplate {
        owner_id: None,
        resource_class_id: None,
        title_property_id: None,
        description_property_id: None,
        label: label.to_string(),
    }
}

#[tokio::test]
async fn template_roundtrip_and_label_uniqueness() {
    let pool = common::memory().await;
    let templates = TemplateRepo::new(&pool);
    let template = templates.create(&new_template("Base")).await.unwrap();
    assert_eq!(
        templates.get(template.id).await.unwrap(),
        Some(template.clone())
    );
    assert_eq!(
        templates.find_by_label("Base").await.unwrap(),
        Some(template.clone())
    );

    let mut renamed = template.clone();
    renamed.label = "Base (renamed)".to_string();
    assert!(templates.update(&renamed).await.unwrap());
    assert_eq!(
        templates.get(template.id).await.unwrap().unwrap().label,
        "Base (renamed)"
    );
    assert_eq!(templates.list().await.unwrap().len(), 1);

    let error = templates
        .create(&new_template("BASE (RENAMED)"))
        .await
        .unwrap_err();
    assert!(error.is_unique_violation(), "{error}: labels are unique");
}

#[tokio::test]
async fn template_properties_order_by_position_as_mysql_sorts_them() {
    let pool = common::memory().await;
    let property_ids = properties(&pool, 3).await;
    let template = TemplateRepo::new(&pool)
        .create(&new_template("Ordered"))
        .await
        .unwrap();
    let templates = TemplateRepo::new(&pool);
    let mut created = Vec::new();
    for (index, position) in [(0usize, Some(2i64)), (1, Some(1)), (2, None)] {
        created.push(
            templates
                .add_property(&NewTemplateProperty {
                    resource_template_id: template.id,
                    property_id: property_ids[index],
                    alternate_label: Some(format!("Alt {index}")),
                    alternate_comment: None,
                    position,
                    data_type: Some(vec!["literal".to_string(), "uri".to_string()]),
                    is_required: index == 0,
                    is_private: false,
                    default_lang: None,
                })
                .await
                .unwrap(),
        );
    }

    let listed = templates.list_properties(template.id).await.unwrap();
    assert_eq!(
        listed.iter().map(|row| row.property_id).collect::<Vec<_>>(),
        vec![property_ids[2], property_ids[1], property_ids[0]],
        "position ascending with MySQL's NULL-first order for the unpositioned row"
    );
    assert_eq!(
        listed[0].data_type,
        Some(vec!["literal".to_string(), "uri".to_string()])
    );
    let stored: String =
        sqlx::query_scalar("SELECT data_type FROM resource_template_property WHERE id = ?")
            .bind(created[0].id.0 as i64)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored, r#"["literal","uri"]"#, "upstream json_array text");

    let mut edited = created[2].clone();
    edited.position = Some(0);
    edited.is_private = true;
    assert!(templates.update_property(&edited).await.unwrap());
    let listed = templates.list_properties(template.id).await.unwrap();
    assert_eq!(listed[0].id, edited.id);
    assert!(listed[0].is_private);

    assert!(templates.delete_property(created[0].id).await.unwrap());
    assert_eq!(
        templates.list_properties(template.id).await.unwrap().len(),
        2
    );
}

#[tokio::test]
async fn the_same_property_cannot_be_added_twice() {
    let pool = common::memory().await;
    let property_ids = properties(&pool, 1).await;
    let template = TemplateRepo::new(&pool)
        .create(&new_template("Unique"))
        .await
        .unwrap();
    let add = |position| NewTemplateProperty {
        resource_template_id: template.id,
        property_id: property_ids[0],
        alternate_label: None,
        alternate_comment: None,
        position,
        data_type: None,
        is_required: false,
        is_private: false,
        default_lang: None,
    };
    TemplateRepo::new(&pool)
        .add_property(&add(Some(1)))
        .await
        .unwrap();
    let error = TemplateRepo::new(&pool)
        .add_property(&add(Some(2)))
        .await
        .unwrap_err();
    assert!(error.is_unique_violation(), "{error}");
}

#[tokio::test]
async fn deleting_a_template_removes_its_properties_and_clears_resources() {
    let pool = common::memory().await;
    let property_ids = properties(&pool, 1).await;
    let templates = TemplateRepo::new(&pool);
    let template = templates.create(&new_template("Doomed")).await.unwrap();
    let property = templates
        .add_property(&NewTemplateProperty {
            resource_template_id: template.id,
            property_id: property_ids[0],
            alternate_label: None,
            alternate_comment: None,
            position: None,
            data_type: None,
            is_required: false,
            is_private: false,
            default_lang: None,
        })
        .await
        .unwrap();
    let item = ItemRepo::new(&pool)
        .create(&NewItem {
            resource: NewResource {
                resource_template_id: Some(template.id),
                ..Default::default()
            },
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(templates.delete(template.id).await.unwrap());

    assert!(templates.get(template.id).await.unwrap().is_none());
    assert!(
        templates
            .list_properties(template.id)
            .await
            .unwrap()
            .is_empty()
    );
    let after = ItemRepo::new(&pool).get(item.id()).await.unwrap().unwrap();
    assert_eq!(
        after.resource.resource_template_id, None,
        "ON DELETE SET NULL keeps the item"
    );
    assert!(
        PropertyRepo::new(&pool)
            .get(property.property_id)
            .await
            .unwrap()
            .is_some()
    );
}
