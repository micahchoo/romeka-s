//! `MediaRepresentation` wire shape.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::common::{
    JsonLdDate, JsonLdType, ResourceRef, ReverseRef, ThumbnailDisplayUrls, ThumbnailUrls, ValueMap,
};

/// `MediaAdapter::getResourceName`.
pub const RESOURCE_NAME: &str = "media";

/// One media document, including the raw `data` column
/// (`MediaRepresentation::mediaData`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Media {
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
    #[serde(rename = "o:ingester")]
    pub ingester: String,
    #[serde(rename = "o:renderer")]
    pub renderer: Option<String>,
    #[serde(rename = "o:item")]
    pub item: ResourceRef,
    #[serde(rename = "o:source")]
    pub source: Option<String>,
    #[serde(rename = "o:media_type")]
    pub media_type: Option<String>,
    #[serde(rename = "o:sha256")]
    pub sha256: Option<String>,
    #[serde(rename = "o:size")]
    pub size: Option<u64>,
    #[serde(rename = "o:filename")]
    pub filename: Option<String>,
    #[serde(rename = "o:lang")]
    pub lang: Option<String>,
    #[serde(rename = "o:alt_text")]
    pub alt_text: Option<String>,
    #[serde(rename = "o:original_url")]
    pub original_url: Option<String>,
    #[serde(rename = "o:thumbnail_urls")]
    pub thumbnail_urls: ThumbnailUrls,
    pub data: Option<serde_json::Value>,
    #[serde(flatten)]
    pub values: ValueMap,
    #[serde(rename = "@reverse", default, skip_serializing_if = "Option::is_none")]
    pub reverse: Option<IndexMap<String, Vec<ReverseRef>>>,
    /// `Module::filterHtmlMediaJsonLd` (`application/Module.php:397-408`) adds
    /// `o-cnt:chars` and `o-cnt:characterEncoding` after the whole document,
    /// for html media only. The ingester stores data only when `html` is set
    /// (`Ingester\Html::ingest`), so an html row carries both members.
    #[serde(
        rename = "o-cnt:chars",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub cnt_chars: Option<String>,
    #[serde(
        rename = "o-cnt:characterEncoding",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub cnt_character_encoding: Option<String>,
}
