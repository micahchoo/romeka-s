//! The admin's lesson state end to end: the two port-only user settings
//! `romeka_lessons_seen` and `romeka_hide_lessons`, written by the user and
//! read back from `GET /api/me`, so the state follows the user to any browser.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use omeka_api::password;
use omeka_db::repos::UserRepo;
use omeka_db::rows::{NewUser, UserRow};
use serde_json::{Value, json};
use sqlx::SqlitePool;

use common::{Answer, get_cookie, json_request, login_cookie, seeded_pool, send};

const PASSWORD: &str = "hunter2";

async fn author(pool: &SqlitePool) -> UserRow {
    UserRepo::new(pool)
        .create(&NewUser {
            email: "sam@example.org".to_string(),
            name: "Sam".to_string(),
            password_hash: Some(password::hash(PASSWORD).expect("hash")),
            role: "author".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .expect("create user")
}

async fn write_own_settings(app: &Router, user: &UserRow, cookie: &str, settings: Value) -> Answer {
    let mut request = json_request(
        "PATCH",
        &format!("/api/users/{}", user.id.0),
        &json!({ "o:user_settings": settings }).to_string(),
    );
    let headers = request.headers_mut();
    headers.insert("cookie", cookie.parse().expect("cookie header"));
    headers.insert("origin", "http://localhost".parse().expect("origin"));
    send(app.clone(), request).await
}

async fn me(app: &Router, cookie: &str) -> Value {
    let request: Request<Body> = get_cookie("/api/me", cookie);
    let answer = send(app.clone(), request).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    answer.json()
}

#[tokio::test]
async fn seen_lessons_and_the_switch_follow_the_user_into_a_new_session() {
    let (pool, _seed) = seeded_pool().await;
    let user = author(&pool).await;
    let app = common::router(pool);
    let cookie = login_cookie(app.clone(), &user.email, PASSWORD).await;
    assert!(
        me(&app, &cookie).await.get("o:user_settings").is_none(),
        "a new user has seen no lesson"
    );

    let written = write_own_settings(
        &app,
        &user,
        &cookie,
        json!({ "romeka_lessons_seen": ["describe.term", "kinds.class"] }),
    )
    .await;
    assert_eq!(written.status, StatusCode::OK, "{}", written.body);
    let hidden =
        write_own_settings(&app, &user, &cookie, json!({ "romeka_hide_lessons": true })).await;
    assert_eq!(hidden.status, StatusCode::OK, "{}", hidden.body);

    // A second sign-in stands for a second browser: it shares no client state.
    let elsewhere = login_cookie(app.clone(), &user.email, PASSWORD).await;
    assert_eq!(
        me(&app, &elsewhere).await["o:user_settings"],
        json!({
            "romeka_hide_lessons": true,
            "romeka_lessons_seen": ["describe.term", "kinds.class"],
        })
    );

    let shown = write_own_settings(
        &app,
        &user,
        &cookie,
        json!({ "romeka_hide_lessons": false }),
    )
    .await;
    assert_eq!(shown.status, StatusCode::OK, "{}", shown.body);
    assert_eq!(
        me(&app, &elsewhere).await["o:user_settings"],
        json!({ "romeka_lessons_seen": ["describe.term", "kinds.class"] }),
        "switching lessons back on removes the row and keeps what was seen"
    );
}

#[tokio::test]
async fn a_lesson_setting_of_the_wrong_shape_is_refused() {
    let (pool, _seed) = seeded_pool().await;
    let user = author(&pool).await;
    let app = common::router(pool);
    let cookie = login_cookie(app.clone(), &user.email, PASSWORD).await;

    for refused in [
        json!({ "romeka_lessons_seen": "describe.term" }),
        json!({ "romeka_lessons_seen": [1] }),
        json!({ "romeka_hide_lessons": "yes" }),
    ] {
        let answer = write_own_settings(&app, &user, &cookie, refused.clone()).await;
        assert_eq!(
            answer.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{refused}: {}",
            answer.body
        );
    }
    assert!(
        me(&app, &cookie).await.get("o:user_settings").is_none(),
        "a refused write stores nothing"
    );
}
