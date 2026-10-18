//! Test doubles and builders.
//!
//! The media crate reaches the network, the filesystem and an HTML purifier
//! only through traits. This module holds the fakes a test uses in their place,
//! so no test touches the network and every test gets a temporary store
//! (AGENTS.md "no test may touch the network").
//!
//! The fakes are public because integration tests under `tests/` use them too.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::NaiveDateTime;
use omeka_core::ids::{ItemId, MediaId};
use omeka_core::resource::{Media, Timestamp};
use serde_json::Value;

use crate::data::MediaData;
use crate::file_render::ThumbnailType;
use crate::http::{FetchError, HttpFetch, HttpResponse};
use crate::ingest::{IngestEnv, Purifier};
use crate::oembed::{Oembed, OembedAllowList};
use crate::render::{MediaView, RenderEnv};
use crate::store::{FileStore, Thumbnailer};

/// A network seam that serves configured routes and records every call.
#[derive(Debug, Default)]
pub struct FakeHttp {
    routes: BTreeMap<String, HttpResponse>,
    calls: RefCell<Vec<String>>,
}

impl FakeHttp {
    /// A fake with no routes; every fetch fails.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a route with an explicit status and reason.
    #[must_use]
    pub fn with_response(
        mut self,
        url: &str,
        status: u16,
        reason: &str,
        body: impl Into<Vec<u8>>,
    ) -> Self {
        self.routes
            .insert(url.to_string(), HttpResponse::new(status, reason, body));
        self
    }

    /// Add a `200 OK` route with a raw body.
    #[must_use]
    pub fn with_body(self, url: &str, body: impl Into<Vec<u8>>) -> Self {
        self.with_response(url, 200, "OK", body)
    }

    /// Add a `200 OK` route with a JSON body.
    #[must_use]
    pub fn with_json(self, url: &str, value: Value) -> Self {
        self.with_body(url, value.to_string().into_bytes())
    }

    /// The URLs fetched so far, in order.
    #[must_use]
    pub fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl HttpFetch for FakeHttp {
    fn fetch(&self, url: &str) -> Result<HttpResponse, FetchError> {
        self.calls.borrow_mut().push(url.to_string());
        match self.routes.get(url) {
            Some(response) => Ok(response.clone()),
            None => Err(FetchError::new(format!("no fake route for {url}"))),
        }
    }
}

/// A purifier that returns its input, standing in for `HtmlPurifier`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PassthroughPurifier;

impl Purifier for PassthroughPurifier {
    fn purify(&self, html: &str) -> String {
        html.to_string()
    }
}

/// A purifier that replaces one substring, so a test can observe that the
/// ingester called it.
#[derive(Debug, Clone, Copy)]
pub struct ReplacePurifier {
    /// The substring to replace.
    pub from: &'static str,
    /// The replacement.
    pub to: &'static str,
}

impl Purifier for ReplacePurifier {
    fn purify(&self, html: &str) -> String {
        html.replace(self.from, self.to)
    }
}

/// A thumbnailer that returns fixed bytes for a fixed set of types.
#[derive(Debug, Clone)]
pub struct FakeThumbnailer {
    /// The types to produce, in order.
    pub types: Vec<ThumbnailType>,
    /// The bytes to produce for each type.
    pub bytes: Vec<u8>,
}

impl Default for FakeThumbnailer {
    fn default() -> Self {
        Self {
            types: ThumbnailType::ALL.to_vec(),
            bytes: b"jpeg".to_vec(),
        }
    }
}

impl Thumbnailer for FakeThumbnailer {
    fn thumbnails(
        &self,
        _original: &[u8],
        _media_type: Option<&str>,
    ) -> Vec<(ThumbnailType, Vec<u8>)> {
        self.types
            .iter()
            .map(|kind| (*kind, self.bytes.clone()))
            .collect()
    }
}

/// A temporary store plus the fakes an ingester needs.
///
/// The store lives under the system temporary directory and is removed when the
/// harness drops.
pub struct Harness {
    /// The temporary file store.
    pub store: FileStore,
    /// The fake network.
    pub http: FakeHttp,
    /// The oEmbed service, using the shipped allowlist.
    pub oembed: Oembed,
    purifier: Box<dyn Purifier>,
    thumbnailer: FakeThumbnailer,
    root: PathBuf,
}

impl Harness {
    /// Build a harness with a fresh temporary store tagged by `tag`.
    #[must_use]
    pub fn new(tag: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("omeka-media-{tag}-{}-{unique}", std::process::id()));
        Self {
            store: FileStore::new(&root, "/files"),
            http: FakeHttp::new(),
            oembed: Oembed::new(OembedAllowList::shipped()),
            purifier: Box::new(PassthroughPurifier),
            thumbnailer: FakeThumbnailer::default(),
            root,
        }
    }

