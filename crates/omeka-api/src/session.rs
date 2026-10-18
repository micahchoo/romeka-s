//! The cookie session: a server-side `session` row behind an httpOnly cookie.
//!
//! Upstream keeps the authenticated `User` entity in a PHP session whose id is
//! the browser cookie and whose rows live in the `session` table
//! (`Session\SaveHandler\Db`). The port keeps the same table and the same
//! shape: `id` is the cookie value, `data` is opaque bytes the port writes as
//! `{"user_id":<id>}`, and `modified` is the Unix second of the last request
//! that used the row.
//!
//! **Expiry.** [`SESSION_LIFETIME_SECS`] counts back from `modified`, and
//! [`SessionStore::resolve`] writes `modified` on every request that uses a
//! row, so the window is one of inactivity, the way PHP's `gc_maxlifetime`
//! is. Upstream's default `gc_maxlifetime` is 1440 seconds; two weeks is the
//! port's choice, documented in the crate README, because a 24 minute idle
//! timeout signs an administrator out mid-edit and the row is the only thing
//! an expired cookie would have reached.
//!
//! **Tradeoff against a stateless signed cookie.** A signed cookie needs no
//! storage and no query, but it cannot be revoked before it expires: logout
//! could only clear the browser's copy, a deactivated user would keep working
//! until the signature expired, and rotation would need a server secret plus a
//! migration path. The row costs one indexed read, one `UPDATE` of `modified`
//! and one read of the user per authenticated request, and in exchange logout
//! deletes the row, a deactivated user stops at once, and the port holds no
//! session secret to leak. The row also means sessions survive a server
//! restart, which is what the admin SPA needs after a deploy.
//!
//! **Rotation.** `LoginController::loginAction` calls
//! `SessionManager::regenerateId` before it authenticates, so a session
//! fixation attempt cannot hold the id the victim logs in with.
//! [`SessionStore::rotate`] is that step: it deletes the presented row and
//! mints a new one.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use omeka_db::ids::UserId;
use omeka_db::repos::{SessionRepo, UserRepo};
use omeka_db::rows::UserRow;

use crate::errors::ApiError;
use crate::token;

/// The session cookie name. A later product rename changes this prefix only.
pub const COOKIE_NAME: &str = "romeka_session";

/// Seconds of inactivity after which a session row stops authenticating.
pub const SESSION_LIFETIME_SECS: i64 = 14 * 24 * 60 * 60;

/// `Set-Cookie` for a login, `LoginController::loginAction`.
///
/// `HttpOnly` keeps the value out of scripts, `SameSite=Lax` keeps it off a
/// cross-site POST, `Path=/` covers the API and the SPA, and `Secure` is added
/// when the configured base URL is https. SameSite is not the CSRF defence on
/// its own; see [`crate::origin`].
#[must_use]
pub fn set_cookie(token: &str, secure: bool) -> String {
    format!(
        "{COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Lax{}",
        secure_suffix(secure)
    )
}

/// `Set-Cookie` for a logout: the same attributes and an immediate expiry.
#[must_use]
pub fn clear_cookie(secure: bool) -> String {
    format!(
        "{COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{}",
        secure_suffix(secure)
    )
}

fn secure_suffix(secure: bool) -> &'static str {
    if secure { "; Secure" } else { "" }
}

/// The current Unix second.
#[must_use]
pub fn unix_now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// The session token in a `Cookie` header, if it carries one.
///
/// The attribute parser PHP runs over `$_COOKIE` is a `;` split with a `=`
/// split inside each part, which is what this does.
#[must_use]
pub fn cookie_token(header: &str) -> Option<&str> {
    header.split(';').find_map(|part| {
        let (name, value) = part.split_once('=')?;
        (name.trim() == COOKIE_NAME).then(|| value.trim())
    })
}

/// The bytes of one session row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct SessionPayload {
    user_id: u64,
}

/// Session rows for one database.
pub struct SessionStore<'a> {
    pool: &'a SqlitePool,
}

