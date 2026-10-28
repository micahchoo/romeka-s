//! The admin's saved lists end to end: the port-only user setting
//! `romeka_saved_lists`, a list of `{name, query}` the user writes and reads back
//! from `GET /api/me`, so the lists follow the user to any browser. Another
//! user's read of the account does not show them.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::StatusCode;
use omeka_api::password;
use omeka_db::repos::UserRepo;
use omeka_db::rows::{NewUser, UserRow};
use serde_json::{Value, json};
use sqlx::SqlitePool;

use common::{Answer, get_cookie, json_request, login_cookie, seeded_pool, send};

const PASSWORD: &str = "hunter2";

async fn account(pool: &SqlitePool, email: &str, role: &str) -> UserRow {
    UserRepo::new(pool)
        .create(&NewUser {
            email: email.to_string(),
            name: email.to_string(),
            password_hash: Some(password::hash(PASSWORD).expect("hash")),
            role: role.to_string(),
            is_active: true,
            created: None,
        })
        .await
        .expect("create user")
}

async fn write_lists(app: &Router, user: &UserRow, cookie: &str, lists: Value) -> Answer {
    let mut request = json_request(
        "PATCH",
        &format!("/api/users/{}", user.id.0),
        &json!({ "o:user_settings": { "romeka_saved_lists": lists } }).to_string(),
    );
    let headers = request.headers_mut();
    headers.insert("cookie", cookie.parse().expect("cookie header"));
    headers.insert("origin", "http://localhost".parse().expect("origin"));
    send(app.clone(), request).await
}

async fn read(app: &Router, uri: &str, cookie: &str) -> Value {
    let answer = send(app.clone(), get_cookie(uri, cookie)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    answer.json()
}

#[tokio::test]
async fn saved_lists_follow_the_user_into_a_new_session() {
    let (pool, _seed) = seeded_pool().await;
    let user = account(&pool, "sam@example.org", "author").await;
    let app = common::router(pool);
    let cookie = login_cookie(app.clone(), &user.email, PASSWORD).await;

    let written = write_lists(
        &app,
        &user,
        &cookie,
        json!([
            { "name": "  Artifacts with no date ", "query": "field=7:empty&sort=dcterms%3Aidentifier" },
            { "name": "Letters", "query": "class=3" },
        ]),
    )
    .await;
    assert_eq!(written.status, StatusCode::OK, "{}", written.body);

    // A second sign-in stands for a second browser: it shares no client state.
    let elsewhere = login_cookie(app.clone(), &user.email, PASSWORD).await;
    assert_eq!(
        read(&app, "/api/me", &elsewhere).await["o:user_settings"]["romeka_saved_lists"],
        json!([
            { "name": "Artifacts with no date", "query": "field=7:empty&sort=dcterms%3Aidentifier" },
            { "name": "Letters", "query": "class=3" },
        ]),
        "the name is stored trimmed"
    );

    let emptied = write_lists(&app, &user, &cookie, json!([])).await;
    assert_eq!(emptied.status, StatusCode::OK, "{}", emptied.body);
    assert!(
        read(&app, "/api/me", &elsewhere)
            .await
            .get("o:user_settings")
            .is_none(),
        "deleting the last list removes the row"
    );
}

#[tokio::test]
async fn another_user_does_not_see_the_lists() {
    let (pool, _seed) = seeded_pool().await;
    let owner = account(&pool, "sam@example.org", "author").await;
    let admin = account(&pool, "ada@example.org", "global_admin").await;
    let app = common::router(pool);
    let own = login_cookie(app.clone(), &owner.email, PASSWORD).await;
    let written = write_lists(
        &app,
        &owner,
        &own,
        json!([{ "name": "Letters", "query": "class=3" }]),
    )
    .await;
    assert_eq!(written.status, StatusCode::OK, "{}", written.body);

    let uri = format!("/api/users/{}", owner.id.0);
    assert_eq!(
        read(&app, &uri, &own).await["o:user_settings"]["romeka_saved_lists"],
        json!([{ "name": "Letters", "query": "class=3" }]),
        "the owner reads their own lists on their account"
    );
    let other = login_cookie(app.clone(), &admin.email, PASSWORD).await;
    assert!(
        read(&app, &uri, &other)
            .await
            .get("o:user_settings")
            .is_none(),
        "an administrator reading the account sees no saved list"
    );
}

#[tokio::test]
async fn a_saved_list_of_the_wrong_shape_is_refused() {
    let (pool, _seed) = seeded_pool().await;
    let user = account(&pool, "sam@example.org", "author").await;
    let app = common::router(pool);
    let cookie = login_cookie(app.clone(), &user.email, PASSWORD).await;

    let too_many: Vec<Value> = (0..101)
        .map(|n| json!({ "name": format!("List {n}"), "query": "" }))
        .collect();
    for refused in [
        json!("class=3"),
        json!(["class=3"]),
        json!([{ "name": "Letters" }]),
        json!([{ "name": "   ", "query": "class=3" }]),
        json!([{ "name": "x".repeat(201), "query": "class=3" }]),
        json!([{ "name": "Letters", "query": 3 }]),
        json!([{ "name": "Letters", "query": "class=3", "colour": "red" }]),
        json!([
            { "name": "Letters", "query": "class=3" },
            { "name": " Letters", "query": "class=4" },
        ]),
        Value::Array(too_many),
    ] {
        let answer = write_lists(&app, &user, &cookie, refused.clone()).await;
        assert_eq!(
            answer.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{refused}: {}",
            answer.body
        );
    }
    assert!(
        read(&app, "/api/me", &cookie)
            .await
            .get("o:user_settings")
            .is_none(),
        "a refused write stores nothing"
    );
}
