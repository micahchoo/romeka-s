//! The site settings the public site layer reads, ported from `site_setting`
//! (`application/src/Entity/SiteSetting.php`) and the defaults each caller
//! passes to `$view->siteSetting($name, $default)`.
//!
//! Upstream stores every site setting as its own `site_setting` row, keyed by
//! `(id, site_id)` with the value JSON-encoded (`AbstractTargetSettings::set`).
//! The keys are the fields `application/src/Form/SiteSettingsForm.php` declares,
//! saved one by one by `Controller/SiteAdmin/IndexController.php:127-130`. A
//! theme's own settings are a separate row, under `theme_settings_<theme>`.
//!
//! A map would let a typo read as an unset setting, so the port names every key
//! this layer reads as a field. [`SITE_SETTINGS`] is the same key list as data:
//! each key with the kind of value it holds and the default its accessor
//! carries, which is the schema a settings editor renders and the admin file
//! `web/admin/src/generated/siteSettings.json` is generated from.

use serde::{Deserialize, Serialize};

use crate::i18n::Locale;
use crate::search::{SearchResource, SearchType};

/// Store-provided site settings. Every field is optional; the accessor carries
/// the default the upstream caller passes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteSettings {
    /// `installation_title`, default `Romeka S`; upstream defaults to
    /// `Omeka S` (`view/layout/layout.phtml`).
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
    /// `show_user_bar`, default `0` (`View/Helper/UserBar.php:35`): `-1` never,
    /// `0` when signed in, `1` always.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_user_bar: Option<i64>,
    /// `disable_jsonld_embed`, default false (`Module.php:243-256`): when true a
    /// site item, item-set or media view prints no `application/ld+json`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_jsonld_embed: Option<bool>,
    /// `favicon`, default unset (`Module.php:188-203`): the `asset` id the
    /// layout links as `rel="icon"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub favicon: Option<String>,
    /// `show_locale_label`, default true (`view/omeka/site/item/show.phtml`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_locale_label: Option<bool>,
    /// `pagination_per_page`, default unset (the global setting, else 25)
    /// (`Service/Delegator/SitePaginatorDelegatorFactory.php:14`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pagination_per_page: Option<u32>,
    /// `browse_defaults_public_items`, default `id` desc
    /// (`Stdlib/Browse.php:148-170`, `config/module.config.php:685-690`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browse_defaults_public_items: Option<BrowseDefaults>,
    /// `show_attached_pages`, default true
    /// (`view/common/block-layout/item-with-metadata.phtml`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_attached_pages: Option<bool>,
    /// `property_label_information`, default `none`
    /// (`view/omeka/site/item/show.phtml`): `none`, `vocab` or `term`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub property_label_information: Option<String>,
    /// `show_value_annotations`, default unset
    /// (`view/omeka/site/item/show.phtml`): `collapsed` or `expanded`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_value_annotations: Option<String>,
    /// `item_media_embed`, default false (`view/omeka/site/item/show.phtml`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_media_embed: Option<bool>,
    /// `vocabulary_scope`, default unset (`SiteSettingsForm.php:382-399`):
    /// `sitewide` or `cross-site`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vocabulary_scope: Option<String>,
    /// `search_apply_templates`, default empty (`SiteSettingsForm.php:401-415`):
    /// the resource template ids a site search is narrowed to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_apply_templates: Option<Vec<u32>>,
    /// `search_restrict_templates`, default false (`SiteSettingsForm.php:416-427`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_restrict_templates: Option<bool>,
    /// `iiif_viewer_sidebar`, default false (`SiteSettingsForm.php:430-440`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iiif_viewer_sidebar: Option<bool>,
    /// `romeka_location_property`, a Romeka-only key, default unset: the
    /// property whose `lat, lng` values place an item on a map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub romeka_location_property: Option<String>,
    /// `romeka_map_tiles`, a Romeka-only key: the tile URL template a map
    /// draws its basemap from, default OpenStreetMap's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub romeka_map_tiles: Option<String>,
    /// `romeka_map_attribution`, a Romeka-only key: the credit line the tile
    /// provider requires, default OpenStreetMap's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub romeka_map_attribution: Option<String>,
}

