//! Who may change a user's role, active switch and account through
//! `PUT /api/users/<id>`.
//!
//! `UserAdapter::hydrate` authorizes `change-role` when the role changes,
//! `change-role-admin` when the new role is an admin role, and `activate-user`
//! when the active switch changes; `AbstractEntityAdapter::update` authorizes
//! `update` on the row first. The matrix in `omeka-core` holds the rules: a
//! lower role may edit only its own account and never its role or switch, an
//! admin never their own role or switch, and a site admin never another admin.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{append_key, caller, json_request, seeded_pool, send};
use serde_json::json;

fn account(name: &str, email: &str, role: &str, active: bool) -> String {
    json!({"o:name": name, "o:email": email, "o:role": role, "o:is_active": active}).to_string()
}

fn denied(privilege: &str) -> String {
    format!(
        r#"{{"errors":{{"error":"Permission denied for the current user to {privilege} the Omeka\\Entity\\User resource."}}}}"#
    )
}

async fn stored_role(pool: &sqlx::SqlitePool, id: u64) -> (String, bool) {
    sqlx::query_as("SELECT role, is_active FROM user WHERE id = ?")
        .bind(i64::try_from(id).unwrap())
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_researcher_cannot_make_themself_a_global_admin() {
    let (pool, _seed) = seeded_pool().await;
    let me = caller(&pool, "researcher").await;
    let app = common::router(pool.clone());
    let uri = me.uri(&format!("/api/users/{}", me.id.0));
    let body = account("researcher", "researcher@example.com", "global_admin", true);
    let answer = send(app, json_request("PUT", &uri, &body)).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
    assert_eq!(answer.body, denied("change-role"));
    assert_eq!(
        stored_role(&pool, me.id.0).await,
        ("researcher".to_string(), true)
    );
}

#[tokio::test]
async fn an_editor_cannot_deactivate_themself() {
    let (pool, _seed) = seeded_pool().await;
    let me = caller(&pool, "editor").await;
    let app = common::router(pool.clone());
    let uri = me.uri(&format!("/api/users/{}", me.id.0));
    let body = account("editor", "editor@example.com", "editor", false);
    let answer = send(app, json_request("PUT", &uri, &body)).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
    assert_eq!(answer.body, denied("activate-user"));
    assert_eq!(
        stored_role(&pool, me.id.0).await,
        ("editor".to_string(), true)
    );
}

#[tokio::test]
async fn an_author_saves_their_own_account_with_the_role_and_switch_unchanged() {
    let (pool, _seed) = seeded_pool().await;
    let me = caller(&pool, "author").await;
    let app = common::router(pool.clone());
    let uri = me.uri(&format!("/api/users/{}", me.id.0));
    let body = account("Renamed author", "author@example.com", "author", true);
    let answer = send(app, json_request("PUT", &uri, &body)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(answer.json()["o:name"], "Renamed author");
}

#[tokio::test]
async fn a_global_admin_cannot_change_their_own_role_or_switch() {
    let (pool, seed) = seeded_pool().await;
    let app = common::router(pool.clone());
    let uri = append_key(&format!("/api/users/{}", seed.user.0));
    let demote = account("Admin", "admin@example.com", "editor", true);
    let answer = send(app.clone(), json_request("PUT", &uri, &demote)).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
    assert_eq!(answer.body, denied("change-role"));

    let off = account("Admin", "admin@example.com", "global_admin", false);
    let answer = send(app.clone(), json_request("PUT", &uri, &off)).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
    assert_eq!(answer.body, denied("activate-user"));
    assert_eq!(
        stored_role(&pool, seed.user.0).await,
        ("global_admin".to_string(), true)
    );

    let rename = account("Admin renamed", "admin@example.com", "global_admin", true);
    let answer = send(app, json_request("PUT", &uri, &rename)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
}

#[tokio::test]
async fn a_site_admin_cannot_give_an_admin_role_or_edit_another_admin() {
    let (pool, seed) = seeded_pool().await;
    let supervisor = caller(&pool, "site_admin").await;
    let author = caller(&pool, "author").await;
    let app = common::router(pool.clone());

    let author_uri = supervisor.uri(&format!("/api/users/{}", author.id.0));
    let promote = account("author", "author@example.com", "global_admin", true);
    let answer = send(app.clone(), json_request("PUT", &author_uri, &promote)).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
    assert_eq!(answer.body, denied("change-role-admin"));

    let to_editor = account("author", "author@example.com", "editor", true);
    let answer = send(app.clone(), json_request("PUT", &author_uri, &to_editor)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(answer.json()["o:role"], "editor");

    let admin_uri = supervisor.uri(&format!("/api/users/{}", seed.user.0));
    let rename = account("Renamed", "admin@example.com", "global_admin", true);
    let answer = send(app, json_request("PUT", &admin_uri, &rename)).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
    assert_eq!(answer.body, denied("update"));
}

#[tokio::test]
async fn an_editor_cannot_edit_another_account() {
    let (pool, seed) = seeded_pool().await;
    let me = caller(&pool, "editor").await;
    let app = common::router(pool);
    let uri = me.uri(&format!("/api/users/{}", seed.user.0));
    let rename = account("Renamed", "admin@example.com", "global_admin", true);
    let answer = send(app, json_request("PUT", &uri, &rename)).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
    assert_eq!(answer.body, denied("update"));
}
