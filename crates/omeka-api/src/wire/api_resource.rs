//! `ApiResourceRepresentation` wire shape.

use serde::{Deserialize, Serialize};

use super::common::JsonLdType;

/// `ApiResourceAdapter::getResourceName`.
pub const RESOURCE_NAME: &str = "api_resources";

/// One API resource document. `o:id` is the resource name, a string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiResource {
    #[serde(rename = "@context")]
    pub context: String,
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@type")]
    pub jsonld_type: JsonLdType,
    #[serde(rename = "o:id")]
    pub o_id: String,
}
