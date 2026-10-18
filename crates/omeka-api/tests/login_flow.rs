//! The login and password flows end to end, against a real migrated database.
//!
//! Each case here is one entry in the brief's test list: a wrong password is 401
//! and never sets a cookie, an inactive user is 403, a one-time key works once,
//! an expired key is refused and deactivates its user, `forgot` answers the same
//! for a known and an unknown address, the login limit trips and then releases,
//! and an administrator mints a link for another account only.
//!
//! The route-level tests in `omeka-server/tests/accounts.rs` drive the same
//! behaviour through HTTP.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Instant;

use chrono::{Duration, NaiveDateTime, Utc};
use http::StatusCode;
use omeka_api::auth::Actor;
use omeka_api::limits::{Limiter, Policy, RateLimits};
use omeka_api::login::{
    AuthError, KEY_LIFETIME_DAYS, PasswordRules, SetPasswordError, current_user, forgot_password,
    issue_key, login, logout, password_link, set_password_at,
};
use omeka_api::password;
use omeka_api::session::SessionStore;
use omeka_api::token;
use omeka_db::repos::{PasswordCreationRepo, UserRepo};
use omeka_db::rows::{NewPasswordCreation, NewUser, UserRow};
use omeka_db::{migrate, open_memory};
use sqlx::SqlitePool;

/// A migrated in-memory database. It lives as long as the pool.
async fn temp_db() -> SqlitePool {
    let pool = open_memory().await.expect("open memory database");
    migrate(&pool).await.expect("run migrations");
    pool
}

/// One user row with no password.
async fn test_user(pool: &SqlitePool, email: &str, role: &str, is_active: bool) -> UserRow {
    create_user(pool, email, role, is_active, None).await
}

/// One user row whose password verifies against `password`.
async fn test_user_with_password(
    pool: &SqlitePool,
    email: &str,
    role: &str,
    is_active: bool,
    password: &str,
) -> UserRow {
    let hash = password::hash(password).expect("hash");
    create_user(pool, email, role, is_active, Some(hash)).await
}

async fn create_user(
    pool: &SqlitePool,
    email: &str,
    role: &str,
    is_active: bool,
    password_hash: Option<String>,
) -> UserRow {
    UserRepo::new(pool)
        .create(&NewUser {
            email: email.to_string(),
            name: "A Person".to_string(),
            password_hash,
            role: role.to_string(),
            is_active,
            created: None,
        })
        .await
        .expect("create user")
}

fn clock() -> NaiveDateTime {
    Utc::now().naive_utc()
}

/// A key row with a creation time the test chooses.
async fn stored_key(pool: &SqlitePool, user: &UserRow, created: NaiveDateTime) -> String {
    let key = token::token().unwrap();
    PasswordCreationRepo::new(pool)
        .create(&NewPasswordCreation {
            id: key.clone(),
            user_id: user.id,
            activate: false,
            created: Some(created),
        })
        .await
        .unwrap();
    key
}

fn short_window(limit: u32) -> Policy {
    Policy {
        limit,
        window: std::time::Duration::from_millis(50),
    }
}

#[tokio::test]
async fn login_action_starts_a_session_for_a_correct_password() {
    let pool = temp_db().await;
    let user =
        test_user_with_password(&pool, "person@example.org", "editor", true, "hunter2").await;
    let limiter = Limiter::new(RateLimits::default());

    let session = login(
        &pool,
        &limiter,
        None,
        "person@example.org",
        "hunter2",
        None,
        Instant::now(),
    )
    .await
    .expect("login");
    assert_eq!(session.user.id, user.id);

    let resolved = current_user(&pool, Some(&session.token))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resolved.id, user.id);
    assert_eq!(resolved.role, "editor");
}

