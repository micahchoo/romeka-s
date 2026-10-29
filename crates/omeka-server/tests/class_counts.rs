//! `GET /api/romeka/classes/counts`, read through HTTP as anonymous, an author, an editor and an
//! admin.
//!
//! The spec of every number is the Items list it opens, for the same caller: a class's `items` is
//! `Omeka-S-Total-Results` of `GET /api/items?resource_class_id=<id>`. A class no item the caller
//! may see uses is left out, so the item picker's class filter offers only classes in use.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::StatusCode;
use common::{Answer, KEY_CREDENTIAL, KEY_IDENTITY, caller, request, router, seeded_pool, send};
use omeka_db::ids::{ClassId, UserId, VocabularyId};
use omeka_db::repos::{ClassRepo, ItemRepo, ItemSetRepo, VocabularyRepo};
use omeka_db::rows::{NewClass, NewItem, NewItemSet, NewResource, NewVocabulary};
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
        let joiner = if path.contains('?') { '&' } else { '?' };
        format!("{path}{joiner}{}", self.key)
    }
}

struct World {
    app: Router,
    callers: [Who; 4],
    dctype_image: ClassId,
    schema_image: ClassId,
    event: ClassId,
}

fn resource(owner: UserId, class: Option<ClassId>, is_public: bool) -> NewResource {
    NewResource {
        owner_id: Some(owner),
        resource_class_id: class,
        title: Some("a record".to_string()),
        is_public,
        ..NewResource::default()
    }
}

async fn item(pool: &SqlitePool, owner: UserId, class: Option<ClassId>, is_public: bool) {
    ItemRepo::new(pool)
        .create(&NewItem {
            resource: resource(owner, class, is_public),
            primary_media_id: None,
        })
        .await
        .unwrap();
}

async fn class(pool: &SqlitePool, vocabulary: VocabularyId, local_name: &str) -> ClassId {
    ClassRepo::new(pool)
        .create(&NewClass {
            owner_id: None,
            vocabulary_id: vocabulary,
            local_name: local_name.to_string(),
            label: local_name.to_string(),
            comment: None,
        })
        .await
        .unwrap()
        .id
}

/// The seed's item is public, of class Image in Dublin Core Type. Schema.org adds a second Image,
/// an Event only an editor's private item uses, and an Agent no item uses.
async fn world() -> World {
    let (pool, seed) = seeded_pool().await;
    let admin = seed.user;
    let author = caller(&pool, "author").await;
    let editor = caller(&pool, "editor").await;
    let schema = VocabularyRepo::new(&pool)
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: "https://schema.org/".to_string(),
            prefix: "schema".to_string(),
            label: "Schema.org".to_string(),
            comment: None,
        })
        .await
        .unwrap()
        .id;
    let schema_image = class(&pool, schema, "Image").await;
    let event = class(&pool, schema, "Event").await;
    class(&pool, schema, "Agent").await;

    item(&pool, author.id, Some(schema_image), false).await;
    item(&pool, admin, Some(schema_image), true).await;
    item(&pool, editor.id, Some(event), false).await;
    item(&pool, admin, None, true).await;
    // An item set is not an item: its class is not counted.
    ItemSetRepo::new(&pool)
        .create(&NewItemSet {
            resource: resource(admin, Some(seed.class), true),
            is_open: true,
        })
        .await
        .unwrap();

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
                name: "editor",
                key: editor.key,
            },
            Who {
                name: "admin",
                key: format!("key_identity={KEY_IDENTITY}&key_credential={KEY_CREDENTIAL}"),
            },
        ],
        dctype_image: seed.class,
        schema_image,
        event,
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

#[derive(Debug, PartialEq, Eq)]
struct Row {
    id: u64,
    term: String,
    label: String,
    vocabulary_label: String,
    items: u64,
}

async fn counts(world: &World, who: &Who) -> Vec<Row> {
    let answer = read(&world.app, &who.uri("/api/romeka/classes/counts")).await;
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "{}: {}",
        who.name,
        answer.body
    );
    assert_eq!(common::header(&answer, "cache-control"), Some("no-store"));
    answer.json()["classes"]
        .as_array()
        .expect("classes")
        .iter()
        .map(|entry| Row {
            id: entry["id"].as_u64().expect("id"),
            term: entry["term"].as_str().expect("term").to_string(),
            label: entry["label"].as_str().expect("label").to_string(),
            vocabulary_label: entry["vocabulary_label"]
                .as_str()
                .expect("vocabulary_label")
                .to_string(),
            items: entry["items"].as_u64().expect("items"),
        })
        .collect()
}

#[tokio::test]
async fn every_count_equals_the_items_list_total_it_opens_for_its_caller() {
    let world = world().await;
    for who in &world.callers {
        for row in counts(&world, who).await {
            assert_eq!(
                row.items,
                list_total(
                    &world.app,
                    &who.uri(&format!("/api/items?resource_class_id={}", row.id))
                )
                .await,
                "{}: class {}",
                who.name,
                row.term
            );
        }
    }
}

#[tokio::test]
async fn only_classes_in_use_are_listed_by_label_with_their_standard() {
    let world = world().await;
    let got = counts(&world, &world.callers[3]).await;
    let row = |id: ClassId, term: &str, label: &str, vocabulary: &str, items: u64| Row {
        id: id.0,
        term: term.to_string(),
        label: label.to_string(),
        vocabulary_label: vocabulary.to_string(),
        items,
    };
    assert_eq!(
        got,
        vec![
            row(world.event, "schema:Event", "Event", "Schema.org", 1),
            row(
                world.dctype_image,
                "dctype:Image",
                "Image",
                "Dublin Core Type",
                1
            ),
            row(world.schema_image, "schema:Image", "Image", "Schema.org", 2),
        ],
        "Agent has no item, and the item set's class is not an item's"
    );
}

#[tokio::test]
async fn counts_name_only_what_the_caller_may_see() {
    let world = world().await;
    // (event, dctype image, schema image); a class at 0 is not listed.
    let expected = [
        ("anonymous", (0, 1, 1)),
        ("author", (0, 1, 2)),
        ("editor", (1, 1, 2)),
        ("admin", (1, 1, 2)),
    ];
    for (who, (name, (event, dctype, schema))) in world.callers.iter().zip(expected) {
        assert_eq!(who.name, name);
        let got = counts(&world, who).await;
        let of = |id: ClassId| {
            got.iter()
                .find(|row| row.id == id.0)
                .map_or(0, |row| row.items)
        };
        assert_eq!(
            (
                of(world.event),
                of(world.dctype_image),
                of(world.schema_image)
            ),
            (event, dctype, schema),
            "{name}"
        );
        assert!(got.iter().all(|row| row.items > 0), "{name}: {got:?}");
    }
}
