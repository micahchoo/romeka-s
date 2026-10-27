//! `GET /api/romeka/item-sets/counts`, `GET /api/romeka/links/{id}` and `sort_by=item_count` on
//! `GET /api/item_sets`, read through HTTP as four callers: anonymous, two non-admins who each own
//! private records, and an admin.
//!
//! The spec of every count is the list it opens: a set's count equals `Omeka-S-Total-Results` of
//! `GET /api/items?item_set_id=<id>`, and a link row's count equals the total of its kind's `res`
//! property search, for the same caller.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::{BTreeMap, BTreeSet};

use axum::Router;
use axum::http::StatusCode;
use common::{Answer, KEY_CREDENTIAL, KEY_IDENTITY, caller, request, router, seeded_pool, send};
use omeka_db::ids::{ItemId, ItemSetId, PropertyId, ResourceId, UserId};
use omeka_db::repos::{ItemRepo, ItemSetRepo, MediaRepo, PropertyRepo, ValueRepo};
use omeka_db::rows::{NewItem, NewItemSet, NewMedia, NewProperty, NewResource, NewValue};
use serde_json::Value;
use sqlx::SqlitePool;

/// One caller: the query string that authenticates them, empty for anonymous.
#[derive(Clone)]
struct Who {
    name: &'static str,
    key: String,
}

impl Who {
    fn uri(&self, path: &str) -> String {
        if self.key.is_empty() {
            return path.to_string();
        }
        let separator = if path.contains('?') { '&' } else { '?' };
        format!("{path}{separator}{}", self.key)
    }
}

struct World {
    app: Router,
    anonymous: Who,
    author: Who,
    other: Who,
    admin: Who,
    seed_set: ItemSetId,
    /// Public, owned by the admin: four items, one of each visibility.
    s1: ItemSetId,
    /// Private, owned by the author.
    s2: ItemSetId,
    /// Private, owned by the other non-admin.
    s3: ItemSetId,
    /// Public and empty.
    s4: ItemSetId,
    creator: PropertyId,
    subject: PropertyId,
    /// A public item that items, item sets and media point at.
    target: ItemId,
    /// A private item of the other non-admin, pointed at by one public item.
    hidden_target: ItemId,
}

impl World {
    fn callers(&self) -> [&Who; 4] {
        [&self.anonymous, &self.author, &self.other, &self.admin]
    }
}

async fn property(
    pool: &SqlitePool,
    vocabulary: omeka_db::ids::VocabularyId,
    name: &str,
) -> PropertyId {
    PropertyRepo::new(pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: vocabulary,
            local_name: name.to_string(),
            label: name.to_string(),
            comment: None,
        })
        .await
        .unwrap()
        .id
}

fn resource(owner: UserId, title: &str, is_public: bool) -> NewResource {
    NewResource {
        owner_id: Some(owner),
        title: Some(title.to_string()),
        is_public,
        ..NewResource::default()
    }
}

async fn item(
    pool: &SqlitePool,
    owner: UserId,
    title: &str,
    is_public: bool,
    sets: &[ItemSetId],
) -> ItemId {
    let id = ItemRepo::new(pool)
        .create(&NewItem {
            resource: resource(owner, title, is_public),
            primary_media_id: None,
        })
        .await
        .unwrap()
        .id();
    for set in sets {
        ItemRepo::new(pool).add_to_item_set(id, *set).await.unwrap();
    }
    id
}

async fn item_set(pool: &SqlitePool, owner: UserId, title: &str, is_public: bool) -> ItemSetId {
    ItemSetRepo::new(pool)
        .create(&NewItemSet {
            resource: resource(owner, title, is_public),
            is_open: true,
        })
        .await
        .unwrap()
        .id()
}

async fn media(pool: &SqlitePool, item: ItemId, owner: UserId, is_public: bool) -> ResourceId {
    MediaRepo::new(pool)
        .create(&NewMedia {
            resource: resource(owner, "a file", is_public),
            item_id: item,
            ingester: "html".to_string(),
            renderer: "html".to_string(),
            ..NewMedia::default()
        })
        .await
        .unwrap()
        .id()
        .as_resource()
}

