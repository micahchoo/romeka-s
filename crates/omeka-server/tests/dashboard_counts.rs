//! `GET /api/romeka/dashboard`, read through HTTP as anonymous, an author, an editor and an admin.
//!
//! The spec of every number is the Items list it opens, for the same caller: `no_item_set` is
//! `Omeka-S-Total-Results` of `GET /api/items?romeka_no_item_set=1`, `no_resource_template` of
//! `?romeka_no_resource_template=1`, and `changed_by_others.items` of
//! `?romeka_changed_by_others_since=<changed_by_others.since>`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::StatusCode;
use chrono::{Duration, Utc};
use common::{Answer, KEY_CREDENTIAL, KEY_IDENTITY, caller, request, router, seeded_pool, send};
use omeka_db::ids::{ItemId, ItemSetId, TemplateId, UserId};
use omeka_db::repos::ItemRepo;
use omeka_db::rows::{NewItem, NewResource};
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
}

async fn item(
    pool: &SqlitePool,
    owner: UserId,
    template: Option<TemplateId>,
    set: Option<ItemSetId>,
    is_public: bool,
) -> ItemId {
    let id = ItemRepo::new(pool)
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
        .id();
    if let Some(set) = set {
        ItemRepo::new(pool).add_to_item_set(id, set).await.unwrap();
    }
    id
}

/// One history entry `days` ago by `user` (`None` is a system job).
async fn changed(pool: &SqlitePool, item: ItemId, user: Option<UserId>, days: i64) {
    let created = (Utc::now() - Duration::days(days))
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    sqlx::query(
        "INSERT INTO romeka_item_history (item_id, created, user_id, source, changes)
         VALUES (?, ?, ?, 'edit', '[]')",
    )
    .bind(i64::try_from(item.0).unwrap())
    .bind(created)
    .bind(user.map(|user| i64::try_from(user.0).unwrap()))
    .execute(pool)
    .await
    .unwrap();
}

/// The seed's item is public, typed and in the seed's set, with no history: no count names it.
async fn world() -> World {
    let (pool, seed) = seeded_pool().await;
    let admin = seed.user;
    let author = caller(&pool, "author").await;
    let editor = caller(&pool, "editor").await;
    let (set, template) = (Some(seed.item_set), Some(seed.template));

    let a1 = item(&pool, admin, template, set, true).await;
    let a2 = item(&pool, admin, None, None, false).await;
    let u1 = item(&pool, author.id, template, None, false).await;
    let u2 = item(&pool, author.id, None, set, true).await;
    let e1 = item(&pool, editor.id, None, None, false).await;

    changed(&pool, a1, Some(author.id), 2).await;
    changed(&pool, u1, Some(admin), 1).await;
    changed(&pool, u2, None, 3).await;
    changed(&pool, u2, Some(admin), 1).await;
    changed(&pool, u2, Some(admin), 1).await;
    // Older than seven days: no caller counts it.
    changed(&pool, a2, Some(editor.id), 10).await;
    changed(&pool, a2, Some(admin), 1).await;
    changed(&pool, e1, Some(editor.id), 1).await;

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

/// `(no_item_set, no_resource_template, changed_by_others.items, changed_by_others.since)`.
async fn dashboard(world: &World, who: &Who) -> (u64, u64, u64, String) {
    let answer = read(&world.app, &who.uri("/api/romeka/dashboard")).await;
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "{}: {}",
        who.name,
        answer.body
    );
    assert_eq!(common::header(&answer, "cache-control"), Some("no-store"));
    let json = answer.json();
    (
        json["no_item_set"].as_u64().expect("no_item_set"),
        json["no_resource_template"]
            .as_u64()
            .expect("no_resource_template"),
        json["changed_by_others"]["items"]
            .as_u64()
            .expect("changed_by_others.items"),
        json["changed_by_others"]["since"]
            .as_str()
            .expect("changed_by_others.since")
            .to_string(),
    )
}

