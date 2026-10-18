//! `DataTypeRepresentation` wire shape.

use serde::{Deserialize, Serialize};

use super::common::JsonLdType;

/// `DataTypeAdapter::getResourceName`.
pub const RESOURCE_NAME: &str = "data_types";

/// One data type document. `o:id` is the registered data type name, a string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataType {
    #[serde(rename = "@context")]
    pub context: String,
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@type")]
    pub jsonld_type: JsonLdType,
    #[serde(rename = "o:id")]
    pub o_id: String,
    #[serde(rename = "o:label")]
    pub label: String,
}
