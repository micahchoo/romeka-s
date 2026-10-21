//! IIIF Presentation 3 documents for the site's own items and item sets.
//!
//! A pure transform: the item's API representation and its media, already
//! read as the caller may see them, become one manifest; an item set and its
//! items become one collection. Reading what the caller may see, and the pixel
//! sizes, is the route's work (`super`).
//!
//! The choices the specification leaves open:
//!
//! - The label is the item's title; the metadata is every value, one entry per
//!   property in the representation's order, keyed by each value's language.
//! - `rights` is the first `dcterms:rights` URI that names a Creative Commons
//!   licence or a RightsStatements.org statement, the two the specification
//!   accepts. Any other rights value stays in the metadata.
//! - A local image paints the "large" derivative at its own size on a canvas
//!   the size of the original, carries the Image API service so a viewer can
//!   deep-zoom, and links the original through `rendering`.
//! - A media ingested from a IIIF image service paints that remote service.
//! - A PDF with a first-page derivative paints that still, the size of the
//!   derivative, and links the PDF through `rendering`.
//! - An audio or video file paints a `Sound` or `Video` body and carries its
//!   duration, which the extent probe measured. A canvas that needs a duration
//!   the probe did not answer is left out.
//! - A canvas whose media has a note the caller may read references the media's
//!   annotation page, so a viewer can show the notes. One with none carries no
//!   `annotations` key at all, so an unchanged manifest stays unchanged.
//! - An item manifest with a canvas names the item's Content Search 2.0
//!   service, and a media manifest is the item manifest's canvas for one media.

use std::collections::BTreeMap;

use omeka_site::html::escape_html;
use serde_json::{Map, Value, json};

const CONTEXT: &str = "http://iiif.io/api/presentation/3/context.json";

/// A width and a height in pixels.
pub(crate) type Size = (u32, u32);

/// A media file this server stores, as a canvas paints it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalImage {
    /// The original file's URL, linked through `rendering`.
    pub(crate) original_url: String,
    /// The original's media type.
    pub(crate) media_type: String,
    /// The original's size, which is the canvas size.
    pub(crate) size: Size,
    /// The "large" derivative's URL, the painted body.
    pub(crate) large_url: String,
    /// The "large" derivative's own size.
    pub(crate) large_size: Size,
    /// The "square" derivative's URL, the canvas thumbnail.
    pub(crate) square_url: Option<String>,
    /// This server's Image API service for the original.
    pub(crate) service: String,
}

/// What one canvas paints.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Painting {
    /// A media file this server stores.
    Local(LocalImage),
    /// A media ingested from a IIIF image service: its stored `info.json`.
    Remote(Value),
    /// A still derivative with no service, such as a PDF's first page.
    Still {
        /// The derivative's URL, the painted body.
        url: String,
        /// The derivative's size, which is also the canvas size.
        size: Size,
        /// The document itself, linked through `rendering`.
        pdf_url: String,
    },
    /// An audio file.
    Sound {
        url: String,
        media_type: String,
        duration_s: f64,
    },
    /// A video file, with its frame size and poster when the extents name one.
    Video {
        url: String,
        media_type: String,
        duration_s: f64,
        size: Option<Size>,
        poster: Option<String>,
    },
}

/// One media that becomes a canvas.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CanvasMedia {
    pub(crate) id: u64,
    pub(crate) label: String,
    pub(crate) painting: Painting,
    /// The media's annotation page, set when the caller may read at least one
    /// note on it.
    pub(crate) annotation_page: Option<String>,
}