#[tokio::test]
async fn login_action_refuses_a_wrong_password_and_an_unknown_email() {
    let pool = temp_db().await;
    test_user_with_password(&pool, "person@example.org", "editor", true, "hunter2").await;
    let limiter = Limiter::new(RateLimits::default());
    let now = Instant::now();

    assert_eq!(
        login(
            &pool,
            &limiter,
            None,
            "person@example.org",
            "wrong",
            None,
            now
        )
        .await,
        Err(AuthError::InvalidCredentials)
    );
    assert_eq!(
        login(
            &pool,
            &limiter,
            None,
            "ghost@example.org",
            "hunter2",
            None,
            now
        )
        .await,
        Err(AuthError::InvalidCredentials)
    );
    // A user who was invited and never set a password has no hash, and
    // `verifyPassword` refuses every password for one.
    let invited = test_user(&pool, "invited@example.org", "researcher", true).await;
    assert_eq!(invited.password_hash, None);
    assert_eq!(
        login(&pool, &limiter, None, "invited@example.org", "", None, now).await,
        Err(AuthError::InvalidCredentials)
    );
}

#[tokio::test]
async fn login_action_answers_403_for_an_inactive_account_with_the_right_password() {
    let pool = temp_db().await;
    test_user_with_password(&pool, "gone@example.org", "editor", false, "hunter2").await;
    let limiter = Limiter::new(RateLimits::default());

    assert_eq!(
        login(
            &pool,
            &limiter,
            None,
            "gone@example.org",
            "hunter2",
            None,
            Instant::now()
        )
        .await,
        Err(AuthError::Inactive)
    );
    // The password still has to be right, so the answer does not enumerate
    // addresses to a guesser.
    assert_eq!(
        login(
            &pool,
            &limiter,
            None,
            "gone@example.org",
            "wrong",
            None,
            Instant::now()
        )
        .await,
        Err(AuthError::InvalidCredentials)
    );
    assert_eq!(AuthError::Inactive.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn login_rotates_the_session_the_request_presented() {
    let pool = temp_db().await;
    let user =
        test_user_with_password(&pool, "person@example.org", "editor", true, "hunter2").await;
    let limiter = Limiter::new(RateLimits::default());
    let first = SessionStore::new(&pool).create(user.id).await.unwrap();

    let session = login(
        &pool,
        &limiter,
        Some(&first),
        "person@example.org",
        "hunter2",
        None,
        Instant::now(),
    )
    .await
    .expect("login");
    assert_ne!(session.token, first);
    assert_eq!(
        current_user(&pool, Some(&first)).await.unwrap(),
        None,
        "the id the request presented is dead"
    );
}

#[tokio::test]
async fn the_login_limit_trips_and_then_releases() {
    let pool = temp_db().await;
    test_user_with_password(&pool, "person@example.org", "editor", true, "hunter2").await;
    let limiter = Limiter::new(RateLimits {
        login_email: short_window(2),
        ..RateLimits::default()
    });
    let start = Instant::now();

    for _ in 0..2 {
        assert_eq!(
            login(
                &pool,
                &limiter,
                None,
                "person@example.org",
                "wrong",
                None,
                start
            )
            .await,
            Err(AuthError::InvalidCredentials),
            "a failed attempt counts"
        );
    }
    assert_eq!(
        login(
            &pool,
            &limiter,
            None,
            "person@example.org",
            "hunter2",
            None,
            start
        )
        .await,
        Err(AuthError::RateLimited {
            retry_after_secs: 1
        }),
        "the limit refuses before it looks at the password"
    );
    let released = start + std::time::Duration::from_millis(50);
    assert_eq!(
        login(
            &pool,
            &limiter,
            None,
            "person@example.org",
            "hunter2",
            None,
            released
        )
        .await
        .expect("the window has released the limit")
        .user
        .email,
        "person@example.org"
    );
}

#[tokio::test]
async fn logout_action_revokes_the_session() {
    let pool = temp_db().await;
    let user = test_user(&pool, "person@example.org", "editor", true).await;
    let token = SessionStore::new(&pool).create(user.id).await.unwrap();

    assert!(current_user(&pool, Some(&token)).await.unwrap().is_some());
    assert!(logout(&pool, Some(&token)).await.unwrap());
    assert!(current_user(&pool, Some(&token)).await.unwrap().is_none());
    assert!(!logout(&pool, None).await.unwrap());
}

#[tokio::test]
async fn create_password_action_sets_the_password_once() {
    let pool = temp_db().await;
    let user = test_user(&pool, "invited@example.org", "editor", false).await;
    let issued = issue_key(&pool, &user, true).await.unwrap();
    let rules = PasswordRules::default();

    let updated = set_password_at(&pool, &rules, &issued.key, "hunter2", clock())
        .await
        .expect("set the password");
    assert!(updated.is_active, "activate true promotes the user");
    assert!(password::verify(
        "hunter2",
        updated.password_hash.as_deref()
    ));

    assert_eq!(
        set_password_at(&pool, &rules, &issued.key, "hunter3", clock()).await,
        Err(SetPasswordError::UnknownKey),
        "the key is spent on use"
    );
    assert!(
        PasswordCreationRepo::new(&pool)
            .get(&issued.key)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn create_password_action_leaves_the_key_when_the_password_is_rejected() {
    let pool = temp_db().await;
    let user = test_user(&pool, "invited@example.org", "editor", true).await;
    let issued = issue_key(&pool, &user, false).await.unwrap();

    assert_eq!(
        set_password_at(
            &pool,
            &PasswordRules::default(),
            &issued.key,
            "short",
            clock()
        )
        .await,
        Err(SetPasswordError::InvalidPassword)
    );
    assert!(
        PasswordCreationRepo::new(&pool)
            .get(&issued.key)
            .await
            .unwrap()
            .is_some(),
        "the person can try again with the same link"
    );
    let document = SetPasswordError::InvalidPassword.document();
    assert_eq!(
        serde_json::to_string(&document).unwrap(),
        r#"{"errors":{"password":["The password is invalid"]}}"#
    );
    assert_eq!(
        SetPasswordError::InvalidPassword.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn create_password_action_refuses_an_expired_key_and_deactivates_like_upstream() {
    let pool = temp_db().await;
    let user = test_user(&pool, "person@example.org", "editor", true).await;
    let created = clock() - Duration::days(KEY_LIFETIME_DAYS + 1);
    let key = stored_key(&pool, &user, created).await;

    assert_eq!(
        set_password_at(&pool, &PasswordRules::default(), &key, "hunter2", clock()).await,
        Err(SetPasswordError::UnknownKey)
    );
    assert!(
        PasswordCreationRepo::new(&pool)
            .get(&key)
            .await
            .unwrap()
            .is_none(),
        "an expired key is deleted"
    );
    let after = UserRepo::new(&pool).get(user.id).await.unwrap().unwrap();
    assert!(
        !after.is_active,
        "createPasswordAction:115 sets isActive false on an expired key"
    );
    assert_eq!(after.password_hash, None, "the password was never written");
}

#[tokio::test]
async fn create_password_action_accepts_a_key_on_its_last_day() {
    let pool = temp_db().await;
    let user = test_user(&pool, "person@example.org", "editor", true).await;
    let created = clock() - Duration::days(KEY_LIFETIME_DAYS) + Duration::hours(1);
    let key = stored_key(&pool, &user, created).await;

    assert!(
        set_password_at(&pool, &PasswordRules::default(), &key, "hunter2", clock())
            .await
            .is_ok(),
        "just inside the two weeks"
    );
}

#[tokio::test]
async fn create_password_action_answers_404_for_an_unknown_key() {
    let pool = temp_db().await;
    assert_eq!(
        set_password_at(
            &pool,
            &PasswordRules::default(),
            "no-such-key",
            "hunter2",
            clock()
        )
        .await,
        Err(SetPasswordError::UnknownKey)
    );
    assert_eq!(SetPasswordError::UnknownKey.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn forgot_password_action_mints_a_reset_key_for_an_active_account_only() {
    let pool = temp_db().await;
    let user = test_user(&pool, "person@example.org", "editor", true).await;
    test_user(&pool, "gone@example.org", "editor", false).await;
    let limiter = Limiter::new(RateLimits::default());
    let now = Instant::now();

    let issued = forgot_password(&pool, &limiter, "person@example.org", None, now)
        .await
        .unwrap()
        .expect("an active account gets a key");
    assert_eq!(issued.user.id, user.id);
    assert!(!issued.activate, "a reset does not activate");
    assert_eq!(issued.key.len(), token::TOKEN_LENGTH);
    assert_eq!(
        PasswordCreationRepo::new(&pool)
            .get(&issued.key)
            .await
            .unwrap()
            .unwrap()
            .created,
        issued.expires_at - Duration::days(KEY_LIFETIME_DAYS),
        "the key expires two weeks after it was created"
    );

    assert!(
        forgot_password(&pool, &limiter, "ghost@example.org", None, now)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        forgot_password(&pool, &limiter, "gone@example.org", None, now)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn forgot_password_action_replaces_the_key_the_user_already_held() {
    let pool = temp_db().await;
    let user = test_user(&pool, "person@example.org", "editor", true).await;
    let first = issue_key(&pool, &user, true).await.unwrap();
    let limiter = Limiter::new(RateLimits::default());

    let second = forgot_password(&pool, &limiter, "person@example.org", None, Instant::now())
        .await
        .unwrap()
        .unwrap();
    assert_ne!(second.key, first.key);
    assert!(
        PasswordCreationRepo::new(&pool)
            .get(&first.key)
            .await
            .unwrap()
            .is_none(),
        "user_id is unique, so the old key goes"
    );
}

#[tokio::test]
async fn forgot_password_action_is_limited_per_email() {
    let pool = temp_db().await;
    test_user(&pool, "person@example.org", "editor", true).await;
    let limiter = Limiter::new(RateLimits {
        forgot_email: short_window(1),
        ..RateLimits::default()
    });
    let start = Instant::now();

    // A known address and an unknown one are counted on separate subjects
    // and answer the same way: 0 for both, then refused for both.
    assert!(
        forgot_password(&pool, &limiter, "person@example.org", None, start)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        forgot_password(&pool, &limiter, "ghost@example.org", None, start)
            .await
            .unwrap()
            .is_none()
    );
    for address in ["person@example.org", "ghost@example.org"] {
        assert_eq!(
            forgot_password(&pool, &limiter, address, None, start).await,
            Err(AuthError::RateLimited {
                retry_after_secs: 1
            }),
            "{address} is over its own limit"
        );
    }
}

#[tokio::test]
async fn mint_password_link_is_for_an_admin_and_another_account() {
    let pool = temp_db().await;
    let admin = test_user(&pool, "admin@example.org", "global_admin", true).await;
    let editor = test_user(&pool, "editor@example.org", "editor", true).await;
    let actor = Actor::new(admin.id, "global_admin");

    let issued = password_link(&pool, &actor, &editor.id.0.to_string())
        .await
        .expect("an admin mints a link for another user");
    assert!(issued.activate);
    assert_eq!(issued.user.id, editor.id);

    assert_eq!(
        password_link(
            &pool,
            &Actor::new(editor.id, "editor"),
            &admin.id.0.to_string()
        )
        .await
        .err()
        .map(|error| error.status()),
        Some(StatusCode::FORBIDDEN),
        "a non-admin is refused"
    );
    assert_eq!(
        password_link(&pool, &actor, &admin.id.0.to_string())
            .await
            .err()
            .map(|error| error.status()),
        Some(StatusCode::FORBIDDEN),
        "an admin does not mint a link for their own account"
    );
    assert_eq!(
        password_link(&pool, &actor, "9999")
            .await
            .err()
            .map(|error| error.status()),
        Some(StatusCode::NOT_FOUND)
    );
    assert_eq!(
        password_link(&pool, &actor, "not-an-id")
            .await
            .err()
            .map(|error| error.status()),
        Some(StatusCode::NOT_FOUND)
    );
}
