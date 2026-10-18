//! `ResourceTemplateRepresentation` wire shape.

use serde::{Deserialize, Serialize};

use super::common::{JsonLdType, ResourceRef};

/// `ResourceTemplateAdapter::getResourceName`.
pub const RESOURCE_NAME: &str = "resource_templates";

/// One resource template document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceTemplate {
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
    #[serde(rename = "o:owner")]
    pub owner: Option<ResourceRef>,
    #[serde(rename = "o:resource_class")]
    pub resource_class: Option<ResourceRef>,
    #[serde(rename = "o:title_property")]
    pub title_property: Option<ResourceRef>,
    #[serde(rename = "o:description_property")]
    pub description_property: Option<ResourceRef>,
    #[serde(rename = "o:resource_template_property")]
    pub resource_template_property: Vec<ResourceTemplateProperty>,
}

/// One entry of `o:resource_template_property`, from
/// `ResourceTemplatePropertyRepresentation::jsonSerialize`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceTemplateProperty {
    #[serde(rename = "o:property")]
    pub property: ResourceRef,
    #[serde(rename = "o:alternate_label")]
    pub alternate_label: Option<String>,
    #[serde(rename = "o:alternate_comment")]
    pub alternate_comment: Option<String>,
    #[serde(rename = "o:data_type")]
    pub data_type: Vec<String>,
    #[serde(rename = "o:is_required")]
    pub is_required: bool,
    #[serde(rename = "o:is_private")]
    pub is_private: bool,
    #[serde(rename = "o:default_lang")]
    pub default_lang: Option<String>,
}
