//! `PropertyRepresentation` wire shape.

use serde::{Deserialize, Serialize};

use super::common::{JsonLdType, ResourceRef};

/// `PropertyAdapter::getResourceName`.
pub const RESOURCE_NAME: &str = "properties";

/// One property document, from
/// `AbstractVocabularyMemberRepresentation::getJsonLd`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Property {
    #[serde(rename = "@context")]
    pub context: String,
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@type")]
    pub jsonld_type: JsonLdType,
    #[serde(rename = "o:id")]
    pub o_id: u64,
    #[serde(rename = "o:local_name")]
    pub local_name: String,
    #[serde(rename = "o:label")]
    pub label: Option<String>,
    #[serde(rename = "o:comment")]
    pub comment: Option<String>,
    #[serde(rename = "o:term")]
    pub term: String,
    #[serde(rename = "o:vocabulary")]
    pub vocabulary: ResourceRef,
}
