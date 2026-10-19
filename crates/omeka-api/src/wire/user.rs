//! `UserRepresentation` wire shape.

use serde::{Deserialize, Serialize};

use super::common::JsonLdType;

/// `UserAdapter::getResourceName`.
pub const RESOURCE_NAME: &str = "users";

/// One user document. `o:created` is a plain ISO 8601 string here, because
/// `UserRepresentation::getJsonLd` stores the `Stdlib\DateTime` wrapper whose
/// `jsonSerialize` returns the formatted date, not a JSON-LD value object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    #[serde(rename = "@context")]
    pub context: String,
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@type")]
    pub jsonld_type: JsonLdType,
    #[serde(rename = "o:id")]
    pub o_id: u64,
    #[serde(rename = "o:name")]
    pub name: String,
    #[serde(rename = "o:email")]
    pub email: String,
    #[serde(rename = "o:created")]
    pub created: String,
    #[serde(rename = "o:role")]
    pub role: String,
    #[serde(rename = "o:is_active")]
    pub is_active: bool,
    /// Port addition: the user's settings the admin edits, on a single-user read.
    #[serde(
        rename = "o:user_settings",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub user_settings: Option<serde_json::Value>,
    /// Port addition: the admin locale of `POST /api/login` and `GET /api/me`.
    #[serde(
        rename = "o:admin_locale",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub admin_locale: Option<String>,
}
