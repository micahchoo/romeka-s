//! `ItemRepresentation` wire shape.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::common::{
    JsonLdDate, JsonLdType, ResourceRef, ReverseRef, ThumbnailDisplayUrls, ValueMap,
};

/// `ItemAdapter::getResourceName`.
pub const RESOURCE_NAME: &str = "items";

/// One item document: `AbstractResourceRepresentation::jsonSerialize` merged
/// with `ItemRepresentation::getResourceJsonLd` through
/// `AbstractResourceEntityRepresentation::getJsonLd`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
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
    #[serde(rename = "o:primary_media")]
    pub primary_media: Option<ResourceRef>,
    #[serde(rename = "o:media")]
    pub media: Vec<ResourceRef>,
    #[serde(rename = "o:item_set")]
    pub item_set: Vec<ResourceRef>,
    #[serde(rename = "o:site")]
    pub site: Vec<ResourceRef>,
    #[serde(flatten)]
    pub values: ValueMap,
    #[serde(rename = "@reverse", default, skip_serializing_if = "Option::is_none")]
    pub reverse: Option<IndexMap<String, Vec<ReverseRef>>>,
}
