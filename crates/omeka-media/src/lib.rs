//! Media ingesters, renderers, file renderers and the file store.
//!
//! The crate ports the upstream media subsystem:
//!
//! - [`store`]: the file store and the storage path rule
//!   (`File/Store/Local.php`, `File/TempFile.php`).
//! - [`media_type`]: the content-based media type read
//!   (`File/TempFile::getMediaType`), a stand-in for PHP's `finfo` built on
//!   the `infer` crate and checked against libmagic.
//! - [`validation`]: the media type and extension allow lists
//!   (`File/Validator.php`) every stored original is checked against.
//! - [`ingest`]: one module per ingester under `Media/Ingester/`, plus the
//!   ingester and mutable-ingester traits and a registry mirroring
//!   `Media/Ingester/Manager.php`.
//! - [`render`]: one module per renderer under `Media/Renderer/`, plus the
//!   renderer trait and a registry mirroring `Media/Renderer/Manager.php`.
//! - [`file_render`]: one module per file renderer under `Media/FileRenderer/`,
//!   plus the file renderer trait, a registry mirroring
//!   `Media/FileRenderer/Manager.php`, and the three thumbnail types.
//! - [`thumbnailer`]: the image thumbnailer (`File/Thumbnailer/Gd.php`), which
//!   decodes gif, jpeg, png, tiff and webp and stores the three derivatives.
//! - [`image_metadata`]: the EXIF tags and pixel size of an image, for the
//!   archive import's tag preview.
//! - [`data`]: the typed `media.data` payloads.
//! - [`oembed`]: the oEmbed service (`Stdlib/Oembed.php`).
//!
//! # The one network seam
//!
//! `Url`, `Youtube`, `IIIF`, `IiifPresentation` and `OEmbed` fetch over HTTP
//! upstream. The port routes every fetch through [`http::HttpFetch`]. In
//! production that is [`HttpClient`], the blocking rustls client in
//! [`http_client`]; in tests it is [`testing::FakeHttp`], so no test touches the
//! network (DESIGN.md "Exclusions: live external services").
//!
//! # Boundaries
//!
//! `media.data` JSON, request bodies and options parse at the edge into
//! [`data::MediaData`], [`ingest::IngestRequest`] and
//! [`render::RenderOptions`]. Inside, every type is trusted and every renderer
//! is a pure function from a [`render::MediaView`] to an HTML fragment.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod data;
pub mod file_render;
pub mod http;
pub mod http_client;
pub mod image_metadata;
pub mod ingest;
pub mod media_type;
pub mod oembed;
pub mod render;
pub mod store;
pub mod testing;
pub mod text;
pub mod thumbnailer;
pub mod uri;
pub mod validation;

pub use file_render::{FileRendererRegistry, ThumbnailType};
pub use http_client::HttpClient;
pub use image_metadata::image_metadata;
pub use ingest::IngesterRegistry;
pub use render::RendererRegistry;
pub use thumbnailer::{ImageThumbnailer, ThumbnailOutcome, ThumbnailSkip, generate_thumbnails};
