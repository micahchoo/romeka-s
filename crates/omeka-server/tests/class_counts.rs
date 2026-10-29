//! `GET /api/romeka/classes/counts`, read through HTTP as anonymous, an author, an editor and an
//! admin, for items (the default), item sets and media.
//!
//! The spec of every number is the list it opens, for the same caller: a class's `count` with
//! `resource_type=<type>` is `Omeka-S-Total-Results` of `GET /api/<type>?resource_class_id=<id>`.
//! A class no resource of that type the caller may see uses is left out, so a class filter offers
//! only classes in use.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::StatusCode;
use common::{Answer, KEY_CREDENTIAL, KEY_IDENTITY, caller, request, router, seeded_pool, send};
use omeka_db::ids::{ClassId, ItemId, UserId, VocabularyId};
use omeka_db::repos::{ClassRepo, ItemRepo, ItemSetRepo, MediaRepo, VocabularyRepo};
use omeka_db::rows::{NewClass, NewItem, NewItemSet, NewMedia, NewResource, NewVocabulary};
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
    agent: ClassId,
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

async fn item(pool: &SqlitePool, owner: UserId, class: Option<ClassId>, is_public: bool) -> ItemId {
    ItemRepo::new(pool)
        .create(&NewItem {
            resource: resource(owner, class, is_public),
            primary_media_id: None,
        })
        .await
        .unwrap()
        .id()
}

async fn item_set(pool: &SqlitePool, owner: UserId, class: Option<ClassId>, is_public: bool) {
    ItemSetRepo::new(pool)
        .create(&NewItemSet {
            resource: resource(owner, class, is_public),
            is_open: true,
        })
        .await
        .unwrap();
}

async fn media(
    pool: &SqlitePool,
    item: ItemId,
    owner: UserId,
    class: Option<ClassId>,
    is_public: bool,
) {
    MediaRepo::new(pool)
        .create(&NewMedia {
            resource: resource(owner, class, is_public),
            item_id: item,
            ingester: "html".to_string(),
            renderer: "html".to_string(),
            ..NewMedia::default()
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
/// an Event only an editor's private item uses, and an Agent no item uses. Item sets and media
/// use other classes than items do, so a count of the wrong type shows.
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
    let agent = class(&pool, schema, "Agent").await;

    item(&pool, author.id, Some(schema_image), false).await;
    let public_item = item(&pool, admin, Some(schema_image), true).await;
    let private_item = item(&pool, editor.id, Some(event), false).await;
    item(&pool, admin, None, true).await;

    item_set(&pool, admin, Some(seed.class), true).await;
    item_set(&pool, author.id, Some(agent), false).await;
    item_set(&pool, admin, Some(agent), true).await;
    item_set(&pool, admin, None, true).await;

    media(&pool, public_item, admin, Some(event), true).await;
    media(&pool, public_item, author.id, Some(event), false).await;
    media(&pool, private_item, editor.id, Some(agent), false).await;
    media(&pool, public_item, admin, None, true).await;

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
        agent,
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
    count: u64,
}

const TYPES: [&str; 3] = ["items", "item_sets", "media"];

async fn counts(world: &World, who: &Who) -> Vec<Row> {
    counts_at(world, who, "/api/romeka/classes/counts").await
}

async fn counts_of(world: &World, who: &Who, resource_type: &str) -> Vec<Row> {
    counts_at(
        world,
        who,
        &format!("/api/romeka/classes/counts?resource_type={resource_type}"),
    )
    .await
}

async fn counts_at(world: &World, who: &Who, path: &str) -> Vec<Row> {
    let answer = read(&world.app, &who.uri(path)).await;
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
            count: entry["count"].as_u64().expect("count"),
        })
        .collect()
}

#[tokio::test]
async fn every_count_equals_the_list_total_it_opens_for_its_caller() {
    let world = world().await;
    for resource_type in TYPES {
        for who in &world.callers {
            let rows = counts_of(&world, who, resource_type).await;
            assert!(!rows.is_empty(), "{resource_type}, {}", who.name);
            for row in rows {
                assert_eq!(
                    row.count,
                    list_total(
                        &world.app,
                        &who.uri(&format!(
                            "/api/{resource_type}?resource_class_id={}",
                            row.id
                        ))
                    )
                    .await,
                    "{resource_type}, {}: class {}",
                    who.name,
                    row.term
                );
            }
        }
    }
}

#[tokio::test]
async fn no_resource_type_counts_items() {
    let world = world().await;
    for who in &world.callers {
        assert_eq!(
            counts(&world, who).await,
            counts_of(&world, who, "items").await,
            "{}",
            who.name
        );
    }
}

#[tokio::test]
async fn a_resource_type_no_list_has_is_a_bad_request() {
    let world = world().await;
    for resource_type in ["sites", "item", ""] {
        let uri = format!("/api/romeka/classes/counts?resource_type={resource_type}");
        let answer = read(&world.app, &world.callers[3].uri(&uri)).await;
        assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{uri}");
    }
}

