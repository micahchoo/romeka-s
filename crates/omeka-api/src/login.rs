//! Session login and the password flows: the port of `LoginController`, and the
//! admin route that mints a password link.
//!
//! | route | upstream |
//! | --- | --- |
//! | `POST /api/login` | `LoginController::loginAction` |
//! | `POST /api/logout` | `LoginController::logoutAction` |
//! | `GET /api/me` | the identity `loginAction` checks with `hasIdentity` |
//! | `POST /api/password` | `LoginController::createPasswordAction` |
//! | `POST /api/password/forgot` | `LoginController::forgotPasswordAction` |
//! | `POST /api/users/{id}/password-link` | `Mailer::getPasswordCreation` from `Admin\UserController::addAction` |
//!
//! Upstream reaches these through web forms, so the port answers them as JSON
//! routes. The decisions the controller makes are all here; `omeka-server` parses
//! a request, calls one function, and writes a cookie header, a status and a
//! body.
//!
//! **Why 403 and 429 exist.** Upstream's web login has one failure: a wrong
//! password, an unknown address and an inactive account all render "Email or
//! password is invalid". `PasswordAdapter::authenticate` returns a distinct
//! result for an inactive account, and the port exposes it, because an
//! administrator who deactivated the account can act on that answer where a
//! person cannot act on "email or password is invalid". The distinct answer only
//! arrives with a correct password, so it does not enumerate addresses to a
//! guesser. 429 is the rate limit; upstream has none.
//!
//! **Password expiry.** `createPasswordAction` refuses a key past its two weeks
//! and sets the user inactive on the way out. The port keeps that, because
//! `Entity\User::$isActive` is what the invite flow sets, and reports it as the
//! one upstream behaviour an operator may not want.

use std::net::IpAddr;
use std::time::Instant;

use chrono::{Duration, NaiveDateTime, Utc};
use http::StatusCode;
use sqlx::SqlitePool;
use thiserror::Error;

use omeka_db::ids::UserId;
use omeka_db::repos::{PasswordCreationRepo, UserRepo};
use omeka_db::rows::{NewPasswordCreation, UserRow};

use crate::auth::Actor;
use crate::errors::{ApiError, ErrorDocument};
use crate::limits::{Limiter, Subject, Verdict};
use crate::password::{self, HashError};
use crate::read::{db_error, not_found, parse_id, permission_denied};
use crate::routes::Operation;
use crate::session::SessionStore;
use crate::token;
use crate::write::access;

/// The password rules a new password must satisfy, re-exported so a caller of
/// [`set_password`] names them without reaching into `crate::password`.
pub use crate::password::PasswordRules;

/// `PasswordCreation::getExpiration`: `created` plus `P2W`.
pub const KEY_LIFETIME_DAYS: i64 = 14;

/// The `o:email` message for a refused login, `LoginController::loginAction:69`.
pub const INVALID_CREDENTIALS: &str = "Email or password is invalid";
/// The port's message for a correct password on an inactive account.
pub const INACTIVE_ACCOUNT: &str = "The user is not active.";
/// Laminas' text for 429.
pub const TOO_MANY_REQUESTS: &str = "Too Many Requests";
/// The body of a 500. The detail goes to the server's log, not to the caller.
pub const INTERNAL_ERROR: &str = "Internal Server Error";
/// `LoginController::createPasswordAction:109` for a key that is not there.
pub const UNKNOWN_KEY: &str = "Invalid password creation key.";

/// A logged-in session: the user the request authenticated as, and the token the
/// response sets as a cookie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub user: UserRow,
    pub token: String,
}

/// A failure of login or forgot-password.
///
/// Forgot-password mints a key or refuses; it cannot return
/// [`AuthError::InvalidCredentials`] or [`AuthError::Inactive`], because it
/// answers whether the address matched. The two routes share the type so one
/// status and document mapping covers both.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AuthError {
    #[error("email or password is invalid")]
    InvalidCredentials,
    #[error("the account is not active")]
    Inactive,
    #[error("too many attempts")]
    RateLimited { retry_after_secs: u64 },
    #[error("internal error: {message}")]
    Internal { message: String },
}

