//! The user and global settings the API exposes, and the admin locale they
//! resolve to.
//!
//! Upstream's admin is server-rendered, so its user settings and its global
//! settings never cross the API: `Admin\UserController::editAction` writes the
//! `user-settings` fieldset straight into `user_setting`, and
//! `Admin\SettingController::browseAction` writes `SettingForm` into `setting`.
//! The port's admin is an SPA, so both travel over HTTP, and this module is the
//! boundary. Only the keys in [`API_SETTINGS`] cross it: a caller cannot reach
//! a row the port's admin has no field for.
//!
//! A user's settings travel as `o:user_settings` on the user document and the
//! user write, the way `o:site_settings` travels on the site
//! ([`crate::site_settings`]). The global settings have their own port-only
//! route, which only a global admin may read or write.
//!
//! The admin locale is `MvcListeners::bootstrapLocale` (lines 132-146): the
//! signed-in user's `locale`, else the global `locale`, else none, which the
//! client reads as its own default.

use omeka_db::ids::UserId;
use omeka_db::repos::{SettingRepo, UserSettingRepo};
use serde_json::{Map, Value};
use sqlx::SqlitePool;

use crate::auth::Actor;
use crate::errors::{ApiError, ValidationErrors};
use crate::read::response::serialize;
use crate::read::{Api, ApiResponse, db_error};

/// The `locale` key, in `setting` and in `user_setting` alike.
pub const LOCALE: &str = "locale";

/// The keys the API reads and writes. Each one has a field in the admin.
pub const API_SETTINGS: &[&str] = &[LOCALE];

/// The member a user's settings travel in.
pub const USER_SETTINGS_MEMBER: &str = "o:user_settings";

/// The message a caller other than a global admin gets on the global settings.
/// `AclFactory` grants `Omeka\Controller\Admin\Setting` to `global_admin` only.
pub const SETTINGS_DENIED: &str =
    "Permission denied for the current user to access the global settings.";

