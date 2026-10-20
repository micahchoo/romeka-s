//! The admin locale end to end: the `locale` user setting, the global `locale`
//! setting, and the `o:admin_locale` a sign-in and `GET /api/me` answer with.
//!
//! The order is `MvcListeners::bootstrapLocale`: the signed-in user's setting,
//! else the global one, else none.

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

use common::{Answer, append_key, get_cookie, json_request, login_cookie, seeded_pool, send};

const PASSWORD: &str = "hunter2";

async fn editor(pool: &SqlitePool) -> UserRow {
    UserRepo::new(pool)
        .create(&NewUser {
            email: "editor@example.org".to_string(),
            name: "An Editor".to_string(),
            password_hash: Some(password::hash(PASSWORD).expect("hash")),
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .expect("create user")
}

/// A JSON write that rides a session cookie, from the configured origin.
fn cookie_write(method: &str, uri: &str, cookie: &str, body: &Value) -> Request<Body> {
    let mut request = json_request(method, uri, &body.to_string());
    let headers = request.headers_mut();
    headers.insert("cookie", cookie.parse().expect("cookie header"));
    headers.insert("origin", "http://localhost".parse().expect("origin"));
    request
}

/// Store the global locale as the seeded global admin, through its API key.
async fn set_global_locale(app: &Router, locale: &str) {
    let answer = send(
        app.clone(),
        json_request(
            "PATCH",
            &append_key("/api/settings"),
            &json!({ "locale": locale }).to_string(),
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(answer.json()["locale"], json!(locale));
}

/// Store a user's own locale as that user, through the user write.
async fn set_own_locale(app: &Router, user: &UserRow, cookie: &str, locale: &str) -> Answer {
    send(
        app.clone(),
        cookie_write(
            "PATCH",
            &format!("/api/users/{}", user.id.0),
            cookie,
            &json!({ "o:user_settings": { "locale": locale } }),
        ),
    )
    .await
}

async fn me(app: &Router, cookie: &str) -> Value {
    let answer = send(app.clone(), get_cookie("/api/me", cookie)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    answer.json()
}

#[tokio::test]
async fn bootstrap_locale_prefers_the_user_setting_over_the_global_one() {
    let (pool, _seed) = seeded_pool().await;
    let user = editor(&pool).await;
    let app = common::router(pool);
    let cookie = login_cookie(app.clone(), &user.email, PASSWORD).await;
    set_global_locale(&app, "fr").await;

    let written = set_own_locale(&app, &user, &cookie, "es").await;
    assert_eq!(written.status, StatusCode::OK, "{}", written.body);
    assert_eq!(written.json()["o:user_settings"], json!({ "locale": "es" }));

    assert_eq!(me(&app, &cookie).await["o:admin_locale"], json!("es"));
    let body = json!({ "email": user.email, "password": PASSWORD }).to_string();
    let signed_in = send(app.clone(), json_request("POST", "/api/login", &body)).await;
    assert_eq!(
        signed_in.json()["o:admin_locale"],
        json!("es"),
        "a sign-in answers the same locale"
    );
}

#[tokio::test]
async fn bootstrap_locale_falls_back_to_the_global_setting() {
    let (pool, _seed) = seeded_pool().await;
    let user = editor(&pool).await;
    let app = common::router(pool);
    let cookie = login_cookie(app.clone(), &user.email, PASSWORD).await;
    set_global_locale(&app, "fr").await;

    assert_eq!(me(&app, &cookie).await["o:admin_locale"], json!("fr"));

    // The Default option clears the user's own row, and the global one shows through.
    set_own_locale(&app, &user, &cookie, "es").await;
    let cleared = set_own_locale(&app, &user, &cookie, "").await;
    assert_eq!(cleared.status, StatusCode::OK, "{}", cleared.body);
    assert!(cleared.json().get("o:user_settings").is_none());
    assert_eq!(me(&app, &cookie).await["o:admin_locale"], json!("fr"));
}

#[tokio::test]
async fn bootstrap_locale_answers_nothing_when_neither_setting_exists() {
    let (pool, _seed) = seeded_pool().await;
    let user = editor(&pool).await;
    let app = common::router(pool);
    let cookie = login_cookie(app.clone(), &user.email, PASSWORD).await;

    let document = me(&app, &cookie).await;
    assert_eq!(document["o:email"], json!("editor@example.org"));
    assert!(
        document.get("o:admin_locale").is_none(),
        "no locale is stored: {document}"
    );
}

#[tokio::test]
async fn the_global_settings_are_for_a_global_admin_only() {
    let (pool, _seed) = seeded_pool().await;
    let user = editor(&pool).await;
    let app = common::router(pool);
    let cookie = login_cookie(app.clone(), &user.email, PASSWORD).await;

    let read = send(app.clone(), get_cookie("/api/settings", &cookie)).await;
    assert_eq!(read.status, StatusCode::FORBIDDEN, "{}", read.body);
    let write = send(
        app.clone(),
        cookie_write(
            "PATCH",
            "/api/settings",
            &cookie,
            &json!({ "locale": "es" }),
        ),
    )
    .await;
    assert_eq!(write.status, StatusCode::FORBIDDEN, "{}", write.body);

    let admin = send(app.clone(), common::get("/api/settings", true)).await;
    assert_eq!(admin.status, StatusCode::OK, "{}", admin.body);
    assert!(
        admin.json().get("locale").is_none(),
        "the seed stores no locale: {}",
        admin.body
    );
}

#[tokio::test]
async fn a_setting_the_admin_has_no_field_for_is_refused() {
    let (pool, _seed) = seeded_pool().await;
    let user = editor(&pool).await;
    let app = common::router(pool);
    let cookie = login_cookie(app.clone(), &user.email, PASSWORD).await;

    let answer = send(
        app.clone(),
        cookie_write(
            "PATCH",
            &format!("/api/users/{}", user.id.0),
            &cookie,
            &json!({ "o:user_settings": { "default_resource_template": 1 } }),
        ),
    )
    .await;
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
    let global = send(
        app.clone(),
        json_request(
            "PATCH",
            &append_key("/api/settings"),
            &json!({ "installation_title": "x" }).to_string(),
        ),
    )
    .await;
    assert_eq!(
        global.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        global.body
    );
}
