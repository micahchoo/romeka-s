//! `GET /api/romeka/items.csv?<item search query>`, read through HTTP as four callers: anonymous,
//! an author and a researcher who each own a private item, and an admin.
//!
//! The spec of the file is the Items list it stands for: one data row per item that
//! `GET /api/items?<same query>` counts in `Omeka-S-Total-Results`, for the same caller, whatever
//! page the query names.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::StatusCode;
use common::{Answer, KEY_CREDENTIAL, KEY_IDENTITY, caller, request, router, seeded_pool, send};
use omeka_db::ids::{ItemId, ItemSetId, PropertyId, UserId};
use omeka_db::repos::{ItemRepo, ItemSetRepo, PropertyRepo, ValueRepo};
use omeka_db::rows::{NewItem, NewItemSet, NewProperty, NewResource, NewValue};
use sqlx::SqlitePool;

struct World {
    app: Router,
    /// Each caller's name and the query string that authenticates them.
    callers: [(&'static str, String); 4],
    set: ItemSetId,
    /// Two items, each with a title, for the ticked-rows query.
    ticked: [ItemId; 2],
}

fn resource(owner: UserId, title: &str, is_public: bool) -> NewResource {
    NewResource {
        owner_id: Some(owner),
        title: Some(title.to_string()),
        is_public,
        ..NewResource::default()
    }
}

async fn value(pool: &SqlitePool, item: ItemId, property: PropertyId, text: &str, is_public: bool) {
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: item.as_resource(),
            property_id: property,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some(text.to_string()),
            uri: None,
            is_public,
        })
        .await
        .unwrap();
}

/// One item with a title value, in `sets`.
async fn item(
    pool: &SqlitePool,
    title_property: PropertyId,
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
    value(pool, id, title_property, title, true).await;
    id
}

async fn world() -> World {
    let (pool, seed) = seeded_pool().await;
    let author = caller(&pool, "author").await;
    let other = caller(&pool, "researcher").await;
    let admin = seed.user;
    let title: (i64,) =
        sqlx::query_as("SELECT id FROM property WHERE local_name = 'title' AND vocabulary_id = ?")
            .bind(i64::try_from(seed.vocabulary.0).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    let title = PropertyId(u64::try_from(title.0).unwrap());
    let creator = PropertyRepo::new(&pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: seed.vocabulary,
            local_name: "creator".to_string(),
            label: "Creator".to_string(),
            comment: None,
        })
        .await
        .unwrap()
        .id;
    let set = ItemSetRepo::new(&pool)
        .create(&NewItemSet {
            resource: resource(admin, "Public set", true),
            is_open: true,
        })
        .await
        .unwrap()
        .id();

    let public = item(&pool, title, admin, "Public with creators", true, &[set]).await;
    value(&pool, public, creator, "Ada, Countess", true).await;
    value(&pool, public, creator, "A private creator", false).await;
    item(&pool, title, admin, "Admin's private", false, &[set]).await;
    let authors = item(&pool, title, author.id, "Author's private", false, &[set]).await;
    item(
        &pool,
        title,
        other.id,
        "Researcher's private",
        false,
        &[set],
    )
    .await;
    item(&pool, title, author.id, "Author's public", true, &[]).await;

    World {
        app: router(pool),
        callers: [
            ("anonymous", String::new()),
            ("author", author.key),
            ("researcher", other.key),
            (
                "admin",
                format!("key_identity={KEY_IDENTITY}&key_credential={KEY_CREDENTIAL}"),
            ),
        ],
        set,
        ticked: [public, authors],
    }
}

fn with_key(path: &str, key: &str) -> String {
    if key.is_empty() {
        return path.to_string();
    }
    let separator = if path.contains('?') { '&' } else { '?' };
    format!("{path}{separator}{key}")
}

async fn read(app: &Router, uri: &str) -> Answer {
    send(app.clone(), request("GET", uri)).await
}

async fn list_total(app: &Router, uri: &str) -> usize {
    let answer = read(app, uri).await;
    assert_eq!(answer.status, StatusCode::OK, "{uri}: {}", answer.body);
    common::header(&answer, "omeka-s-total-results")
        .expect("a total header")
        .parse()
        .unwrap()
}

