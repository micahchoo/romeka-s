//! `MediaRepresentation` wire shape.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::common::{
    JsonLdDate, JsonLdType, ResourceRef, ReverseRef, ThumbnailDisplayUrls, ThumbnailUrls, ValueMap,
};

/// `MediaAdapter::getResourceName`.
pub const RESOURCE_NAME: &str = "media";

/// Where a media's derivatives stand, as `o-module-romeka:preview.state` reports them.
///
/// `pending` covers a queued row and a worker that has claimed it: from a reader's side both
/// mean "not yet". `none` means there is nothing to wait for, either because the kind has no
/// derivative or because a tool this install does not have makes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreviewState {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "none")]
    None,
}

/// The port's derivative state for one media (`o-module-romeka:preview`).
///
/// Upstream has no such member: its admin reads `media.has_thumbnails` and shows a placeholder
/// while a derivative is missing. The reason is the typed skip or failure a reader can show, and
/// progress is the percentage a long derivative reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaPreview {
    pub state: PreviewState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
}

impl MediaPreview {
    #[must_use]
    pub fn pending(progress: Option<u8>) -> Self {
        Self {
            state: PreviewState::Pending,
            reason: None,
            progress,
        }
    }

    #[must_use]
    pub fn ready() -> Self {
        Self {
            state: PreviewState::Ready,
            reason: None,
            progress: None,
        }
    }

    #[must_use]
    pub fn failed(reason: Option<String>) -> Self {
        Self {
            state: PreviewState::Failed,
            reason,
            progress: None,
        }
    }

    #[must_use]
    pub fn none(reason: Option<String>) -> Self {
        Self {
            state: PreviewState::None,
            reason,
            progress: None,
        }
    }
}

/// One media document, including the raw `data` column
/// (`MediaRepresentation::mediaData`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Media {
    #[serde(rename = "@context")]
    pub context: String,
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@type")]
    pub jsonld_type: JsonLdType,
    #[serde(rename = "o:id")]
    pub o_id: u64,
    #[serde(rename = "o:is_public")]
    pub is_public: bool,
    #[serde(rename = "o:owner")]
    pub owner: Option<ResourceRef>,
    #[serde(rename = "o:resource_class")]
    pub resource_class: Option<ResourceRef>,
    #[serde(rename = "o:resource_template")]
    pub resource_template: Option<ResourceRef>,
    #[serde(rename = "o:thumbnail")]
    pub thumbnail: Option<ResourceRef>,
    #[serde(rename = "o:title")]
    pub title: Option<String>,
    pub thumbnail_display_urls: ThumbnailDisplayUrls,
    #[serde(rename = "o:created")]
    pub created: JsonLdDate,
    #[serde(rename = "o:modified")]
    pub modified: Option<JsonLdDate>,
    #[serde(rename = "o:ingester")]
    pub ingester: String,
    #[serde(rename = "o:renderer")]
    pub renderer: Option<String>,
    #[serde(rename = "o:item")]
    pub item: ResourceRef,
    #[serde(rename = "o:source")]
    pub source: Option<String>,
    #[serde(rename = "o:media_type")]
    pub media_type: Option<String>,
    /// The port's per-kind name (`omeka_media::kind::MediaKind::name`). An
    /// older client reads its absence as "derive the kind yourself".
    #[serde(rename = "o:kind", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(rename = "o:sha256")]
    pub sha256: Option<String>,
    #[serde(rename = "o:size")]
    pub size: Option<u64>,
    #[serde(rename = "o:filename")]
    pub filename: Option<String>,
    #[serde(rename = "o:lang")]
    pub lang: Option<String>,
    #[serde(rename = "o:alt_text")]
    pub alt_text: Option<String>,
    #[serde(rename = "o:original_url")]
    pub original_url: Option<String>,
    /// The port's derivative state for this media (`PreviewState`). Upstream has no such
    /// member and reads none of it.
    #[serde(
        rename = "o-module-romeka:preview",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub preview: Option<MediaPreview>,
    #[serde(rename = "o:thumbnail_urls")]
    pub thumbnail_urls: ThumbnailUrls,
    pub data: Option<serde_json::Value>,
    #[serde(flatten)]
    pub values: ValueMap,
    #[serde(rename = "@reverse", default, skip_serializing_if = "Option::is_none")]
    pub reverse: Option<IndexMap<String, Vec<ReverseRef>>>,
    /// `Module::filterHtmlMediaJsonLd` (`application/Module.php:397-408`) adds
    /// `o-cnt:chars` and `o-cnt:characterEncoding` after the whole document,
    /// for html media only. The ingester stores data only when `html` is set
    /// (`Ingester\Html::ingest`), so an html row carries both members.
    #[serde(
        rename = "o-cnt:chars",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub cnt_chars: Option<String>,
    #[serde(
        rename = "o-cnt:characterEncoding",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub cnt_character_encoding: Option<String>,
}
