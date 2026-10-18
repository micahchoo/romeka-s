//! The site settings the public site layer reads, ported from `site_setting`
//! (`application/src/Entity/SiteSetting.php`) and the defaults each caller
//! passes to `$view->siteSetting($name, $default)`.
//!
//! Upstream stores settings as a `name = value` map. A map would let a typo
//! read as an unset setting, so the port names every key this layer reads as a
//! field. Keys the theme and blocks never touch stay out.
//!
//! [`SETTINGS`] is the same key list as data: each key with the kind of value
//! it holds and the default its accessor carries, which is the settings schema
//! a theme declares (`ThemeInfo::settings`) and a settings editor renders.

use serde::{Deserialize, Serialize};

use crate::i18n::Locale;
use crate::search::{SearchResource, SearchType};

/// Store-provided site settings. Every field is optional; the accessor carries
/// the default the upstream caller passes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteSettings {
    /// `installation_title`, default `Omeka S` (`view/layout/layout.phtml`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation_title: Option<String>,
    /// `locale`, default `en`: the site's rendering locale
    /// (`Mvc\MvcListeners::bootstrapLocale`, `application/src/Mvc/MvcListeners.php:376-381`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// `attachment_link_type`, default `item`; `item`, `media` or `original`
    /// (`BlockLayout/Media.php`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachment_link_type: Option<String>,
    /// `browse_attached_items`, default false (`BlockLayout/BrowsePreview.php`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browse_attached_items: Option<bool>,
    /// `browse_heading_property_term`, default unset (`view/omeka/site/item/browse.phtml`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browse_heading_property_term: Option<String>,
    /// `browse_body_property_term`, default unset (`view/omeka/site/item/browse.phtml`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browse_body_property_term: Option<String>,
    /// `filter_locale_values`, default false (`view/omeka/site/item/browse.phtml`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_locale_values: Option<bool>,
    /// `exclude_resources_not_in_site`, default false
    /// (`view/common/block-layout/item-with-metadata.phtml`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude_resources_not_in_site: Option<bool>,
    /// `iiif_viewer_theme`, default `light`
    /// (`view/common/block-layout/iiif-presentation.phtml`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iiif_viewer_theme: Option<String>,
    /// `subnav_display`, default unset; `show` or `hide`
    /// (`view/omeka/site/page/show.phtml`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subnav_display: Option<String>,
    /// `show_page_pagination`, default true (`view/omeka/site/page/show.phtml`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_page_pagination: Option<bool>,
    /// `search_type`, default `sitewide`
    /// (`view/common/search-form.phtml`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_type: Option<String>,
    /// `search_resource_names`, default `site_pages` and `items`
    /// (`Controller/Site/IndexController.php::searchAction`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_resource_names: Option<Vec<String>>,
}

impl SiteSettings {
    /// `$view->siteSetting('installation_title', 'Omeka S')`.
    #[must_use]
    pub fn installation_title(&self) -> &str {
        self.installation_title.as_deref().unwrap_or("Omeka S")
    }

    /// `$view->siteSetting('locale')` as a shipped locale. An unset or unknown
    /// tag is the port's default, English.
    #[must_use]
    pub fn locale(&self) -> Locale {
        self.locale
            .as_deref()
            .map_or(Locale::default(), Locale::parse)
    }

    /// `$view->siteSetting('attachment_link_type', 'item')`.
    #[must_use]
    pub fn attachment_link_type(&self) -> &str {
        self.attachment_link_type.as_deref().unwrap_or("item")
    }

    /// `$view->siteSetting('browse_attached_items', false)`.
    #[must_use]
    pub fn browse_attached_items(&self) -> bool {
        self.browse_attached_items.unwrap_or(false)
    }

    /// `$view->siteSetting('filter_locale_values')`, default false.
    #[must_use]
    pub fn filter_locale_values(&self) -> bool {
        self.filter_locale_values.unwrap_or(false)
    }

    /// `$view->siteSetting('exclude_resources_not_in_site')`, default false.
    #[must_use]
    pub fn exclude_resources_not_in_site(&self) -> bool {
        self.exclude_resources_not_in_site.unwrap_or(false)
    }

    /// `$view->siteSetting('iiif_viewer_theme', 'light')`.
    #[must_use]
    pub fn iiif_viewer_theme(&self) -> &str {
        self.iiif_viewer_theme.as_deref().unwrap_or("light")
    }

    /// `$view->siteSetting('show_page_pagination', true)`.
    #[must_use]
    pub fn show_page_pagination(&self) -> bool {
        self.show_page_pagination.unwrap_or(true)
    }

    /// `$view->siteSetting('browse_heading_property_term')`, default unset.
    #[must_use]
    pub fn browse_heading_property_term(&self) -> Option<&str> {
        self.browse_heading_property_term.as_deref()
    }

    /// `$view->siteSetting('browse_body_property_term')`, default unset.
    #[must_use]
    pub fn browse_body_property_term(&self) -> Option<&str> {
        self.browse_body_property_term.as_deref()
    }

