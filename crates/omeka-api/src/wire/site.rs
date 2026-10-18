//! `SiteRepresentation` wire shape.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::common::{ApiUrl, JsonLdDate, JsonLdType, NavLink, ResourceRef};

/// `SiteAdapter::getResourceName`.
pub const RESOURCE_NAME: &str = "sites";

/// One site document. Sites are not resource entities, so the common
/// `o:is_public` block of `AbstractResourceEntityRepresentation` does not
/// apply; this class owns its own `o:created` / `o:modified` fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Site {
    #[serde(rename = "@context")]
    pub context: String,
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@type")]
    pub jsonld_type: JsonLdType,
    #[serde(rename = "o:id")]
    pub o_id: u64,
    #[serde(rename = "o:slug")]
    pub slug: String,
    #[serde(rename = "o:theme")]
    pub theme: String,
    #[serde(rename = "o:title")]
    pub title: String,
    #[serde(rename = "o:summary")]
    pub summary: Option<String>,
    #[serde(rename = "o:thumbnail")]
    pub thumbnail: Option<ResourceRef>,
    #[serde(rename = "o:navigation")]
    pub navigation: Vec<NavLink>,
    #[serde(rename = "o:homepage")]
    pub homepage: Option<ResourceRef>,
    #[serde(rename = "o:item_pool")]
    pub item_pool: serde_json::Value,
    #[serde(rename = "o:owner")]
    pub owner: Option<ResourceRef>,
    #[serde(rename = "o:created")]
    pub created: JsonLdDate,
    #[serde(rename = "o:modified")]
    pub modified: Option<JsonLdDate>,
    #[serde(rename = "o:is_public")]
    pub is_public: bool,
    #[serde(rename = "o:assign_new_items")]
    pub assign_new_items: bool,
    #[serde(rename = "o:page")]
    pub page: Vec<ResourceRef>,
    #[serde(rename = "o:site_permission")]
    pub site_permission: Vec<SitePermission>,
    #[serde(rename = "o:site_item_set")]
    pub site_item_set: Vec<SiteItemSet>,
    /// The theme's own settings map, a port addition.
    ///
    /// Upstream stores the map in `site_setting` under `theme_settings_<theme>`
    /// and edits it through a server-rendered form, so no API member carries
    /// it. The SPA needs the map to render the theme settings form, so the site
    /// document names it. Absent when the site stores no settings, which keeps
    /// the document byte-identical to upstream's for a site that has none.
    #[serde(
        rename = "o:theme_settings",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub theme_settings: Option<serde_json::Value>,
    /// The site's site-settings map, a port addition.
    ///
    /// Upstream stores each key as its own `site_setting` row and edits them
    /// through a server-rendered form, so no API member carries them. The SPA
    /// needs the map to render the site settings form, so the site document
    /// names it. Absent when the site stores none, which keeps the document
    /// byte-identical to upstream's for a site with no settings.
    #[serde(
        rename = "o:site_settings",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub site_settings: Option<serde_json::Value>,
    #[serde(rename = "o:site_item")]
    pub site_item: ApiUrl,
    /// Seam for `o-module-*` keys. Upstream modules attach them through the
    /// `rep.resource.json` event; no module ships at the frozen SHA, so this
    /// map serializes empty. A later module unit fills it.
    #[serde(flatten)]
    pub extensions: IndexMap<String, serde_json::Value>,
}

/// One entry of `o:site_permission`, from
/// `SitePermissionRepresentation::jsonSerialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SitePermission {
    #[serde(rename = "o:user")]
    pub user: ResourceRef,
    #[serde(rename = "o:role")]
    pub role: String,
}

/// One entry of `o:site_item_set`, from
/// `SiteItemSetRepresentation::jsonSerialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteItemSet {
    #[serde(rename = "o:item_set")]
    pub item_set: ResourceRef,
}
