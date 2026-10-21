//! The typed `media.data` payloads.
//!
//! `media.data` is a JSON column whose shape depends on the ingester
//! (`schema.sql:90`, `Entity/Media.php`). Upstream reads it as an untyped
//! array; the port parses it into one variant per ingester at the storage
//! boundary, so the renderers read typed fields instead of string keys
//! (DESIGN.md "Media").
//!
//! | ingester | upstream payload |
//! |---|---|
//! | `upload`, `url` | `null` (`Ingester/Upload.php`, `Url.php` never call `setData`) |
//! | `html` | `['html' => string]` (`Ingester/Html.php:66`) |
//! | `oembed` | the oEmbed response (`Ingester/OEmbed.php:74`) |
//! | `youtube` | `['id' => string, 'start' => ?, 'end' => ?]` (`Ingester/Youtube.php:79`) |
//! | `iiif` | the IIIF image information document (`Ingester/IIIF.php:73`) |
//! | `iiif_presentation` | the IIIF presentation document (`Ingester/IiifPresentation.php:107`) |

use serde_json::{Map, Value};
use thiserror::Error;

/// A parse failure for a `media.data` value.
#[derive(Debug, Error)]
#[error("media data for ingester \"{ingester}\" is malformed: {message}")]
pub struct MediaDataError {
    /// The ingester name the data was parsed against.
    pub ingester: String,
    /// What was wrong with the value.
    pub message: String,
}

/// The typed `media.data` value.
#[derive(Debug, Clone, PartialEq)]
pub enum MediaData {
    /// An ingester that writes no payload, or a row with `data` null.
    None,
    /// `Ingester/Html.php`.
    Html(HtmlData),
    /// `Ingester/OEmbed.php`.
    Oembed(OembedData),
    /// `Ingester/Youtube.php`.
    Youtube(YoutubeData),
    /// `Ingester/IIIF.php`.
    Iiif(IiifImageData),
    /// `Ingester/IiifPresentation.php`.
    IiifPresentation(IiifPresentationData),
}

impl MediaData {
    /// Parse a `media.data` value against the ingester that wrote it.
    ///
    /// A missing value is [`MediaData::None`], which is what an upload or url
    /// row stores. An unknown ingester also yields [`MediaData::None`]: the
    /// upstream fallback ingester writes nothing, and `Renderer\Fallback`
    /// renders nothing (`Ingester/Fallback.php:37`).
    pub fn parse(ingester: &str, value: Option<&Value>) -> Result<Self, MediaDataError> {
        let malformed = |message: &str| MediaDataError {
            ingester: ingester.to_string(),
            message: message.to_string(),
        };
        let Some(value) = value.filter(|value| !value.is_null()) else {
            return Ok(Self::None);
        };
        match ingester {
            "html" => {
                let html = value
                    .get("html")
                    .and_then(Value::as_str)
                    .ok_or_else(|| malformed("expected an object with a string \"html\""))?;
                Ok(Self::Html(HtmlData {
                    html: html.to_string(),
                }))
            }
            "oembed" => {
                let map = value
                    .as_object()
                    .ok_or_else(|| malformed("expected an oEmbed object"))?;
                Ok(Self::Oembed(OembedData { map: map.clone() }))
            }
            "youtube" => {
                let id = value
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| malformed("expected an object with a string \"id\""))?;
                Ok(Self::Youtube(YoutubeData {
                    id: id.to_string(),
                    start: value.get("start").and_then(scalar_to_string),
                    end: value.get("end").and_then(scalar_to_string),
                }))
            }
            "iiif" => Ok(Self::Iiif(IiifImageData {
                value: value.clone(),
            })),
            "iiif_presentation" => Ok(Self::IiifPresentation(IiifPresentationData {
                value: value.clone(),
            })),
            _ => Ok(Self::None),
        }
    }

    /// Serialise back to the JSON shape upstream stores.
    #[must_use]
    pub fn to_value(&self) -> Value {
        match self {
            Self::None => Value::Null,
            Self::Html(data) => {
                let mut map = Map::new();
                map.insert("html".to_string(), Value::String(data.html.clone()));
                Value::Object(map)
            }
            Self::Oembed(data) => Value::Object(data.map.clone()),
            Self::Youtube(data) => {
                let mut map = Map::new();
                map.insert("id".to_string(), Value::String(data.id.clone()));
                if let Some(start) = &data.start {
                    map.insert("start".to_string(), Value::String(start.clone()));
                }
                if let Some(end) = &data.end {
                    map.insert("end".to_string(), Value::String(end.clone()));
                }
                Value::Object(map)
            }
            Self::Iiif(data) => data.value.clone(),
            Self::IiifPresentation(data) => data.value.clone(),
        }
    }
}