#[tokio::test]
async fn item_sets_and_media_list_only_their_own_classes_in_use() {
    let world = world().await;
    let admin = &world.callers[3];
    let ids = |rows: Vec<Row>| {
        rows.into_iter()
            .map(|row| (row.id, row.count))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        ids(counts_of(&world, admin, "item_sets").await),
        vec![(world.agent.0, 2), (world.dctype_image.0, 1)],
        "by label: Agent, then Image"
    );
    assert_eq!(
        ids(counts_of(&world, admin, "media").await),
        vec![(world.agent.0, 1), (world.event.0, 2)]
    );
    assert_eq!(
        ids(counts_of(&world, &world.callers[0], "media").await),
        vec![(world.event.0, 1)],
        "anonymous sees the public media only"
    );
}

#[tokio::test]
async fn only_classes_in_use_are_listed_by_label_with_their_standard() {
    let world = world().await;
    let got = counts(&world, &world.callers[3]).await;
    let row = |id: ClassId, term: &str, label: &str, vocabulary: &str, count: u64| Row {
        id: id.0,
        term: term.to_string(),
        label: label.to_string(),
        vocabulary_label: vocabulary.to_string(),
        count,
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
        "Agent has no item: only item sets and media use it"
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
                .map_or(0, |row| row.count)
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
        assert!(got.iter().all(|row| row.count > 0), "{name}: {got:?}");
    }
}

/// The route's latency at 40,000 resources of each type, as anonymous and as an admin. Run it with
/// `cargo test -p omeka-server --test class_counts -- --ignored --nocapture`.
#[tokio::test]
#[ignore = "a measurement, not a check"]
async fn class_counts_at_40000_resources_of_each_type() {
    const ROWS: i64 = 40_000;
    let (pool, seed) = seeded_pool().await;
    let mut tx = pool.begin().await.unwrap();
    let first: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(id), 0) + 1 FROM resource")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let classes: Vec<i64> = sqlx::query_scalar("SELECT id FROM resource_class ORDER BY id LIMIT 5")
        .fetch_all(&mut *tx)
        .await
        .unwrap();
    let class_of = classes
        .iter()
        .enumerate()
        .map(|(at, id)| format!("WHEN {at} THEN {id}"))
        .collect::<Vec<_>>()
        .join(" ");
    // Items take ids [first, first + ROWS), item sets the next ROWS, media the last ROWS; every
    // third row is private, and the classes cycle over the seed's first five.
    for (offset, kind) in [
        (0, "Omeka\\Entity\\Item"),
        (ROWS, "Omeka\\Entity\\ItemSet"),
        (2 * ROWS, "Omeka\\Entity\\Media"),
    ] {
        let sql = format!(
            "WITH RECURSIVE n(i) AS (SELECT 0 UNION ALL SELECT i + 1 FROM n WHERE i + 1 < ?) \
             INSERT INTO resource (id, owner_id, resource_class_id, title, is_public, created, \
             resource_type) SELECT ? + i, ?, CASE i % {} {class_of} END, 'a record', \
             i % 3 <> 0, '2026-10-29T00:00:00', ? FROM n",
            classes.len()
        );
        sqlx::query(&sql)
            .bind(ROWS)
            .bind(first + offset)
            .bind(seed.user.0 as i64)
            .bind(kind)
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    for (table, offset) in [("item", 0), ("item_set", ROWS), ("media", 2 * ROWS)] {
        let columns = match table {
            "item" => "(id) SELECT id",
            "item_set" => "(id, is_open) SELECT id, 1",
            _ => {
                "(id, item_id, ingester, renderer, has_original, has_thumbnails) \
                 SELECT id, id - ? , 'html', 'html', 0, 0"
            }
        };
        let sql = format!("INSERT INTO {table} {columns} FROM resource WHERE id >= ? AND id < ?");
        let mut query = sqlx::query(&sql);
        if table == "media" {
            query = query.bind(2 * ROWS);
        }
        query
            .bind(first + offset)
            .bind(first + offset + ROWS)
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();
    let app = router(pool);
    let admin = format!("key_identity={KEY_IDENTITY}&key_credential={KEY_CREDENTIAL}");
    for resource_type in TYPES {
        for (who, key) in [("anonymous", ""), ("admin", admin.as_str())] {
            let uri = format!("/api/romeka/classes/counts?resource_type={resource_type}&{key}");
            let mut runs = Vec::new();
            let mut answer = None;
            for _ in 0..11 {
                let started = std::time::Instant::now();
                let read = read(&app, &uri).await;
                runs.push(started.elapsed());
                assert_eq!(read.status, StatusCode::OK);
                answer = Some(read);
            }
            if who == "admin" {
                let counted: u64 = answer.expect("an answer").json()["classes"]
                    .as_array()
                    .expect("classes")
                    .iter()
                    .map(|row| row["count"].as_u64().expect("count"))
                    .sum();
                assert!(
                    (ROWS as u64..=ROWS as u64 + 2).contains(&counted),
                    "{resource_type}: the seeded rows of this type, plus the seed's own: {counted}"
                );
            }
            runs.sort();
            println!(
                "{resource_type}, {who}, {ROWS} rows: median {:?} of 11",
                runs[runs.len() / 2]
            );
        }
    }
}
