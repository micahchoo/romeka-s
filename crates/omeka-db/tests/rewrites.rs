//! `ValueRewriteRepo`: a promotion or a merge commits whole or not at all.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::ids::{PropertyId, ResourceId, TemplateId};
use omeka_db::repos::{
    CustomVocabRepo, ItemRepo, PropertyRepo, ValueRepo, ValueRewriteRepo, VocabularyRepo,
};
use omeka_db::rows::{
    MergeWrite, NewCustomVocab, NewItem, NewProperty, NewValue, NewVocabulary, PromoteList,
    PromoteWrite,
};
use serde_json::json;
use sqlx::SqlitePool;

async fn subject(pool: &SqlitePool) -> PropertyId {
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
    PropertyRepo::new(pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: "subject".to_string(),
            label: "Subject".to_string(),
            comment: None,
        })
        .await
        .unwrap()
        .id
}

async fn tagged(pool: &SqlitePool, property: PropertyId, tag: &str) -> ResourceId {
    let item = ItemRepo::new(pool)
        .create(&NewItem::default())
        .await
        .unwrap()
        .id()
        .as_resource();
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: item,
            property_id: property,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some(tag.to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
    item
}

/// A template that does not exist fails the template row's foreign key after the list and the
/// values were written in the same transaction; none of them stays.
#[tokio::test]
async fn a_failing_promotion_leaves_no_list_and_no_retyped_value() {
    let pool = common::memory().await;
    let property = subject(&pool).await;
    let item = tagged(&pool, property, "emigration").await;
    let repo = ValueRewriteRepo::new(&pool);
    let values = repo.text_values("literal", Some(property)).await.unwrap();
    assert_eq!(values.len(), 1);

    let failed = repo
        .promote(&PromoteWrite {
            list: PromoteList::Create(NewCustomVocab {
                item_set_id: None,
                owner_id: None,
                label: "Topics".to_string(),
                lang: None,
                terms: Some(json!(["emigration"])),
                uris: None,
            }),
            values: vec![values[0].id],
            from_type: "literal".to_string(),
            property,
            templates: vec![TemplateId(999)],
            unrestricted: vec!["literal".to_string()],
            resources: vec![item],
            terms_read: None,
        })
        .await;

    assert!(failed.is_err());
    assert!(CustomVocabRepo::new(&pool).list().await.unwrap().is_empty());
    let stored = ValueRepo::new(&pool).list_for_resource(item).await.unwrap();
    assert_eq!(stored[0].r#type, "literal");
}

/// Each statement re-checks the type it was planned for, so a value another writer retyped after
/// the read is left as that writer left it.
#[tokio::test]
async fn a_merge_rewrites_only_values_of_the_type_it_names() {
    let pool = common::memory().await;
    let property = subject(&pool).await;
    let item = tagged(&pool, property, "Co. Cork").await;
    let repo = ValueRewriteRepo::new(&pool);
    let value = repo.text_values("literal", None).await.unwrap()[0].id;

    repo.merge(&MergeWrite {
        rewrite: vec![value],
        to: "Cork".to_string(),
        of_type: "customvocab:1".to_string(),
        delete: vec![value],
        list_terms: None,
        list_states: None,
        resources: vec![item],
        default_title_property: None,
        terms_read: None,
    })
    .await
    .unwrap();

    let stored = ValueRepo::new(&pool).list_for_resource(item).await.unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].value.as_deref(), Some("Co. Cork"));
}

/// A promotion planned from a list's words refuses, writing nothing, when another write changed
/// those words first; planned from the words as stored, it goes through.
#[tokio::test]
async fn a_promotion_planned_from_stale_words_is_refused_whole() {
    let pool = common::memory().await;
    let property = subject(&pool).await;
    let item = tagged(&pool, property, "emigration").await;
    let lists = CustomVocabRepo::new(&pool);
    let list = lists
        .create(&NewCustomVocab {
            item_set_id: None,
            owner_id: None,
            label: "Topics".to_string(),
            lang: None,
            terms: Some(json!(["famine"])),
            uris: None,
        })
        .await
        .unwrap();
    let repo = ValueRewriteRepo::new(&pool);
    let value = repo.text_values("literal", Some(property)).await.unwrap()[0].id;
    let write = |read: serde_json::Value| PromoteWrite {
        list: PromoteList::Terms(list.id, json!(["emigration", "famine"])),
        values: vec![value],
        from_type: "literal".to_string(),
        property,
        templates: Vec::new(),
        unrestricted: Vec::new(),
        resources: vec![item],
        terms_read: Some(Some(read)),
    };

    let stale = repo.promote(&write(json!([]))).await;
    assert!(
        stale.as_ref().is_err_and(|error| error.is_conflict()),
        "{stale:?}"
    );
    let kept = lists.get(list.id).await.unwrap().unwrap();
    assert_eq!(kept.terms, Some(json!(["famine"])));
    let stored = ValueRepo::new(&pool).list_for_resource(item).await.unwrap();
    assert_eq!(stored[0].r#type, "literal");

    repo.promote(&write(json!(["famine"]))).await.unwrap();
    let promoted = lists.get(list.id).await.unwrap().unwrap();
    assert_eq!(promoted.terms, Some(json!(["emigration", "famine"])));
}
