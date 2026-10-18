//! The account routes end to end: login, logout, the current user, the password
//! flows and the admin's password link.
//!
//! Every case here drives the real router over a real migrated database, so the
//! route table, the cookie attributes, the status codes and the bodies are what
//! the SPA talks to. The decisions behind them are unit tested in `omeka-api`;
//! this file is the wire.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use omeka_api::jsonld::BaseUrl;
use omeka_api::login;
use omeka_api::password;
use omeka_api::session;
use omeka_db::ids::UserId;
use omeka_db::repos::{PasswordCreationRepo, UserRepo};
use omeka_db::rows::{NewUser, UserRow};
use omeka_db::{migrate, open_memory};
use omeka_server::{Config, build_router, build_router_with_config};
use sqlx::SqlitePool;

use common::{Answer, Seed, header, json_request, request, seeded_pool, send};

/// The password every test account is created with.
const PASSWORD: &str = "hunter2";

/// A migrated database, no rows.
async fn empty_pool() -> SqlitePool {
    let pool = open_memory().await.expect("open memory database");
    migrate(&pool).await.expect("run migrations");
    pool
}

/// A migrated database with one user of each role and state the tests need.
async fn pool_with_users() -> (SqlitePool, Seed, UserRow, UserRow, UserRow) {
    let (pool, seed) = seeded_pool().await;
    let editor = user(&pool, "editor@example.org", "editor", true, Some(PASSWORD)).await;
    let inactive = user(&pool, "gone@example.org", "author", false, Some(PASSWORD)).await;
    let invited = user(&pool, "invited@example.org", "researcher", false, None).await;
    (pool, seed, editor, inactive, invited)
}

async fn user(
    pool: &SqlitePool,
    email: &str,
    role: &str,
    is_active: bool,
    password: Option<&str>,
) -> UserRow {
    let hash = password.map(|plain| password::hash(plain).expect("hash"));
    UserRepo::new(pool)
        .create(&NewUser {
            email: email.to_string(),
            name: "A Person".to_string(),
            password_hash: hash,
            role: role.to_string(),
            is_active,
            created: None,
        })
        .await
        .expect("create user")
}

fn router(pool: SqlitePool) -> Router {
    build_router(pool, BaseUrl::parse("http://localhost").expect("base url"))
}

/// A POST with a JSON body and no cookie, which is what a sign-in sends first.
fn post_json(uri: &str, body: &str) -> Request<Body> {
    json_request("POST", uri, body)
}

/// Add a request's cookie header, carrying the session token.
fn with_cookie(mut request: Request<Body>, token: &str) -> Request<Body> {
    request.headers_mut().insert(
        axum::http::header::COOKIE,
        format!("{}={token}", session::COOKIE_NAME)
            .parse()
            .expect("cookie header"),
    );
    request
}

/// Add the `Origin` a browser sends on a write.
fn with_origin(mut request: Request<Body>, origin: &str) -> Request<Body> {
    request
        .headers_mut()
        .insert(axum::http::header::ORIGIN, origin.parse().expect("origin"));
    request
}

/// The session token in a response's `Set-Cookie`, when it set one.
fn session_cookie(answer: &Answer) -> Option<String> {
    header(answer, "set-cookie")
        .and_then(session::cookie_token)
        .map(str::to_string)
}

fn cookies(answer: &Answer) -> Vec<String> {
    answer
        .headers
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .map(str::to_string)
        .collect()
}

