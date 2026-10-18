//! Every text sort folds case the way upstream does.
//!
//! Upstream reads and orders every text column through MySQL's
//! `utf8mb4_unicode_ci` collation, so `sort_by=title` puts `apple` before
//! `Banana`. The port compared bytes, so `Banana` came first. Each test asserts
//! the order an HTTP caller observes on the list endpoint the adapter serves,
//! so it fails on the port before the fix and passes after.
//!
//! Oracle measurement, 2026-10-17: `GET /api/properties?sort_by=label` answers
//! `Abstract` before `abstract` before `Access Rights`, and the collation
//! report measured `apple < Banana < Zebra` on the oracle for the item title
//! set. The three tests use the same mixed-case set for items, users and
//! properties.
//!
//! Upstream sources: `AbstractEntityAdapter::sortQuery` (the `sortFields` maps
//! of `ItemAdapter`, `UserAdapter` and `PropertyAdapter`) and the collation
//! `MySQL` gives every text column.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{Answer, get, router, seeded_pool, send};
use omeka_db::ids::{UserId, VocabularyId};
use omeka_db::repos::{ItemRepo, ItemSetRepo, PropertyRepo, UserRepo};
use omeka_db::rows::{NewItem, NewItemSet, NewProperty, NewResource, NewUser};
use serde_json::Value;
use sqlx::SqlitePool;

/// The mixed-case set, in the order upstream's collation gives it.
const FOLDED: [&str; 3] = ["apple", "Banana", "Zebra"];

/// The same three values, created in an order no sort of them produces, so an
/// `id` fallback cannot pass the assertion by accident.
const CREATED: [&str; 3] = ["Zebra", "apple", "Banana"];

/// A public item titled `title`, held by a new item set with id `set`.
async fn item_in_set(pool: &SqlitePool, title: &str, owner: UserId, set: u64) {
    let item = ItemRepo::new(pool)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(owner),
                title: Some(title.to_string()),
                is_public: true,
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap();
    ItemRepo::new(pool)
        .add_to_item_set(item.id(), omeka_db::ids::ItemSetId(set))
        .await
        .unwrap();
}

/// A new item set owned by `owner`, so an item list can be narrowed to it.
async fn item_set(pool: &SqlitePool, owner: UserId) -> u64 {
    ItemSetRepo::new(pool)
        .create(&NewItemSet {
            resource: NewResource {
                owner_id: Some(owner),
                title: Some("Sortwire set".to_string()),
                is_public: true,
                ..NewResource::default()
            },
            is_open: false,
        })
        .await
        .unwrap()
        .id()
        .0
}

/// A user named `name`, so the users list has a mixed-case name to order.
async fn user_named(pool: &SqlitePool, name: &str) {
    UserRepo::new(pool)
        .create(&NewUser {
            email: format!("{}@example.com", name.to_lowercase()),
            name: name.to_string(),
            password_hash: None,
            role: "researcher".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
}

/// A property labelled `label` on the seeded `dcterms` vocabulary.
async fn property_labelled(pool: &SqlitePool, label: &str, vocabulary: u64) {
    PropertyRepo::new(pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: VocabularyId(vocabulary),
            local_name: format!("sortwire_{}", label.to_lowercase()),
            label: label.to_string(),
            comment: None,
        })
        .await
        .unwrap();
}

/// The documents of a list body, in order.
fn documents(answer: &Answer) -> Vec<Value> {
    answer
        .json()
        .as_array()
        .cloned()
        .expect("a list body is an array")
}

/// Every `o:title` in a list body, in order, dropping the `null` entries the
/// untitled seeded rows carry.
fn titles(answer: &Answer) -> Vec<String> {
    documents(answer)
        .iter()
        .filter_map(|document| document["o:title"].as_str().map(str::to_string))
        .collect()
}

/// The `o:name` of every user in a list body, in order.
fn names(answer: &Answer) -> Vec<String> {
    documents(answer)
        .iter()
        .filter_map(|document| document["o:name"].as_str().map(str::to_string))
        .collect()
}

/// The `o:label` of every property in a list body, in order.
fn labels(answer: &Answer) -> Vec<String> {
    documents(answer)
        .iter()
        .filter_map(|document| document["o:label"].as_str().map(str::to_string))
        .collect()
}

/// Keep only the mixed-case set, so a seeded row cannot change the assertion.
fn mixed_case(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .filter(|value| FOLDED.contains(&value.as_str()))
        .collect()
}

/// `ItemAdapter::$sortFields['title']` orders the item list by `title`, which
/// `AbstractEntityAdapter::sortQuery` gives to the collation.
#[tokio::test]
async fn item_title_sort_folds_case_like_the_oracle() {
    let (pool, seed) = seeded_pool().await;
    let set = item_set(&pool, seed.user).await;
    for title in CREATED {
        item_in_set(&pool, title, seed.user, set).await;
    }

    let uri = format!("/api/items?item_set_id={set}&sort_by=title&sort_order=asc");
    let answer = send(router(pool), get(&uri, true)).await;

    assert_eq!(answer.status, axum::http::StatusCode::OK);
    assert_eq!(titles(&answer), FOLDED);
}

/// `UserAdapter::$sortFields['name']` orders the users list by `name`.
#[tokio::test]
async fn user_name_sort_folds_case_like_the_oracle() {
    let (pool, _seed) = seeded_pool().await;
    for name in CREATED {
        user_named(&pool, name).await;
    }

    let answer = send(
        router(pool),
        get("/api/users?sort_by=name&sort_order=asc", true),
    )
    .await;

    assert_eq!(answer.status, axum::http::StatusCode::OK);
    assert_eq!(mixed_case(names(&answer)), FOLDED);
}

/// `PropertyAdapter::$sortFields['label']` orders the property list by `label`.
#[tokio::test]
async fn property_label_sort_folds_case_like_the_oracle() {
    let (pool, seed) = seeded_pool().await;
    for label in CREATED {
        property_labelled(&pool, label, seed.vocabulary.0).await;
    }

    let answer = send(
        router(pool),
        get("/api/properties?sort_by=label&sort_order=asc", true),
    )
    .await;

    assert_eq!(answer.status, axum::http::StatusCode::OK);
    assert_eq!(mixed_case(labels(&answer)), FOLDED);
}

/// A case-only difference ties under the fold, and `id` decides, which is the
/// tiebreak `AbstractEntityAdapter::sortQuery` appends and `ordered_page` keeps
/// through its stable sort.
#[tokio::test]
async fn a_case_only_difference_keeps_the_id_tiebreak() {
    let (pool, seed) = seeded_pool().await;
    let set = item_set(&pool, seed.user).await;
    item_in_set(&pool, "Apple", seed.user, set).await;
    item_in_set(&pool, "apple", seed.user, set).await;

    let uri = format!("/api/items?item_set_id={set}&sort_by=title&sort_order=asc");
    let answer = send(router(pool), get(&uri, true)).await;

    assert_eq!(titles(&answer), ["Apple", "apple"]);
}