/// The manifest of one item at `manifest_id`, with one canvas per media in
/// the order given. A remote media whose `info.json` names no size is left
/// out, because a canvas must have one. `search` is the item's Content Search
/// 2.0 service, advertised when the manifest has a canvas for it to search.
pub(crate) fn item_manifest(
    manifest_id: &str,
    item: &Value,
    media: &[CanvasMedia],
    search: &str,
) -> Value {
    let base = manifest_id.trim_end_matches("/manifest");
    let canvases: Vec<Value> = media
        .iter()
        .filter_map(|media| canvas(base, media))
        .collect();
    let mut manifest = Map::new();
    manifest.insert("@context".into(), json!(CONTEXT));
    manifest.insert("id".into(), json!(manifest_id));
    manifest.insert("type".into(), json!("Manifest"));
    manifest.insert("label".into(), none_map(&title(item)));
    let metadata = metadata(item);
    if !metadata.is_empty() {
        manifest.insert("metadata".into(), Value::Array(metadata));
    }
    if let Some(summary) = first_literal(item, "dcterms:description") {
        manifest.insert("summary".into(), summary);
    }
    if let Some(rights) = rights(item) {
        manifest.insert("rights".into(), json!(rights));
    }
    if let Some(thumbnail) = canvases.iter().find_map(|canvas| canvas.get("thumbnail")) {
        manifest.insert("thumbnail".into(), thumbnail.clone());
    }
    if !canvases.is_empty() {
        manifest.insert(
            "service".into(),
            json!([{"id": search, "type": "SearchService2"}]),
        );
    }
    manifest.insert("items".into(), Value::Array(canvases));
    Value::Object(manifest)
}

/// The media manifest at `manifest_id`: the item manifest's canvas for one
/// media, alone, with the item's label, metadata and rights, and `partOf` the
/// item manifest. The canvas IRI is the item manifest's, so a note made through
/// either manifest targets the same canvas.
///
/// A media whose painting names no size gets no canvas here either; the route
/// answers 404 for it through [`paints`].
pub(crate) fn media_manifest(
    manifest_id: &str,
    item_manifest_id: &str,
    item: &Value,
    media: &CanvasMedia,
) -> Value {
    let base = item_manifest_id.trim_end_matches("/manifest");
    let canvases: Vec<Value> = canvas(base, media).into_iter().collect();
    let mut manifest = Map::new();
    manifest.insert("@context".into(), json!(CONTEXT));
    manifest.insert("id".into(), json!(manifest_id));
    manifest.insert("type".into(), json!("Manifest"));
    manifest.insert("label".into(), none_map(&title(item)));
    let metadata = metadata(item);
    if !metadata.is_empty() {
        manifest.insert("metadata".into(), Value::Array(metadata));
    }
    if let Some(summary) = first_literal(item, "dcterms:description") {
        manifest.insert("summary".into(), summary);
    }
    if let Some(rights) = rights(item) {
        manifest.insert("rights".into(), json!(rights));
    }
    manifest.insert(
        "partOf".into(),
        json!([{"id": item_manifest_id, "type": "Manifest"}]),
    );
    manifest.insert("items".into(), Value::Array(canvases));
    Value::Object(manifest)
}

/// Whether this media has a canvas to show at all: a remote service with no
/// size has none, and a kind that paints nothing never reaches here.
pub(crate) fn paints(media: &CanvasMedia) -> bool {
    painted(&media.painting).is_some()
}

/// The collection of one item set at `collection_id`: one manifest reference
/// per item, `(manifest id, item representation)`.
pub(crate) fn item_set_collection(
    collection_id: &str,
    item_set: &Value,
    items: &[(String, Value)],
) -> Value {
    let members: Vec<Value> = items
        .iter()
        .map(|(id, item)| json!({"id": id, "type": "Manifest", "label": none_map(&title(item))}))
        .collect();
    let mut collection = Map::new();
    collection.insert("@context".into(), json!(CONTEXT));
    collection.insert("id".into(), json!(collection_id));
    collection.insert("type".into(), json!("Collection"));
    collection.insert("label".into(), none_map(&title(item_set)));
    let metadata = metadata(item_set);
    if !metadata.is_empty() {
        collection.insert("metadata".into(), Value::Array(metadata));
    }
    collection.insert("items".into(), Value::Array(members));
    Value::Object(collection)
}

