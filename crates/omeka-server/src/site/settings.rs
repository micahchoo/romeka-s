//! The site settings one render reads, loaded from the `site_setting` rows.
//!
//! Every key is its own row (`AbstractTargetSettings::set`), and upstream forms
//! store values as strings (`"1"` for a checked checkbox, `"50"` for a number
//! field), so the loader coerces each row to the field's type and treats an
//! empty value as unset. A theme's own settings row (`theme_settings_<theme>`)
//! is not a site setting and is skipped.

use omeka_db::DbError;
use omeka_db::DbPool;
use omeka_db::ids as db;
use omeka_db::repos::{SettingRepo, SiteSettingRepo};
use omeka_db::with_db;
use omeka_site::settings::{BrowseDefaults, SiteSettings};
use serde_json::Value;

/// The site settings a site stores, with each key's default in force where no
/// row holds it.
///
/// An unset or empty site `locale` takes the global `locale` setting: upstream
/// bootstraps the translator from the global setting
/// (`MvcListeners::bootstrapLocale`, `MvcListeners.php:132-146`) and the site
/// setting overrides it only when it is set (`:378-381`).
pub(crate) async fn load(pool: &DbPool, site: db::SiteId) -> Result<SiteSettings, DbError> {
    let rows = with_db!(pool, |db| SiteSettingRepo::new(db).list(site).await)?;
    let mut settings = SiteSettings::default();
    for row in rows {
        apply(&mut settings, &row.id, &row.value);
    }
    if settings.locale.is_none() {
        let global = with_db!(pool, |db| SettingRepo::new(db).get("locale").await)?;
        settings.locale = global.as_ref().and_then(text);
    }
    Ok(settings)
}

/// Set one field from a stored row. A key this layer does not read (a theme row
/// or a module key) is ignored.
fn apply(settings: &mut SiteSettings, key: &str, value: &Value) {
    match key {
        "installation_title" => settings.installation_title = text(value),
        "locale" => settings.locale = text(value),
        "attachment_link_type" => settings.attachment_link_type = text(value),
        "browse_attached_items" => settings.browse_attached_items = boolean(value),
        "browse_heading_property_term" => settings.browse_heading_property_term = text(value),
        "browse_body_property_term" => settings.browse_body_property_term = text(value),
        "filter_locale_values" => settings.filter_locale_values = boolean(value),
        "exclude_resources_not_in_site" => {
            settings.exclude_resources_not_in_site = boolean(value);
        }
        "iiif_viewer_theme" => settings.iiif_viewer_theme = text(value),
        "subnav_display" => settings.subnav_display = text(value),
        "show_page_pagination" => settings.show_page_pagination = boolean(value),
        "search_type" => settings.search_type = text(value),
        "search_resource_names" => settings.search_resource_names = string_list(value),
        "show_user_bar" => settings.show_user_bar = integer(value),
        "disable_jsonld_embed" => settings.disable_jsonld_embed = boolean(value),
        "favicon" => settings.favicon = text(value),
        "show_locale_label" => settings.show_locale_label = boolean(value),
        "pagination_per_page" => settings.pagination_per_page = number(value),
        "browse_defaults_public_items" => {
            settings.browse_defaults_public_items = browse_defaults(value);
        }
        "show_attached_pages" => settings.show_attached_pages = boolean(value),
        "property_label_information" => settings.property_label_information = text(value),
        "show_value_annotations" => settings.show_value_annotations = text(value),
        "item_media_embed" => settings.item_media_embed = boolean(value),
        "vocabulary_scope" => settings.vocabulary_scope = text(value),
        "search_apply_templates" => settings.search_apply_templates = number_list(value),
        "search_restrict_templates" => settings.search_restrict_templates = boolean(value),
        "iiif_viewer_sidebar" => settings.iiif_viewer_sidebar = boolean(value),
        "romeka_location_property" => settings.romeka_location_property = text(value),
        "romeka_map_tiles" => settings.romeka_map_tiles = text(value),
        "romeka_map_attribution" => settings.romeka_map_attribution = text(value),
        _ => {}
    }
}

