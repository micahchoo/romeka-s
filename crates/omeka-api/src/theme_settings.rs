//! The theme settings a site stores in `site_setting`.
//!
//! `SiteAdmin\IndexController::themeSettingsAction` reads and writes the
//! settings map for the site's current theme through `siteSettings()`, under
//! the id `theme_settings_<theme>` (`Site\Theme\Theme::getSettingsKey`), with
//! the map as the JSON value. Upstream has no API member for it because its
//! admin is server-rendered. The port is API-first, so the map travels as
//! `o:theme_settings` on the site document and the site write accepts it.
//!
//! `omeka-db` has no repository for `site_setting`, so these two statements
//! live here beside their callers, as `read/system.rs` does for `job` and
//! `module`.

use omeka_db::ids::SiteId;
use serde_json::Value;
use sqlx::SqlitePool;

use crate::errors::ApiError;
use crate::read::db_error;

/// The `site_setting.id` a theme's settings are stored under
/// (`Theme::getSettingsKey`).
#[must_use]
pub fn key(theme: &str) -> String {
    format!("theme_settings_{theme}")
}

/// The stored settings map for a site's theme, `None` when no row stores one.
///
/// # Errors
/// [`ApiError::Internal`] when the stored value is not JSON.
pub async fn load(pool: &SqlitePool, site: SiteId, theme: &str) -> Result<Option<Value>, ApiError> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT value FROM site_setting WHERE site_id = ? AND id = ?")
            .bind(site.0 as i64)
            .bind(key(theme))
            .fetch_optional(pool)
            .await
            .map_err(|error| db_error(error.into()))?;
    let Some((text,)) = row else {
        return Ok(None);
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|error| ApiError::Internal {
            message: format!("site_setting value is not JSON: {error}"),
        })
}

/// Store, or with `None` remove, the settings map for a site's theme.
///
/// # Errors
/// [`ApiError::Internal`] when the value cannot be encoded.
pub async fn store(
    pool: &SqlitePool,
    site: SiteId,
    theme: &str,
    value: Option<&Value>,
) -> Result<(), ApiError> {
    let Some(value) = value else {
        sqlx::query("DELETE FROM site_setting WHERE site_id = ? AND id = ?")
            .bind(site.0 as i64)
            .bind(key(theme))
            .execute(pool)
            .await
            .map_err(|error| db_error(error.into()))?;
        return Ok(());
    };
    let text = serde_json::to_string(value).map_err(|error| ApiError::Internal {
        message: format!("cannot store site settings: {error}"),
    })?;
    sqlx::query(
        "INSERT INTO site_setting (id, site_id, value) VALUES (?, ?, ?)
         ON CONFLICT(id, site_id) DO UPDATE SET value = excluded.value",
    )
    .bind(key(theme))
    .bind(site.0 as i64)
    .bind(text)
    .execute(pool)
    .await
    .map_err(|error| db_error(error.into()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// The key is the theme name, so two themes on one site never collide.
    #[test]
    fn settings_key_names_the_theme() {
        assert_eq!(key("default"), "theme_settings_default");
        assert_eq!(key("other"), "theme_settings_other");
    }
}