fn canvas(base: &str, media: &CanvasMedia) -> Option<Value> {
    let canvas_id = format!("{base}/canvas/{}", media.id);
    let painted = painted(&media.painting)?;
    let mut canvas = Map::new();
    canvas.insert("id".into(), json!(canvas_id));
    canvas.insert("type".into(), json!("Canvas"));
    canvas.insert("label".into(), none_map(&media.label));
    canvas.insert("width".into(), json!(painted.canvas_size.0));
    canvas.insert("height".into(), json!(painted.canvas_size.1));
    if let Some(duration) = painted.duration_s {
        canvas.insert("duration".into(), json!(duration));
    }
    if let Some(thumbnail) = painted.thumbnail {
        canvas.insert("thumbnail".into(), thumbnail);
    }
    if let Some(rendering) = painted.rendering {
        canvas.insert("rendering".into(), rendering);
    }
    if let Some(page) = &media.annotation_page {
        canvas.insert(
            "annotations".into(),
            json!([{"id": page, "type": "AnnotationPage"}]),
        );
    }
    canvas.insert(
        "items".into(),
        json!([{
            "id": format!("{base}/page/{}", media.id),
            "type": "AnnotationPage",
            "items": [{
                "id": format!("{base}/annotation/{}", media.id),
                "type": "Annotation",
                "motivation": "painting",
                "body": painted.body,
                "target": canvas_id,
            }],
        }]),
    );
    Some(Value::Object(canvas))
}

/// The canvas's pixel size and optionally its duration, the body it paints,
/// and the thumbnail and `rendering` links that suit the body.
struct Painted {
    canvas_size: Size,
    duration_s: Option<f64>,
    body: Value,
    thumbnail: Option<Value>,
    rendering: Option<Value>,
}

/// One canvas's parts.
///
/// A canvas must have a size (Presentation 3.0), and a sound recording has no
/// pixels, so it gets [`SOUND_CANVAS`], a strip its player fits. A video
/// without a frame size gets [`VIDEO_CANVAS`]. A viewer lays a time-based
/// canvas out from its duration.
fn painted(painting: &Painting) -> Option<Painted> {
    match painting {
        Painting::Local(image) => Some(Painted {
            canvas_size: image.size,
            duration_s: None,
            body: json!({
                "id": image.large_url,
                "type": "Image",
                "format": "image/jpeg",
                "width": image.large_size.0,
                "height": image.large_size.1,
                "service": [{"id": image.service, "type": "ImageService3", "profile": "level1"}],
            }),
            thumbnail: image
                .square_url
                .as_ref()
                .map(|url| json!([{"id": url, "type": "Image", "format": "image/jpeg"}])),
            rendering: Some(json!([{
                "id": image.original_url,
                "type": "Image",
                "label": {"en": ["Original file"]},
                "format": image.media_type,
            }])),
        }),
        Painting::Remote(info) => {
            let (body, size) = remote_body(info)?;
            Some(Painted {
                canvas_size: size,
                duration_s: None,
                body,
                thumbnail: None,
                rendering: None,
            })
        }
        Painting::Still { url, size, pdf_url } => Some(Painted {
            canvas_size: *size,
            duration_s: None,
            body: json!({
                "id": url,
                "type": "Image",
                "format": "image/jpeg",
                "width": size.0,
                "height": size.1,
            }),
            thumbnail: None,
            rendering: Some(json!([{
                "id": pdf_url,
                "type": "Text",
                "label": {"en": ["Original file"]},
                "format": "application/pdf",
            }])),
        }),
        Painting::Sound {
            url,
            media_type,
            duration_s,
        } => Some(Painted {
            canvas_size: SOUND_CANVAS,
            duration_s: Some(*duration_s),
            body: json!({
                "id": url,
                "type": "Sound",
                "format": media_type,
                "duration": duration_s,
            }),
            thumbnail: None,
            rendering: None,
        }),
        Painting::Video {
            url,
            media_type,
            duration_s,
            size,
            poster,
        } => {
            let canvas_size = size.unwrap_or(VIDEO_CANVAS);
            Some(Painted {
                canvas_size,
                duration_s: Some(*duration_s),
                body: json!({
                    "id": url,
                    "type": "Video",
                    "format": media_type,
                    "duration": duration_s,
                    "width": canvas_size.0,
                    "height": canvas_size.1,
                }),
                thumbnail: poster
                    .as_ref()
                    .map(|url| json!([{"id": url, "type": "Image", "format": "image/jpeg"}])),
                rendering: None,
            })
        }
    }
}

