//! `ModuleRepresentation` wire shape.

use serde::{Deserialize, Serialize};

use super::common::JsonLdType;

/// `ModuleAdapter::getResourceName`.
pub const RESOURCE_NAME: &str = "modules";

/// One module document. `o:id` is the module directory name, a string.
///
/// `o:state` is a string too: `Module\Manager::STATE_*` (`active`,
/// `not_active`, `not_installed`, `not_found`, `invalid_module`,
/// `invalid_ini`, `invalid_omeka_version`, `needs_upgrade`). It is not the
/// numeric flag the module's INI carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Module {
    #[serde(rename = "@context")]
    pub context: String,
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@type")]
    pub jsonld_type: JsonLdType,
    #[serde(rename = "o:id")]
    pub o_id: String,
    #[serde(rename = "o:state")]
    pub state: String,
    #[serde(rename = "o:ini")]
    pub ini: serde_json::Value,
}