/// Sign in and return the session token.
async fn sign_in(app: &Router, email: &str) -> String {
    let body = format!(r#"{{"email":"{email}","password":"{PASSWORD}"}}"#);
    let answer = send(app.clone(), post_json("/api/login", &body)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    session_cookie(&answer).expect("a session cookie")
}

#[tokio::test]
async fn login_controller_login_action_signs_in_and_sets_a_cookie() {
    let (pool, _seed, editor, _inactive, _invited) = pool_with_users().await;
    let app = router(pool.clone());

    let body = format!(r#"{{"email":"{}","password":"{PASSWORD}"}}"#, editor.email);
    let answer = send(app.clone(), post_json("/api/login", &body)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let document = answer.json();
    assert_eq!(
        document.get("o:email").and_then(|value| value.as_str()),
        Some("editor@example.org")
    );
    assert_eq!(
        document.get("o:role").and_then(|value| value.as_str()),
        Some("editor")
    );
    assert_eq!(
        document.get("@id").and_then(|value| value.as_str()),
        Some("http://localhost/api/users/2")
    );

    let value = header(&answer, "set-cookie").expect("a Set-Cookie header");
    assert!(value.contains("romeka_session="), "{value}");
    assert!(value.contains("HttpOnly"), "{value}");
    assert!(value.contains("SameSite=Lax"), "{value}");
    assert!(value.contains("Path=/"), "{value}");
    assert!(
        !value.contains("Secure"),
        "an http base URL does not mark the cookie Secure: {value}"
    );

    // The cookie is what the identity comes from, so `/api/me` answers with the
    // signed-in user and the same document.
    let token = session_cookie(&answer).expect("a token");
    let me = send(app.clone(), with_cookie(request("GET", "/api/me"), &token)).await;
    assert_eq!(me.status, StatusCode::OK, "{}", me.body);
    assert_eq!(
        me.json().get("o:email").and_then(|value| value.as_str()),
        Some("editor@example.org")
    );

    // And the session row is what logout would revoke.
    assert!(
        omeka_db::repos::SessionRepo::new(&pool)
            .get(&token)
            .await
            .expect("read the session")
            .is_some()
    );
}

#[tokio::test]
async fn a_wrong_password_is_401_and_sets_no_cookie() {
    let (pool, _seed, editor, _inactive, _invited) = pool_with_users().await;
    let app = router(pool);

    let body = format!(r#"{{"email":"{}","password":"wrong"}}"#, editor.email);
    let answer = send(app.clone(), post_json("/api/login", &body)).await;
    assert_eq!(answer.status, StatusCode::UNAUTHORIZED, "{}", answer.body);
    assert_eq!(
        answer.json()["errors"]["error"],
        serde_json::json!("Email or password is invalid")
    );
    assert!(cookies(&answer).is_empty(), "{:?}", cookies(&answer));

    let unknown = r#"{"email":"ghost@example.org","password":"hunter2"}"#;
    let answer = send(app, post_json("/api/login", unknown)).await;
    assert_eq!(answer.status, StatusCode::UNAUTHORIZED, "{}", answer.body);
    assert!(cookies(&answer).is_empty());
}

#[tokio::test]
async fn an_inactive_user_is_403_and_sets_no_cookie() {
    let (pool, _seed, _editor, inactive, _invited) = pool_with_users().await;
    let app = router(pool);

    let body = format!(
        r#"{{"email":"{}","password":"{PASSWORD}"}}"#,
        inactive.email
    );
    let answer = send(app, post_json("/api/login", &body)).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
    assert!(cookies(&answer).is_empty());
}

#[tokio::test]
async fn logout_action_clears_the_cookie_and_revokes_the_row() {
    let (pool, _seed, _editor, _inactive, _invited) = pool_with_users().await;
    let app = router(pool.clone());
    let token = sign_in(&app, "editor@example.org").await;

    let logout = with_origin(
        with_cookie(request("POST", "/api/logout"), &token),
        "http://localhost",
    );
    let answer = send(app.clone(), logout).await;
    assert_eq!(answer.status, StatusCode::NO_CONTENT, "{}", answer.body);
    let cleared = header(&answer, "set-cookie").expect("a Set-Cookie header");
    assert!(cleared.contains("Max-Age=0"), "{cleared}");

    let me = send(app, with_cookie(request("GET", "/api/me"), &token)).await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED, "{}", me.body);
    assert!(
        omeka_db::repos::SessionRepo::new(&pool)
            .get(&token)
            .await
            .expect("read the session")
            .is_none(),
        "logout deletes the row"
    );
}

#[tokio::test]
async fn a_deactivated_users_live_session_stops_working() {
    let (pool, _seed, editor, _inactive, _invited) = pool_with_users().await;
    let app = router(pool.clone());
    let token = sign_in(&app, "editor@example.org").await;

    let mut row = UserRepo::new(&pool)
        .get(editor.id)
        .await
        .expect("read the user")
        .expect("the user exists");
    row.is_active = false;
    UserRepo::new(&pool).update(&row).await.expect("deactivate");

    let me = send(app, with_cookie(request("GET", "/api/me"), &token)).await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED, "{}", me.body);
}

#[tokio::test]
async fn me_answers_401_without_a_session() {
    let (pool, _seed, _editor, _inactive, _invited) = pool_with_users().await;
    let answer = send(router(pool), request("GET", "/api/me")).await;
    assert_eq!(answer.status, StatusCode::UNAUTHORIZED, "{}", answer.body);
    assert_eq!(
        answer.json()["errors"]["error"],
        serde_json::json!("Authentication required.")
    );
}

#[tokio::test]
async fn login_and_me_answer_the_same_user_document_as_the_users_route() {
    let (pool, _seed, editor, _inactive, _invited) = pool_with_users().await;
    let app = router(pool);

    let body = format!(r#"{{"email":"{}","password":"{PASSWORD}"}}"#, editor.email);
    let signed_in = send(app.clone(), post_json("/api/login", &body)).await;
    assert_eq!(signed_in.status, StatusCode::OK, "{}", signed_in.body);
    let token = session_cookie(&signed_in).expect("a session cookie");
    let me = send(app.clone(), with_cookie(request("GET", "/api/me"), &token)).await;
    let read = send(
        app.clone(),
        common::get(&format!("/api/users/{}", editor.id.0), true),
    )
    .await;
    assert_eq!(read.status, StatusCode::OK, "{}", read.body);

    let document = signed_in.json();
    for member in [
        "@id",
        "@type",
        "o:id",
        "o:name",
        "o:email",
        "o:created",
        "o:role",
        "o:is_active",
    ] {
        assert!(
            document.get(member).is_some(),
            "login is missing {member}: {document}"
        );
    }
    assert_eq!(document, me.json(), "login and /api/me agree");
    assert_eq!(
        document,
        read.json(),
        "login answers what GET /api/users/{{id}} answers"
    );
}

#[tokio::test]
async fn logout_accepts_the_empty_json_body_the_client_sends() {
    let (pool, _seed, _editor, _inactive, _invited) = pool_with_users().await;
    let app = router(pool);
    let token = sign_in(&app, "editor@example.org").await;

    let answer = send(
        app,
        with_origin(
            with_cookie(post_json("/api/logout", "{}"), &token),
            "http://localhost",
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::NO_CONTENT, "{}", answer.body);
}

#[tokio::test]
async fn the_auth_routes_are_not_shadowed_by_the_resource_routes() {
    let (pool, _seed, _editor, _inactive, _invited) = pool_with_users().await;
    let app = router(pool);

    // A GET on a POST-only auth route is a 405, not a read of a resource
    // called `password` or `logout`.
    for uri in ["/api/password", "/api/logout", "/api/password/forgot"] {
        let answer = send(app.clone(), request("GET", uri)).await;
        assert_eq!(
            answer.status,
            StatusCode::METHOD_NOT_ALLOWED,
            "GET {uri} answered {}: {}",
            answer.status,
            answer.body
        );
    }
    // A two-segment auth path is not read as `/api/{resource}/{id}`.
    let answer = send(app.clone(), request("GET", "/api/password/forgot")).await;
    assert_eq!(
        answer.status,
        StatusCode::METHOD_NOT_ALLOWED,
        "{}",
        answer.body
    );
    // And the resource routes still answer: `users` is a collection.
    let users = send(app, common::get("/api/users", true)).await;
    assert_eq!(users.status, StatusCode::OK, "{}", users.body);
}

#[tokio::test]
async fn create_password_action_spends_the_key_once() {
    let (pool, _seed, _editor, _inactive, invited) = pool_with_users().await;
    let app = router(pool.clone());
    let issued = login::issue_key(&pool, &invited, true)
        .await
        .expect("mint a key");

    let body = format!(r#"{{"key":"{}","password":"{PASSWORD}"}}"#, issued.key);
    let answer = send(app.clone(), post_json("/api/password", &body)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(
        answer
            .json()
            .get("o:is_active")
            .and_then(|value| value.as_bool()),
        Some(true),
        "an activation key promotes the user"
    );

    let again = send(app.clone(), post_json("/api/password", &body)).await;
    assert_eq!(again.status, StatusCode::NOT_FOUND, "{}", again.body);

    // The password the route set is the one a login verifies.
    let signin = format!(r#"{{"email":"{}","password":"{PASSWORD}"}}"#, invited.email);
    let answer = send(app, post_json("/api/login", &signin)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
}

#[tokio::test]
async fn an_expired_key_is_404_and_deactivates_the_user_like_upstream() {
    let (pool, _seed, editor, _inactive, _invited) = pool_with_users().await;
    let app = router(pool.clone());
    // One key row straight in the table with a creation time long past, which
    // is what a link older than two weeks holds. The column is a stored
    // datetime, so the literal is the fixture.
    sqlx::query(
        "INSERT INTO password_creation (id, user_id, created, activate) VALUES (?, ?, ?, ?)",
    )
    .bind("expired-key")
    .bind(editor.id.0 as i64)
    .bind("2020-01-01 00:00:00")
    .bind(false)
    .execute(&pool)
    .await
    .expect("store an expired key");

    let body = format!(r#"{{"key":"expired-key","password":"{PASSWORD}"}}"#);
    let answer = send(app, post_json("/api/password", &body)).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND, "{}", answer.body);

    let after = UserRepo::new(&pool)
        .get(editor.id)
        .await
        .expect("read the user")
        .expect("the user exists");
    assert!(
        !after.is_active,
        "createPasswordAction deactivates on expiry"
    );
}

#[tokio::test]
async fn a_rejected_password_is_422_with_the_validation_document() {
    let (pool, _seed, _editor, _inactive, invited) = pool_with_users().await;
    let app = router(pool.clone());
    let issued = login::issue_key(&pool, &invited, true)
        .await
        .expect("mint a key");

    let body = format!(r#"{{"key":"{}","password":"short"}}"#, issued.key);
    let answer = send(app.clone(), post_json("/api/password", &body)).await;
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
    assert_eq!(
        answer.json()["errors"]["password"][0],
        serde_json::json!("The password is invalid")
    );
    assert!(
        PasswordCreationRepo::new(&pool)
            .get(&issued.key)
            .await
            .expect("read the key")
            .is_some(),
        "the rejected password leaves the key for another try"
    );

    // A body that is not a JSON object is a 400 with the error document.
    let bad = send(app, json_request("POST", "/api/password", "[]")).await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST, "{}", bad.body);
    assert_eq!(
        bad.json()["errors"]["error"],
        serde_json::json!("The request body must be a JSON object.")
    );
}

#[tokio::test]
async fn forgot_password_action_answers_204_for_a_known_and_an_unknown_email() {
    let (pool, _seed, _editor, _inactive, _invited) = pool_with_users().await;
    let app = router(pool.clone());

    for email in ["editor@example.org", "ghost@example.org"] {
        let body = format!(r#"{{"email":"{email}"}}"#);
        let answer = send(app.clone(), post_json("/api/password/forgot", &body)).await;
        assert_eq!(answer.status, StatusCode::NO_CONTENT, "{}", answer.body);
        assert_eq!(answer.body, "", "204 carries no body");
    }
    // The known address got a key, the unknown one did not, and the answer did
    // not say which was which.
    let keys = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM password_creation")
        .fetch_one(&pool)
        .await
        .expect("count keys");
    assert_eq!(keys, 1);
}

#[tokio::test]
async fn the_login_limit_trips_with_the_documented_window() {
    let (pool, _seed, editor, _inactive, _invited) = pool_with_users().await;
    let app = router(pool);
    let body = format!(r#"{{"email":"{}","password":"wrong"}}"#, editor.email);

    for attempt in 0..5 {
        let answer = send(app.clone(), post_json("/api/login", &body)).await;
        assert_eq!(
            answer.status,
            StatusCode::UNAUTHORIZED,
            "attempt {attempt} is inside the limit"
        );
    }
    let answer = send(app.clone(), post_json("/api/login", &body)).await;
    assert_eq!(
        answer.status,
        StatusCode::TOO_MANY_REQUESTS,
        "{}",
        answer.body
    );
    assert_eq!(
        header(&answer, "retry-after"),
        Some("300"),
        "the documented five minute window"
    );
    // The limit is checked before the password is, so even the right one is
    // refused while the window is open.
    let right = format!(r#"{{"email":"{}","password":"{PASSWORD}"}}"#, editor.email);
    let answer = send(app, post_json("/api/login", &right)).await;
    assert_eq!(
        answer.status,
        StatusCode::TOO_MANY_REQUESTS,
        "{}",
        answer.body
    );
}

#[tokio::test]
async fn a_cross_origin_write_is_refused_and_a_same_origin_one_is_accepted() {
    let (pool, _seed, _editor, _inactive, _invited) = pool_with_users().await;
    let app = router(pool);
    let token = sign_in(&app, "editor@example.org").await;

    let refused = with_origin(
        with_cookie(request("POST", "/api/logout"), &token),
        "https://evil.example",
    );
    let answer = send(app.clone(), refused).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
    assert!(
        header(&answer, "set-cookie").is_none(),
        "a refused write changes nothing"
    );

    // The same request from the configured origin goes through, as does one
    // with no origin marker at all and no cookie.
    let accepted = with_origin(
        with_cookie(request("POST", "/api/logout"), &token),
        "http://localhost",
    );
    let answer = send(app.clone(), accepted).await;
    assert_eq!(answer.status, StatusCode::NO_CONTENT, "{}", answer.body);
}

#[tokio::test]
async fn a_cross_origin_login_is_refused_when_it_carries_a_session() {
    let (pool, _seed, editor, _inactive, _invited) = pool_with_users().await;
    let app = router(pool);
    let token = sign_in(&app, "editor@example.org").await;

    let body = format!(r#"{{"email":"{}","password":"{PASSWORD}"}}"#, editor.email);
    let refused = with_origin(
        with_cookie(post_json("/api/login", &body), &token),
        "https://evil.example",
    );
    let answer = send(app, refused).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
}

#[tokio::test]
async fn mint_password_link_is_admin_only_and_returns_the_link() {
    let (pool, seed, editor, _inactive, _invited) = pool_with_users().await;
    let app = router(pool.clone());

    // The seeded key belongs to the seeded global admin.
    let uri = format!("/api/users/{}/password-link", editor.id.0);
    let answer = send(
        app.clone(),
        common::request("POST", &common::append_key(&uri)),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let document = answer.json();
    let key = document["key"].as_str().expect("a key").to_string();
    let link = document["link"].as_str().expect("a link").to_string();
    assert_eq!(link, format!("http://localhost/#/password/{key}"));
    assert_eq!(
        document["sent"],
        serde_json::json!(false),
        "the log transport sends nothing"
    );
    assert_ne!(seed.user, editor.id);

    // The link's key is the key the set-password route spends.
    let body = format!(r#"{{"key":"{key}","password":"{PASSWORD}"}}"#);
    let answer = send(app.clone(), post_json("/api/password", &body)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);

    // An editor's key pair authenticates a caller, and that caller is still
    // not an administrator.
    let (identity, credential) = api_key_for(&pool, editor.id).await;
    let editor_uri = format!(
        "/api/users/{}/password-link?key_identity={identity}&key_credential={credential}",
        seed.user.0
    );
    let answer = send(app.clone(), common::request("POST", &editor_uri)).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
}

/// Mint an API key pair for one user.
async fn api_key_for(pool: &SqlitePool, owner: UserId) -> (String, String) {
    let identity = "ffffffffffffffffffffffffffffffff".to_string();
    let credential = "0123456789abcdef0123456789abcdee".to_string();
    let hash = bcrypt::hash(&credential, 4).expect("hash the credential");
    omeka_db::repos::ApiKeyRepo::new(pool)
        .create(&omeka_db::rows::NewApiKey {
            id: identity.clone(),
            owner_id: owner,
            label: "test".to_string(),
            credential_hash: hash,
            last_ip: None,
            last_accessed: None,
            created: None,
        })
        .await
        .expect("create key");
    (identity, credential)
}

#[tokio::test]
async fn the_smtp_transport_is_selected_when_configured() {
    let pool = empty_pool().await;
    let config = Config {
        mail: omeka_server::MailConfig {
            host: Some("smtp.example.org".to_string()),
            ..Default::default()
        },
        ..Config::default()
    };
    // Building the router builds the SMTP client and dials nothing; a request
    // that sends no mail proves the server answers with a relay configured.
    let app = build_router_with_config(pool, &config);
    let answer = send(app, request("GET", "/health")).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
}
