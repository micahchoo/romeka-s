//! `ItemSetRepresentation` wire shape.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::common::{
    ApiUrl, JsonLdDate, JsonLdType, ResourceRef, ReverseRef, ThumbnailDisplayUrls, ValueMap,
};

/// `ItemSetAdapter::getResourceName`.
pub const RESOURCE_NAME: &str = "item_sets";

/// One item set document, including the `o:items` search link that
/// `ItemSetRepresentation::getResourceJsonLd` appends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemSet {
    #[serde(rename = "@context")]
    pub context: String,
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@type")]
    pub jsonld_type: JsonLdType,
    #[serde(rename = "o:id")]
    pub o_id: u64,
    #[serde(rename = "o:is_public")]
    pub is_public: bool,
    #[serde(rename = "o:owner")]
    pub owner: Option<ResourceRef>,
    #[serde(rename = "o:resource_class")]
    pub resource_class: Option<ResourceRef>,
    #[serde(rename = "o:resource_template")]
    pub resource_template: Option<ResourceRef>,
    #[serde(rename = "o:thumbnail")]
    pub thumbnail: Option<ResourceRef>,
    #[serde(rename = "o:title")]
    pub title: Option<String>,
    pub thumbnail_display_urls: ThumbnailDisplayUrls,
    #[serde(rename = "o:created")]
    pub created: JsonLdDate,
    #[serde(rename = "o:modified")]
    pub modified: Option<JsonLdDate>,
    #[serde(rename = "o:is_open")]
    pub is_open: bool,
    #[serde(rename = "o:items")]
    pub items: ApiUrl,
    #[serde(flatten)]
    pub values: ValueMap,
    #[serde(rename = "@reverse", default, skip_serializing_if = "Option::is_none")]
    pub reverse: Option<IndexMap<String, Vec<ReverseRef>>>,
}
