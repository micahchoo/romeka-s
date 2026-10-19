//! A built-in vocabulary's labels on a public site in another language.
//!
//! Upstream prints `alternate_label ?: $translate($property->label())`
//! (`view/common/resource-values.phtml:28`), so a Spanish site shows a Dublin
//! Core label in Spanish, and a user's own vocabulary and a template's
//! alternate label as stored.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{Seed, get, router, send, site_pool};
use omeka_api::vocab_import::defaults::DEFAULT_VOCABULARIES;
use omeka_db::ids::SiteId;
use omeka_db::repos::{PropertyRepo, SiteSettingRepo, TemplateRepo, ValueRepo, VocabularyRepo};
use omeka_db::rows::{NewProperty, NewValue, NewVocabulary};
use sqlx::SqlitePool;

async fn spanish_site(pool: &SqlitePool, seed: &Seed) {
    SiteSettingRepo::new(pool)
        .set(SiteId::from(seed.site), "locale", &serde_json::json!("es"))
        .await
        .unwrap();
}

/// A literal value of a property in a vocabulary the user imported.
async fn add_user_vocabulary_value(pool: &SqlitePool, seed: &Seed) {
    let vocabulary = VocabularyRepo::new(pool)
        .create(&NewVocabulary {
            owner_id: Some(seed.user),
            namespace_uri: "https://example.org/shelf#".to_string(),
            prefix: "shelf".to_string(),
            label: "Shelf list".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    let property = PropertyRepo::new(pool)
        .create(&NewProperty {
            owner_id: Some(seed.user),
            vocabulary_id: vocabulary.id,
            local_name: "title".to_string(),
            label: "Title".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: seed.item.as_resource(),
            property_id: property.id,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some("Shelf 4".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
}

/// `resource-values.phtml` on a Spanish site: `dcterms:title` prints as
/// `es.po` translates it, and a user vocabulary's property named "Title" too
/// prints as stored, because the translation is the built-in term's.
#[tokio::test]
async fn a_spanish_site_prints_built_in_labels_in_spanish() {
    let (pool, seed) = site_pool().await;
    spanish_site(&pool, &seed).await;
    add_user_vocabulary_value(&pool, &seed).await;
    let answer = send(
        router(pool),
        get(&format!("/s/museum/item/{}", seed.item.0), false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(answer.body.contains("<dt>Título</dt>"), "{}", answer.body);
    assert!(answer.body.contains("<dt>Title</dt>"), "{}", answer.body);
    assert!(answer.body.contains("Shelf 4"), "{}", answer.body);
}

/// FOAF as the install stores it, from the bundled `foaf.rdf`, with one value
/// of `foaf:mbox` on the seeded item.
async fn add_installed_foaf_mbox_value(pool: &SqlitePool, seed: &Seed) {
    let foaf = &DEFAULT_VOCABULARIES[3];
    let vocabulary = VocabularyRepo::new(pool)
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: foaf.namespace_uri.to_string(),
            prefix: foaf.prefix.to_string(),
            label: foaf.label.to_string(),
            comment: Some(foaf.comment.to_string()),
        })
        .await
        .unwrap();
    let members = foaf.members().unwrap();
    let mbox = members
        .properties
        .iter()
        .find(|term| term.local_name == "mbox")
        .unwrap();
    let property = PropertyRepo::new(pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: mbox.local_name.clone(),
            label: mbox.label.clone(),
            comment: mbox.comment.clone(),
        })
        .await
        .unwrap();
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: seed.item.as_resource(),
            property_id: property.id,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some("archive@example.org".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
}

/// A FOAF label as the install stores it ("personal mailbox") prints as
/// `es.po` translates it.
#[tokio::test]
async fn a_spanish_site_prints_an_installed_foaf_label_in_spanish() {
    let (pool, seed) = site_pool().await;
    spanish_site(&pool, &seed).await;
    add_installed_foaf_mbox_value(&pool, &seed).await;
    let answer = send(
        router(pool),
        get(&format!("/s/museum/item/{}", seed.item.0), false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(
        answer.body.contains("archive@example.org"),
        "{}",
        answer.body
    );
    assert!(
        answer.body.contains("<dt>buzón personal</dt>"),
        "{}",
        answer.body
    );
}

/// `alternate_label ?: ...`: the item's template names `dcterms:title`
/// "Caption", and the caption prints as stored, untranslated.
#[tokio::test]
async fn a_template_alternate_label_wins_over_the_translation() {
    let (pool, seed) = site_pool().await;
    spanish_site(&pool, &seed).await;
    sqlx::query("UPDATE resource_template_property SET alternate_label = 'Caption' WHERE resource_template_id = ?")
        .bind(i64::try_from(seed.template.0).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let rows = TemplateRepo::new(&pool)
        .list_properties(seed.template)
        .await
        .unwrap();
    assert_eq!(rows[0].alternate_label.as_deref(), Some("Caption"));
    let answer = send(
        router(pool),
        get(&format!("/s/museum/item/{}", seed.item.0), false),
    )
    .await;
    assert!(answer.body.contains("<dt>Caption</dt>"), "{}", answer.body);
    assert!(!answer.body.contains("<dt>Título</dt>"), "{}", answer.body);
}

/// The advanced search form's property and class selects translate each
/// option label, as Laminas `formSelect` does.
#[tokio::test]
async fn the_advanced_search_form_lists_built_in_terms_in_spanish() {
    let (pool, seed) = site_pool().await;
    spanish_site(&pool, &seed).await;
    let answer = send(router(pool), get("/s/museum/item/search", false)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(answer.body.contains(">Título</option>"), "{}", answer.body);
    assert!(answer.body.contains(">Imagen</option>"), "{}", answer.body);
}