/// A text setting. An empty string reads as unset, which is how an upstream
/// form's blank field arrives after `ToNull`.
fn text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// A boolean setting from a JSON boolean, a number, or the strings an upstream
/// form stores (`"1"`, `"true"`, `"on"`).
fn boolean(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(value) => Some(*value),
        Value::Number(number) => Some(number.as_i64().is_some_and(|n| n != 0)),
        Value::String(text) => match text.to_ascii_lowercase().as_str() {
            "1" | "true" | "on" | "yes" => Some(true),
            "0" | "false" | "off" | "no" | "" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// An integer setting from a number or a numeric string.
fn integer(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

/// A non-negative number setting (`pagination_per_page`).
fn number(value: &Value) -> Option<u32> {
    integer(value).and_then(|value| u32::try_from(value).ok())
}

/// A list of strings (`search_resource_names`).
fn string_list(value: &Value) -> Option<Vec<String>> {
    let array = value.as_array()?;
    Some(
        array
            .iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect(),
    )
}

/// A list of numbers (`search_apply_templates`).
fn number_list(value: &Value) -> Option<Vec<u32>> {
    let array = value.as_array()?;
    Some(array.iter().filter_map(number).collect())
}

/// The item browse's default sort (`Stdlib/Browse.php:167-170`).
fn browse_defaults(value: &Value) -> Option<BrowseDefaults> {
    let object = value.as_object()?;
    Some(BrowseDefaults {
        sort_by: object
            .get("sort_by")
            .and_then(Value::as_str)
            .unwrap_or("id")
            .to_string(),
        sort_order: object
            .get("sort_order")
            .and_then(Value::as_str)
            .unwrap_or("desc")
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Upstream form values arrive as strings; the coercion reads them.
    #[test]
    fn stored_strings_coerce_to_the_field_types() {
        let mut settings = SiteSettings::default();
        apply(&mut settings, "show_page_pagination", &json!("0"));
        apply(&mut settings, "filter_locale_values", &json!("1"));
        apply(&mut settings, "show_user_bar", &json!("-1"));
        apply(&mut settings, "pagination_per_page", &json!("50"));
        apply(&mut settings, "locale", &json!("es"));
        apply(&mut settings, "search_resource_names", &json!(["items"]));
        apply(
            &mut settings,
            "browse_defaults_public_items",
            &json!({"sort_by": "title", "sort_order": "asc"}),
        );
        assert!(!settings.show_page_pagination());
        assert!(settings.filter_locale_values());
        assert_eq!(settings.show_user_bar(), -1);
        assert_eq!(settings.pagination_per_page(), Some(50));
        assert_eq!(settings.locale(), omeka_site::i18n::Locale::Es);
        assert_eq!(settings.search_resource_names().len(), 1);
        assert_eq!(settings.browse_defaults_items().sort_by, "title");
    }

    /// An empty string reads as unset, and a bool reads as itself.
    #[test]
    fn empty_values_are_unset_and_bools_pass_through() {
        let mut settings = SiteSettings {
            subnav_display: Some("show".to_string()),
            ..SiteSettings::default()
        };
        apply(&mut settings, "subnav_display", &json!(""));
        assert_eq!(settings.subnav_display, None);
        apply(&mut settings, "item_media_embed", &json!(true));
        assert!(settings.item_media_embed());
    }

    /// A key this layer does not read is ignored.
    #[test]
    fn unknown_keys_are_ignored() {
        let mut settings = SiteSettings::default();
        apply(&mut settings, "theme_settings_default", &json!({"logo": 1}));
        apply(&mut settings, "some_module_key", &json!("x"));
        assert_eq!(settings, SiteSettings::default());
    }
}