/// The tile URL template a map uses when the site names none: OpenStreetMap's
/// own tile server, whose usage policy asks for the attribution line and
/// limits heavy use.
pub const DEFAULT_MAP_TILES: &str = "https://tile.openstreetmap.org/{z}/{x}/{y}.png";

/// The attribution OpenStreetMap's tiles require.
pub const DEFAULT_MAP_ATTRIBUTION: &str = "\u{a9} OpenStreetMap contributors";

/// The `browse_defaults_public_items` value: the sort a public browse starts
/// with (`Stdlib/Browse.php:167-170`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowseDefaults {
    /// `sort_by`, default `id`.
    #[serde(default = "default_sort_by")]
    pub sort_by: String,
    /// `sort_order`, default `desc`.
    #[serde(default = "default_sort_order")]
    pub sort_order: String,
}

fn default_sort_by() -> String {
    "id".to_string()
}

fn default_sort_order() -> String {
    "desc".to_string()
}

impl Default for BrowseDefaults {
    fn default() -> Self {
        Self {
            sort_by: default_sort_by(),
            sort_order: default_sort_order(),
        }
    }
}

impl SiteSettings {
    /// `$view->siteSetting('installation_title', 'Romeka S')`.
    #[must_use]
    pub fn installation_title(&self) -> &str {
        self.installation_title.as_deref().unwrap_or("Romeka S")
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

    /// `$view->siteSetting('show_user_bar', 0)`, the raw `-1`/`0`/`1`.
    #[must_use]
    pub fn show_user_bar(&self) -> i64 {
        self.show_user_bar.unwrap_or(0)
    }

    /// `View/Helper/UserBar.php:35-43`: `-1` never, `1` always, `0` only when
    /// the caller is signed in.
    #[must_use]
    pub fn shows_user_bar(&self, signed_in: bool) -> bool {
        match self.show_user_bar() {
            -1 => false,
            1 => true,
            _ => signed_in,
        }
    }

    /// `$view->siteSetting('disable_jsonld_embed')`, default false.
    #[must_use]
    pub fn disable_jsonld_embed(&self) -> bool {
        self.disable_jsonld_embed.unwrap_or(false)
    }

    /// `$view->siteSetting('favicon')`, default unset.
    #[must_use]
    pub fn favicon(&self) -> Option<&str> {
        self.favicon.as_deref()
    }

    /// `$view->siteSetting('show_locale_label')`, default true.
    #[must_use]
    pub fn show_locale_label(&self) -> bool {
        self.show_locale_label.unwrap_or(true)
    }

    /// `$view->siteSetting('pagination_per_page')`, default unset.
    #[must_use]
    pub fn pagination_per_page(&self) -> Option<u32> {
        self.pagination_per_page
    }

    /// `$view->siteSetting('browse_defaults_public_items')`, default `id` desc
    /// (`config/module.config.php:685-690`).
    #[must_use]
    pub fn browse_defaults_items(&self) -> BrowseDefaults {
        self.browse_defaults_public_items
            .clone()
            .unwrap_or_default()
    }

    /// `$view->siteSetting('show_attached_pages')`, default true.
    #[must_use]
    pub fn show_attached_pages(&self) -> bool {
        self.show_attached_pages.unwrap_or(true)
    }

    /// `$view->siteSetting('property_label_information', 'none')`.
    #[must_use]
    pub fn property_label_information(&self) -> &str {
        self.property_label_information.as_deref().unwrap_or("none")
    }

    /// `$view->siteSetting('show_value_annotations')`, default unset (hidden).
    #[must_use]
    pub fn show_value_annotations(&self) -> Option<&str> {
        self.show_value_annotations.as_deref()
    }

    /// `$view->siteSetting('item_media_embed')`, default false.
    #[must_use]
    pub fn item_media_embed(&self) -> bool {
        self.item_media_embed.unwrap_or(false)
    }

    /// `$view->siteSetting('vocabulary_scope')`, default unset.
    #[must_use]
    pub fn vocabulary_scope(&self) -> Option<&str> {
        self.vocabulary_scope.as_deref()
    }

    /// `$view->siteSetting('search_apply_templates')`, default empty.
    #[must_use]
    pub fn search_apply_templates(&self) -> &[u32] {
        self.search_apply_templates.as_deref().unwrap_or(&[])
    }

    /// `$view->siteSetting('search_restrict_templates')`, default false.
    #[must_use]
    pub fn search_restrict_templates(&self) -> bool {
        self.search_restrict_templates.unwrap_or(false)
    }

    /// `$view->siteSetting('iiif_viewer_sidebar')`, default false.
    #[must_use]
    pub fn iiif_viewer_sidebar(&self) -> bool {
        self.iiif_viewer_sidebar.unwrap_or(false)
    }

    /// The property term whose values hold an item's places, `None` when the
    /// site names none and no map has places.
    #[must_use]
    pub fn location_property(&self) -> Option<&str> {
        self.romeka_location_property.as_deref()
    }

    /// The basemap's tile URL template. A stored template that is not an
    /// `http` or `https` URL with `{z}`, `{x}` and `{y}` cannot draw a map, so
    /// the default stands in for it.
    #[must_use]
    pub fn map_tiles(&self) -> &str {
        self.romeka_map_tiles
            .as_deref()
            .filter(|template| is_tile_template(template))
            .unwrap_or(DEFAULT_MAP_TILES)
    }

    /// The basemap's attribution line, as plain text.
    #[must_use]
    pub fn map_attribution(&self) -> &str {
        self.romeka_map_attribution
            .as_deref()
            .unwrap_or(DEFAULT_MAP_ATTRIBUTION)
    }
}

/// Whether a tile URL template can draw a map: a web URL naming the zoom and
/// both tile coordinates.
fn is_tile_template(template: &str) -> bool {
    (template.starts_with("https://") || template.starts_with("http://"))
        && ["{z}", "{x}", "{y}"]
            .iter()
            .all(|part| template.contains(part))
}

/// The kind of value a site setting holds, so a caller can render the right
/// control (`SiteSettingsForm` upstream).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKind {
    /// A free string.
    Text,
    /// A property term such as `dcterms:title`, picked from the vocabularies.
    Property,
    /// An asset id, picked from the assets or uploaded.
    Asset,
    /// A boolean.
    Boolean,
    /// A whole number.
    Number,
    /// One of a fixed set of strings.
    Choice(&'static [&'static str]),
    /// Any number of a fixed set of strings, a multi-checkbox.
    MultiChoice(&'static [&'static str]),
    /// A `{sort_by, sort_order}` pair (`Stdlib/Browse.php`).
    Sort,
}