/// `['html' => string]` (`Ingester/Html.php:66`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlData {
    /// The purified HTML.
    pub html: String,
}

/// A full oEmbed response (`Ingester/OEmbed.php:74`).
#[derive(Debug, Clone, PartialEq)]
pub struct OembedData {
    /// The response object, kept whole so JSON-LD round-trips.
    pub map: Map<String, Value>,
}

impl OembedData {
    /// The `html` member, used verbatim by `Stdlib/Oembed::renderOembed`.
    #[must_use]
    pub fn html(&self) -> Option<&str> {
        self.map.get("html").and_then(Value::as_str)
    }

    /// The `type` member, e.g. `photo`, `video`, `link`.
    #[must_use]
    pub fn kind(&self) -> Option<&str> {
        self.map.get("type").and_then(Value::as_str)
    }

    /// A scalar member rendered as a string, for the `photo` markup.
    #[must_use]
    pub fn field(&self, key: &str) -> Option<String> {
        self.map.get(key).and_then(scalar_to_string)
    }

    /// The `title` member, if it is non-empty (`Renderer/OEmbed.php:38`).
    #[must_use]
    pub fn title(&self) -> Option<String> {
        self.field("title").filter(|title| !title.is_empty())
    }
}

/// `['id' => string, 'start' => ?, 'end' => ?]` (`Ingester/Youtube.php:79`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YoutubeData {
    /// The YouTube video id.
    pub id: String,
    /// The `start` parameter, stored as a numeric string.
    pub start: Option<String>,
    /// The `end` parameter, stored as a numeric string.
    pub end: Option<String>,
}

/// A IIIF image information document (`Ingester/IIIF.php:73`).
#[derive(Debug, Clone, PartialEq)]
pub struct IiifImageData {
    /// The document, kept whole.
    pub value: Value,
}

impl IiifImageData {
    /// The `@context` member.
    #[must_use]
    pub fn context(&self) -> Option<&str> {
        self.value.get("@context").and_then(Value::as_str)
    }

    /// A string member of the document.
    #[must_use]
    pub fn member(&self, key: &str) -> Option<&str> {
        self.value.get(key).and_then(Value::as_str)
    }

