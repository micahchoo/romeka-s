//! [`FileUrls`]: the one place the API and the site turn a storage path into
//! the URL a client fetches, and the rule that picks the URL's [`Access`]
//! (docs/import-v2.md "Storage and file access").
//!
//! - An original is always signed. Upstream links every original in the clear
//!   (`/files/original/...`); here an original is fetched through a URL that
//!   expires, also for a public resource.
//! - A thumbnail is plain when its resource is public, otherwise signed.
//! - An asset is plain: upstream assets carry no visibility.
//!
//! On the local store both kinds are the same `/files/...` path, and Caddy's
//! `forward_auth` decides. A store that answers a path (it starts with `/`) is
//! joined onto the request origin; an object store answers whole URLs.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use url::Url;

use super::{Access, FileStore, Store};

/// What a downloaded original answers with: the media's stored type and the
/// name it is saved under.
///
/// An object store signs both into the URL (`response-content-type` and
/// `response-content-disposition`), so the bucket stops answering every
/// original as `binary/octet-stream`. The local origin sets the same two
/// headers itself, from its own lookup, so it ignores this.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Download {
    /// The media's stored `o:media_type`.
    pub media_type: Option<String>,
    /// The name a "save as" offers, from the media's `o:source`.
    pub filename: Option<String>,
}

impl Download {
    /// The download facts of a media original. The name is the source's own
    /// last path segment when it has one, else the stored file name.
    #[must_use]
    pub fn of(media_type: Option<&str>, source: Option<&str>, stored_name: &str) -> Self {
        Self {
            media_type: media_type.map(str::to_string),
            filename: Some(download_name(source, stored_name)),
        }
    }

    /// The `Content-Disposition` value for this download.
    #[must_use]
    pub fn disposition(&self) -> String {
        content_disposition(self.filename.as_deref().unwrap_or("download"))
    }

    /// The type the download answers with: the media's stored type.
    ///
    /// The ingest names a file the byte detector cannot tell from its
    /// container (`omeka_media::media_type::detected_media_type_of`), so the
    /// row is the one authority here. An SVG's stored type describes the
    /// filtered bytes the row stores.
    #[must_use]
    pub fn served_type(&self) -> Option<&str> {
        self.media_type.as_deref()
    }
}

/// The name a client saves an original as: the last path segment of the source
/// when the source names one, otherwise the stored file name.
///
/// A URL source is read as a URL, so a percent-encoded name arrives decoded.
#[must_use]
pub fn download_name(source: Option<&str>, stored_name: &str) -> String {
    let Some(source) = source.map(str::trim).filter(|source| !source.is_empty()) else {
        return stored_name.to_string();
    };
    if let Ok(url) = Url::parse(source)
        && let Some(last) = url
            .path_segments()
            .and_then(|mut segments| segments.next_back())
        && !last.is_empty()
    {
        return decode_percent(last);
    }
    let path = source.split(['?', '#']).next().unwrap_or_default();
    match path.rsplit('/').next().map(str::trim) {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => stored_name.to_string(),
    }
}

