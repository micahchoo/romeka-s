//! Vocabulary, class and property behaviour, against upstream's constraint set.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::ids::VocabularyId;
use omeka_db::repos::{
    ClassFilter, ClassRepo, Paging, PropertyFilter, PropertyRepo, VocabularyFilter, VocabularyRepo,
};
use omeka_db::rows::{NewClass, NewProperty, NewVocabulary};

fn dcterms() -> NewVocabulary {
    NewVocabulary {
        owner_id: None,
        namespace_uri: "http://purl.org/dc/terms/".to_string(),
        prefix: "dcterms".to_string(),
        label: "Dublin Core".to_string(),
        comment: Some("Dublin Core Metadata Initiative terms".to_string()),
    }
}

#[tokio::test]
async fn vocabulary_class_and_property_roundtrip() {
    let pool = common::memory().await;
    let vocabularies = VocabularyRepo::new(&pool);
    let vocabulary = vocabularies.create(&dcterms()).await.unwrap();
    assert_eq!(vocabulary.id, VocabularyId::from(1));
    assert_eq!(
        vocabularies.get(vocabulary.id).await.unwrap(),
        Some(vocabulary.clone())
    );
    assert_eq!(
        vocabularies.get_by_prefix("dcterms").await.unwrap(),
        Some(vocabulary.clone())
    );

    let classes = ClassRepo::new(&pool);
    let class = classes
        .create(&NewClass {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: "Image".to_string(),
            label: "Image".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    let properties = PropertyRepo::new(&pool);
    let property = properties
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: "title".to_string(),
            label: "Title".to_string(),
            comment: None,
        })
        .await
        .unwrap();

    assert_eq!(
        properties.find(vocabulary.id, "title").await.unwrap(),
        Some(property.clone())
    );
    assert_eq!(
        classes.find(vocabulary.id, "Image").await.unwrap(),
        Some(class.clone())
    );

    let mut renamed = property.clone();
    renamed.label = "Title (renamed)".to_string();
    assert!(properties.update(&renamed).await.unwrap());
    assert_eq!(
        properties.get(property.id).await.unwrap().unwrap().label,
        "Title (renamed)"
    );

    let listed = properties
        .list(
            &PropertyFilter {
                vocabulary_id: Some(vocabulary.id),
                ..Default::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    let listed = properties
        .list(
            &PropertyFilter {
                vocabulary_id: Some(vocabulary.id),
                owner_id: Some(omeka_db::ids::UserId::from(99)),
                ..Default::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert!(listed.is_empty());
}

#[tokio::test]
async fn deleting_a_vocabulary_removes_its_classes_and_properties() {
    let pool = common::memory().await;
    let vocabulary = VocabularyRepo::new(&pool).create(&dcterms()).await.unwrap();
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

    assert!(
        VocabularyRepo::new(&pool)
            .delete(vocabulary.id)
            .await
            .unwrap()
    );

    assert!(
        VocabularyRepo::new(&pool)
            .get(vocabulary.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        PropertyRepo::new(&pool)
            .get(property.id)
            .await
            .unwrap()
            .is_none()
    );
    let leftover: i64 = sqlx::query_scalar("SELECT count(*) FROM resource_class")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(leftover, 0);
}

#[tokio::test]
async fn prefix_and_namespace_are_unique() {
    let pool = common::memory().await;
    let vocabularies = VocabularyRepo::new(&pool);
    vocabularies.create(&dcterms()).await.unwrap();

    let mut duplicate = dcterms();
    duplicate.namespace_uri = "http://example.org/dcterms/".to_string();
    let error = vocabularies.create(&duplicate).await.unwrap_err();
    assert!(error.is_unique_violation(), "{error}");

    let mut duplicate = dcterms();
    duplicate.prefix = "other".to_string();
    let error = vocabularies.create(&duplicate).await.unwrap_err();
    assert!(error.is_unique_violation(), "{error}");
}

#[tokio::test]
async fn local_name_is_unique_within_a_vocabulary() {
    let pool = common::memory().await;
    let vocabulary = VocabularyRepo::new(&pool).create(&dcterms()).await.unwrap();
    let properties = PropertyRepo::new(&pool);
    let new = NewProperty {
        owner_id: None,
        vocabulary_id: vocabulary.id,
        local_name: "title".to_string(),
        label: "Title".to_string(),
        comment: None,
    };
    properties.create(&new).await.unwrap();
    let error = properties.create(&new).await.unwrap_err();
    assert!(error.is_unique_violation(), "{error}");
    // `local_name` is `utf8mb4_unicode_ci` upstream, so the key is case-insensitive.
    let error = properties
        .create(&NewProperty {
            local_name: "TITLE".to_string(),
            ..new.clone()
        })
        .await
        .unwrap_err();
    assert!(error.is_unique_violation(), "{error}");

    let second = VocabularyRepo::new(&pool)
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: "http://example.org/second/".to_string(),
            prefix: "second".to_string(),
            label: "Second".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    properties
        .create(&NewProperty {
            vocabulary_id: second.id,
            ..new
        })
        .await
        .expect("the same local name in another vocabulary is allowed");
}

#[tokio::test]
async fn class_filters_narrow_by_vocabulary_and_local_name() {
    let pool = common::memory().await;
    let vocabulary = VocabularyRepo::new(&pool).create(&dcterms()).await.unwrap();
    let classes = ClassRepo::new(&pool);
    for local_name in ["Image", "Text"] {
        classes
            .create(&NewClass {
                owner_id: None,
                vocabulary_id: vocabulary.id,
                local_name: local_name.to_string(),
                label: local_name.to_string(),
                comment: None,
            })
            .await
            .unwrap();
    }
    let listed = classes
        .list(
            &ClassFilter {
                local_name: Some("Text".to_string()),
                ..Default::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].local_name, "Text");

    let vocabulary_filtered = VocabularyRepo::new(&pool)
        .list(
            &VocabularyFilter {
                namespace_uri: Some("http://purl.org/dc/terms/".to_string()),
                ..Default::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(vocabulary_filtered.len(), 1);
}
