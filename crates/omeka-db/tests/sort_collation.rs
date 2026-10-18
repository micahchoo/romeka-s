//! The upstream text order for a mixed-case set.
//!
//! `GET /api/items?sort_by=title&sort_order=asc` puts `annotated TOKEN` before
//! `Item with media rows` upstream and after it in the port, because the port's sorter compared
//! bytes. These tests pin the order the port's text sorts must produce, taken from the oracle's
//! captured answer (`x_sort_title` and `x_users_sort_email` in the extended probe, 2026-10-17).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::repos::{
    ItemFilter, ItemRepo, Paging, QueryFilter, UserRepo, compare_opt_text_ci, compare_text_ci,
};
use omeka_db::rows::{NewItem, NewResource, NewUser};
use sqlx::Row;

/// The probe's `x_sort_title` item set, in the id order the repository returns.
///
/// Titles are the ones the golden carries; two items have no title, as items 1 and 6 do upstream.
const TITLES: [Option<&str>; 7] = [
    None,
    Some("URI value item updated"),
    Some("Resource value item"),
    Some("Item with media rows"),
    Some("Private item TOKEN"),
    None,
    Some("annotated TOKEN"),
];

/// The id order the oracle answers for that set, ascending by title.
const UPSTREAM_ITEM_ORDER: [u64; 7] = [1, 6, 7, 4, 5, 3, 2];

async fn seed_items(pool: &sqlx::SqlitePool) -> Vec<omeka_db::rows::ItemRow> {
    let items = ItemRepo::new(pool);
    for title in TITLES {
        items
            .create(&NewItem {
                resource: NewResource {
                    title: title.map(str::to_string),
                    ..Default::default()
                },
                ..Default::default()
            })
            .await
            .unwrap();
    }
    items
        .list_filtered(
            &ItemFilter::default(),
            &QueryFilter::default(),
            &Paging::default(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn item_title_sort_folds_case_like_upstream() {
    let pool = common::memory().await;
    let mut rows = seed_items(&pool).await;

    rows.sort_by(|left, right| {
        compare_opt_text_ci(
            left.resource.title.as_deref(),
            right.resource.title.as_deref(),
        )
    });

    let ids: Vec<u64> = rows.iter().map(|row| row.resource.id.0).collect();
    assert_eq!(ids, UPSTREAM_ITEM_ORDER);
}

#[tokio::test]
async fn a_user_name_sort_folds_case_like_upstream() {
    let pool = common::memory().await;
    let users = UserRepo::new(&pool);
    for name in ["Curator", "admin", "Zebra"] {
        users
            .create(&NewUser {
                email: format!("{}@example.org", name.to_ascii_lowercase()),
                name: name.to_string(),
                password_hash: None,
                role: "editor".to_string(),
                is_active: true,
                created: None,
            })
            .await
            .unwrap();
    }

    let mut rows = users
        .list(&Default::default(), &Paging::default())
        .await
        .unwrap();
    rows.sort_by(|left, right| compare_text_ci(&left.name, &right.name));

    let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
    // The folded order. Byte order would be `Curator`, `Zebra`, `admin`, which is the bug.
    assert_eq!(names, ["admin", "Curator", "Zebra"]);
}

/// The Rust rule and a `COLLATE NOCASE` clause agree, which is what lets one rule serve both a
/// Rust sorter and a repository statement.
#[tokio::test]
async fn the_rust_rule_matches_collate_nocase() {
    let pool = common::memory().await;
    let mut rows = seed_items(&pool).await;

    let sql = "SELECT resource.id FROM item JOIN resource ON resource.id = item.id \
               WHERE resource.title IS NOT NULL \
               ORDER BY resource.title COLLATE NOCASE, resource.id";
    let by_sql: Vec<u64> = sqlx::query(sql)
        .fetch_all(&pool)
        .await
        .unwrap()
        .iter()
        .map(|row| row.get::<i64, _>("id") as u64)
        .collect();

    rows.retain(|row| row.resource.title.is_some());
    rows.sort_by(|left, right| {
        compare_opt_text_ci(
            left.resource.title.as_deref(),
            right.resource.title.as_deref(),
        )
    });
    let by_rust: Vec<u64> = rows.iter().map(|row| row.resource.id.0).collect();

    assert_eq!(by_rust, by_sql);
    assert_eq!(by_rust, [7, 4, 5, 3, 2]);
}
