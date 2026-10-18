//! `JobRepresentation` wire shape.

use serde::{Deserialize, Serialize};

use super::common::{JsonLdDate, JsonLdType, ResourceRef};

/// `JobAdapter::getResourceName`.
pub const RESOURCE_NAME: &str = "jobs";

/// One job document. `o:args` is the raw `args` JSON column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Job {
    #[serde(rename = "@context")]
    pub context: String,
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@type")]
    pub jsonld_type: JsonLdType,
    #[serde(rename = "o:id")]
    pub o_id: u64,
    /// `Entity\Job::$status` is nullable, so a job row with no status emits
    /// `null`.
    #[serde(rename = "o:status")]
    pub status: Option<String>,
    #[serde(rename = "o:job_class")]
    pub job_class: String,
    #[serde(rename = "o:args")]
    pub args: serde_json::Value,
    #[serde(rename = "o:owner")]
    pub owner: Option<ResourceRef>,
    #[serde(rename = "o:started")]
    pub started: Option<JsonLdDate>,
    #[serde(rename = "o:ended")]
    pub ended: Option<JsonLdDate>,
}
