//! `SitePageRepresentation` and `SitePageBlockRepresentation` wire shapes.
//!
//! `SitePageRepresentation::getJsonLd` builds the page block;
//! `SitePageBlockRepresentation::jsonSerialize` builds one block. Pages are not
//! resource entities, so neither class carries the common resource envelope.

use serde::{Deserialize, Serialize};

use super::common::{JsonLdDate, JsonLdType, ResourceRef};

/// `SitePageAdapter::getResourceName`.
pub const RESOURCE_NAME: &str = "site_pages";

/// One site page document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SitePage {
    #[serde(rename = "@context")]
    pub context: String,
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@type")]
    pub jsonld_type: JsonLdType,
    #[serde(rename = "o:id")]
    pub o_id: u64,
    #[serde(rename = "o:slug")]
    pub slug: String,
    #[serde(rename = "o:title")]
    pub title: String,
    #[serde(rename = "o:is_public")]
    pub is_public: bool,
    #[serde(rename = "o:layout")]
    pub layout: Option<String>,
    /// `SitePageRepresentation::getJsonLd` writes `?? []`, so a page with no
    /// stored layout data emits the empty array, never `null`.
    #[serde(rename = "o:layout_data")]
    pub layout_data: serde_json::Value,
    #[serde(rename = "o:block")]
    pub block: Vec<SitePageBlock>,
    #[serde(rename = "o:site")]
    pub site: ResourceRef,
    #[serde(rename = "o:created")]
    pub created: JsonLdDate,
    #[serde(rename = "o:modified")]
    pub modified: Option<JsonLdDate>,
}

/// One entry of `o:block`, from `SitePageBlockRepresentation::jsonSerialize`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SitePageBlock {
    #[serde(rename = "o:layout")]
    pub layout: String,
    /// `SitePageBlock::getData()`: the block's stored data, `{}` or `[]` for a
    /// layout whose form posted nothing.
    #[serde(rename = "o:data")]
    pub data: serde_json::Value,
    #[serde(rename = "o:layout_data")]
    pub layout_data: Option<serde_json::Value>,
    #[serde(rename = "o:attachment")]
    pub attachment: Vec<SiteBlockAttachment>,
}

/// One entry of `o:attachment`, from
/// `SiteBlockAttachmentRepresentation::jsonSerialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteBlockAttachment {
    #[serde(rename = "o:item")]
    pub item: Option<ResourceRef>,
    #[serde(rename = "o:media")]
    pub media: Option<ResourceRef>,
    #[serde(rename = "o:caption")]
    pub caption: String,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// The document `GET /api/site_pages/1` returned in the oracle capture,
    /// with the base URL and timestamp normalised. Freezes the key set and the
    /// two empty-array members (`o:layout_data`, the pageTitle block's data).
    #[test]
    fn the_page_shape_matches_the_oracle_capture() {
        let page = SitePage {
            context: "http://HOST/api-context".to_string(),
            id: "http://HOST/api/site_pages/1".to_string(),
            jsonld_type: JsonLdType::single("o:SitePage"),
            o_id: 1,
            slug: "scenarios-page".to_string(),
            title: "Scenarios page".to_string(),
            is_public: true,
            layout: None,
            layout_data: serde_json::json!([]),
            block: vec![
                SitePageBlock {
                    layout: "html".to_string(),
                    data: serde_json::json!({"html": "<p>First scenario block.</p>"}),
                    layout_data: None,
                    attachment: Vec::new(),
                },
                SitePageBlock {
                    layout: "pageTitle".to_string(),
                    data: serde_json::json!([]),
                    layout_data: None,
                    attachment: Vec::new(),
                },
            ],
            site: ResourceRef::new("http://HOST/api/sites/1", 1),
            created: JsonLdDate::new("2026-10-16T12:00:00+00:00"),
            modified: Some(JsonLdDate::new("2026-10-16T12:00:00+00:00")),
        };
        let emitted = crate::jsonld::to_string(&page).unwrap();
        assert!(emitted.contains(r#""o:layout_data":[]"#), "{emitted}");
        assert!(emitted.contains(r#""o:data":[]"#), "{emitted}");
        assert!(
            emitted.contains(r#""o:block":[{"o:layout":"html""#),
            "{emitted}"
        );
        assert!(emitted.contains(r#""o:attachment":[]"#), "{emitted}");
    }
}