/// The value a setting has when no `site_setting` row stores it: the default
/// the upstream caller passes to `$view->siteSetting($name, $default)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingDefault {
    /// A text default.
    Text(&'static str),
    /// A boolean default.
    Boolean(bool),
    /// A number default.
    Number(u32),
    /// A list default, in schema order.
    MultiChoice(&'static [&'static str]),
    /// A sort default.
    Sort(&'static str, &'static str),
    /// No default; the caller supplies a fallback.
    Unset,
}

/// One `site_setting` key the public site layer reads, with the kind of value
/// it holds and the default its accessor carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiteSettingSpec {
    /// The `site_setting` key.
    pub key: &'static str,
    /// The label a settings editor shows.
    pub label: &'static str,
    /// The kind of value the key holds.
    pub kind: SettingKind,
    /// The value in force when no row stores the key.
    pub default: SettingDefault,
    /// The form section the key sits in. `None` when no renderer reads the
    /// key yet: the form leaves it out, and a save keeps its stored row.
    pub group: Option<SettingGroup>,
}

/// A section of the site settings form, named for what a visitor sees change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingGroup {
    /// The browse lists.
    Browsing,
    /// The search box and its results.
    Search,
    /// A single item or media page.
    ItemPages,
    /// The site language and values in other languages.
    Language,
    /// The menus, bars and browser tab around every page.
    Navigation,
    /// The maps: where an item's place is stored and what the map draws.
    Maps,
    /// Settings only an Omeka expert changes.
    Advanced,
}

