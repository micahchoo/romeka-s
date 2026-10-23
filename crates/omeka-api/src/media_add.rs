//! What a client reads before it adds a media: the upload limits and a probe
//! of a pasted link (`crates/omeka-api/README.md`, "Staged uploads and
//! adding a media").
//!
//! [`limits`] answers the file size an upload may declare and the validation
//! lists a file is checked against, so the add panel can refuse an oversized
//! file or explain a kind before it sends a byte. [`probe`] fetches a link
//! through the guarded client, reads at most [`PROBE_HEAD`] bytes, and names
//! the ingesters that could read it without writing a row.
//!
//! The readers are the ingester names the media create accepts. `url` is
//! always last: any http body can be stored as a file. `youtube`, `iiif` and
//! `iiif_presentation` are offered when the link's shape says their ingester
//! would accept it, and `oembed` when the oEmbed allow list names the host or
//! the page advertises an oEmbed endpoint. `suggested` is the first reader, so
//! a YouTube link suggests YouTube and a plain image suggests a file.

use std::io::Read;
use std::time::Duration;

use omeka_db::{DbError, DbPool};
use omeka_media::ingest::iiif::Iiif;
use omeka_media::ingest::iiif_presentation::IiifPresentation;
use omeka_media::ingest::youtube;
use omeka_media::media_type::detected_media_type_of;
use omeka_media::oembed::{Oembed, OembedAllowList};
use omeka_media::store::extension_from_source_name;
use omeka_media::uri::HttpUri;
use omeka_media::validation::FileValidation;
use omeka_media::{HttpClient, image_metadata};
use serde::Serialize;
use serde_json::Value;

use crate::errors::{ApiError, ValidationErrors};
use crate::messages;
use crate::settings;

/// The cap on one probe fetch, every redirect and the body included.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// The bytes a probe reads from a body it does not have to parse as JSON.
const PROBE_HEAD: usize = 1024 * 1024;

/// What an add panel reads before it sends a file (`GET /api/romeka/upload-limits`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadLimits {
    /// The largest file one upload may declare, as `import_upload_max_file`.
    pub max_file_bytes: u64,
    /// Every media kind the server names, in `omeka_media::kind::MediaKind::ALL` order.
    pub kinds: Vec<&'static str>,
    /// The extensions the file validation accepts; empty when it is off.
    pub extensions: Vec<String>,
    /// True when `disable_file_validation` is on, so every file passes.
    pub validation_off: bool,
}

/// The limits as the install applies them.
///
/// # Errors
/// [`DbError`] when a setting row cannot be read.
pub async fn limits(pool: &DbPool) -> Result<UploadLimits, DbError> {
    let uploads = settings::upload_settings(pool).await?;
    let validation = settings::file_validation(pool).await?;
    let (extensions, validation_off) = match validation {
        FileValidation::Disabled => (Vec::new(), true),
        FileValidation::Allow { extensions, .. } => (extensions, false),
    };
    Ok(UploadLimits {
        max_file_bytes: uploads.max_file,
        kinds: omeka_media::kind::MediaKind::names(),
        extensions,
        validation_off,
    })
}

/// The pixel size of an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ImageSize {
    pub w: u32,
    pub h: u32,
}

/// What a link turned out to be (`POST /api/romeka/media/probe`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    /// The ingester names that could read the link, preferred first.
    pub readers: Vec<&'static str>,
    /// The reader the panel offers first: `readers[0]`.
    pub suggested: &'static str,
    /// The body's type, or `None` when the reader stores no file.
    pub media_type: Option<String>,
    /// The body's `Content-Length`, when the response declared one.
    pub size: Option<u64>,
    /// The page, oEmbed or IIIF label, when the link names itself.
    pub title: Option<String>,
    /// The pixel size of an image body.
    pub image: Option<ImageSize>,
    /// The canvases of a IIIF manifest; `None` for every other link.
    pub canvases: Option<u64>,
}