async fn link(
    pool: &SqlitePool,
    from: ResourceId,
    property: PropertyId,
    to: ItemId,
    is_public: bool,
) {
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: from,
            property_id: property,
            value_resource_id: Some(to.as_resource()),
            value_annotation_id: None,
            r#type: "resource:item".to_string(),
            lang: None,
            value: None,
            uri: None,
            is_public,
        })
        .await
        .unwrap();
}

async fn world() -> World {
    let (pool, seed) = seeded_pool().await;
    let author = caller(&pool, "author").await;
    let other = caller(&pool, "researcher").await;
    let admin = seed.user;
    let creator = property(&pool, seed.vocabulary, "creator").await;
    let subject = property(&pool, seed.vocabulary, "subject").await;

    let s1 = item_set(&pool, admin, "Public set", true).await;
    let s2 = item_set(&pool, author.id, "Author's private set", false).await;
    let s3 = item_set(&pool, other.id, "Other's private set", false).await;
    let s4 = item_set(&pool, admin, "Empty set", true).await;

    let target = item(&pool, admin, "A person", true, &[]).await;
    let hidden_target = item(&pool, other.id, "A private person", false, &[]).await;

    let i1 = item(&pool, admin, "public", true, &[s1]).await;
    let i2 = item(&pool, admin, "admin's private", false, &[s1]).await;
    let i3 = item(&pool, author.id, "author's private", false, &[s1, s2]).await;
    let i4 = item(&pool, other.id, "other's private", false, &[s1, s3]).await;
    let i5 = item(&pool, author.id, "author's public", true, &[s2]).await;
    for from in [i1, i2, i3, i4] {
        link(&pool, from.as_resource(), creator, target, true).await;
    }
    link(&pool, i3.as_resource(), subject, target, true).await;
    // A private value on a public item: the list follows whatever the `res` search does with it.
    link(&pool, i5.as_resource(), creator, target, false).await;
    // Two values of one item on one property count the item once.
    link(&pool, i1.as_resource(), subject, target, true).await;
    link(&pool, i1.as_resource(), subject, target, true).await;
    link(&pool, s4.as_resource(), subject, target, true).await;
    link(&pool, s3.as_resource(), subject, target, true).await;
    link(&pool, seed.media.as_resource(), creator, target, true).await;
    let private_media = media(&pool, i4, other.id, false).await;
    link(&pool, private_media, creator, target, true).await;
    link(&pool, i1.as_resource(), creator, hidden_target, true).await;

    World {
        app: router(pool),
        anonymous: Who {
            name: "anonymous",
            key: String::new(),
        },
        author: Who {
            name: "author",
            key: author.key,
        },
        other: Who {
            name: "other",
            key: other.key,
        },
        admin: Who {
            name: "admin",
            key: format!("key_identity={KEY_IDENTITY}&key_credential={KEY_CREDENTIAL}"),
        },
        seed_set: seed.item_set,
        s1,
        s2,
        s3,
        s4,
        creator,
        subject,
        target,
        hidden_target,
    }
}

async fn read(app: &Router, uri: &str) -> Answer {
    send(app.clone(), request("GET", uri)).await
}

/// `Omeka-S-Total-Results` of one list read.
async fn list_total(app: &Router, uri: &str) -> u64 {
    let answer = read(app, uri).await;
    assert_eq!(answer.status, StatusCode::OK, "{uri}: {}", answer.body);
    common::header(&answer, "omeka-s-total-results")
        .expect("a total header")
        .parse()
        .unwrap()
}

/// The `item_sets` member of a counts answer, as items by set id, in the order it came.
fn set_counts(answer: &Answer) -> Vec<(u64, u64)> {
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    answer.json()["item_sets"]
        .as_array()
        .expect("an item_sets array")
        .iter()
        .map(|row| (row["id"].as_u64().unwrap(), row["items"].as_u64().unwrap()))
        .collect()
}

fn counts_uri(ids: &[String]) -> String {
    let query: Vec<String> = ids.iter().map(|id| format!("id[]={id}")).collect();
    format!("/api/romeka/item-sets/counts?{}", query.join("&"))
}