    /// `$view->siteSetting('search_type', 'sitewide')`.
    #[must_use]
    pub fn search_type(&self) -> SearchType {
        self.search_type
            .as_deref()
            .map_or(SearchType::default(), SearchType::parse)
    }

    /// `$view->siteSetting('search_resource_names', ['site_pages', 'items'])`
    /// as the resource names the table knows. A name the table does not know is
    /// dropped, which keeps a bad setting from failing a render.
    #[must_use]
    pub fn search_resource_names(&self) -> Vec<SearchResource> {
        match &self.search_resource_names {
            Some(names) => names
                .iter()
                .filter_map(|name| SearchResource::from_name(name))
                .collect(),
            None => vec![SearchResource::SitePages, SearchResource::Items],
        }
    }
}

/// The kind of value a site setting holds, so a caller can render the right
/// control (`SiteSettingsForm` upstream).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKind {
    /// A free string.
    Text,
    /// A boolean.
    Boolean,
    /// One of a fixed set of strings.
    Choice(&'static [&'static str]),
    /// Any number of a fixed set of strings, a multi-checkbox.
    MultiChoice(&'static [&'static str]),
}

/// The value a setting has when no `site_setting` row stores it: the default
/// the upstream caller passes to `$view->siteSetting($name, $default)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingDefault {
    /// A text default.
    Text(&'static str),
    /// A boolean default.
    Boolean(bool),
    /// A list default, in schema order.
    MultiChoice(&'static [&'static str]),
    /// No default; the caller supplies a fallback.
    Unset,
}

/// One `site_setting` key the public site layer reads, with the kind of value
/// it holds and the default its accessor carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeSetting {
    /// The `site_setting.name` key.
    pub key: &'static str,
    /// The label a settings editor shows.
    pub label: &'static str,
    /// The kind of value the key holds.
    pub kind: SettingKind,
    /// The value in force when no row stores the key.
    pub default: SettingDefault,
}