/// Every key [`SiteSettings`] reads, in `SiteSettingsForm` order. The admin's
/// site settings form renders these, and the site loader reads the stored rows
/// into a [`SiteSettings`].
pub static SITE_SETTINGS: [SiteSettingSpec; 30] = [
    SiteSettingSpec {
        key: "installation_title",
        label: "Name in the browser tab",
        kind: SettingKind::Text,
        default: SettingDefault::Text("Romeka S"),
        group: Some(SettingGroup::Navigation),
    },
    SiteSettingSpec {
        key: "locale",
        label: "Site language",
        kind: SettingKind::Choice(&Locale::TAGS),
        default: SettingDefault::Text("en"),
        group: Some(SettingGroup::Language),
    },
    SiteSettingSpec {
        key: "attachment_link_type",
        label: "Where image links go",
        kind: SettingKind::Choice(&["item", "media", "original"]),
        default: SettingDefault::Text("item"),
        group: Some(SettingGroup::ItemPages),
    },
    SiteSettingSpec {
        key: "browse_attached_items",
        label: "Only items placed on pages",
        kind: SettingKind::Boolean,
        default: SettingDefault::Boolean(false),
        group: Some(SettingGroup::Browsing),
    },
    SiteSettingSpec {
        key: "browse_heading_property_term",
        label: "Title shown in lists",
        kind: SettingKind::Property,
        default: SettingDefault::Unset,
        group: Some(SettingGroup::Browsing),
    },
    SiteSettingSpec {
        key: "browse_body_property_term",
        label: "Text shown under each title",
        kind: SettingKind::Property,
        default: SettingDefault::Unset,
        group: Some(SettingGroup::Browsing),
    },
    SiteSettingSpec {
        key: "filter_locale_values",
        label: "Only show values in the site language",
        kind: SettingKind::Boolean,
        default: SettingDefault::Boolean(false),
        group: Some(SettingGroup::Language),
    },
    SiteSettingSpec {
        key: "exclude_resources_not_in_site",
        label: "Hide links to items outside this site",
        kind: SettingKind::Boolean,
        default: SettingDefault::Boolean(false),
        group: Some(SettingGroup::Advanced),
    },
    SiteSettingSpec {
        key: "iiif_viewer_theme",
        label: "IIIF viewer colours",
        kind: SettingKind::Choice(&["light", "dark"]),
        default: SettingDefault::Text("light"),
        group: Some(SettingGroup::ItemPages),
    },
    SiteSettingSpec {
        key: "iiif_viewer_sidebar",
        label: "IIIF viewer sidebar",
        kind: SettingKind::Boolean,
        default: SettingDefault::Boolean(false),
        group: Some(SettingGroup::ItemPages),
    },
    SiteSettingSpec {
        key: "subnav_display",
        label: "Sub-menu on pages",
        kind: SettingKind::Choice(&["show", "hide"]),
        default: SettingDefault::Unset,
        group: Some(SettingGroup::Navigation),
    },
    SiteSettingSpec {
        key: "show_page_pagination",
        label: "Previous and next page links",
        kind: SettingKind::Boolean,
        default: SettingDefault::Boolean(true),
        group: Some(SettingGroup::Navigation),
    },
    SiteSettingSpec {
        key: "search_type",
        label: "Search scope",
        kind: SettingKind::Choice(&["sitewide", "cross-site"]),
        default: SettingDefault::Text("sitewide"),
        group: Some(SettingGroup::Search),
    },
    SiteSettingSpec {
        key: "search_resource_names",
        label: "What search finds",
        kind: SettingKind::MultiChoice(&["site_pages", "items", "item_sets"]),
        default: SettingDefault::MultiChoice(&["site_pages", "items"]),
        group: Some(SettingGroup::Search),
    },
    SiteSettingSpec {
        key: "show_user_bar",
        label: "Admin bar on the public site",
        kind: SettingKind::Choice(&["-1", "0", "1"]),
        default: SettingDefault::Text("0"),
        group: Some(SettingGroup::Navigation),
    },
    SiteSettingSpec {
        key: "disable_jsonld_embed",
        label: "Leave out linked data",
        kind: SettingKind::Boolean,
        default: SettingDefault::Boolean(false),
        group: Some(SettingGroup::Advanced),
    },
    SiteSettingSpec {
        key: "favicon",
        label: "Browser tab icon",
        kind: SettingKind::Asset,
        default: SettingDefault::Unset,
        group: Some(SettingGroup::Navigation),
    },
    SiteSettingSpec {
        key: "show_locale_label",
        label: "Show language tags",
        kind: SettingKind::Boolean,
        default: SettingDefault::Boolean(true),
        group: Some(SettingGroup::Language),
    },
    SiteSettingSpec {
        key: "pagination_per_page",
        label: "Items per page",
        kind: SettingKind::Number,
        default: SettingDefault::Unset,
        group: Some(SettingGroup::Browsing),
    },
    SiteSettingSpec {
        key: "browse_defaults_public_items",
        label: "Item list order",
        kind: SettingKind::Sort,
        default: SettingDefault::Sort("id", "desc"),
        group: Some(SettingGroup::Browsing),
    },
    SiteSettingSpec {
        key: "show_attached_pages",
        label: "List pages that feature the item",
        kind: SettingKind::Boolean,
        default: SettingDefault::Boolean(true),
        group: Some(SettingGroup::ItemPages),
    },
    SiteSettingSpec {
        key: "property_label_information",
        label: "Extra detail on field names",
        kind: SettingKind::Choice(&["none", "vocab", "term"]),
        default: SettingDefault::Text("none"),
        group: Some(SettingGroup::ItemPages),
    },
    SiteSettingSpec {
        key: "show_value_annotations",
        label: "Show value annotations",
        kind: SettingKind::Choice(&["collapsed", "expanded"]),
        default: SettingDefault::Unset,
        group: None,
    },
    SiteSettingSpec {
        key: "item_media_embed",
        label: "Item media embed",
        kind: SettingKind::Boolean,
        default: SettingDefault::Boolean(false),
        group: None,
    },
    SiteSettingSpec {
        key: "vocabulary_scope",
        label: "Vocabulary scope",
        kind: SettingKind::Choice(&["sitewide", "cross-site"]),
        default: SettingDefault::Unset,
        group: None,
    },
    SiteSettingSpec {
        key: "search_apply_templates",
        label: "Search apply templates",
        kind: SettingKind::Text,
        default: SettingDefault::Unset,
        group: None,
    },
    SiteSettingSpec {
        key: "search_restrict_templates",
        label: "Search restrict templates",
        kind: SettingKind::Boolean,
        default: SettingDefault::Boolean(false),
        group: None,
    },
    SiteSettingSpec {
        key: "romeka_location_property",
        label: "Location field",
        kind: SettingKind::Property,
        default: SettingDefault::Unset,
        group: Some(SettingGroup::Maps),
    },
    SiteSettingSpec {
        key: "romeka_map_tiles",
        label: "Map tiles",
        kind: SettingKind::Text,
        default: SettingDefault::Text(DEFAULT_MAP_TILES),
        group: Some(SettingGroup::Maps),
    },
    SiteSettingSpec {
        key: "romeka_map_attribution",
        label: "Map credit",
        kind: SettingKind::Text,
        default: SettingDefault::Text(DEFAULT_MAP_ATTRIBUTION),
        group: Some(SettingGroup::Maps),
    },
];