impl AuthError {
    /// The status the route answers with.
    #[must_use]
    pub fn status(&self) -> StatusCode {
        match self {
            Self::InvalidCredentials => StatusCode::UNAUTHORIZED,
            Self::Inactive => StatusCode::FORBIDDEN,
            Self::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// The error document the route answers with.
    ///
    /// The two port statuses carry the port's own sentence, and a 500 carries
    /// the generic one: the storage detail is logged, not sent to a caller.
    #[must_use]
    pub fn document(&self) -> ErrorDocument {
        match self {
            Self::InvalidCredentials => ErrorDocument::general(INVALID_CREDENTIALS),
            Self::Inactive => ErrorDocument::general(INACTIVE_ACCOUNT),
            Self::RateLimited { .. } => ErrorDocument::general(TOO_MANY_REQUESTS),
            Self::Internal { .. } => ErrorDocument::general(INTERNAL_ERROR),
        }
    }

    /// The `Retry-After` the route answers with, when it is a limit.
    #[must_use]
    pub fn retry_after_secs(&self) -> Option<u64> {
        match self {
            Self::RateLimited { retry_after_secs } => Some(*retry_after_secs),
            _ => None,
        }
    }
}

impl From<ApiError> for AuthError {
    fn from(error: ApiError) -> Self {
        Self::Internal {
            message: error.to_string(),
        }
    }
}

impl From<HashError> for AuthError {
    fn from(error: HashError) -> Self {
        Self::Internal {
            message: error.to_string(),
        }
    }
}

impl From<omeka_db::DbError> for AuthError {
    fn from(error: omeka_db::DbError) -> Self {
        Self::Internal {
            message: error.to_string(),
        }
    }
}

/// A one-time password key, with the facts the mail and the answer need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedKey {
    pub key: String,
    pub user: UserRow,
    /// Whether setting the password activates the account
    /// (`PasswordCreation::$activate`).
    pub activate: bool,
    pub expires_at: NaiveDateTime,
}

impl IssuedKey {
    /// The expiry as the mail body writes it.
    ///
    /// Upstream formats it with the i18n date helper's medium pattern
    /// (`Mailer::getExpiration`); the port has no locale here, so the instant
    /// is written in UTC.
    #[must_use]
    pub fn expires_display(&self) -> String {
        self.expires_at.format("%Y-%m-%d %H:%M UTC").to_string()
    }
}

/// `LoginController::loginAction`: authenticate, then start a session.
///
/// The limit is checked before the user is read and before bcrypt runs, so a
/// guesser pays for one map lookup rather than a hash.
///
/// # Errors
/// [`AuthError`]: 401 for an unknown address or a wrong password, 403 for a
/// correct password on an inactive account, 429 when the limit trips.
pub async fn login(
    pool: &SqlitePool,
    limiter: &Limiter,
    presented: Option<&str>,
    email: &str,
    password: &str,
    address: Option<IpAddr>,
    now: Instant,
) -> Result<Session, AuthError> {
    let subjects = login_subjects(email, address);
    if let Verdict::Refused { retry_after_secs } = limiter.admit(&subjects, now) {
        return Err(AuthError::RateLimited { retry_after_secs });
    }
    let Some(user) = UserRepo::new(pool).get_by_email(email).await? else {
        return Err(AuthError::InvalidCredentials);
    };
    if !password::verify(password, user.password_hash.as_deref()) {
        return Err(AuthError::InvalidCredentials);
    }
    if !user.is_active {
        return Err(AuthError::Inactive);
    }
    limiter.reset(&Subject::LoginEmail(email.to_string()));
    let store = SessionStore::new(pool);
    // `SessionManager` garbage-collects on session start; one login is a cheap
    // place to drop the rows whose windows have closed.
    let _ = store.gc_expired(Utc::now().timestamp()).await;
    let token = store.rotate(presented, user.id).await?;
    Ok(Session { user, token })
}

/// `LoginController::logoutAction`: destroy the session behind the cookie.
///
/// # Errors
/// An [`ApiError`] when the delete fails.
pub async fn logout(pool: &SqlitePool, token: Option<&str>) -> Result<bool, ApiError> {
    match token {
        Some(token) => SessionStore::new(pool).revoke(token).await,
        None => Ok(false),
    }
}

/// The identity for one request: the user behind a live session.
///
/// # Errors
/// An [`ApiError`] when a query fails.
pub async fn current_user(
    pool: &SqlitePool,
    token: Option<&str>,
) -> Result<Option<UserRow>, ApiError> {
    match token {
        Some(token) => {
            SessionStore::new(pool)
                .resolve(token, crate::session::unix_now())
                .await
        }
        None => Ok(None),
    }
}

/// The message `GET /api/me` answers an anonymous caller with.
pub const AUTHENTICATION_REQUIRED: &str = "Authentication required.";

/// Why `POST /api/password` refused a key.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SetPasswordError {
    /// No key row, or one whose two weeks have passed.
    #[error("invalid password creation key")]
    UnknownKey,
    /// The password does not satisfy `Config['password']`.
    #[error("the password is invalid")]
    InvalidPassword,
    #[error("internal error: {message}")]
    Internal { message: String },
}