/// Every key [`SiteSettings`] reads, in field order. A theme reads these
/// through a [`SiteSettings`]; the key list is the theme's settings schema.
pub static SETTINGS: [ThemeSetting; 13] = [
    ThemeSetting {
        key: "installation_title",
        label: "Installation title",
        kind: SettingKind::Text,
        default: SettingDefault::Text("Omeka S"),
    },
    ThemeSetting {
        key: "locale",
        label: "Locale",
        kind: SettingKind::Choice(&["en", "es"]),
        default: SettingDefault::Text("en"),
    },
    ThemeSetting {
        key: "attachment_link_type",
        label: "Attachment link type",
        kind: SettingKind::Choice(&["item", "media", "original"]),
        default: SettingDefault::Text("item"),
    },
    ThemeSetting {
        key: "browse_attached_items",
        label: "Browse attached items",
        kind: SettingKind::Boolean,
        default: SettingDefault::Boolean(false),
    },
    ThemeSetting {
        key: "browse_heading_property_term",
        label: "Browse heading property",
        kind: SettingKind::Text,
        default: SettingDefault::Unset,
    },
    ThemeSetting {
        key: "browse_body_property_term",
        label: "Browse body property",
        kind: SettingKind::Text,
        default: SettingDefault::Unset,
    },
    ThemeSetting {
        key: "filter_locale_values",
        label: "Filter locale values",
        kind: SettingKind::Boolean,
        default: SettingDefault::Boolean(false),
    },
    ThemeSetting {
        key: "exclude_resources_not_in_site",
        label: "Exclude resources not in site",
        kind: SettingKind::Boolean,
        default: SettingDefault::Boolean(false),
    },
    ThemeSetting {
        key: "iiif_viewer_theme",
        label: "IIIF viewer theme",
        kind: SettingKind::Choice(&["light", "dark"]),
        default: SettingDefault::Text("light"),
    },
    ThemeSetting {
        key: "subnav_display",
        label: "Sub-navigation display",
        kind: SettingKind::Choice(&["show", "hide"]),
        default: SettingDefault::Unset,
    },
    ThemeSetting {
        key: "show_page_pagination",
        label: "Show page pagination",
        kind: SettingKind::Boolean,
        default: SettingDefault::Boolean(true),
    },
    ThemeSetting {
        key: "search_type",
        label: "Search type",
        kind: SettingKind::Choice(&["sitewide", "cross-site"]),
        default: SettingDefault::Text("sitewide"),
    },
    ThemeSetting {
        key: "search_resource_names",
        label: "Search resources",
        kind: SettingKind::MultiChoice(&["site_pages", "items", "item_sets"]),
        default: SettingDefault::MultiChoice(&["site_pages", "items"]),
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Each accessor returns the default its upstream caller passes.
    #[test]
    fn settings_defaults_match_upstream_callers() {
        let settings = SiteSettings::default();
        assert_eq!(settings.installation_title(), "Omeka S");
        assert_eq!(settings.locale(), Locale::En);
        assert_eq!(settings.attachment_link_type(), "item");
        assert_eq!(settings.iiif_viewer_theme(), "light");
        assert!(!settings.browse_attached_items());
        assert!(!settings.filter_locale_values());
        assert!(!settings.exclude_resources_not_in_site());
        assert!(settings.show_page_pagination());
        assert_eq!(settings.browse_heading_property_term(), None);
        assert_eq!(settings.search_type(), SearchType::Sitewide);
        assert_eq!(
            settings.search_resource_names(),
            vec![SearchResource::SitePages, SearchResource::Items]
        );
    }

    /// A stored value overrides its default.
    #[test]
    fn settings_read_stored_values() {
        let settings = SiteSettings {
            installation_title: Some("Library".to_string()),
            attachment_link_type: Some("media".to_string()),
            filter_locale_values: Some(true),
            show_page_pagination: Some(false),
            locale: Some("es_ES".to_string()),
            ..SiteSettings::default()
        };
        assert_eq!(settings.installation_title(), "Library");
        assert_eq!(settings.attachment_link_type(), "media");
        assert!(settings.filter_locale_values());
        assert!(!settings.show_page_pagination());
        assert_eq!(settings.locale(), Locale::Es);
    }

    /// A stored search setting picks the form's target and the sections, and an
    /// unknown resource name is dropped rather than failing the render.
    #[test]
    fn stored_search_settings_override_the_defaults() {
        let settings = SiteSettings {
            search_type: Some("cross-site".to_string()),
            search_resource_names: Some(vec![
                "item_sets".to_string(),
                "sites".to_string(),
                "site_pages".to_string(),
            ]),
            ..SiteSettings::default()
        };
        assert_eq!(settings.search_type(), SearchType::CrossSite);
        assert_eq!(
            settings.search_resource_names(),
            vec![SearchResource::ItemSets, SearchResource::SitePages]
        );

        let empty = SiteSettings {
            search_resource_names: Some(Vec::new()),
            ..SiteSettings::default()
        };
        assert!(empty.search_resource_names().is_empty());
    }

    /// What each accessor returns, as text, so a schema default can be checked
    /// against it.
    fn read(settings: &SiteSettings, key: &str) -> Option<String> {
        match key {
            "installation_title" => Some(settings.installation_title().to_string()),
            "locale" => Some(settings.locale().tag().to_string()),
            "attachment_link_type" => Some(settings.attachment_link_type().to_string()),
            "browse_attached_items" => Some(settings.browse_attached_items().to_string()),
            "browse_heading_property_term" => {
                settings.browse_heading_property_term().map(str::to_string)
            }
            "browse_body_property_term" => settings.browse_body_property_term().map(str::to_string),
            "filter_locale_values" => Some(settings.filter_locale_values().to_string()),
            "exclude_resources_not_in_site" => {
                Some(settings.exclude_resources_not_in_site().to_string())
            }
            "iiif_viewer_theme" => Some(settings.iiif_viewer_theme().to_string()),
            "subnav_display" => settings.subnav_display.clone(),
            "show_page_pagination" => Some(settings.show_page_pagination().to_string()),
            "search_type" => Some(match settings.search_type() {
                SearchType::Sitewide => "sitewide".to_string(),
                SearchType::CrossSite => "cross-site".to_string(),
            }),
            "search_resource_names" => Some(
                settings
                    .search_resource_names()
                    .iter()
                    .map(|resource| resource.name())
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            other => panic!("{other} has no accessor"),
        }
    }

    /// A schema default as text, `None` for an unset default.
    fn listed(default: SettingDefault) -> Option<String> {
        match default {
            SettingDefault::Text(text) => Some(text.to_string()),
            SettingDefault::Boolean(value) => Some(value.to_string()),
            SettingDefault::MultiChoice(values) => Some(values.join(",")),
            SettingDefault::Unset => None,
        }
    }

    /// The schema lists every key the layer reads, and each key's default is
    /// what its accessor returns for a settings map with no stored value.
    #[test]
    fn settings_schema_matches_the_accessors() {
        let defaults = SiteSettings::default();
        assert_eq!(SETTINGS.len(), 13);
        for entry in &SETTINGS {
            assert!(!entry.label.is_empty(), "{}", entry.key);
            assert_eq!(
                listed(entry.default),
                read(&defaults, entry.key),
                "{}",
                entry.key
            );
        }
        let keys: std::collections::BTreeSet<&str> =
            SETTINGS.iter().map(|entry| entry.key).collect();
        assert_eq!(keys.len(), SETTINGS.len(), "a key appears twice");
    }

    /// A stored value reaches the accessor, so the schema's default is only the
    /// default.
    #[test]
    fn settings_schema_defaults_yield_to_stored_values() {
        let stored = SiteSettings {
            iiif_viewer_theme: Some("dark".to_string()),
            browse_attached_items: Some(true),
            subnav_display: Some("hide".to_string()),
            ..SiteSettings::default()
        };
        assert_eq!(read(&stored, "iiif_viewer_theme"), Some("dark".to_string()));
        assert_eq!(
            read(&stored, "browse_attached_items"),
            Some("true".to_string())
        );
        assert_eq!(read(&stored, "subnav_display"), Some("hide".to_string()));
        assert_ne!(
            listed(SettingDefault::Text("light")),
            read(&stored, "iiif_viewer_theme")
        );
    }
}
