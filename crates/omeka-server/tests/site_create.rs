//! A site create through the API is one unit: upstream hydrates the site, its
//! welcome page and its membership rows and writes them in one Doctrine flush,
//! so a create either stores all of them or none.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{append_key, json_request, router, seeded_pool, send};
use sqlx::SqlitePool;

/// Rows in each table a site create writes.
async fn site_rows(pool: &SqlitePool) -> [i64; 6] {
    let mut counts = [0; 6];
    for (count, table) in counts.iter_mut().zip([
        "site",
        "site_page",
        "site_page_block",
        "site_permission",
        "site_item_set",
        "site_setting",
    ]) {
        *count = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(pool)
            .await
            .unwrap();
    }
    counts
}

/// `o:site_settings` that is neither an object nor null is refused, and the
/// refusal leaves no site, page, block, permission, item set or setting row.
#[tokio::test]
async fn a_refused_create_leaves_no_site_rows() {
    let (pool, seed) = seeded_pool().await;
    let before = site_rows(&pool).await;
    let body = format!(
        r#"{{"o:slug":"partial","o:title":"Partial","o:theme":"default",
            "o:site_item_set":[{{"o:item_set":{{"o:id":{}}}}}],
            "o:theme_settings":{{"preset":"paper"}},
            "o:site_settings":5}}"#,
        seed.item_set.0
    );
    let answer = send(
        router(pool.clone()),
        json_request("POST", &append_key("/api/sites"), &body),
    )
    .await;
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
    assert!(answer.json()["errors"]["o:site_settings"].is_array());
    assert_eq!(site_rows(&pool).await, before);
}

/// `AbstractEntityAdapter::updateTimestamps` sets `created` and `modified`
/// together on create, so a fresh site reads back with the two equal, and the
/// default navigation that links the welcome page does not count as an edit.
#[tokio::test]
async fn a_new_site_is_modified_when_it_is_created() {
    let (pool, _seed) = seeded_pool().await;
    let body = r#"{"o:slug":"fresh","o:title":"Fresh","o:theme":"default"}"#;
    let answer = send(
        router(pool),
        json_request("POST", &append_key("/api/sites"), body),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let document = answer.json();
    assert_eq!(document["o:navigation"][0]["type"], "page");
    assert!(document["o:created"]["@value"].is_string());
    assert_eq!(document["o:modified"], document["o:created"]);
}