/// The site settings schema as the admin reads it: each key the form offers,
/// with the catalogue keys for its label and help, its group, the control
/// kind, and the default as JSON. A key with no group is left out.
#[must_use]
pub fn site_settings_registry() -> SiteSettingsRegistry {
    SiteSettingsRegistry {
        settings: SITE_SETTINGS
            .iter()
            .filter_map(|entry| {
                let group = entry.group?;
                let name = camel(entry.key);
                Some(SiteSettingEntry {
                    key: entry.key,
                    label: entry.label,
                    label_key: format!("siteSetting.{name}"),
                    help_key: format!("siteSetting.{name}Help"),
                    group,
                    schema: schema(entry),
                })
            })
            .collect(),
    }
}

/// The control schema a spec describes.
fn schema(entry: &SiteSettingSpec) -> SettingSchema {
    match (entry.kind, entry.default) {
        (SettingKind::Text, default) => SettingSchema::Text {
            default: match default {
                SettingDefault::Text(text) => Some(text),
                _ => None,
            },
        },
        (SettingKind::Property, _) => SettingSchema::Property,
        (SettingKind::Asset, _) => SettingSchema::Asset,
        (SettingKind::Boolean, SettingDefault::Boolean(default)) => {
            SettingSchema::Boolean { default }
        }
        (SettingKind::Boolean, _) => SettingSchema::Boolean { default: false },
        (SettingKind::Number, SettingDefault::Number(default)) => SettingSchema::Number {
            default: Some(default),
        },
        (SettingKind::Number, _) => SettingSchema::Number { default: None },
        (SettingKind::Choice(choices), default) => SettingSchema::Choice {
            choices,
            default: match default {
                SettingDefault::Text(text) => Some(text),
                _ => None,
            },
        },
        (SettingKind::MultiChoice(choices), SettingDefault::MultiChoice(default)) => {
            SettingSchema::MultiChoice { choices, default }
        }
        (SettingKind::MultiChoice(choices), _) => SettingSchema::MultiChoice {
            choices,
            default: &[],
        },
        (SettingKind::Sort, SettingDefault::Sort(sort_by, sort_order)) => SettingSchema::Sort {
            sort_by,
            sort_order,
        },
        (SettingKind::Sort, _) => SettingSchema::Sort {
            sort_by: "id",
            sort_order: "desc",
        },
    }
}