/// The file's records: the byte order mark taken off, one record per CRLF. No fixture value holds
/// a line break, so a split is a parse.
fn records(answer: &Answer) -> Vec<String> {
    let text = answer
        .body
        .strip_prefix('\u{feff}')
        .expect("a byte order mark");
    let mut lines: Vec<String> = text.split("\r\n").map(str::to_string).collect();
    assert_eq!(
        lines.pop().as_deref(),
        Some(""),
        "the file ends with a CRLF"
    );
    lines
}

/// The title cell of each data row.
fn titles(answer: &Answer) -> Vec<String> {
    let records = records(answer);
    assert!(records[0].starts_with("dcterms:title"), "{}", records[0]);
    records[1..]
        .iter()
        .map(|line| line.split(',').next().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn the_file_has_one_row_per_item_the_list_counts_for_each_caller() {
    let world = world().await;
    let set = world.set.0;
    let [first, second] = world.ticked;
    let queries = [
        String::new(),
        format!("item_set_id={set}"),
        "sort_by=id&sort_order=desc".to_string(),
        "fulltext_search=private".to_string(),
        "search=private".to_string(),
        "is_public=0".to_string(),
        format!("item_set_id={set}&page=2&per_page=1"),
        format!("id={},{}", first.0, second.0),
    ];
    for (name, key) in &world.callers {
        for query in &queries {
            let total =
                list_total(&world.app, &with_key(&format!("/api/items?{query}"), key)).await;
            let answer = read(
                &world.app,
                &with_key(&format!("/api/romeka/items.csv?{query}"), key),
            )
            .await;
            assert_eq!(
                answer.status,
                StatusCode::OK,
                "{name} {query}: {}",
                answer.body
            );
            let rows = records(&answer).len().saturating_sub(1);
            assert_eq!(rows, total, "{name} {query}: rows against the list total");
        }
    }
}

#[tokio::test]
async fn a_private_item_reaches_only_a_caller_who_may_see_it() {
    let world = world().await;
    let query = format!("/api/romeka/items.csv?item_set_id={}", world.set.0);
    let expected: [&[&str]; 4] = [
        &["Public with creators"],
        &["Public with creators", "Author's private"],
        &["Public with creators", "Researcher's private"],
        &[
            "Public with creators",
            "Admin's private",
            "Author's private",
            "Researcher's private",
        ],
    ];
    for ((name, key), expected) in world.callers.iter().zip(expected) {
        let answer = read(&world.app, &with_key(&query, key)).await;
        assert_eq!(titles(&answer), expected, "{name}");
    }
}

/// The file carries the values the list carries: the same item read from `/api/items` and from the
/// file names the same creators, for each caller, a comma in a value quoted.
#[tokio::test]
async fn the_file_carries_the_values_the_list_carries() {
    let world = world().await;
    let id = world.ticked[0].0;
    for (name, key) in &world.callers {
        let list = read(&world.app, &with_key(&format!("/api/items?id={id}"), key)).await;
        let creators: Vec<String> = list.json()[0]["dcterms:creator"]
            .as_array()
            .expect("creators")
            .iter()
            .map(|value| value["@value"].as_str().unwrap().to_string())
            .collect();
        let quoted: Vec<String> = creators
            .iter()
            .map(|text| {
                if text.contains(',') {
                    format!("\"{text}\"")
                } else {
                    text.clone()
                }
            })
            .collect();
        let mut header = vec!["dcterms:title"];
        header.extend(creators.iter().map(|_| "dcterms:creator"));
        let file = read(
            &world.app,
            &with_key(&format!("/api/romeka/items.csv?id={id}"), key),
        )
        .await;
        assert_eq!(
            records(&file),
            [
                header.join(","),
                format!("Public with creators,{}", quoted.join(","))
            ],
            "{name}"
        );
    }
}

#[tokio::test]
async fn the_file_is_a_dated_attachment_no_cache_keeps() {
    let world = world().await;
    let answer = read(&world.app, "/api/romeka/items.csv").await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(
        common::header(&answer, "content-type"),
        Some("text/csv; charset=utf-8")
    );
    assert_eq!(common::header(&answer, "cache-control"), Some("no-store"));
    let today = chrono::Utc::now().date_naive();
    assert_eq!(
        common::header(&answer, "content-disposition"),
        Some(format!("attachment; filename=\"items-{today}.csv\"").as_str())
    );
}
