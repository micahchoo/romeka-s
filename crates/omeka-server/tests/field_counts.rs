//! `GET /api/romeka/field-counts?resource_template_id=<id>`, read through HTTP as four callers:
//! anonymous, two non-admins who each own private items, and an admin.
//!
//! The spec of every number is the list it opens, for the same caller: `total` is
//! `Omeka-S-Total-Results` of `GET /api/items?resource_template_id=<id>`, a field's `filled` is
//! that list narrowed by `property[i][type]=ex` on the field, and `total - filled` is the list
//! narrowed by `nex`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeMap;

use axum::Router;
use axum::http::StatusCode;
use common::{Answer, KEY_CREDENTIAL, KEY_IDENTITY, caller, request, router, seeded_pool, send};
use omeka_db::ids::{ItemId, PropertyId, TemplateId, UserId};
use omeka_db::repos::{ItemRepo, PropertyRepo, TemplateRepo, ValueRepo};
use omeka_db::rows::{NewItem, NewProperty, NewResource, NewTemplate, NewValue};
use sqlx::SqlitePool;

struct Who {
    name: &'static str,
    key: String,
}

impl Who {
    fn uri(&self, path: &str) -> String {
        if self.key.is_empty() {
            return path.to_string();
        }
        format!("{path}&{}", self.key)
    }
}

struct World {
    app: Router,
    callers: [Who; 4],
    template: TemplateId,
    date: PropertyId,
    creator: PropertyId,
    /// On no item of the template.
    subject: PropertyId,
    /// A public item of another template that `creator` points at, for the links filter.
    person: ItemId,
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

async fn item(
    pool: &SqlitePool,
    owner: UserId,
    template: Option<TemplateId>,
    is_public: bool,
) -> ItemId {
    ItemRepo::new(pool)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(owner),
                resource_template_id: template,
                title: Some("an item".to_string()),
                is_public,
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap()
        .id()
}

async fn value(
    pool: &SqlitePool,
    item: ItemId,
    property: PropertyId,
    to: Option<ItemId>,
    is_public: bool,
) {
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: item.as_resource(),
            property_id: property,
            value_resource_id: to.map(ItemId::as_resource),
            value_annotation_id: None,
            r#type: if to.is_some() {
                "resource:item"
            } else {
                "literal"
            }
            .to_string(),
            lang: None,
            value: to.is_none().then(|| "1901".to_string()),
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
    let date = property(&pool, seed.vocabulary, "date").await;
    let creator = property(&pool, seed.vocabulary, "creator").await;
    let subject = property(&pool, seed.vocabulary, "subject").await;
    let template = TemplateRepo::new(&pool)
        .create(&NewTemplate {
            owner_id: Some(admin),
            resource_class_id: None,
            title_property_id: None,
            description_property_id: None,
            label: "Letter".to_string(),
        })
        .await
        .unwrap()
        .id;
    let person = item(&pool, admin, None, true).await;

    let i1 = item(&pool, admin, Some(template), true).await;
    value(&pool, i1, date, None, true).await;
    value(&pool, i1, creator, Some(person), true).await;
    let i2 = item(&pool, admin, Some(template), false).await;
    value(&pool, i2, date, None, true).await;
    let i3 = item(&pool, author.id, Some(template), false).await;
    // Two values on one field fill it once.
    value(&pool, i3, creator, Some(person), true).await;
    value(&pool, i3, creator, Some(person), true).await;
    let i4 = item(&pool, other.id, Some(template), false).await;
    value(&pool, i4, date, None, true).await;
    let i5 = item(&pool, author.id, Some(template), true).await;
    // A private value on a public item: only its owner and view-all callers see it (`ValueVisibilityFilter`).
    value(&pool, i5, date, None, false).await;
    // Another type's item fills nothing here.
    let elsewhere = item(&pool, admin, Some(seed.template), true).await;
    value(&pool, elsewhere, date, None, true).await;
    value(&pool, elsewhere, subject, None, true).await;

    World {
        app: router(pool),
        callers: [
            Who {
                name: "anonymous",
                key: String::new(),
            },
            Who {
                name: "author",
                key: author.key,
            },
            Who {
                name: "other",
                key: other.key,
            },
            Who {
                name: "admin",
                key: format!("key_identity={KEY_IDENTITY}&key_credential={KEY_CREDENTIAL}"),
            },
        ],
        template,
        date,
        creator,
        subject,
        person,
    }
}

async fn read(app: &Router, uri: &str) -> Answer {
    send(app.clone(), request("GET", uri)).await
}

