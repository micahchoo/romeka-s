//! The theme's own settings, the one map a site stores under
//! `theme_settings_<theme>`.
//!
//! `SiteAdmin\IndexController::themeSettingsAction` reads and writes the
//! settings map for the site's current theme through `siteSettings()`, under
//! the id `theme_settings_<theme>` (`Site\Theme\Theme::getSettingsKey`), with
//! the map as the JSON value. Those are the theme's `[config]` settings, not
//! the per-key site settings ([`crate::site_settings`]).
//!
//! Upstream has no API member for the map because its admin is server-rendered.
//! The port is API-first, so the map travels as `o:theme_settings` on the site
//! document and the site write accepts it.

use omeka_db::ids::SiteId;
use omeka_db::repos::SiteSettingRepo;
use serde_json::Value;
use sqlx::SqlitePool;

use crate::errors::ApiError;
use crate::read::db_error;

/// The key prefix every theme's own settings row starts with
/// (`Theme::getSettingsKey`).
const PREFIX: &str = "theme_settings_";

/// The `site_setting.id` a theme's settings are stored under
/// (`Theme::getSettingsKey`).
#[must_use]
pub fn key(theme: &str) -> String {
    format!("{PREFIX}{theme}")
}

/// Whether a `site_setting.id` is a theme's own settings row rather than a
/// per-key site setting.
#[must_use]
pub fn owns(id: &str) -> bool {
    id.starts_with(PREFIX)
}

/// The stored settings map for a site's theme, `None` when no row stores one.
///
/// # Errors
/// [`ApiError::Internal`] when the row cannot be read.
pub async fn load(pool: &SqlitePool, site: SiteId, theme: &str) -> Result<Option<Value>, ApiError> {
    SiteSettingRepo::new(pool)
        .get(site, &key(theme))
        .await
        .map_err(db_error)
}

/// Store, or with `None` remove, the settings map for a site's theme.
///
/// # Errors
/// [`ApiError::Internal`] when the value cannot be written.
pub async fn store(
    pool: &SqlitePool,
    site: SiteId,
    theme: &str,
    value: Option<&Value>,
) -> Result<(), ApiError> {
    let repo = SiteSettingRepo::new(pool);
    match value {
        None => {
            repo.delete(site, &key(theme)).await.map_err(db_error)?;
        }
        Some(value) => {
            repo.set(site, &key(theme), value).await.map_err(db_error)?;
        }
    }
    Ok(())
}

/// The sanitiser's refusals for a map's `custom_css`, one message per refusal
/// with its line and column, or `None` when the CSS passes or the map carries
/// none. The site write and the preview answer these as a 422 under
/// `o:theme_settings`. The render sanitises again, so this check is what tells
/// the curator, not what keeps the page safe.
#[must_use]
pub fn custom_css_errors(settings: &Value) -> Option<Vec<String>> {
    let css = settings.get("custom_css")?.as_str()?;
    let refusals = omeka_site::theme::custom_css::sanitize(css.trim()).err()?;
    Some(
        refusals
            .iter()
            .map(|refusal| format!("Custom CSS: {refusal}"))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The key is the theme name, so two themes on one site never collide.
    #[test]
    fn settings_key_names_the_theme() {
        assert_eq!(key("default"), "theme_settings_default");
        assert_eq!(key("other"), "theme_settings_other");
    }

    /// A theme row is told apart from a per-key site setting.
    #[test]
    fn owns_recognises_the_theme_prefix() {
        assert!(owns("theme_settings_default"));
        assert!(!owns("pagination_per_page"));
        assert!(!owns("theme_settings"));
    }
}
