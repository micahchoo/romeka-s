//! `romeka_publication` behaviour: a publish appends the next version and fixes the address, term
//! state saves without a version, a slug is taken once, and the versions outlive the list they were
//! published from.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use chrono::NaiveDate;
use omeka_db::ids::CustomVocabId;
use omeka_db::repos::{CustomVocabRepo, NewPublicationVersion, PublicationRepo, VocabularyRepo};
use omeka_db::rows::{NewCustomVocab, NewVocabulary, PublicationTarget};
use serde_json::{Value, json};
use sqlx::SqlitePool;

async fn list(pool: &SqlitePool, label: &str) -> CustomVocabId {
    CustomVocabRepo::new(pool)
        .create(&NewCustomVocab {
            item_set_id: None,
            owner_id: None,
            label: label.to_string(),
            lang: Some("en".to_string()),
            terms: Some(json!(["Cork", "Dublin"])),
            uris: None,
        })
        .await
        .unwrap()
        .id
}

fn at(day: u32) -> chrono::NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 10, day)
        .unwrap()
        .and_hms_opt(9, 30, 0)
        .unwrap()
}

fn version<'a>(slug: &'a str, terms: &'a Value, snapshot: &'a Value) -> NewPublicationVersion<'a> {
    NewPublicationVersion {
        slug,
        namespace_uri: "http://localhost/vocab/places/",
        terms,
        snapshot,
    }
}

#[tokio::test]
async fn each_publish_appends_the_next_version_and_keeps_the_earlier_ones() {
    let pool = common::memory().await;
    let target = PublicationTarget::TermList(list(&pool, "Places").await);
    let repo = PublicationRepo::new(&pool);
    let terms = json!({"Cork": {"local": "1"}});

    let first = repo
        .publish(target, &version("places", &terms, &json!({"n": 1})), at(1))
        .await
        .unwrap();
    let second = repo
        .publish(target, &version("places", &terms, &json!({"n": 2})), at(2))
        .await
        .unwrap();

    assert_eq!((first.version, second.version), (1, 2));
    let record = repo.get(target).await.unwrap().unwrap();
    assert_eq!(record.slug.as_deref(), Some("places"));
    assert_eq!(
        record.namespace_uri.as_deref(),
        Some("http://localhost/vocab/places/")
    );
    assert_eq!(record.terms, terms);
    let versions = repo.versions(record.id).await.unwrap();
    assert_eq!(
        versions
            .iter()
            .map(|row| (row.version, row.created, row.snapshot.clone()))
            .collect::<Vec<_>>(),
        [(1, at(1), json!({"n": 1})), (2, at(2), json!({"n": 2}))]
    );
    assert_eq!(repo.latest(record.id).await.unwrap(), Some(second));
    assert_eq!(repo.version(record.id, 1).await.unwrap(), Some(first));
}

#[tokio::test]
async fn term_state_saves_as_a_draft_with_no_version() {
    let pool = common::memory().await;
    let target = PublicationTarget::TermList(list(&pool, "Places").await);
    let repo = PublicationRepo::new(&pool);

    let created = repo
        .save_terms(
            target,
            &json!({"Cork": {"exact_match": "https://sws.geonames.org/2965140/"}}),
        )
        .await
        .unwrap();
    let updated = repo
        .save_terms(
            target,
            &json!({"Cork": {"close_match": "https://www.wikidata.org/wiki/Q36405"}}),
        )
        .await
        .unwrap();

    assert_eq!(created.id, updated.id, "one record per term list");
    assert_eq!(updated.slug, None);
    assert_eq!(
        repo.get(target).await.unwrap().unwrap().terms,
        json!({"Cork": {"close_match": "https://www.wikidata.org/wiki/Q36405"}})
    );
    assert_eq!(repo.latest(updated.id).await.unwrap(), None);
}

#[tokio::test]
async fn a_slug_belongs_to_one_publication() {
    let pool = common::memory().await;
    let repo = PublicationRepo::new(&pool);
    let terms = json!({});
    let places = PublicationTarget::TermList(list(&pool, "Places").await);
    repo.publish(places, &version("places", &terms, &json!({})), at(1))
        .await
        .unwrap();
    let vocabulary = VocabularyRepo::new(&pool)
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: "http://localhost/vocab/places#".to_string(),
            prefix: "places".to_string(),
            label: "Places".to_string(),
            comment: None,
        })
        .await
        .unwrap();

    let taken = repo
        .publish(
            PublicationTarget::Vocabulary(vocabulary.id),
            &version("places", &terms, &json!({})),
            at(2),
        )
        .await;

    assert!(
        taken.is_err(),
        "the slug is unique across lists and vocabularies"
    );
    let record = repo.by_slug("places").await.unwrap().unwrap();
    assert_eq!(record.target, Some(places));
    assert_eq!(repo.versions(record.id).await.unwrap().len(), 1);
}

#[tokio::test]
async fn the_versions_outlive_the_deleted_list() {
    let pool = common::memory().await;
    let id = list(&pool, "Places").await;
    let repo = PublicationRepo::new(&pool);
    let terms = json!({});
    repo.publish(
        PublicationTarget::TermList(id),
        &version("places", &terms, &json!({"title": "Places"})),
        at(1),
    )
    .await
    .unwrap();

    assert!(CustomVocabRepo::new(&pool).delete(id).await.unwrap());

    let record = repo.by_slug("places").await.unwrap().unwrap();
    assert_eq!(record.target, None);
    assert_eq!(
        repo.latest(record.id).await.unwrap().unwrap().snapshot,
        json!({"title": "Places"})
    );
}

#[tokio::test]
async fn a_record_cannot_name_a_list_and_a_vocabulary() {
    let pool = common::memory().await;
    let id = list(&pool, "Places").await;
    let vocabulary = VocabularyRepo::new(&pool)
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: "http://localhost/vocab/local/".to_string(),
            prefix: "local".to_string(),
            label: "Local fields".to_string(),
            comment: None,
        })
        .await
        .unwrap();

    let both = sqlx::query(
        "INSERT INTO romeka_publication (custom_vocab_id, vocabulary_id, terms) VALUES (?, ?, '{}')",
    )
    .bind(id.0 as i64)
    .bind(vocabulary.id.0 as i64)
    .execute(&pool)
    .await;

    assert!(both.is_err());
}