#[tokio::test]
async fn each_set_count_equals_the_items_list_total_for_its_caller() {
    let world = world().await;
    let mut named: Vec<String> = [world.seed_set, world.s1, world.s2, world.s3, world.s4]
        .iter()
        .map(|id| id.0.to_string())
        .collect();
    named.extend(["999999".to_string(), "abc".to_string(), "0".to_string()]);
    let expected: [(&str, Vec<(ItemSetId, u64)>); 4] = [
        (
            "anonymous",
            vec![(world.seed_set, 1), (world.s1, 1), (world.s4, 0)],
        ),
        (
            "author",
            vec![
                (world.seed_set, 1),
                (world.s1, 2),
                (world.s2, 2),
                (world.s4, 0),
            ],
        ),
        (
            "other",
            vec![
                (world.seed_set, 1),
                (world.s1, 2),
                (world.s3, 1),
                (world.s4, 0),
            ],
        ),
        (
            "admin",
            vec![
                (world.seed_set, 1),
                (world.s1, 4),
                (world.s2, 2),
                (world.s3, 1),
                (world.s4, 0),
            ],
        ),
    ];
    for (who, (name, sets)) in world.callers().into_iter().zip(expected) {
        assert_eq!(who.name, name);
        let answer = read(&world.app, &who.uri(&counts_uri(&named))).await;
        assert_eq!(
            common::header(&answer, "cache-control"),
            Some("no-store"),
            "a count answers one caller"
        );
        let counts = set_counts(&answer);
        let want: Vec<(u64, u64)> = sets.iter().map(|(id, n)| (id.0, *n)).collect();
        assert_eq!(
            counts, want,
            "{name}: the sets they may see, with their items"
        );
        for (id, items) in counts {
            let total = list_total(
                &world.app,
                &who.uri(&format!("/api/items?item_set_id={id}")),
            )
            .await;
            assert_eq!(
                items, total,
                "{name}: set {id} count equals the items list total"
            );
        }
    }
}

#[tokio::test]
async fn a_hidden_set_and_a_missing_one_are_both_left_out() {
    let world = world().await;
    let hidden = read(
        &world.app,
        &world.author.uri(&counts_uri(&[world.s3.0.to_string()])),
    )
    .await;
    let missing = read(
        &world.app,
        &world.author.uri(&counts_uri(&["999999".to_string()])),
    )
    .await;
    assert_eq!(hidden.status, StatusCode::OK);
    assert_eq!(hidden.body, missing.body);
    assert_eq!(hidden.json(), serde_json::json!({ "item_sets": [] }));
}

#[tokio::test]
async fn more_than_two_hundred_distinct_set_ids_is_a_bad_request() {
    let world = world().await;
    let at_cap: Vec<String> = (1..=200).map(|id| id.to_string()).collect();
    let answer = read(&world.app, &world.admin.uri(&counts_uri(&at_cap))).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);

    let mut repeated = at_cap.clone();
    repeated.push("1".to_string());
    let answer = read(&world.app, &world.admin.uri(&counts_uri(&repeated))).await;
    assert_eq!(answer.status, StatusCode::OK, "a repeated id counts once");

    let mut past = at_cap;
    past.push("201".to_string());
    let answer = read(&world.app, &world.admin.uri(&counts_uri(&past))).await;
    assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{}", answer.body);
    assert_eq!(answer.json()["errors"]["error"], "Bad Request");
}

/// The link rows of one answer, keyed by (kind, property id), with their term.
fn link_rows(answer: &Answer) -> BTreeMap<(String, u64), (String, u64)> {
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    answer.json()["links"]
        .as_array()
        .expect("a links array")
        .iter()
        .map(|row: &Value| {
            (
                (
                    row["kind"].as_str().unwrap().to_string(),
                    row["property_id"].as_u64().unwrap(),
                ),
                (
                    row["term"].as_str().unwrap().to_string(),
                    row["count"].as_u64().unwrap(),
                ),
            )
        })
        .collect()
}

#[tokio::test]
async fn each_link_count_equals_its_kinds_res_search_total_for_its_caller() {
    let world = world().await;
    for who in world.callers() {
        let answer = read(
            &world.app,
            &who.uri(&format!("/api/romeka/links/{}", world.target.0)),
        )
        .await;
        assert_eq!(common::header(&answer, "cache-control"), Some("no-store"));
        let rows = link_rows(&answer);
        let mut seen = BTreeSet::new();
        for kind in ["items", "item_sets", "media"] {
            for property in [world.creator, world.subject] {
                let total = list_total(
                    &world.app,
                    &who.uri(&format!(
                        "/api/{kind}?property[0][property]={}&property[0][type]=res&property[0][text]={}",
                        property.0, world.target.0
                    )),
                )
                .await;
                let key = (kind.to_string(), property.0);
                match rows.get(&key) {
                    Some((term, count)) => {
                        assert_eq!(*count, total, "{}: {kind} via {term}", who.name);
                        assert!(term.ends_with(":creator") || term.ends_with(":subject"));
                        seen.insert(key);
                    }
                    None => assert_eq!(
                        total, 0,
                        "{}: {kind} via {} has no row",
                        who.name, property.0
                    ),
                }
            }
        }
        assert_eq!(
            seen.len(),
            rows.len(),
            "{}: no row the lists do not stand for",
            who.name
        );
    }
}