impl SetPasswordError {
    #[must_use]
    pub fn status(&self) -> StatusCode {
        match self {
            Self::UnknownKey => StatusCode::NOT_FOUND,
            Self::InvalidPassword => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// The error document the route answers with.
    #[must_use]
    pub fn document(&self) -> ErrorDocument {
        match self {
            Self::UnknownKey => ErrorDocument::general(UNKNOWN_KEY),
            Self::InvalidPassword => {
                let mut errors = crate::errors::ValidationErrors::new();
                errors.insert(
                    "password".to_string(),
                    vec![password::INVALID_PASSWORD.to_string()],
                );
                ErrorDocument::validation(errors)
            }
            Self::Internal { .. } => ErrorDocument::general(INTERNAL_ERROR),
        }
    }
}

impl From<ApiError> for SetPasswordError {
    fn from(error: ApiError) -> Self {
        Self::Internal {
            message: error.to_string(),
        }
    }
}

impl From<HashError> for SetPasswordError {
    fn from(error: HashError) -> Self {
        Self::Internal {
            message: error.to_string(),
        }
    }
}

impl From<omeka_db::DbError> for SetPasswordError {
    fn from(error: omeka_db::DbError) -> Self {
        Self::Internal {
            message: error.to_string(),
        }
    }
}
/// `LoginController::createPasswordAction`: spend a one-time key on a password.
///
/// The order is upstream's: the key must exist, it must be inside its two weeks,
/// the password must satisfy the rules, and only then is anything written. An
/// expired key deactivates its user and is deleted, which is what
/// `createPasswordAction:115` does; a rejected password leaves the key in place.
///
/// # Errors
/// A [`SetPasswordError`]: 404 for an unknown or expired key, 422 for a password
/// the rules reject.
pub async fn set_password(
    pool: &SqlitePool,
    rules: &PasswordRules,
    key: &str,
    password: &str,
) -> Result<UserRow, SetPasswordError> {
    set_password_at(pool, rules, key, password, Utc::now().naive_utc()).await
}

/// [`set_password`] against a caller's clock, so a test can place itself before
/// or after the key's expiry.
///
/// # Errors
/// A [`SetPasswordError`], as [`set_password`].
pub async fn set_password_at(
    pool: &SqlitePool,
    rules: &PasswordRules,
    key: &str,
    password: &str,
    now: NaiveDateTime,
) -> Result<UserRow, SetPasswordError> {
    let keys = PasswordCreationRepo::new(pool);
    let Some(creation) = keys.get(key).await.map_err(db_error)? else {
        return Err(SetPasswordError::UnknownKey);
    };
    let users = UserRepo::new(pool);
    let Some(mut user) = users.get(creation.user_id).await.map_err(db_error)? else {
        keys.delete(key).await.map_err(db_error)?;
        return Err(SetPasswordError::UnknownKey);
    };
    if now > expires_at(creation.created) {
        user.is_active = false;
        users.update(&user).await.map_err(db_error)?;
        keys.delete(key).await.map_err(db_error)?;
        return Err(SetPasswordError::UnknownKey);
    }
    if !rules.accepts(password) {
        return Err(SetPasswordError::InvalidPassword);
    }
    user.password_hash = Some(password::hash(password)?);
    if creation.activate {
        user.is_active = true;
    }
    users.update(&user).await.map_err(db_error)?;
    keys.delete(key).await.map_err(db_error)?;
    Ok(user)
}

/// `LoginController::forgotPasswordAction`: mint a reset key for an active
/// account, and say nothing either way.
///
/// `None` covers an unknown address and an inactive account, which upstream
/// filters with `isActive => true` before it mints anything. Upstream deletes the
/// user's existing key before it mints the new one; [`issue_key`] does the same,
/// so a user holds one live key.
///
/// # Errors
/// An [`AuthError`]: 429 when the limit trips, 500 when storage fails.
pub async fn forgot_password(
    pool: &SqlitePool,
    limiter: &Limiter,
    email: &str,
    address: Option<IpAddr>,
    now: Instant,
) -> Result<Option<IssuedKey>, AuthError> {
    let subjects = forgot_subjects(email, address);
    if let Verdict::Refused { retry_after_secs } = limiter.admit(&subjects, now) {
        return Err(AuthError::RateLimited { retry_after_secs });
    }
    let Some(user) = UserRepo::new(pool).get_by_email(email).await? else {
        return Ok(None);
    };
    if !user.is_active {
        return Ok(None);
    }
    Ok(Some(issue_key(pool, &user, false).await?))
}

/// `Mailer::getPasswordCreation` plus `setId`: a fresh key for the user,
/// replacing whatever key they held.
///
/// # Errors
/// An [`ApiError`] when the token cannot be drawn or a row cannot be written.
pub async fn issue_key(
    pool: &SqlitePool,
    user: &UserRow,
    activate: bool,
) -> Result<IssuedKey, ApiError> {
    let keys = PasswordCreationRepo::new(pool);
    let key = token::token()?;
    keys.delete_for_user(user.id)
        .await
        .map_err(ApiError::internal)?;
    let row = keys
        .create(&NewPasswordCreation {
            id: key,
            user_id: user.id,
            activate,
            created: None,
        })
        .await
        .map_err(ApiError::internal)?;
    Ok(IssuedKey {
        key: row.id,
        user: user.clone(),
        activate: row.activate,
        expires_at: expires_at(row.created),
    })
}

/// `PasswordCreation::getExpiration`.
#[must_use]
pub fn expires_at(created: NaiveDateTime) -> NaiveDateTime {
    created + Duration::days(KEY_LIFETIME_DAYS)
}

/// The admin route: mint a key for one user and hand the caller the link.
///
/// The actor must hold an admin role and must not be the user the key is for.
/// Upstream gates the invite with the `activate-user` privilege, which
/// `AclFactory` grants the two admin roles over another user and denies over the
/// actor's own account (`IsSelfAssertion`); an administrator resets their own
/// password with a key minted by `omeka user add` or with the forgot route.
///
/// # Errors
/// An [`ApiError`]: 404 for an id that names no user, 403 for a caller who is
/// not an administrator or who names their own account.
pub async fn password_link(
    pool: &SqlitePool,
    actor: &Actor,
    raw_id: &str,
) -> Result<IssuedKey, ApiError> {
    const CLASS: &str = "Omeka\\Entity\\User";
    let id = parse_id(CLASS, raw_id)?;
    let Some(user) = UserRepo::new(pool)
        .get(UserId(id))
        .await
        .map_err(db_error)?
    else {
        return Err(not_found(CLASS, raw_id));
    };
    if !access::is_admin_role(actor.role()) {
        return Err(permission_denied("users", Operation::Create));
    }
    if actor.user() == Some(user.id) {
        return Err(crate::read::entity_denied(CLASS, Operation::Create));
    }
    // The invite is an activation: `Admin\UserController::addAction` sends
    // `sendUserActivation`, and `Mailer` builds that key with activate true.
    issue_key(pool, &user, true).await
}

/// The subjects one login attempt is counted against.
fn login_subjects(email: &str, address: Option<IpAddr>) -> Vec<Subject> {
    let mut subjects = vec![Subject::LoginEmail(email.to_string())];
    if let Some(address) = address {
        subjects.push(Subject::LoginAddress(address));
    }
    subjects
}

/// The subjects one forgot-password request is counted against.
fn forgot_subjects(email: &str, address: Option<IpAddr>) -> Vec<Subject> {
    let mut subjects = vec![Subject::ForgotEmail(email.to_string())];
    if let Some(address) = address {
        subjects.push(Subject::ForgotAddress(address));
    }
    subjects
}