/// Probe `url` through the guarded client.
///
/// The client is built from the install's address policy, like an ingester's,
/// so a link to a private address is refused unless the operator allowed it.
/// Nothing is stored and no row is written.
///
/// # Errors
/// [`ApiError::Validation`] under `url` for an address that is not http(s), a
/// fetch the guard refuses, an answer that is not 2xx, or a body that breaks
/// off.
pub async fn probe(pool: &DbPool, url: &str) -> Result<Probe, ApiError> {
    let policy = settings::address_policy(pool)
        .await
        .map_err(ApiError::internal)?;
    let http = HttpClient::new()
        .with_address_policy(policy)
        .with_timeout(PROBE_TIMEOUT);
    let url = url.trim().to_string();
    tokio::task::spawn_blocking(move || sniff(&http, &url))
        .await
        .map_err(|error| ApiError::internal(format!("the link probe did not finish: {error}")))?
}

/// The probe's blocking half: one fetch, or one oEmbed fetch for a YouTube
/// link, and no write.
fn sniff(http: &HttpClient, raw: &str) -> Result<Probe, ApiError> {
    let Some(uri) = HttpUri::parse(raw) else {
        return Err(invalid(raw));
    };
    if youtube::video_id(uri.raw()).is_ok() {
        return Ok(Probe {
            readers: vec!["youtube", "oembed", "url"],
            suggested: "youtube",
            media_type: None,
            size: None,
            title: oembed_title(http, uri.raw()),
            image: None,
            canvases: None,
        });
    }
    let mut response = http
        .fetch_stream(uri.raw())
        .map_err(|error| unreadable(raw, &error.message))?;
    if !response.is_ok() {
        return Err(status(raw, response.status, &response.reason));
    }
    let declared = response.content_type.clone();
    let size = response.content_length;
    let mut head = Vec::new();
    (&mut response)
        .take(PROBE_HEAD as u64)
        .read_to_end(&mut head)
        .map_err(|error| unreadable(raw, &error.to_string()))?;
    let whole = size.is_none_or(|length| length <= head.len() as u64);
    let extension = extension_from_source_name(uri.path());
    let sniffed = detected_media_type_of(&head, extension.as_deref());
    let media_type = declared
        .as_deref()
        .map(without_parameters)
        .filter(|kind| !kind.is_empty())
        .map_or(sniffed.clone(), str::to_string);
    let oembed = oembed_applies(uri.raw(), &head);
    if whole
        && let Ok(value) = serde_json::from_slice::<Value>(&head)
        && value.is_object()
    {
        let presentation = omeka_media::data::IiifPresentationData {
            value: value.clone(),
        };
        if let Some(version) = IiifPresentation.version(&presentation) {
            let mut readers = vec!["iiif_presentation"];
            if oembed {
                readers.push("oembed");
            }
            readers.push("url");
            return Ok(Probe {
                readers,
                suggested: "iiif_presentation",
                media_type: Some(media_type),
                size,
                title: presentation.label(),
                image: None,
                canvases: presentation.canvas_count(version),
            });
        }
        let image = omeka_media::data::IiifImageData { value };
        if Iiif::default().validate(&image) {
            return Ok(Probe {
                readers: vec!["iiif", "url"],
                suggested: "iiif",
                media_type: Some(media_type),
                size,
                title: None,
                image: json_size(&image.value),
                canvases: None,
            });
        }
    }
    let mut readers = Vec::new();
    if oembed {
        readers.push("oembed");
    }
    readers.push("url");
    let suggested = readers[0];
    let title = if media_type.starts_with("text/html") {
        let oembed_title = oembed.then(|| oembed_title(http, uri.raw())).flatten();
        oembed_title.or_else(|| html_title(&head))
    } else {
        None
    };
    let image = if media_type.starts_with("image/") {
        head_size(&head)
    } else {
        None
    };
    Ok(Probe {
        readers,
        suggested,
        media_type: Some(media_type),
        size,
        title,
        image,
        canvases: None,
    })
}