/// The size a sound recording's canvas takes: a strip with no pixels of its
/// own, wide enough for a player's controls and timeline.
const SOUND_CANVAS: Size = (600, 100);

/// The size a video canvas takes when its probe named no frame size.
const VIDEO_CANVAS: Size = (640, 360);

/// The painted body for a remote image service, from its `info.json` in
/// either Image API version, and the service's full size.
fn remote_body(info: &Value) -> Option<(Value, Size)> {
    let id = info
        .get("id")
        .or_else(|| info.get("@id"))
        .and_then(Value::as_str)?
        .trim_end_matches('/');
    let width = u32::try_from(info.get("width")?.as_u64()?).ok()?;
    let height = u32::try_from(info.get("height")?.as_u64()?).ok()?;
    let version_two = info
        .get("@context")
        .and_then(Value::as_str)
        .is_some_and(|context| context.contains("/image/2/"));
    let profile = match info.get("profile") {
        Some(Value::String(profile)) => level_of(profile),
        Some(Value::Array(profiles)) => profiles
            .first()
            .and_then(Value::as_str)
            .map_or("level0", level_of),
        _ => "level0",
    };
    let (path, service) = if version_two {
        (
            "full/full/0/default.jpg",
            json!({"@id": id, "@type": "ImageService2", "profile": profile}),
        )
    } else {
        (
            "full/max/0/default.jpg",
            json!({"id": id, "type": "ImageService3", "profile": profile}),
        )
    };
    let body = json!({
        "id": format!("{id}/{path}"),
        "type": "Image",
        "format": "image/jpeg",
        "width": width,
        "height": height,
        "service": [service],
    });
    Some((body, (width, height)))
}

/// `level1` from `level1` or from `http://iiif.io/api/image/2/level1.json`.
fn level_of(profile: &str) -> &'static str {
    ["level2", "level1"]
        .into_iter()
        .find(|level| profile.contains(level))
        .unwrap_or("level0")
}

fn title(resource: &Value) -> String {
    resource
        .get("o:title")
        .and_then(Value::as_str)
        .filter(|title| !title.is_empty())
        .unwrap_or("[Untitled]")
        .to_string()
}

fn none_map(text: &str) -> Value {
    json!({"none": [text]})
}

/// The values of every property, as `metadata` entries in representation
/// order. A value's language keys it; a value without one is `none`.
fn metadata(resource: &Value) -> Vec<Value> {
    let Some(object) = resource.as_object() else {
        return Vec::new();
    };
    object
        .iter()
        .filter(|(key, _)| !key.starts_with("o:") && !key.starts_with('@') && key.contains(':'))
        .filter_map(|(_, values)| {
            let values = values.as_array()?;
            let label = values.first()?.get("property_label")?.as_str()?;
            let mut by_language: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for value in values {
                let language = value
                    .get("@language")
                    .and_then(Value::as_str)
                    .filter(|language| !language.is_empty())
                    .unwrap_or("none");
                if let Some(text) = value_html(value) {
                    by_language
                        .entry(language.to_string())
                        .or_default()
                        .push(text);
                }
            }
            (!by_language.is_empty())
                .then(|| json!({"label": none_map(label), "value": by_language}))
        })
        .collect()
}

/// One value as the metadata shows it: the escaped text of a literal, a link
/// for a URI, the title of a linked resource.
fn value_html(value: &Value) -> Option<String> {
    if let Some(text) = value.get("@value").and_then(Value::as_str) {
        return Some(escape_html(text));
    }
    if let Some(title) = value.get("display_title").and_then(Value::as_str) {
        return Some(escape_html(title));
    }
    let uri = value.get("@id").and_then(Value::as_str)?;
    let label = value
        .get("o:label")
        .and_then(Value::as_str)
        .filter(|label| !label.is_empty())
        .unwrap_or(uri);
    Some(format!(
        "<a href=\"{}\">{}</a>",
        escape_html(uri),
        escape_html(label)
    ))
}

