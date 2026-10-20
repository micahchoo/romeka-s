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

use super::{Access, FileStore, Store};

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
        let url = self
            .store
            .url(storage_path, kind.access(self.store.url_ttl()));
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
}