    /// Add a route to the fake network.
    #[must_use]
    pub fn with_response(
        mut self,
        url: &str,
        status: u16,
        reason: &str,
        body: impl Into<Vec<u8>>,
    ) -> Self {
        let http = std::mem::take(&mut self.http);
        self.http = http.with_response(url, status, reason, body);
        self
    }

    /// Add a `200 OK` body route.
    #[must_use]
    pub fn with_body(mut self, url: &str, body: impl Into<Vec<u8>>) -> Self {
        let http = std::mem::take(&mut self.http);
        self.http = http.with_body(url, body);
        self
    }

    /// Add a `200 OK` JSON route.
    #[must_use]
    pub fn with_json(mut self, url: &str, value: Value) -> Self {
        let http = std::mem::take(&mut self.http);
        self.http = http.with_json(url, value);
        self
    }

    /// Replace the purifier.
    #[must_use]
    pub fn with_purifier(mut self, purifier: impl Purifier + 'static) -> Self {
        self.purifier = Box::new(purifier);
        self
    }

    /// Replace the thumbnail types the fake thumbnailer produces.
    #[must_use]
    pub fn with_thumbnail_types(mut self, types: Vec<ThumbnailType>) -> Self {
        self.thumbnailer.types = types;
        self
    }

    /// Borrow the pieces as an [`IngestEnv`].
    #[must_use]
    pub fn env(&self) -> IngestEnv<'_> {
        IngestEnv {
            store: &self.store,
            http: &self.http,
            purifier: self.purifier.as_ref(),
            thumbnailer: &self.thumbnailer,
            oembed: &self.oembed,
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A media row with upstream's non-null column defaults.
#[must_use]
pub fn media_row(id: u64, ingester: &str, renderer: &str) -> Media {
    Media::new(
        MediaId::new(id),
        ItemId::new(id),
        ingester.to_string(),
        renderer.to_string(),
        Timestamp::from_naive(NaiveDateTime::default()),
    )
}

/// A media view for a row with no payload.
#[must_use]
pub fn media_view() -> MediaView {
    media_view_from(&media_row(1, "upload", "file"))
}

/// A media view for a given row.
#[must_use]
pub fn media_view_from(media: &Media) -> MediaView {
    MediaView::new(
        media.clone(),
        MediaData::None,
        RenderEnv::default(),
        format!("/item/{}", media.item),
        format!("/media/{}", media.core.id),
    )
}

/// A media view for a fresh row carrying a payload.
#[must_use]
pub fn media_view_with_data(data: MediaData) -> MediaView {
    MediaView::new(
        media_row(1, "upload", "file"),
        data,
        RenderEnv::default(),
        "/item/1",
        "/media/1",
    )
}

/// A media view for a fresh row with a payload and a custom environment.
#[must_use]
pub fn media_view_with_data_and_env(data: MediaData, env: RenderEnv) -> MediaView {
    MediaView::new(
        media_row(1, "upload", "file"),
        data,
        env,
        "/item/1",
        "/media/1",
    )
}
