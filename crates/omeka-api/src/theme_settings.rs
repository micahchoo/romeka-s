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

use omeka_db::DbPool;
use omeka_db::ids::SiteId;
use omeka_db::repos::SiteSettingRepo;
use omeka_db::with_db;
use omeka_site::theme::custom_css::{CssError, CssProblem, MAX_BYTES, sanitize};
use serde_json::Value;

use crate::errors::ApiError;
use crate::messages;
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
pub async fn load(pool: &DbPool, site: SiteId, theme: &str) -> Result<Option<Value>, ApiError> {
    with_db!(pool, |db| SiteSettingRepo::new(db)
        .get(site, &key(theme))
        .await)
    .map_err(db_error)
}

/// Store, or with `None` remove, the settings map for a site's theme.
///
/// # Errors
/// [`ApiError::Internal`] when the value cannot be written.
pub async fn store(
    pool: &DbPool,
    site: SiteId,
    theme: &str,
    value: Option<&Value>,
) -> Result<(), ApiError> {
    match value {
        None => {
            with_db!(pool, |db| SiteSettingRepo::new(db)
                .delete(site, &key(theme))
                .await)
            .map_err(db_error)?;
        }
        Some(value) => {
            with_db!(pool, |db| SiteSettingRepo::new(db)
                .set(site, &key(theme), value)
                .await)
            .map_err(db_error)?;
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
    let refusals = sanitize(css.trim()).err()?;
    Some(refusals.iter().map(refusal_message).collect())
}

/// The registry sentence for one refusal, so the admin translates it whole.
fn refusal_message(error: &CssError) -> String {
    let (line, column) = (error.line, error.column);
    match &error.problem {
        CssProblem::TooLarge(bytes) => messages::css_too_large(line, column, MAX_BYTES, bytes),
        CssProblem::Import => messages::css_import(line, column),
        CssProblem::Document => messages::css_document(line, column),
        CssProblem::ForeignUrl(url) => messages::css_foreign_url(line, column, url),
        CssProblem::Expression => messages::css_expression(line, column),
        CssProblem::Behavior => messages::css_behavior(line, column),
        CssProblem::MozBinding => messages::css_moz_binding(line, column),
        CssProblem::StyleBreakout => messages::css_style_breakout(line, column),
        CssProblem::BadUrl => messages::css_bad_url(line, column),
        CssProblem::UnclosedString => messages::css_unclosed_string(line, column),
        CssProblem::StrayClose(bracket) => messages::css_stray_close(line, column, bracket),
    }
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

    /// The refusals for one custom CSS text, as the 422 carries them.
    fn refusals(css: &str) -> Vec<String> {
        custom_css_errors(&serde_json::json!({ "custom_css": css })).unwrap_or_default()
    }

    /// Each refusal kind is its own registry sentence, with the line, column
    /// and value it names, so the admin translates the whole sentence.
    #[test]
    fn each_refusal_kind_is_its_own_registry_sentence() {
        let cases = [
            (
                "@import 'x.css';",
                "Custom CSS: Line 1, column 1: @import is not allowed.",
            ),
            (
                "@-moz-document url-prefix() {}",
                "Custom CSS: Line 1, column 1: @document is not allowed.",
            ),
            (
                "a {\n  background: url(https://evil.example/x.png);\n}",
                "Custom CSS: Line 2, column 15: url(https://evil.example/x.png) is not allowed; \
                 a URL must be a path under /files/ or /assets/.",
            ),
            (
                "a { width: expression(1) }",
                "Custom CSS: Line 1, column 12: expression() is not allowed.",
            ),
            (
                "a { behavior: none }",
                "Custom CSS: Line 1, column 5: behavior is not allowed.",
            ),
            (
                "a { -moz-binding: none }",
                "Custom CSS: Line 1, column 5: -moz-binding is not allowed.",
            ),
            (
                "a { content: '</style>' }",
                "Custom CSS: Line 1, column 15: </style is not allowed.",
            ),
            (
                "a { background: url(/files/a b.png) }",
                "Custom CSS: Line 1, column 17: this url( is not valid CSS.",
            ),
            (
                "a { content: 'open\n }",
                "Custom CSS: Line 1, column 14: this string is not closed.",
            ),
            (
                "a { color: red } }",
                "Custom CSS: Line 1, column 18: } closes nothing.",
            ),
            (
                "a { color: red ) }",
                "Custom CSS: Line 1, column 16: ) closes nothing.",
            ),
        ];
        for (css, expected) in cases {
            assert_eq!(refusals(css), vec![expected.to_string()], "{css:?}");
        }
        let big = format!("/*{}*/", "x".repeat(MAX_BYTES));
        assert_eq!(
            refusals(&big),
            vec![format!(
                "Custom CSS: Line 1, column 1: custom CSS may be at most {MAX_BYTES} bytes; this is {}.",
                big.len()
            )]
        );
    }

    /// A theme row is told apart from a per-key site setting.
    #[test]
    fn owns_recognises_the_theme_prefix() {
        assert!(owns("theme_settings_default"));
        assert!(!owns("pagination_per_page"));
        assert!(!owns("theme_settings"));
    }
}