/// The oEmbed response's title, when the allow list or the page names an
/// endpoint; `None` otherwise or when the fetch fails. A probe reports a
/// missing title; it does not fail over one.
fn oembed_title(http: &HttpClient, url: &str) -> Option<String> {
    let oembed = Oembed::new(OembedAllowList::shipped());
    let mut errors = omeka_media::ingest::ErrorStore::default();
    oembed
        .get_oembed(http, url, &mut errors, "o:source")
        .and_then(|data| data.title())
}

/// Whether the oEmbed ingester has an endpoint for `url`: the allow list names
/// the host, or the page's head advertises `application/json+oembed`.
fn oembed_applies(url: &str, head: &[u8]) -> bool {
    let known = OembedAllowList::shipped().endpoint_for(url).is_some();
    const MARKER: &[u8] = b"application/json+oembed";
    let discovered = head
        .windows(MARKER.len())
        .any(|window| window.eq_ignore_ascii_case(MARKER));
    known || discovered
}

/// The `Content-Type` without its parameters.
fn without_parameters(value: &str) -> &str {
    value.split(';').next().unwrap_or(value).trim()
}

/// The pixel size of an image body, read from its header bytes.
fn head_size(head: &[u8]) -> Option<ImageSize> {
    let mut cursor = std::io::Cursor::new(head);
    let metadata = image_metadata(&mut cursor);
    let width = metadata.get("Width")?.values.first()?.parse().ok()?;
    let height = metadata.get("Height")?.values.first()?.parse().ok()?;
    Some(ImageSize {
        w: width,
        h: height,
    })
}

/// The width and height a IIIF image information document declares.
fn json_size(value: &Value) -> Option<ImageSize> {
    let w = u32::try_from(value.get("width")?.as_u64()?).ok()?;
    let h = u32::try_from(value.get("height")?.as_u64()?).ok()?;
    (w > 0 && h > 0).then_some(ImageSize { w, h })
}

/// The text of the first `<title>`, with whitespace collapsed and the common
/// named entities read. A page's title is a label, not markup.
fn html_title(head: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(head);
    let lower = text.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let open = lower[start..].find('>')? + start + 1;
    let end = lower[open..].find("</title")? + open;
    let raw = text.get(open..end)?.trim();
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let decoded = decode_entities(&collapsed);
    (!decoded.is_empty()).then(|| decoded.chars().take(300).collect())
}

/// The five named entities a title usually carries.
fn decode_entities(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}

fn invalid(url: &str) -> ApiError {
    validation(messages::media_link_invalid(url))
}

fn unreadable(url: &str, reason: &str) -> ApiError {
    validation(messages::media_link_unreadable(url, reason))
}

fn status(url: &str, status: u16, reason: &str) -> ApiError {
    validation(messages::media_link_status(url, status, reason))
}

fn validation(message: String) -> ApiError {
    let mut errors = ValidationErrors::new();
    errors.insert("url".to_string(), vec![message]);
    ApiError::Validation(errors)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;

    /// The readers a panel shows for a plain file and for a YouTube link, and
    /// the title reader, need no network.
    #[test]
    fn a_page_title_reads_its_text_and_entities() {
        let head = b"<html><head><title>\n A &amp; B  photo </title></head></html>";
        assert_eq!(html_title(head).as_deref(), Some("A & B photo"));
        assert_eq!(html_title(b"no title here"), None);
    }

    #[test]
    fn an_oembed_endpoint_is_offered_only_when_one_exists() {
        assert!(oembed_applies("https://www.youtube.com/watch?v=x", b""));
        assert!(!oembed_applies("https://example.org/a.jpg", b""));
        assert!(oembed_applies(
            "https://example.org/page",
            br#"<link rel="alternate" type="application/json+oembed">"#
        ));
    }

    #[test]
    fn a_manifests_canvases_are_named_by_version_three_items() {
        let manifest = omeka_media::data::IiifPresentationData {
            value: serde_json::json!({
                "@context": "http://iiif.io/api/presentation/3/context.json",
                "type": "Manifest",
                "label": {"en": ["A book"]},
                "items": [{"id": "c1"}, {"id": "c2"}]
            }),
        };
        assert_eq!(manifest.canvas_count(3), Some(2));
        assert_eq!(manifest.label().as_deref(), Some("A book"));
    }
}
