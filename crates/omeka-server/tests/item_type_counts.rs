//! `GET /api/romeka/item-types/counts`, read through HTTP as anonymous, an author, an editor and
//! an admin.
//!
//! The spec of every number is the Items list it opens, for the same caller: an item type's
//! `items` is `Omeka-S-Total-Results` of `GET /api/items?resource_template_id=<id>`,
//! `no_item_type` of `?romeka_no_resource_template=1`, and `with_class` of
//! `?romeka_has_resource_class=1`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::StatusCode;
use common::{Answer, KEY_CREDENTIAL, KEY_IDENTITY, caller, request, router, seeded_pool, send};
use omeka_db::ids::{ClassId, TemplateId, UserId};
use omeka_db::repos::{ItemRepo, TemplateRepo};
use omeka_db::rows::{NewItem, NewResource, NewTemplate};
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
    picture: TemplateId,
    letter: TemplateId,
    unused: TemplateId,
}

async fn item(
    pool: &SqlitePool,
    owner: UserId,
    template: Option<TemplateId>,
    class: Option<ClassId>,
    is_public: bool,
) {
    ItemRepo::new(pool)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(owner),
                resource_template_id: template,
                resource_class_id: class,
                title: Some("an item".to_string()),
                is_public,
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap();
}

async fn template(pool: &SqlitePool, owner: UserId, label: &str) -> TemplateId {
    TemplateRepo::new(pool)
        .create(&NewTemplate {
            owner_id: Some(owner),
            resource_class_id: None,
            title_property_id: None,
            description_property_id: None,
            label: label.to_string(),
        })
        .await
        .unwrap()
        .id
}

/// The seed's item is public, a Picture, of class Image.
async fn world() -> World {
    let (pool, seed) = seeded_pool().await;
    let admin = seed.user;
    let author = caller(&pool, "author").await;
    let editor = caller(&pool, "editor").await;
    let letter = template(&pool, admin, "Letter").await;
    let unused = template(&pool, admin, "Map").await;
    let picture = Some(seed.template);

    item(&pool, admin, picture, None, false).await;
    item(&pool, author.id, Some(letter), Some(seed.class), false).await;
    item(&pool, author.id, None, None, true).await;
    item(&pool, editor.id, None, Some(seed.class), false).await;
    item(&pool, admin, Some(letter), None, true).await;

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
        picture: seed.template,
        letter,
        unused,
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

struct Counts {
    /// `(template id, items)` in the order the route answers them.
    item_types: Vec<(u64, u64)>,
    no_item_type: u64,
    with_class: u64,
}

impl Counts {
    fn of(&self, id: TemplateId) -> u64 {
        self.item_types
            .iter()
            .find(|(type_id, _)| *type_id == id.0)
            .map(|(_, items)| *items)
            .expect("every item type is counted")
    }
}

async fn counts(world: &World, who: &Who) -> Counts {
    let answer = read(&world.app, &who.uri("/api/romeka/item-types/counts")).await;
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "{}: {}",
        who.name,
        answer.body
    );
    assert_eq!(common::header(&answer, "cache-control"), Some("no-store"));
    let json = answer.json();
    Counts {
        item_types: json["item_types"]
            .as_array()
            .expect("item_types")
            .iter()
            .map(|entry| {
                (
                    entry["id"].as_u64().expect("id"),
                    entry["items"].as_u64().expect("items"),
                )
            })
            .collect(),
        no_item_type: json["no_item_type"].as_u64().expect("no_item_type"),
        with_class: json["with_class"].as_u64().expect("with_class"),
    }
}

#[tokio::test]
async fn every_count_equals_the_items_list_total_it_opens_for_its_caller() {
    let world = world().await;
    for who in &world.callers {
        let got = counts(&world, who).await;
        for (id, items) in &got.item_types {
            assert_eq!(
                *items,
                list_total(
                    &world.app,
                    &who.uri(&format!("/api/items?resource_template_id={id}"))
                )
                .await,
                "{}: item type {id}",
                who.name
            );
        }
        assert_eq!(
            got.no_item_type,
            list_total(
                &world.app,
                &who.uri("/api/items?romeka_no_resource_template=1")
            )
            .await,
            "{}: no item type",
            who.name
        );
        assert_eq!(
            got.with_class,
            list_total(
                &world.app,
                &who.uri("/api/items?romeka_has_resource_class=1")
            )
            .await,
            "{}: with a class",
            who.name
        );
    }
}

#[tokio::test]
async fn every_item_type_is_counted_in_id_order_with_an_unused_one_at_zero() {
    let world = world().await;
    let got = counts(&world, &world.callers[3]).await;
    let ids: Vec<u64> = got.item_types.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, vec![world.picture.0, world.letter.0, world.unused.0]);
    assert_eq!(got.of(world.unused), 0);
}

#[tokio::test]
async fn counts_name_only_what_the_caller_may_see() {
    let world = world().await;
    // (picture, letter, no item type, with a class)
    let expected = [
        // The seed's item and the public letter and untyped item.
        ("anonymous", (1, 1, 1, 1)),
        // Public items and her own: her letter has a class.
        ("author", (1, 2, 1, 2)),
        // Every item.
        ("editor", (2, 2, 2, 3)),
        ("admin", (2, 2, 2, 3)),
    ];
    for (who, (name, (picture, letter, untyped, classed))) in world.callers.iter().zip(expected) {
        assert_eq!(who.name, name);
        let got = counts(&world, who).await;
        assert_eq!(
            (
                got.of(world.picture),
                got.of(world.letter),
                got.no_item_type,
                got.with_class
            ),
            (picture, letter, untyped, classed),
            "{name}"
        );
    }
}

#[tokio::test]
async fn the_class_filter_narrows_together_with_upstream_ones() {
    let world = world().await;
    let admin = &world.callers[3];
    let typed = admin.uri(&format!(
        "/api/items?romeka_has_resource_class=1&resource_template_id={}",
        world.letter.0
    ));
    assert_eq!(
        list_total(&world.app, &typed).await,
        1,
        "the author's letter"
    );
    let public = admin.uri("/api/items?romeka_has_resource_class=1&is_public=1");
    assert_eq!(list_total(&world.app, &public).await, 1, "the seed's item");
}