/// One write a settings map asks for: a value to store, or `None` to remove the row.
type Change = (&'static str, Option<Value>);

/// The admin locale for a signed-in user: their `locale` setting, else the
/// global one, else `None`. An empty string counts as unset, because upstream
/// tests `!$locale`.
///
/// # Errors
/// [`ApiError::Internal`] when a row cannot be read.
pub async fn admin_locale(pool: &SqlitePool, user: UserId) -> Result<Option<String>, ApiError> {
    let own = UserSettingRepo::new(pool)
        .get(user, LOCALE)
        .await
        .map_err(db_error)?;
    if let Some(locale) = non_empty(own) {
        return Ok(Some(locale));
    }
    let global = SettingRepo::new(pool).get(LOCALE).await.map_err(db_error)?;
    Ok(non_empty(global))
}

/// A user's API settings as one map, `None` when the user stores none.
///
/// # Errors
/// [`ApiError::Internal`] when a row cannot be read.
pub async fn load_user(pool: &SqlitePool, user: UserId) -> Result<Option<Value>, ApiError> {
    let repo = UserSettingRepo::new(pool);
    let mut map = Map::new();
    for key in API_SETTINGS {
        if let Some(value) = repo.get(user, key).await.map_err(db_error)? {
            map.insert((*key).to_string(), value);
        }
    }
    Ok((!map.is_empty()).then_some(Value::Object(map)))
}

/// Validate an `o:user_settings` member into the writes it asks for. The caller
/// validates before it writes the user row and stores after, so a refused map
/// leaves the user as it was.
///
/// # Errors
/// [`ApiError::Validation`], keyed by `o:user_settings`, for a member that is
/// not an object, a key the API does not expose, or a value that is not a
/// string or `null`.
pub fn parse_user(value: &Value) -> Result<Vec<Change>, ApiError> {
    parse_map(USER_SETTINGS_MEMBER, value)
}

/// Apply the writes [`parse_user`] returned.
///
/// # Errors
/// [`ApiError::Internal`] for a storage failure.
pub async fn store_user(
    pool: &SqlitePool,
    user: UserId,
    changes: &[Change],
) -> Result<(), ApiError> {
    let repo = UserSettingRepo::new(pool);
    for (key, value) in changes {
        match value {
            Some(value) => repo.set(user, key, value).await.map_err(db_error)?,
            None => {
                repo.delete(user, key).await.map_err(db_error)?;
            }
        }
    }
    Ok(())
}

/// `GET /api/settings`: the global API settings as one map, for a global admin.
///
/// # Errors
/// [`ApiError::PermissionDenied`] for any other caller, [`ApiError::Internal`]
/// when a row cannot be read.
pub async fn read_global(api: &Api<'_>) -> Result<ApiResponse, ApiError> {
    authorize_global(api.actor())?;
    let repo = SettingRepo::new(api.pool());
    let mut map = Map::new();
    for key in API_SETTINGS {
        if let Some(value) = repo.get(key).await.map_err(db_error)? {
            map.insert((*key).to_string(), value);
        }
    }
    Ok(ApiResponse::document(serialize(&Value::Object(map))?))
}

/// `PATCH /api/settings`: merge a map into the global settings, for a global
/// admin, and answer the stored map. A key the map omits keeps its value; a
/// `null` or an empty string removes the row.
///
/// # Errors
/// [`ApiError::PermissionDenied`] for any other caller, a 400 for a body that
/// is not a JSON object, [`ApiError::Validation`] for a map [`read_global`]
/// could not have answered, [`ApiError::Internal`] for a storage failure.
pub async fn write_global(api: &Api<'_>, bytes: &[u8]) -> Result<ApiResponse, ApiError> {
    authorize_global(api.actor())?;
    let body: Value =
        serde_json::from_slice(bytes).map_err(|_| ApiError::bad_request("JSON: Syntax error"))?;
    let repo = SettingRepo::new(api.pool());
    for (key, value) in parse_map("settings", &body)? {
        match value {
            Some(value) => repo.set(key, &value).await.map_err(db_error)?,
            None => {
                repo.delete(key).await.map_err(db_error)?;
            }
        }
    }
    read_global(api).await
}

fn authorize_global(actor: &Actor) -> Result<(), ApiError> {
    if actor.role() == Some("global_admin") {
        Ok(())
    } else {
        Err(ApiError::PermissionDenied {
            message: SETTINGS_DENIED.to_string(),
        })
    }
}

fn parse_map(member: &str, value: &Value) -> Result<Vec<Change>, ApiError> {
    let invalid = |message: String| {
        let mut errors = ValidationErrors::new();
        errors.insert(member.to_string(), vec![message]);
        ApiError::Validation(errors)
    };
    let Value::Object(map) = value else {
        return Err(invalid("Expected an object.".to_string()));
    };
    let mut changes = Vec::with_capacity(map.len());
    for (key, stored) in map {
        let Some(known) = API_SETTINGS.iter().find(|known| **known == key.as_str()) else {
            return Err(invalid(format!("The setting {key} is not available.")));
        };
        let change = match stored {
            Value::Null => None,
            Value::String(text) if text.is_empty() => None,
            Value::String(_) => Some(stored.clone()),
            _ => {
                return Err(invalid(format!(
                    "The setting {key} must be a string or null."
                )));
            }
        };
        changes.push((*known, change));
    }
    Ok(changes)
}

fn non_empty(value: Option<Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) if !text.is_empty() => Some(text),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use serde_json::json;

    #[test]
    fn a_map_names_only_the_exposed_keys_and_strings() {
        assert_eq!(
            parse_user(&json!({"locale": "es"})).unwrap(),
            vec![(LOCALE, Some(json!("es")))]
        );
        assert_eq!(
            parse_user(&json!({"locale": ""})).unwrap(),
            vec![(LOCALE, None)],
            "an empty locale is the Default option, which removes the row"
        );
        assert!(parse_user(&json!({"default_resource_template": 1})).is_err());
        assert!(parse_user(&json!({"locale": 3})).is_err());
        assert!(parse_user(&json!(["locale"])).is_err());
    }
}
