//! `CustomVocabRepresentation` wire shape (Custom Vocab module v2.1.0).

use serde::{Deserialize, Serialize};

use super::common::{JsonLdType, ResourceRef};

/// `CustomVocabAdapter::getResourceName`.
pub const RESOURCE_NAME: &str = "custom_vocabs";

/// One term list document. `o:terms` is the list as stored and `o:uris` the
/// URI to label map as stored, each `null` when the column is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomVocab {
    #[serde(rename = "@context")]
    pub context: String,
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@type")]
    pub jsonld_type: JsonLdType,
    #[serde(rename = "o:id")]
    pub o_id: u64,
    #[serde(rename = "o:label")]
    pub label: String,
    #[serde(rename = "o:lang")]
    pub lang: Option<String>,
    #[serde(rename = "o:terms")]
    pub terms: Option<serde_json::Value>,
    #[serde(rename = "o:uris")]
    pub uris: Option<serde_json::Value>,
    #[serde(rename = "o:item_set")]
    pub item_set: Option<ResourceRef>,
    #[serde(rename = "o:owner")]
    pub owner: Option<ResourceRef>,
}
