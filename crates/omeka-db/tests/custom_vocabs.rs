//! `custom_vocab` behaviour: the JSON columns round-trip, the label is unique ignoring case, the
//! foreign keys clear on delete, and deleting a list turns its data type name back into a plain one
//! (`Module::setVocabTypeToDefaultState`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::ids::PropertyId;
use omeka_db::repos::{
    CustomVocabRepo, ItemRepo, ItemSetRepo, PropertyRepo, TemplateRepo, UserRepo, ValueRepo,
    VocabularyRepo,
};
use omeka_db::rows::{
    NewCustomVocab, NewItem, NewItemSet, NewProperty, NewTemplate, NewTemplateProperty, NewUser,
    NewValue, NewVocabulary,
};
use serde_json::json;
use sqlx::SqlitePool;

fn places() -> NewCustomVocab {
    NewCustomVocab {
        item_set_id: None,
        owner_id: None,
        label: "Places".to_string(),
        lang: Some("en".to_string()),
        terms: Some(json!(["Cork", "Dublin"])),
        uris: None,
    }
}

async fn property(pool: &SqlitePool) -> PropertyId {
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
            local_name: "spatial".to_string(),
            label: "Spatial Coverage".to_string(),
            comment: None,
        })
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn a_list_round_trips_its_terms_and_uris() {
    let pool = common::memory().await;
    let repo = CustomVocabRepo::new(&pool);
    let terms = repo.create(&places()).await.unwrap();
    let links = repo
        .create(&NewCustomVocab {
            label: "Subjects".to_string(),
            lang: None,
            terms: None,
            uris: Some(json!({"http://id.loc.gov/sh1": "Bogs", "http://id.loc.gov/sh2": ""})),
            ..places()
        })
        .await
        .unwrap();

    assert_eq!(repo.get(terms.id).await.unwrap(), Some(terms.clone()));
    assert_eq!(repo.get(links.id).await.unwrap(), Some(links.clone()));
    assert_eq!(repo.list().await.unwrap(), vec![terms.clone(), links]);

    let mut renamed = terms.clone();
    renamed.label = "Irish places".to_string();
    renamed.terms = Some(json!(["Cork", "Dublin", "Galway"]));
    assert!(repo.update(&renamed).await.unwrap());
    assert_eq!(repo.get(terms.id).await.unwrap(), Some(renamed));
}

/// `UNIQ_8533D2A5EA750E8` on `label`, compared through `utf8mb4_unicode_ci` upstream.
#[tokio::test]
async fn the_label_is_unique_ignoring_case() {
    let pool = common::memory().await;
    let repo = CustomVocabRepo::new(&pool);
    let stored = repo.create(&places()).await.unwrap();
    let error = repo
        .create(&NewCustomVocab {
            label: "PLACES".to_string(),
            ..places()
        })
        .await
        .unwrap_err();
    assert!(error.is_unique_violation(), "{error}");
    assert_eq!(repo.find_by_label("places").await.unwrap(), Some(stored));
}

/// Both foreign keys are `ON DELETE SET NULL` in `Module::install`.
#[tokio::test]
async fn deleting_the_item_set_or_the_owner_keeps_the_list() {
    let pool = common::memory().await;
    let owner = UserRepo::new(&pool)
        .create(&NewUser {
            email: "ada@example.org".to_string(),
            name: "Ada".to_string(),
            password_hash: None,
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    let set = ItemSetRepo::new(&pool)
        .create(&NewItemSet::default())
        .await
        .unwrap();
    let repo = CustomVocabRepo::new(&pool);
    let list = repo
        .create(&NewCustomVocab {
            item_set_id: Some(set.id()),
            owner_id: Some(owner.id),
            terms: None,
            ..places()
        })
        .await
        .unwrap();

    let stamp = omeka_db::rows::HistoryStamp {
        user: None,
        source: omeka_db::rows::HistorySource::Edit,
    };
    ItemSetRepo::new(&pool)
        .delete(set.id(), &stamp)
        .await
        .unwrap();
    UserRepo::new(&pool).delete(owner.id).await.unwrap();

    let stored = repo.get(list.id).await.unwrap().unwrap();
    assert_eq!(stored.item_set_id, None);
    assert_eq!(stored.owner_id, None);
}

#[tokio::test]
async fn deleting_a_list_sets_its_values_to_literal_and_drops_it_from_templates() {
    let pool = common::memory().await;
    let property = property(&pool).await;
    let repo = CustomVocabRepo::new(&pool);
    let list = repo.create(&places()).await.unwrap();
    let other = repo
        .create(&NewCustomVocab {
            label: "Counties".to_string(),
            ..places()
        })
        .await
        .unwrap();
    let name = format!("customvocab:{}", list.id);
    let other_name = format!("customvocab:{}", other.id);

    let item = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    let value = |r#type: &str, text: &str| NewValue {
        resource_id: item.resource.id,
        property_id: property,
        value_resource_id: None,
        value_annotation_id: None,
        r#type: r#type.to_string(),
        lang: Some("en".to_string()),
        value: Some(text.to_string()),
        uri: None,
        is_public: true,
    };
    let values = ValueRepo::new(&pool);
    let listed = values.create(&value(&name, "Cork")).await.unwrap();
    let kept = values.create(&value(&other_name, "Kerry")).await.unwrap();

    let templates = TemplateRepo::new(&pool);
    let template = templates
        .create(&NewTemplate {
            owner_id: None,
            resource_class_id: None,
            title_property_id: None,
            description_property_id: None,
            label: "Place".to_string(),
        })
        .await
        .unwrap();
    let row = |data_type: Vec<String>| NewTemplateProperty {
        resource_template_id: template.id,
        property_id: property,
        alternate_label: None,
        alternate_comment: None,
        position: Some(1),
        data_type: Some(data_type),
        is_required: false,
        is_private: false,
        default_lang: None,
    };
    let mixed = templates
        .add_property(&row(vec![name.clone(), other_name.clone()]))
        .await
        .unwrap();

    assert!(repo.delete(list.id).await.unwrap());

    assert_eq!(repo.get(list.id).await.unwrap(), None);
    let listed = values.get(listed.id).await.unwrap().unwrap();
    assert_eq!(listed.r#type, "literal");
    assert_eq!(listed.value.as_deref(), Some("Cork"));
    assert_eq!(listed.lang.as_deref(), Some("en"));
    assert_eq!(
        values.get(kept.id).await.unwrap().unwrap().r#type,
        other_name
    );
    let rows = templates.list_properties(template.id).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, mixed.id);
    assert_eq!(rows[0].data_type, Some(vec![other_name]));

    assert!(repo.delete(other.id).await.unwrap());
    let rows = templates.list_properties(template.id).await.unwrap();
    assert_eq!(
        rows[0].data_type, None,
        "a row left with no data type stores NULL"
    );
}