/// Percent-decode one path segment. A broken escape stays as it was written.
fn decode_percent(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let Some(hex) = text.get(index + 1..index + 3)
            && let Ok(byte) = u8::from_str_radix(hex, 16)
        {
            out.push(byte);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `inline` and the name to save an original under, encoded as RFC 6266 asks:
/// a quoted ASCII `filename` and, for a name that is not ASCII, a UTF-8
/// `filename*`. Control characters are dropped, because no header may carry
/// one.
#[must_use]
pub fn content_disposition(name: &str) -> String {
    // A source is arbitrary text, and a header is bounded: 255 characters is
    // the longest name a filesystem accepts anyway.
    let name: String = name.chars().filter(|c| !c.is_control()).take(255).collect();
    let name = name.trim();
    let name = if name.is_empty() { "download" } else { name };
    let fallback: String = name
        .chars()
        .map(|c| if c.is_ascii() { c } else { '_' })
        .collect();
    let quoted = fallback.replace('\\', "\\\\").replace('"', "\\\"");
    if name.is_ascii() {
        format!("inline; filename=\"{quoted}\"")
    } else {
        format!(
            "inline; filename=\"{quoted}\"; filename*=UTF-8''{}",
            encode_attr(name)
        )
    }
}

/// RFC 5987 `attr-char`, everything else percent-encoded from the UTF-8 bytes.
fn encode_attr(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"!#$&+-.^_`|~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// What a stored file is to the resource that shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// `original/<name>`, the ingested file.
    Original,
    /// A thumbnail of a media whose resource is public or not. A media is
    /// public when it and its item are.
    Thumbnail {
        /// Whether anyone may see the resource.
        public: bool,
    },
    /// `asset/<name>`, a site logo or a custom resource thumbnail.
    Asset,
}

impl FileKind {
    /// The access a URL for this file needs.
    #[must_use]
    pub const fn access(self, ttl: Duration) -> Access {
        match self {
            Self::Thumbnail { public: true } | Self::Asset => Access::Public,
            Self::Original | Self::Thumbnail { public: false } => Access::Signed { ttl },
        }
    }
}

/// The process's store, and the origin its paths are joined onto.
#[derive(Clone)]
pub struct FileUrls {
    store: Arc<dyn Store>,
    origin: String,
}

impl FileUrls {
    /// URLs from `store`, with a path answer joined onto `origin` (such as
    /// `https://example.org`, no trailing slash).
    #[must_use]
    pub fn new(store: Arc<dyn Store>, origin: impl Into<String>) -> Self {
        Self {
            store,
            origin: origin.into(),
        }
    }

    /// The local store's `/files/<path>` URLs on `origin`, for a caller that
    /// holds no store.
    #[must_use]
    pub fn local(origin: impl Into<String>) -> Self {
        Self::new(Arc::new(FileStore::new("files", "/files")), origin)
    }

    /// The URL of the file at `storage_path`.
    #[must_use]
    pub fn url(&self, storage_path: &str, kind: FileKind) -> String {
        self.resolve(
            self.store
                .url(storage_path, kind.access(self.store.url_ttl())),
        )
    }

    /// The URL of an original, which a reader plays or saves. An object store
    /// signs the media's type and name into it; the local origin sets them from
    /// its own lookup.
    #[must_use]
    pub fn original(&self, storage_path: &str, download: &Download) -> String {
        self.resolve(self.store.download_url(
            storage_path,
            FileKind::Original.access(self.store.url_ttl()),
            download,
        ))
    }

    fn resolve(&self, url: String) -> String {
        if url.starts_with('/') {
            format!("{}{url}", self.origin)
        } else {
            url
        }
    }
}

impl fmt::Debug for FileUrls {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileUrls")
            .field("store", &self.store)
            .field("origin", &self.origin)
            .finish()
    }
}

/// Two handles are equal when they share the store and the origin.
impl PartialEq for FileUrls {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.store, &other.store) && self.origin == other.origin
    }
}

