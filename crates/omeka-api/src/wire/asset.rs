//! `AssetRepresentation` wire shape.

use serde::{Deserialize, Serialize};

use super::common::{JsonLdType, ResourceRef};

/// `AssetAdapter::getResourceName`.
pub const RESOURCE_NAME: &str = "assets";

/// One asset document. `AssetRepresentation::getJsonLd` repeats `o:id`, which
/// the envelope already set; PHP array merge collapses the duplicate, so the
/// wire shape carries it once.
///
/// `o:name`, `o:filename`, and `o:media_type` are non-null on the wire: the
/// Asset columns `name` and `mediaType` are not nullable and `getFilename`
/// builds from the non-null `storageId`. `o:alt_text` is nullable in the
/// database too.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    #[serde(rename = "@context")]
    pub context: String,
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@type")]
    pub jsonld_type: JsonLdType,
    #[serde(rename = "o:id")]
    pub o_id: u64,
    #[serde(rename = "o:owner")]
    pub owner: Option<ResourceRef>,
    #[serde(rename = "o:name")]
    pub name: String,
    #[serde(rename = "o:filename")]
    pub filename: String,
    #[serde(rename = "o:media_type")]
    pub media_type: String,
    #[serde(rename = "o:asset_url")]
    pub asset_url: String,
    #[serde(rename = "o:alt_text")]
    pub alt_text: Option<String>,
}