fn first_literal(resource: &Value, term: &str) -> Option<Value> {
    let value = resource
        .get(term)?
        .as_array()?
        .iter()
        .find(|value| value.get("@value").is_some())?;
    let text = value.get("@value")?.as_str()?;
    let language = value
        .get("@language")
        .and_then(Value::as_str)
        .filter(|language| !language.is_empty())
        .unwrap_or("none");
    Some(json!({ language: [text] }))
}

/// The first `dcterms:rights` URI the specification accepts as `rights`.
fn rights(item: &Value) -> Option<String> {
    item.get("dcterms:rights")?
        .as_array()?
        .iter()
        .filter_map(|value| {
            value
                .get("@id")
                .or_else(|| value.get("@value"))
                .and_then(Value::as_str)
        })
        .find(|uri| {
            ["creativecommons.org/", "rightsstatements.org/"]
                .iter()
                .any(|host| {
                    uri.strip_prefix("http://")
                        .or_else(|| uri.strip_prefix("https://"))
                        .is_some_and(|rest| rest.starts_with(host))
                })
        })
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn literal(label: &str, text: &str) -> Value {
        json!([{"type": "literal", "property_label": label, "@value": text}])
    }

    /// A remote Image API 2 service paints its full image and is referenced
    /// as an `ImageService2`; one with no size makes no canvas.
    #[test]
    fn a_remote_service_paints_its_full_image() {
        let info = json!({
            "@context": "http://iiif.io/api/image/2/context.json",
            "@id": "https://images.example.org/iiif/abc/",
            "width": 3000,
            "height": 2000,
            "profile": ["http://iiif.io/api/image/2/level2.json", {}],
        });
        let (body, size) = remote_body(&info).expect("a body");
        assert_eq!(size, (3000, 2000));
        assert_eq!(
            body["id"],
            "https://images.example.org/iiif/abc/full/full/0/default.jpg"
        );
        assert_eq!(body["service"][0]["@type"], "ImageService2");
        assert_eq!(body["service"][0]["profile"], "level2");
        assert!(remote_body(&json!({"id": "https://x.example/i"})).is_none());
    }

    /// Only a Creative Commons or RightsStatements.org URI becomes `rights`.
    #[test]
    fn rights_take_only_the_accepted_uris() {
        let item = json!({"dcterms:rights": [
            {"type": "literal", "property_label": "Rights", "@value": "All rights reserved"},
            {"type": "uri", "property_label": "Rights", "@id": "https://example.org/licence"},
            {"type": "uri", "property_label": "Rights", "@id": "http://rightsstatements.org/vocab/InC/1.0/"},
        ]});
        assert_eq!(
            rights(&item).as_deref(),
            Some("http://rightsstatements.org/vocab/InC/1.0/")
        );
        assert_eq!(
            rights(&json!({"dcterms:rights": literal("Rights", "CC BY")})),
            None
        );
    }

    /// Metadata keeps the representation's property order, groups a
    /// property's values by language, and escapes literal text.
    #[test]
    fn metadata_follows_the_values() {
        let item = json!({
            "o:id": 1,
            "dcterms:title": literal("Title", "A <b>map</b>"),
            "dcterms:subject": [
                {"type": "literal", "property_label": "Subject", "@value": "Maps", "@language": "en"},
                {"type": "uri", "property_label": "Subject", "@id": "https://id.example/maps", "o:label": "Maps"},
            ],
        });
        let metadata = metadata(&item);
        assert_eq!(metadata[0]["label"], json!({"none": ["Title"]}));
        assert_eq!(
            metadata[0]["value"],
            json!({"none": ["A &lt;b&gt;map&lt;/b&gt;"]})
        );
        assert_eq!(
            metadata[1]["value"],
            json!({"en": ["Maps"], "none": ["<a href=\"https://id.example/maps\">Maps</a>"]})
        );
    }
}
