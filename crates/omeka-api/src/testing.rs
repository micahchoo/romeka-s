//! Fixtures the unit tests in this crate share: a migrated in-memory database
//! and the users the auth modules need.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use omeka_db::repos::UserRepo;
use omeka_db::rows::{NewUser, UserRow};
use omeka_db::{migrate, open_memory};
use sqlx::SqlitePool;

/// A migrated in-memory database. It lives as long as the pool.
pub async fn temp_db() -> SqlitePool {
    let pool = open_memory().await.expect("open memory database");
    migrate(&pool).await.expect("run migrations");
    pool
}

/// One user row with no password.
pub async fn test_user(pool: &SqlitePool, email: &str, role: &str, is_active: bool) -> UserRow {
    create(pool, email, role, is_active, None).await
}

async fn create(
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

/// The current Unix second, the unit a session `modified` and an attempt time
/// are written in.
#[must_use]
pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
