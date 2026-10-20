//! The term catalog covers every row a document prints.
//!
//! The API read surface loads only the term rows a document can print
//! (`Catalog::load_for_read`): the properties a `value` or a
//! `resource_template_property` row names, and the classes a `resource` row
//! names. A write, and a list request carrying `property[i][property]`, load the
//! whole table (`Catalog::fill_all`).
//!
//! These tests pin the three shapes where the narrow set would silently drop a
//! term if it were narrower than it is: a value whose property no template
//! names, a class only the item's own row names, and a term named only by the
//! request or by a write body.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use omeka_api::auth::Actor;
use omeka_api::jsonld::BaseUrl;
use omeka_api::read::{Api, Params};
use omeka_db::ids::{ClassId, ItemId, PropertyId, UserId};
use omeka_db::repos::{ClassRepo, ItemRepo, PropertyRepo, UserRepo, ValueRepo, VocabularyRepo};
use omeka_db::rows::{
    NewClass, NewItem, NewProperty, NewResource, NewUser, NewValue, NewVocabulary,
};
use omeka_db::{migrate, open_memory};
use serde_json::Value;
use sqlx::SqlitePool;

/// One vocabulary with two properties and a class, and one item carrying a
/// value on the first property and the class. The second property has no value
/// and no template names either.
struct Seed {
    item: ItemId,
    user: UserId,
    note: PropertyId,
}

async fn seed(pool: &SqlitePool) -> Seed {
    let user = UserRepo::new(pool)
        .create(&NewUser {
            email: "terms@example.com".to_string(),
            name: "Terms".to_string(),
            password_hash: None,
            role: "global_admin".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap()
        .id;
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
    let note = PropertyRepo::new(pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: "note".to_string(),
            label: "Note".to_string(),
            comment: None,
        })
        .await
        .unwrap()
        .id;
    PropertyRepo::new(pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: "memo".to_string(),
            label: "Memo".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    let class: ClassId = ClassRepo::new(pool)
        .create(&NewClass {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: "Note".to_string(),
            label: "Note".to_string(),
            comment: None,
        })
        .await
        .unwrap()
        .id;
    let item = ItemRepo::new(pool)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(user),
                resource_class_id: Some(class),
                title: Some("Noted".to_string()),
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap()
        .id();
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: item.as_resource(),
            property_id: note,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some("hello".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
    Seed { item, user, note }
}

async fn open_seeded() -> (SqlitePool, Seed) {
    let pool = open_memory().await.unwrap();
    migrate(&pool).await.unwrap();
    let seed = seed(&pool).await;
    (pool, seed)
}

fn base() -> BaseUrl {
    BaseUrl::parse("http://localhost").unwrap()
}

async fn api<'a>(pool: &'a omeka_db::DbPool, seed: &Seed) -> Api<'a> {
    Api::load(pool, base(), Actor::new(seed.user, "global_admin"))
        .await
        .unwrap()
}

fn document(body: &str) -> Value {
    serde_json::from_str(body).unwrap()
}

fn documents(body: &str) -> Vec<Value> {
    match serde_json::from_str(body).unwrap() {
        Value::Array(documents) => documents,
        other => panic!("a list body is an array, got {other}"),
    }
}

/// A value's property is named by the `value` row alone. The read loads its
/// term and label, and the resource's class term, from that.
#[tokio::test]
async fn a_read_prints_the_term_of_a_value_and_the_class_of_the_row() {
    let (pool, seed) = open_seeded().await;
    let db = omeka_db::DbPool::from(pool.clone());
    let api = api(&db, &seed).await;

    let response = api
        .read_member("items", &seed.item.0.to_string())
        .await
        .unwrap();
    let document = document(&response.body);

    assert_eq!(document["dcterms:note"][0]["property_label"], "Note");
    assert_eq!(document["dcterms:note"][0]["@value"], "hello");
    assert_eq!(document["dcterms:note"][0]["property_id"], seed.note.0);
    assert_eq!(
        document["@type"],
        serde_json::json!(["o:Item", "dcterms:Note"])
    );
    pool.close().await;
}

/// A `property[i][property]` row names a term the page's own rows need not
/// name. The request loads the term table before it resolves the id, so the
/// row still matches.
#[tokio::test]
async fn a_property_row_resolves_a_term_the_page_does_not_name() {
    let (pool, seed) = open_seeded().await;
    let db = omeka_db::DbPool::from(pool.clone());
    let api = api(&db, &seed).await;

    let params = Params::parse(
        "property[0][property]=dcterms:note&property[0][type]=in&property[0][text]=hello",
    );
    let response = api.search("items", &params).await.unwrap();
    let page = documents(&response.body);
    assert_eq!(page.len(), 1);
    assert_eq!(page[0]["o:id"], seed.item.0);

    let params = Params::parse(
        "property[0][property]=dcterms:note&property[0][type]=in&property[0][text]=absent",
    );
    let response = api.search("items", &params).await.unwrap();
    assert!(documents(&response.body).is_empty());
    pool.close().await;
}

/// A write names a property before any value exists for it. The write loads the
/// term table, so the body key resolves and the answer prints the value.
#[tokio::test]
async fn a_write_prints_a_property_it_names_before_any_value_exists() {
    let (pool, seed) = open_seeded().await;
    let db = omeka_db::DbPool::from(pool.clone());
    let api = api(&db, &seed).await;

    let body =
        br#"{"dcterms:memo":[{"property_id":"auto","type":"literal","@value":"first memo"}]}"#;
    let response = api.create("items", body, None).await.unwrap();
    let document = document(&response.body);

    assert_eq!(document["dcterms:memo"][0]["property_label"], "Memo");
    assert_eq!(document["dcterms:memo"][0]["@value"], "first memo");
    pool.close().await;
}