/// The query-string form of a value: every byte but an unreserved one percent-encoded.
fn encoded(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                char::from(byte).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[tokio::test]
async fn every_count_equals_the_items_list_total_it_opens_for_its_caller() {
    let world = world().await;
    for who in &world.callers {
        let (no_set, no_type, changed, since) = dashboard(&world, who).await;
        assert_eq!(
            no_set,
            list_total(&world.app, &who.uri("/api/items?romeka_no_item_set=1")).await,
            "{}: no item set",
            who.name
        );
        assert_eq!(
            no_type,
            list_total(
                &world.app,
                &who.uri("/api/items?romeka_no_resource_template=1")
            )
            .await,
            "{}: no item type",
            who.name
        );
        let changed_list = format!(
            "/api/items?romeka_changed_by_others_since={}",
            encoded(&since)
        );
        assert_eq!(
            changed,
            list_total(&world.app, &who.uri(&changed_list)).await,
            "{}: changed by others",
            who.name
        );
    }
}

#[tokio::test]
async fn counts_name_only_what_the_caller_may_see_and_what_others_changed() {
    let world = world().await;
    let expected = [
        // Public items only; changes are for people who sign in.
        ("anonymous", 0, 1, 0),
        // a1, u1 and u2 are hers to see; a1's change is her own.
        ("author", 1, 1, 2),
        // Every item; e1's change is his own.
        ("editor", 3, 3, 4),
        // Every item; a2's change by someone else is older than seven days.
        ("admin", 3, 3, 3),
    ];
    for (who, (name, no_set, no_type, changed)) in world.callers.iter().zip(expected) {
        assert_eq!(who.name, name);
        let (got_set, got_type, got_changed, _) = dashboard(&world, who).await;
        assert_eq!(
            (got_set, got_type, got_changed),
            (no_set, no_type, changed),
            "{name}"
        );
    }
}

#[tokio::test]
async fn since_is_seven_days_before_now() {
    let world = world().await;
    let (_, _, _, since) = dashboard(&world, &world.callers[3]).await;
    let since = chrono::DateTime::parse_from_rfc3339(&since)
        .expect("an RFC 3339 time")
        .with_timezone(&Utc);
    let age = Utc::now() - since;
    assert!(
        age >= Duration::days(7) && age < Duration::days(7) + Duration::minutes(1),
        "{age}"
    );
}

#[tokio::test]
async fn a_since_that_is_not_a_time_keeps_no_item() {
    let world = world().await;
    let admin = &world.callers[3];
    let uri = admin.uri("/api/items?romeka_changed_by_others_since=last-week");
    assert_eq!(list_total(&world.app, &uri).await, 0);
    let day = (Utc::now() - Duration::days(7))
        .format("%Y-%m-%d")
        .to_string();
    let uri = admin.uri(&format!("/api/items?romeka_changed_by_others_since={day}"));
    assert_eq!(
        list_total(&world.app, &uri).await,
        3,
        "a date reads as its midnight"
    );
}

#[tokio::test]
async fn the_romeka_filters_narrow_together_with_upstream_ones() {
    let world = world().await;
    let admin = &world.callers[3];
    let both = admin.uri("/api/items?romeka_no_item_set=1&romeka_no_resource_template=1");
    assert_eq!(list_total(&world.app, &both).await, 2, "a2 and e1");
    let public = admin.uri("/api/items?romeka_no_resource_template=1&is_public=1");
    assert_eq!(list_total(&world.app, &public).await, 1, "u2");
}

/// The dashboard's latency on 50 000 items, in-memory SQLite. Run in release:
/// `cargo test --release -p omeka-server --test dashboard_counts measure -- --ignored --nocapture`.
#[tokio::test]
#[ignore = "a measurement, not a check"]
async fn measure_the_dashboard_on_50000_items() {
    const ITEMS: i64 = 50_000;
    let (pool, seed) = seeded_pool().await;
    let author = caller(&pool, "author").await;
    let now = Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    let first: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(id), 0) + 1 FROM resource")
        .fetch_one(&pool)
        .await
        .unwrap();
    // Every third item is the author's, every fourth private, every second typed, every third
    // in the seed's set; every fifth was changed this week by the other person.
    sqlx::query(
        "WITH RECURSIVE n(i) AS (SELECT 0 UNION ALL SELECT i + 1 FROM n WHERE i + 1 < ?)
         INSERT INTO resource (id, owner_id, resource_template_id, title, is_public, created,
                               resource_type)
         SELECT ? + i, CASE WHEN i % 3 = 0 THEN ? ELSE ? END,
                CASE WHEN i % 2 = 0 THEN ? END, 'an item', i % 4 <> 0, ?, 'Omeka\\Entity\\Item'
         FROM n",
    )
    .bind(ITEMS)
    .bind(first)
    .bind(i64::try_from(author.id.0).unwrap())
    .bind(i64::try_from(seed.user.0).unwrap())
    .bind(i64::try_from(seed.template.0).unwrap())
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO item (id) SELECT id FROM resource WHERE id >= ?")
        .bind(first)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO item_item_set (item_id, item_set_id)
         SELECT id, ? FROM resource WHERE id >= ? AND (id - ?) % 3 = 1",
    )
    .bind(i64::try_from(seed.item_set.0).unwrap())
    .bind(first)
    .bind(first)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO romeka_item_history (item_id, created, user_id, source, changes)
         SELECT r.id, ?, CASE WHEN r.owner_id = ? THEN ? ELSE ? END, 'edit', '[]'
         FROM resource r WHERE r.id >= ? AND (r.id - ?) % 5 = 0",
    )
    .bind(&now)
    .bind(i64::try_from(author.id.0).unwrap())
    .bind(i64::try_from(seed.user.0).unwrap())
    .bind(i64::try_from(author.id.0).unwrap())
    .bind(first)
    .bind(first)
    .execute(&pool)
    .await
    .unwrap();
    let app = router(pool);
    let admin = format!("key_identity={KEY_IDENTITY}&key_credential={KEY_CREDENTIAL}");
    for (name, key) in [("admin", admin), ("author", author.key.clone())] {
        let uri = Who { name, key }.uri("/api/romeka/dashboard");
        let mut times = Vec::new();
        let mut body = String::new();
        for _ in 0..7 {
            let started = std::time::Instant::now();
            let answer = read(&app, &uri).await;
            times.push(started.elapsed());
            assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
            body = answer.body.clone();
        }
        times.sort();
        println!(
            "{name}: median {:?}, min {:?}, max {:?}: {body}",
            times[3], times[0], times[6]
        );
    }
}