impl<'a> SessionStore<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    /// Mint a session for `user` and store it.
    ///
    /// # Errors
    /// An [`ApiError`] when the token cannot be drawn or the row cannot be written.
    pub async fn create(&self, user_id: UserId) -> Result<String, ApiError> {
        let token = token::token()?;
        let payload = serde_json::to_vec(&SessionPayload { user_id: user_id.0 })
            .map_err(ApiError::internal)?;
        SessionRepo::new(self.pool)
            .save(&token, &payload, None)
            .await
            .map_err(ApiError::internal)?;
        Ok(token)
    }

    /// `SessionManager::regenerateId` plus the login: drop the session the
    /// request presented, then start a new one.
    ///
    /// # Errors
    /// An [`ApiError`] when the token cannot be drawn or a row cannot be written.
    pub async fn rotate(
        &self,
        presented: Option<&str>,
        user_id: UserId,
    ) -> Result<String, ApiError> {
        if let Some(previous) = presented {
            SessionRepo::new(self.pool)
                .delete(previous)
                .await
                .map_err(ApiError::internal)?;
        }
        self.create(user_id).await
    }

    /// The user this token authenticates, or `None`.
    ///
    /// `None` covers an unknown token, a row whose window has closed (the same
    /// instant [`gc_expired`](Self::gc_expired) removes), a payload that is not
    /// a session payload, a user who no longer exists, and a user whose
    /// `is_active` is false. The first three delete the row they refused; the
    /// deactivated user keeps theirs, so re-activating the account restores the
    /// session.
    ///
    /// A row that resolves is written back with `now` as `modified`, which is
    /// the sliding window.
    ///
    /// # Errors
    /// An [`ApiError`] when a query fails.
    pub async fn resolve(&self, token: &str, now: i64) -> Result<Option<UserRow>, ApiError> {
        let repo = SessionRepo::new(self.pool);
        let Some(row) = repo.get(token).await.map_err(ApiError::internal)? else {
            return Ok(None);
        };
        if now.saturating_sub(row.modified) > SESSION_LIFETIME_SECS {
            repo.delete(token).await.map_err(ApiError::internal)?;
            return Ok(None);
        }
        let Some(payload) = serde_json::from_slice::<SessionPayload>(&row.data).ok() else {
            repo.delete(token).await.map_err(ApiError::internal)?;
            return Ok(None);
        };
        let Some(user) = UserRepo::new(self.pool)
            .get(UserId(payload.user_id))
            .await
            .map_err(ApiError::internal)?
        else {
            repo.delete(token).await.map_err(ApiError::internal)?;
            return Ok(None);
        };
        if !user.is_active {
            return Ok(None);
        }
        repo.save(token, &row.data, Some(now))
            .await
            .map_err(ApiError::internal)?;
        Ok(Some(user))
    }

    /// `SaveHandler\Db::destroy`: the row behind a logout.
    ///
    /// # Errors
    /// An [`ApiError`] when the delete fails.
    pub async fn revoke(&self, token: &str) -> Result<bool, ApiError> {
        SessionRepo::new(self.pool)
            .delete(token)
            .await
            .map_err(ApiError::internal)
    }

    /// `SaveHandler\Db::gc`: drop every row whose window has closed.
    ///
    /// # Errors
    /// An [`ApiError`] when the delete fails.
    pub async fn gc_expired(&self, now: i64) -> Result<u64, ApiError> {
        SessionRepo::new(self.pool)
            .gc(now.saturating_sub(SESSION_LIFETIME_SECS))
            .await
            .map_err(ApiError::internal)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::testing::{now, temp_db, test_user};

    fn payload(user: UserId) -> Vec<u8> {
        serde_json::to_vec(&SessionPayload { user_id: user.0 }).unwrap()
    }

    #[tokio::test]
    async fn a_session_resolves_to_its_user() {
        let pool = temp_db().await;
        let user = test_user(&pool, "person@example.org", "researcher", true).await;
        let store = SessionStore::new(&pool);
        let token = store.create(user.id).await.unwrap();

        let at = now();
        let resolved = store.resolve(&token, at).await.unwrap().unwrap();
        assert_eq!(resolved.id, user.id);
        assert_eq!(resolved.role, "researcher");
        assert_eq!(store.resolve("not-a-token", at).await.unwrap(), None);
    }

    #[tokio::test]
    async fn a_session_past_its_lifetime_is_refused_and_removed() {
        let pool = temp_db().await;
        let user = test_user(&pool, "person@example.org", "author", true).await;
        let store = SessionStore::new(&pool);
        let token = store.create(user.id).await.unwrap();

        let later = now() + SESSION_LIFETIME_SECS + 1;
        assert_eq!(store.resolve(&token, later).await.unwrap(), None);
        assert!(
            SessionRepo::new(&pool).get(&token).await.unwrap().is_none(),
            "an expired row is deleted, as the PHP save handler destroys it"
        );
    }

    #[tokio::test]
    async fn a_resolve_slides_the_window() {
        let pool = temp_db().await;
        let user = test_user(&pool, "person@example.org", "author", true).await;
        let store = SessionStore::new(&pool);
        let token = store.create(user.id).await.unwrap();

        let almost = now() + SESSION_LIFETIME_SECS - 1;
        assert!(store.resolve(&token, almost).await.unwrap().is_some());
        // That request moved the window, so the original deadline passes
        // without the session ending.
        assert!(store.resolve(&token, almost + 1).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn logout_revokes_the_row() {
        let pool = temp_db().await;
        let user = test_user(&pool, "person@example.org", "author", true).await;
        let store = SessionStore::new(&pool);
        let token = store.create(user.id).await.unwrap();

        assert!(store.revoke(&token).await.unwrap());
        assert_eq!(store.resolve(&token, now()).await.unwrap(), None);
        assert!(
            !store.revoke(&token).await.unwrap(),
            "the row is already gone"
        );
    }

    #[tokio::test]
    async fn a_deactivated_user_has_no_session() {
        let pool = temp_db().await;
        let user = test_user(&pool, "person@example.org", "author", true).await;
        let store = SessionStore::new(&pool);
        let token = store.create(user.id).await.unwrap();
        let at = now();

        assert!(store.resolve(&token, at).await.unwrap().is_some());
        let mut row = UserRepo::new(&pool).get(user.id).await.unwrap().unwrap();
        row.is_active = false;
        UserRepo::new(&pool).update(&row).await.unwrap();
        assert_eq!(store.resolve(&token, at).await.unwrap(), None);
    }

    #[tokio::test]
    async fn rotating_replaces_the_presented_token() {
        let pool = temp_db().await;
        let user = test_user(&pool, "person@example.org", "author", true).await;
        let store = SessionStore::new(&pool);
        let first = store.create(user.id).await.unwrap();

        let second = store.rotate(Some(&first), user.id).await.unwrap();
        assert_ne!(first, second);
        assert_eq!(
            store.resolve(&first, now()).await.unwrap(),
            None,
            "the presented id is gone, so a fixed id cannot be reused"
        );
        assert!(store.resolve(&second, now()).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn a_payload_that_is_not_a_session_is_refused_and_removed() {
        let pool = temp_db().await;
        let store = SessionStore::new(&pool);
        SessionRepo::new(&pool)
            .save("junk", b"not json", None)
            .await
            .unwrap();

        assert_eq!(store.resolve("junk", now()).await.unwrap(), None);
        assert!(SessionRepo::new(&pool).get("junk").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn garbage_collection_drops_only_the_closed_windows() {
        let pool = temp_db().await;
        let user = test_user(&pool, "person@example.org", "author", true).await;
        let store = SessionStore::new(&pool);
        let fresh = "fresh-session";
        SessionRepo::new(&pool)
            .save(fresh, &payload(user.id), Some(1 + SESSION_LIFETIME_SECS))
            .await
            .unwrap();
        SessionRepo::new(&pool)
            .save("stale-session", &payload(user.id), Some(0))
            .await
            .unwrap();

        let at = 1 + SESSION_LIFETIME_SECS;
        assert_eq!(store.gc_expired(at).await.unwrap(), 1);
        assert!(store.resolve(fresh, at).await.unwrap().is_some());
        assert!(
            SessionRepo::new(&pool)
                .get("stale-session")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn a_cookie_header_carries_the_token() {
        assert_eq!(cookie_token("romeka_session=abc"), Some("abc"));
        assert_eq!(
            cookie_token("locale=en; romeka_session=abc; other=1"),
            Some("abc")
        );
        assert_eq!(cookie_token("romeka_session="), Some(""));
        assert_eq!(cookie_token("other=1"), None);
        assert_eq!(cookie_token(""), None);
    }

    #[test]
    fn the_cookie_attributes_are_the_agreed_ones() {
        assert_eq!(
            set_cookie("abc", false),
            "romeka_session=abc; Path=/; HttpOnly; SameSite=Lax"
        );
        assert_eq!(
            set_cookie("abc", true),
            "romeka_session=abc; Path=/; HttpOnly; SameSite=Lax; Secure"
        );
        assert_eq!(
            clear_cookie(false),
            "romeka_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"
        );
        assert!(clear_cookie(true).ends_with("; Secure"));
    }
}
