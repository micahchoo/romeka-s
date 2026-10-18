//! `user`, `api_key`, `password_creation` and `user_setting` (upstream `schema.sql`).
//!
//! The `password_hash` column holds a bcrypt hash from the PHP side; nothing here checks it.

use crate::backend::Backend;
use chrono::NaiveDateTime;
use serde_json::Value;

use crate::error::DbError;
use crate::ids::UserId;

/// One row of `user`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserRow {
    pub id: UserId,
    pub email: String,
    pub name: String,
    pub created: NaiveDateTime,
    pub modified: Option<NaiveDateTime>,
    pub password_hash: Option<String>,
    pub role: String,
    pub is_active: bool,
}

impl UserRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            email: DB::text(row, "email")?,
            name: DB::text(row, "name")?,
            created: DB::datetime(row, "created")?,
            modified: DB::opt_datetime(row, "modified")?,
            password_hash: DB::opt_text(row, "password_hash")?,
            role: DB::text(row, "role")?,
            is_active: DB::boolean(row, "is_active")?,
        })
    }
}

/// Insert input for a `user`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewUser {
    pub email: String,
    pub name: String,
    pub password_hash: Option<String>,
    pub role: String,
    pub is_active: bool,
    /// `None` means now.
    pub created: Option<NaiveDateTime>,
}

/// One row of `api_key`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKeyRow {
    /// Upstream stores a 32 character random id here, not an auto-increment integer.
    pub id: String,
    pub owner_id: UserId,
    pub label: String,
    pub credential_hash: String,
    /// A packed IPv4 or IPv6 address (`varbinary(16)` upstream, wrapped by an IP address type).
    pub last_ip: Option<Vec<u8>>,
    pub last_accessed: Option<NaiveDateTime>,
    pub created: NaiveDateTime,
}

impl ApiKeyRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::text(row, "id")?,
            owner_id: DB::id(row, "owner_id")?,
            label: DB::text(row, "label")?,
            credential_hash: DB::text(row, "credential_hash")?,
            last_ip: DB::opt_blob(row, "last_ip")?,
            last_accessed: DB::opt_datetime(row, "last_accessed")?,
            created: DB::datetime(row, "created")?,
        })
    }
}

/// Insert input for an `api_key`. The caller generates the id, as upstream's key repository does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewApiKey {
    pub id: String,
    pub owner_id: UserId,
    pub label: String,
    pub credential_hash: String,
    pub last_ip: Option<Vec<u8>>,
    pub last_accessed: Option<NaiveDateTime>,
    /// `None` means now.
    pub created: Option<NaiveDateTime>,
}

/// One row of `password_creation`, the one-time password activation token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordCreationRow {
    pub id: String,
    pub user_id: UserId,
    pub created: NaiveDateTime,
    pub activate: bool,
}

impl PasswordCreationRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::text(row, "id")?,
            user_id: DB::id(row, "user_id")?,
            created: DB::datetime(row, "created")?,
            activate: DB::boolean(row, "activate")?,
        })
    }
}

/// Insert input for a `password_creation`. The caller generates the id, as upstream's `Mailer`
/// does through `ApiKey`'s key generator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPasswordCreation {
    pub id: String,
    pub user_id: UserId,
    pub activate: bool,
    /// `None` means now.
    pub created: Option<NaiveDateTime>,
}

/// One row of `user_setting`.
#[derive(Debug, Clone, PartialEq)]
pub struct UserSettingRow {
    pub id: String,
    pub user_id: UserId,
    pub value: Value,
}

impl UserSettingRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::text(row, "id")?,
            user_id: DB::id(row, "user_id")?,
            value: DB::json(row, "value")?,
        })
    }
}