/// `/^[a-z]+(_[a-z0-9]+)*$/` to camelCase, for the catalogue key.
fn camel(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    let mut upper = false;
    for ch in key.chars() {
        if ch == '_' {
            upper = true;
            continue;
        }
        if upper {
            out.extend(ch.to_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// The whole site settings registry, as `siteSettings.json` holds it.
#[derive(Debug, Serialize)]
pub struct SiteSettingsRegistry {
    /// Every key, in schema order.
    pub settings: Vec<SiteSettingEntry>,
}

/// One setting as the admin reads it.
#[derive(Debug, Serialize)]
pub struct SiteSettingEntry {
    /// The `site_setting` key.
    pub key: &'static str,
    /// The English label, the source text the catalogue key resolves to. Not
    /// serialised: the admin reads the label through `labelKey`.
    #[serde(skip)]
    pub label: &'static str,
    /// The catalogue key for the label.
    #[serde(rename = "labelKey")]
    pub label_key: String,
    /// The catalogue key for the one-sentence help under the control.
    #[serde(rename = "helpKey")]
    pub help_key: String,
    /// The form section the control sits in.
    pub group: SettingGroup,
    /// The control kind and its default, flattened beside the key.
    #[serde(flatten)]
    pub schema: SettingSchema,
}

/// The control a setting renders: a `kind` tag plus that kind's fields.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SettingSchema {
    /// A text control.
    Text {
        /// The default text, or `null`.
        default: Option<&'static str>,
    },
    /// A property picker; the stored value is the term, and there is no
    /// default.
    Property,
    /// An asset picker; the stored value is the asset id, and there is no
    /// default.
    Asset,
    /// A boolean control.
    Boolean {
        /// The default boolean.
        default: bool,
    },
    /// A number control.
    Number {
        /// The default number, or `null`.
        default: Option<u32>,
    },
    /// A single-select control.
    Choice {
        /// The offered values, in schema order.
        choices: &'static [&'static str],
        /// The default value, or `null`.
        default: Option<&'static str>,
    },
    /// A multi-checkbox control.
    MultiChoice {
        /// The offered values, in schema order.
        choices: &'static [&'static str],
        /// The default values, in schema order.
        default: &'static [&'static str],
    },
    /// A two-select sort control.
    Sort {
        /// The default `sort_by`.
        #[serde(rename = "sortBy")]
        sort_by: &'static str,
        /// The default `sort_order`.
        #[serde(rename = "sortOrder")]
        sort_order: &'static str,
    },
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// Where the admin reads the generated settings schema.
    const ADMIN_FILE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/admin/src/generated/siteSettings.json"
    );

    /// The admin's file is the registry's serialisation, byte for byte. A
    /// difference fails with the command that rewrites it.
    #[test]
    fn generated_registry_matches_the_admin_file() {
        let rendered = format!(
            "{}\n",
            serde_json::to_string_pretty(&site_settings_registry())
                .expect("the registry serialises")
        );
        if std::env::var_os("UPDATE_GENERATED").is_some() {
            let path = std::path::Path::new(ADMIN_FILE);
            std::fs::create_dir_all(path.parent().expect("the file has a parent"))
                .expect("the generated directory exists");
            std::fs::write(path, &rendered).expect("the registry file is writable");
            return;
        }
        let current = std::fs::read_to_string(ADMIN_FILE).unwrap_or_default();
        assert_eq!(
            current, rendered,
            "run UPDATE_GENERATED=1 cargo test -p omeka-site to rewrite {ADMIN_FILE}"
        );
    }

    /// Each accessor returns the default its upstream caller passes.
    #[test]
    fn settings_defaults_match_upstream_callers() {
        let settings = SiteSettings::default();
        assert_eq!(settings.installation_title(), "Romeka S");
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
        assert_eq!(settings.show_user_bar(), 0);
        assert!(!settings.disable_jsonld_embed());
        assert_eq!(settings.favicon(), None);
        assert!(settings.show_locale_label());
        assert_eq!(settings.pagination_per_page(), None);
        assert_eq!(
            settings.browse_defaults_items(),
            BrowseDefaults {
                sort_by: "id".to_string(),
                sort_order: "desc".to_string()
            }
        );
        assert!(settings.show_attached_pages());
        assert_eq!(settings.property_label_information(), "none");
        assert_eq!(settings.show_value_annotations(), None);
        assert!(!settings.item_media_embed());
        assert_eq!(settings.vocabulary_scope(), None);
        assert!(settings.search_apply_templates().is_empty());
        assert!(!settings.search_restrict_templates());
        assert!(!settings.iiif_viewer_sidebar());
        assert_eq!(settings.location_property(), None);
        assert_eq!(
            settings.map_tiles(),
            "https://tile.openstreetmap.org/{z}/{x}/{y}.png"
        );
        assert_eq!(
            settings.map_attribution(),
            "\u{a9} OpenStreetMap contributors"
        );
    }

    /// A stored tile template draws the map only when it is a web URL with the
    /// zoom and both coordinates; anything else keeps OpenStreetMap's.
    #[test]
    fn map_tiles_keep_the_default_for_a_template_that_cannot_draw() {
        let stored = |template: &str| SiteSettings {
            romeka_map_tiles: Some(template.to_string()),
            ..SiteSettings::default()
        };
        let custom = "https://tiles.example.org/{z}/{x}/{y}.png";
        assert_eq!(stored(custom).map_tiles(), custom);
        for broken in [
            "javascript:alert(1)//{z}{x}{y}",
            "https://tiles.example.org/{z}/{x}.png",
            "/tiles/{z}/{x}/{y}.png",
        ] {
            assert_eq!(stored(broken).map_tiles(), DEFAULT_MAP_TILES, "{broken}");
        }
    }

    /// The user bar shows for a signed-in caller under the default, never under
    /// `-1`, and always under `1` (`UserBar.php:35-43`).
    #[test]
    fn user_bar_follows_show_user_bar() {
        let mut settings = SiteSettings::default();
        assert!(settings.shows_user_bar(true));
        assert!(!settings.shows_user_bar(false));
        settings.show_user_bar = Some(-1);
        assert!(!settings.shows_user_bar(true));
        assert!(!settings.shows_user_bar(false));
        settings.show_user_bar = Some(1);
        assert!(settings.shows_user_bar(false));
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
            "show_user_bar" => Some(settings.show_user_bar().to_string()),
            "disable_jsonld_embed" => Some(settings.disable_jsonld_embed().to_string()),
            "favicon" => settings.favicon().map(str::to_string),
            "show_locale_label" => Some(settings.show_locale_label().to_string()),
            "pagination_per_page" => settings
                .pagination_per_page()
                .map(|value| value.to_string()),
            "browse_defaults_public_items" => {
                let defaults = settings.browse_defaults_items();
                Some(format!("{},{}", defaults.sort_by, defaults.sort_order))
            }
            "show_attached_pages" => Some(settings.show_attached_pages().to_string()),
            "property_label_information" => Some(settings.property_label_information().to_string()),
            "show_value_annotations" => settings.show_value_annotations().map(str::to_string),
            "item_media_embed" => Some(settings.item_media_embed().to_string()),
            "vocabulary_scope" => settings.vocabulary_scope().map(str::to_string),
            "search_apply_templates" => {
                let values = settings.search_apply_templates();
                (!values.is_empty()).then(|| {
                    values
                        .iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                })
            }
            "search_restrict_templates" => Some(settings.search_restrict_templates().to_string()),
            "iiif_viewer_sidebar" => Some(settings.iiif_viewer_sidebar().to_string()),
            "romeka_location_property" => settings.location_property().map(str::to_string),
            "romeka_map_tiles" => Some(settings.map_tiles().to_string()),
            "romeka_map_attribution" => Some(settings.map_attribution().to_string()),
            other => panic!("{other} has no accessor"),
        }
    }

    /// A schema default as text, `None` for an unset default.
    fn listed(default: SettingDefault) -> Option<String> {
        match default {
            SettingDefault::Text(text) => Some(text.to_string()),
            SettingDefault::Boolean(value) => Some(value.to_string()),
            SettingDefault::Number(value) => Some(value.to_string()),
            SettingDefault::MultiChoice(values) => Some(values.join(",")),
            SettingDefault::Sort(sort_by, sort_order) => Some(format!("{sort_by},{sort_order}")),
            SettingDefault::Unset => None,
        }
    }

    /// The schema lists every key the layer reads, and each key's default is
    /// what its accessor returns for a settings map with no stored value.
    #[test]
    fn settings_schema_matches_the_accessors() {
        let defaults = SiteSettings::default();
        assert_eq!(SITE_SETTINGS.len(), 30);
        for entry in &SITE_SETTINGS {
            assert!(!entry.label.is_empty(), "{}", entry.key);
            assert_eq!(
                listed(entry.default),
                read(&defaults, entry.key),
                "{}",
                entry.key
            );
        }
        let keys: std::collections::BTreeSet<&str> =
            SITE_SETTINGS.iter().map(|entry| entry.key).collect();
        assert_eq!(keys.len(), SITE_SETTINGS.len(), "a key appears twice");
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

    /// Every key's catalogue label key is `siteSetting.<camelCase>`.
    #[test]
    fn registry_label_keys_are_camel_case() {
        let registry = site_settings_registry();
        let grouped = SITE_SETTINGS.iter().filter(|entry| entry.group.is_some());
        assert_eq!(registry.settings.len(), grouped.count());
        assert_eq!(
            registry.settings[0].label_key,
            "siteSetting.installationTitle".to_string()
        );
        assert_eq!(
            registry
                .settings
                .iter()
                .find(|entry| entry.key == "browse_defaults_public_items")
                .map(|entry| entry.label_key.clone()),
            Some("siteSetting.browseDefaultsPublicItems".to_string())
        );
    }
}