    /// The `sizes` member: candidate pre-computed sizes.
    #[must_use]
    pub fn sizes(&self) -> &[Value] {
        self.value
            .get("sizes")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

/// A IIIF presentation document (`Ingester/IiifPresentation.php:107`).
#[derive(Debug, Clone, PartialEq)]
pub struct IiifPresentationData {
    /// The document, kept whole.
    pub value: Value,
}

impl IiifPresentationData {
    /// The `@context` member.
    #[must_use]
    pub fn context(&self) -> Option<&str> {
        self.value.get("@context").and_then(Value::as_str)
    }

    /// The `@type` member (presentation API 2).
    #[must_use]
    pub fn at_type(&self) -> Option<&str> {
        self.value.get("@type").and_then(Value::as_str)
    }

    /// The `type` member (presentation API 3).
    #[must_use]
    pub fn type_value(&self) -> Option<&str> {
        self.value.get("type").and_then(Value::as_str)
    }

    /// The top-level thumbnail URL for the given presentation version,
    /// matching `IiifPresentation::ingest` (`Ingester/IiifPresentation.php:110`).
    #[must_use]
    pub fn thumbnail_url(&self, version: u8) -> Option<String> {
        match version {
            2 => self
                .value
                .get("thumbnail")
                .and_then(|thumb| thumb.get("@id"))
                .and_then(Value::as_str)
                .map(str::to_string),
            3 => self
                .value
                .get("thumbnail")
                .and_then(Value::as_array)
                .and_then(|thumbs| thumbs.first())
                .and_then(|thumb| thumb.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string),
            _ => None,
        }
    }

    /// The canvases the document paints. A manifest names them in version 2's
    /// `sequences[0].canvases` and version 3's `items`; a collection names
    /// manifests instead, so it answers `None`.
    #[must_use]
    pub fn canvas_count(&self, version: u8) -> Option<u64> {
        let canvases = match version {
            2 => self
                .value
                .get("sequences")?
                .as_array()?
                .first()?
                .get("canvases")?
                .as_array()?,
            3 => self.value.get("items")?.as_array()?,
            _ => return None,
        };
        Some(canvases.len() as u64)
    }

    /// The document's label: the string of version 2, or the first value for
    /// English (else `none`, else any language) of version 3's language map.
    #[must_use]
    pub fn label(&self) -> Option<String> {
        let label = self.value.get("label")?;
        if let Some(text) = label.as_str() {
            return Some(text.to_string());
        }
        let map = label.as_object()?;
        for key in ["en", "none"] {
            if let Some(text) = map
                .get(key)
                .and_then(Value::as_array)
                .and_then(|values| values.first())
                .and_then(Value::as_str)
            {
                return Some(text.to_string());
            }
        }
        map.values()
            .find_map(|value| value.as_array()?.first()?.as_str().map(str::to_string))
    }
}

fn scalar_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Ingester/Youtube.php` stores `id` plus numeric `start`/`end`.
    #[test]
    fn youtube_data_round_trips_through_parse() {
        let raw = serde_json::json!({"id": "abc", "start": "30", "end": 60});
        let data = MediaData::parse("youtube", Some(&raw)).expect("parse");
        assert_eq!(
            data,
            MediaData::Youtube(YoutubeData {
                id: "abc".into(),
                start: Some("30".into()),
                end: Some("60".into()),
            })
        );
        assert_eq!(
            data.to_value(),
            serde_json::json!({"id": "abc", "start": "30", "end": "60"})
        );
    }

    /// An upload or url row has `data` null, which parses to `None`.
    #[test]
    fn null_data_is_none_for_upload_and_url() {
        assert_eq!(
            MediaData::parse("upload", None).expect("parse"),
            MediaData::None
        );
        assert_eq!(
            MediaData::parse("url", Some(&Value::Null)).expect("parse"),
            MediaData::None
        );
    }

    /// A malformed payload is an error at the boundary, not a panic.
    #[test]
    fn malformed_html_payload_is_an_error() {
        let raw = serde_json::json!({"text": "no html key"});
        assert!(MediaData::parse("html", Some(&raw)).is_err());
    }

    /// The oEmbed response is kept whole so every member survives a round trip.
    #[test]
    fn oembed_response_round_trips_whole() {
        let raw = serde_json::json!({"type": "photo", "url": "https://x/y.jpg", "width": 640});
        let data = MediaData::parse("oembed", Some(&raw)).expect("parse");
        assert_eq!(data.to_value(), raw);
        let MediaData::Oembed(oembed) = data else {
            panic!("expected oembed");
        };
        assert_eq!(oembed.kind(), Some("photo"));
        assert_eq!(oembed.field("width").as_deref(), Some("640"));
        assert_eq!(oembed.title(), None);
    }

    /// The IIIF presentation version picks the right thumbnail member.
    #[test]
    fn iiif_presentation_reads_version_specific_thumbnail() {
        let v2 = serde_json::json!({"@context": "http://iiif.io/api/presentation/2/context.json",
            "thumbnail": {"@id": "https://x/thumb.jpg"}});
        let data = MediaData::parse("iiif_presentation", Some(&v2)).expect("parse");
        let MediaData::IiifPresentation(presentation) = data else {
            panic!("expected iiif_presentation");
        };
        assert_eq!(
            presentation.thumbnail_url(2).as_deref(),
            Some("https://x/thumb.jpg")
        );
        assert_eq!(presentation.thumbnail_url(3), None);

        let v3 = serde_json::json!({"@context": "http://iiif.io/api/presentation/3/context.json",
            "thumbnail": [{"id": "https://x/thumb3.jpg"}]});
        let data = MediaData::parse("iiif_presentation", Some(&v3)).expect("parse");
        let MediaData::IiifPresentation(presentation) = data else {
            panic!("expected iiif_presentation");
        };
        assert_eq!(
            presentation.thumbnail_url(3).as_deref(),
            Some("https://x/thumb3.jpg")
        );
    }
}
