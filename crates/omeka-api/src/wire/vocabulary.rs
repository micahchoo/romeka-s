//! `VocabularyRepresentation` wire shape.

use serde::{Deserialize, Serialize};

use super::common::{JsonLdType, ResourceRef};

/// `VocabularyAdapter::getResourceName`.
pub const RESOURCE_NAME: &str = "vocabularies";

/// One vocabulary document. `VocabularyRepresentation` does not extend the
/// resource entity class, so there is no `o:is_public` block here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vocabulary {
    #[serde(rename = "@context")]
    pub context: String,
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@type")]
    pub jsonld_type: JsonLdType,
    #[serde(rename = "o:id")]
    pub o_id: u64,
    #[serde(rename = "o:namespace_uri")]
    pub namespace_uri: String,
    #[serde(rename = "o:prefix")]
    pub prefix: String,
    #[serde(rename = "o:label")]
    pub label: String,
    #[serde(rename = "o:comment")]
    pub comment: Option<String>,
    #[serde(rename = "o:owner")]
    pub owner: Option<ResourceRef>,
}