async fn list_total(app: &Router, uri: &str) -> u64 {
    let answer = read(app, uri).await;
    assert_eq!(answer.status, StatusCode::OK, "{uri}: {}", answer.body);
    common::header(&answer, "omeka-s-total-results")
        .expect("a total header")
        .parse()
        .unwrap()
}

/// `total` and `filled` by property id.
fn counts(answer: &Answer) -> (u64, BTreeMap<u64, u64>) {
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let json = answer.json();
    let fields = json["fields"]
        .as_array()
        .expect("a fields array")
        .iter()
        .map(|row| {
            (
                row["property_id"].as_u64().unwrap(),
                row["filled"].as_u64().unwrap(),
            )
        })
        .collect();
    (json["total"].as_u64().expect("a total"), fields)
}

fn counts_uri(template: TemplateId) -> String {
    format!(
        "/api/romeka/field-counts?resource_template_id={}",
        template.0
    )
}

#[tokio::test]
async fn every_count_equals_the_items_list_total_it_opens_for_its_caller() {
    let world = world().await;
    for who in &world.callers {
        let answer = read(&world.app, &who.uri(&counts_uri(world.template))).await;
        assert_eq!(common::header(&answer, "cache-control"), Some("no-store"));
        let (total, filled) = counts(&answer);
        let type_list = format!("/api/items?resource_template_id={}", world.template.0);
        assert_eq!(
            total,
            list_total(&world.app, &who.uri(&type_list)).await,
            "{}: total",
            who.name
        );
        for property in [world.date, world.creator, world.subject] {
            let with = |kind: &str| {
                who.uri(&format!(
                    "{type_list}&property[1][property]={}&property[1][type]={kind}",
                    property.0
                ))
            };
            let has = filled.get(&property.0).copied().unwrap_or(0);
            assert_eq!(
                has,
                list_total(&world.app, &with("ex")).await,
                "{}: filled {}",
                who.name,
                property.0
            );
            assert_eq!(
                total - has,
                list_total(&world.app, &with("nex")).await,
                "{}: empty {}",
                who.name,
                property.0
            );
        }
        assert!(
            filled.values().all(|count| *count > 0),
            "{}: a field no item fills is left out",
            who.name
        );
    }
}

#[tokio::test]
async fn counts_name_only_the_items_the_caller_may_see() {
    let world = world().await;
    let expected = [
        ("anonymous", 2, vec![(world.date, 1), (world.creator, 1)]),
        ("author", 3, vec![(world.date, 2), (world.creator, 2)]),
        ("other", 3, vec![(world.date, 2), (world.creator, 1)]),
        ("admin", 5, vec![(world.date, 4), (world.creator, 2)]),
    ];
    for (who, (name, total, fields)) in world.callers.iter().zip(expected) {
        assert_eq!(who.name, name);
        let answer = read(&world.app, &who.uri(&counts_uri(world.template))).await;
        let fields: BTreeMap<u64, u64> = fields.into_iter().map(|(p, n)| (p.0, n)).collect();
        assert_eq!(counts(&answer), (total, fields), "{name}");
    }
}

#[tokio::test]
async fn a_missing_type_is_404_and_a_request_without_one_is_400() {
    let world = world().await;
    for who in &world.callers {
        for raw in ["999999", "abc", "0"] {
            let uri = format!("/api/romeka/field-counts?resource_template_id={raw}");
            let answer = read(&world.app, &who.uri(&uri)).await;
            assert_eq!(answer.status, StatusCode::NOT_FOUND, "{}: {raw}", who.name);
        }
        let answer = read(&world.app, &who.uri("/api/romeka/field-counts?page=1")).await;
        assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{}", who.name);
    }
}

/// The admin sends the links filter as `property[0]` and the field filter as `property[1]`, so
/// both narrow one list together. A links filter through any field still names `property`, empty:
/// `buildPropertyQuery` skips a row without that key.
#[tokio::test]
async fn the_field_filter_and_the_links_filter_narrow_together() {
    let world = world().await;
    let admin = &world.callers[3];
    let links = format!(
        "/api/items?property[0][property]=&property[0][type]=res&property[0][text]={}",
        world.person.0
    );
    let both = |kind: &str| {
        admin.uri(&format!(
            "{links}&property[1][property]={}&property[1][type]={kind}",
            world.date.0
        ))
    };
    assert_eq!(list_total(&world.app, &admin.uri(&links)).await, 2);
    assert_eq!(
        list_total(&world.app, &both("ex")).await,
        1,
        "i1 has a date"
    );
    assert_eq!(list_total(&world.app, &both("nex")).await, 1, "i3 has none");
}
