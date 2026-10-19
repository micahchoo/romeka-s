//! The site settings a site stores in `site_setting`, one row per key.
//!
//! Upstream saves every field of `View/Form/SiteSettingsForm` as its own
//! `site_setting` row (`Controller/SiteAdmin/IndexController.php:127-130`),
//! keyed by `(id, site_id)` with the value JSON-encoded
//! (`AbstractTargetSettings::set`). A theme's own settings are a separate row,
//! under `theme_settings_<theme>` ([`crate::theme_settings`]).
//!
//! Upstream has no API member for site settings because its admin is
//! server-rendered. The port is API-first, so the map travels as
//! `o:site_settings` on the site document and the site write accepts it. This
//! module is the boundary: it lists the rows as one map and writes a map back
//! as rows, never touching the theme's own row.

use omeka_db::ids::SiteId;
use omeka_db::repos::SiteSettingRepo;
use serde_json::{Map, Value};
use sqlx::SqlitePool;

use crate::errors::{ApiError, ValidationErrors};
use crate::messages;
use crate::read::db_error;
use crate::theme_settings;

/// The stored site settings as one map, `None` when the site stores none.
/// Theme-owned rows (`theme_settings_<theme>`) are not site settings and stay
/// out of the map.
///
/// # Errors
/// [`ApiError::Internal`] when the rows cannot be read.
pub async fn load(pool: &SqlitePool, site: SiteId) -> Result<Option<Value>, ApiError> {
    let rows = SiteSettingRepo::new(pool)
        .list(site)
        .await
        .map_err(db_error)?;
    let mut map = Map::new();
    for row in rows {
        if theme_settings::owns(&row.id) {
            continue;
        }
        map.insert(row.id, row.value);
    }
    if map.is_empty() {
        Ok(None)
    } else {
        Ok(Some(Value::Object(map)))
    }
}

/// Write a site settings map as rows.
///
/// `None` removes every site setting this site stores; an object upserts each
/// key and removes a key whose value is `null`; a key the object omits keeps
/// what it holds, so a `PATCH` from one admin form does not clear the settings
/// another form owns. A member that is neither an object nor `null` is a
/// validation error, keyed by `o:site_settings`.
///
/// # Errors
/// [`ApiError::Validation`] for a non-object member, [`ApiError::Internal`] for
/// a storage failure.
pub async fn store(pool: &SqlitePool, site: SiteId, value: Option<&Value>) -> Result<(), ApiError> {
    let repo = SiteSettingRepo::new(pool);
    match value {
        None => {
            for row in repo.list(site).await.map_err(db_error)? {
                if !theme_settings::owns(&row.id) {
                    repo.delete(site, &row.id).await.map_err(db_error)?;
                }
            }
        }
        Some(Value::Object(map)) => {
            for (key, stored) in map {
                // A theme's map stays the theme's: the caller writes it through
                // `o:theme_settings`, not here.
                if theme_settings::owns(key) {
                    continue;
                }
                if stored.is_null() {
                    repo.delete(site, key).await.map_err(db_error)?;
                } else {
                    repo.set(site, key, stored).await.map_err(db_error)?;
                }
            }
        }
        Some(other) => return Err(not_object(other)),
    }
    Ok(())
}

/// The rows a site created with this `o:site_settings` member starts with.
///
/// [`store`] on a site that has no rows yet, as a value: a `null` member or
/// key writes nothing, and a theme-owned key stays the theme's.
///
/// # Errors
/// [`ApiError::Validation`] for a member that is neither an object nor `null`.
pub fn new_site_rows(value: Option<&Value>) -> Result<Vec<(String, Value)>, ApiError> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Object(map)) => Ok(map
            .iter()
            .filter(|(key, stored)| !theme_settings::owns(key) && !stored.is_null())
            .map(|(key, stored)| (key.clone(), stored.clone()))
            .collect()),
        Some(other) => Err(not_object(other)),
    }
}

fn not_object(value: &Value) -> ApiError {
    let mut errors = ValidationErrors::new();
    errors.insert(
        "o:site_settings".to_string(),
        vec![messages::site_settings_not_object(json_kind(value))],
    );
    ApiError::Validation(errors)
}

/// The JSON type name a value carries, for the error message.
fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_kind_names_each_shape() {
        assert_eq!(json_kind(&Value::Array(Vec::new())), "an array");
        assert_eq!(json_kind(&Value::String("x".into())), "a string");
    }
}