#[tokio::test]
async fn link_rows_count_only_what_the_caller_may_see() {
    let world = world().await;
    let uri = format!("/api/romeka/links/{}", world.target.0);
    let counts = |answer: &Answer| -> Vec<(String, String, u64)> {
        answer.json()["links"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                (
                    row["kind"].as_str().unwrap().to_string(),
                    row["label"].as_str().unwrap().to_string(),
                    row["count"].as_u64().unwrap(),
                )
            })
            .collect()
    };
    let row = |kind: &str, label: &str, count: u64| (kind.to_string(), label.to_string(), count);
    let author = read(&world.app, &world.author.uri(&uri)).await;
    assert_eq!(
        counts(&author),
        vec![
            row("items", "creator", 3),
            row("items", "subject", 2),
            row("item_sets", "subject", 1),
            row("media", "creator", 1),
        ],
        "the author: public items, theirs, and the public set and file"
    );
    let admin = read(&world.app, &world.admin.uri(&uri)).await;
    assert_eq!(
        counts(&admin),
        vec![
            row("items", "creator", 5),
            row("items", "subject", 2),
            row("item_sets", "subject", 2),
            row("media", "creator", 2),
        ]
    );
}

#[tokio::test]
async fn a_hidden_target_and_a_missing_one_are_the_same_not_found() {
    let world = world().await;
    let hidden = format!("/api/romeka/links/{}", world.hidden_target.0);
    let other = read(&world.app, &world.other.uri(&hidden)).await;
    assert_eq!(other.status, StatusCode::OK, "its owner reads it");
    for who in [&world.anonymous, &world.author] {
        let answer = read(&world.app, &who.uri(&hidden)).await;
        let missing = read(&world.app, &who.uri("/api/romeka/links/999999")).await;
        assert_eq!(answer.status, StatusCode::NOT_FOUND, "{}", who.name);
        assert_eq!(missing.status, StatusCode::NOT_FOUND);
        assert_eq!(
            answer.body.replace(&world.hidden_target.0.to_string(), "N"),
            missing.body.replace("999999", "N"),
            "{}: a hidden target reads as a missing one",
            who.name
        );
    }
    let garbage = read(&world.app, &world.admin.uri("/api/romeka/links/abc")).await;
    assert_eq!(garbage.status, StatusCode::NOT_FOUND);
}

/// The ids of one item set list, in order.
async fn set_order(world: &World, who: &Who, order: &str) -> Vec<u64> {
    let answer = read(
        &world.app,
        &who.uri(&format!(
            "/api/item_sets?sort_by=item_count&sort_order={order}"
        )),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    answer
        .json()
        .as_array()
        .unwrap()
        .iter()
        .map(|set| set["o:id"].as_u64().unwrap())
        .collect()
}

#[tokio::test]
async fn sort_by_item_count_orders_by_every_item_a_set_holds_as_upstream_does() {
    let world = world().await;
    let ids = |sets: &[ItemSetId]| sets.iter().map(|id| id.0).collect::<Vec<_>>();
    // Every item: s4 0, seed 1, s3 1, s2 2, s1 4. A tie goes to the id, in the same direction.
    assert_eq!(
        set_order(&world, &world.admin, "asc").await,
        ids(&[world.s4, world.seed_set, world.s3, world.s2, world.s1])
    );
    assert_eq!(
        set_order(&world, &world.admin, "desc").await,
        ids(&[world.s1, world.s2, world.s3, world.seed_set, world.s4])
    );
    // The author sees 2 items in s1 and 2 in s2, but s1 holds 4: upstream's `SIZE` counts them all.
    assert_eq!(
        set_order(&world, &world.author, "asc").await,
        ids(&[world.s4, world.seed_set, world.s2, world.s1])
    );
}