impl Eq for FileUrls {}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::store::{S3Config, S3Store};

    const TTL: Duration = Duration::from_secs(900);

    #[test]
    fn originals_are_signed_thumbnails_follow_the_resource_and_assets_are_plain() {
        let signed = Access::Signed { ttl: TTL };
        assert_eq!(FileKind::Original.access(TTL), signed);
        assert_eq!(FileKind::Thumbnail { public: false }.access(TTL), signed);
        assert_eq!(
            FileKind::Thumbnail { public: true }.access(TTL),
            Access::Public
        );
        assert_eq!(FileKind::Asset.access(TTL), Access::Public);
    }

    #[test]
    fn a_local_path_is_joined_onto_the_origin() {
        let urls = FileUrls::local("https://example.org");
        assert_eq!(
            urls.url("original/abc.jpg", FileKind::Original),
            "https://example.org/files/original/abc.jpg"
        );
        assert_eq!(
            urls.url("square/abc.jpg", FileKind::Thumbnail { public: true }),
            "https://example.org/files/square/abc.jpg"
        );
    }

    #[test]
    fn an_object_store_answers_whole_urls_with_its_own_ttl() {
        let store = S3Store::new(S3Config {
            endpoint: "http://127.0.0.1:9000".parse().unwrap(),
            bucket: "romeka".to_string(),
            region: "us-east-1".to_string(),
            access_key: "key".to_string(),
            secret_key: "secret".to_string(),
            public_base: Some("https://cdn.example.org".to_string()),
            url_ttl: Duration::from_secs(120),
        })
        .unwrap();
        let urls = FileUrls::new(Arc::new(store), "https://example.org");
        let original = urls.url("original/abc.jpg", FileKind::Original);
        assert!(
            original.starts_with("http://127.0.0.1:9000/romeka/original/abc.jpg?"),
            "{original}"
        );
        assert!(original.contains("X-Amz-Expires=120"), "{original}");
        let private = urls.url("square/abc.jpg", FileKind::Thumbnail { public: false });
        assert!(private.contains("X-Amz-Signature="), "{private}");
        assert_eq!(
            urls.url("square/abc.jpg", FileKind::Thumbnail { public: true }),
            "https://cdn.example.org/square/abc.jpg"
        );
        assert_eq!(
            urls.url("asset/logo.png", FileKind::Asset),
            "https://cdn.example.org/asset/logo.png"
        );
    }

    #[test]
    fn a_download_name_comes_from_the_sources_last_segment() {
        assert_eq!(
            download_name(Some("My Video.mp4"), "abc.mp4"),
            "My Video.mp4"
        );
        assert_eq!(
            download_name(Some("https://example.org/a/My%20Video.mp4?x=1"), "abc.webm"),
            "My Video.mp4"
        );
        assert_eq!(
            download_name(Some("/tmp/clip.webm"), "abc.webm"),
            "clip.webm"
        );
        assert_eq!(download_name(Some("   "), "abc.webm"), "abc.webm");
        assert_eq!(download_name(None, "abc.webm"), "abc.webm");
    }

    #[test]
    fn a_disposition_is_inline_with_rfc_6266_encoding() {
        assert_eq!(
            content_disposition("My Video.mp4"),
            "inline; filename=\"My Video.mp4\""
        );
        assert_eq!(
            content_disposition("say \"hi\".mp4"),
            "inline; filename=\"say \\\"hi\\\".mp4\""
        );
        assert_eq!(
            content_disposition("caf\u{e9}.mp4"),
            "inline; filename=\"caf_.mp4\"; filename*=UTF-8''caf%C3%A9.mp4"
        );
        assert_eq!(
            content_disposition("a\r\nb.mp4"),
            "inline; filename=\"ab.mp4\""
        );
        assert_eq!(content_disposition(""), "inline; filename=\"download\"");
        let long = content_disposition(&"a".repeat(300));
        assert!(long.contains(&"a".repeat(255)), "{}", long.len());
        assert!(
            !long.contains(&"a".repeat(256)),
            "truncated to a name a filesystem accepts"
        );
    }

    #[test]
    fn a_download_answers_with_the_stored_type() {
        let type_of = |stored: &str, name: &str| {
            Download::of(Some(stored), Some(name), name)
                .served_type()
                .map(str::to_string)
        };
        // The ingest names these from the source extension, so the row says
        // what the file is and the download does not second-guess it.
        assert_eq!(type_of("text/vtt", "talk.vtt"), Some("text/vtt".into()));
        assert_eq!(
            type_of("application/x-subrip", "talk.srt"),
            Some("application/x-subrip".into())
        );
        assert_eq!(
            type_of("model/gltf+json", "scene.gltf"),
            Some("model/gltf+json".into())
        );
        assert_eq!(
            type_of("model/gltf-binary", "scene.GLB"),
            Some("model/gltf-binary".into())
        );
        assert_eq!(type_of("video/ogg", "clip.ogv"), Some("video/ogg".into()));
        assert_eq!(
            type_of("video/x-matroska", "talk.mkv"),
            Some("video/x-matroska".into())
        );
        assert_eq!(
            type_of("text/plain", "notes.txt"),
            Some("text/plain".into())
        );
        assert_eq!(Download::default().served_type(), None);
    }

    #[test]
    fn a_signed_download_signs_the_type_and_the_name() {
        let store = S3Store::new(S3Config {
            endpoint: "http://127.0.0.1:9000".parse().unwrap(),
            bucket: "romeka".to_string(),
            region: "us-east-1".to_string(),
            access_key: "key".to_string(),
            secret_key: "secret".to_string(),
            public_base: None,
            url_ttl: TTL,
        })
        .unwrap();
        let urls = FileUrls::new(Arc::new(store), "https://example.org");
        let download = Download::of(Some("video/webm"), Some("A Talk.webm"), "abc.webm");
        let url = urls.original("original/abc.webm", &download);
        assert!(url.contains("response-content-type=video%2Fwebm"), "{url}");
        assert!(
            url.contains("response-content-disposition=inline%3B%20filename%3D%22A%20Talk.webm%22"),
            "{url}"
        );
        assert!(url.contains("X-Amz-Signature="), "{url}");
        let local = FileUrls::local("https://example.org").original("original/abc.webm", &download);
        assert_eq!(local, "https://example.org/files/original/abc.webm");
    }
}
