//! Media ingesters, renderers, file renderers and the file store.
//!
//! The crate ports the upstream media subsystem:
//!
//! - [`store`]: the [`store::Store`] trait, its local implementation and the
//!   storage path rule (`File/Store/Local.php`, `File/TempFile.php`).
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
//! - [`derivative`]: the still one stored original makes and the three
//!   thumbnails from it, which the background worker runs.
//! - [`transcode`]: the playable copy one stored original makes, an H.264/AAC
//!   MP4 or an AAC M4A, on its own thread with progress and a stop.
//! - [`thumbnailer`]: the image thumbnailer (`File/Thumbnailer/Gd.php`), which
//!   decodes gif, jpeg, png, tiff, webp, JPEG 2000 and SVG and stores the three
//!   derivatives.
//! - [`svg`]: the SVG filter an upload runs before it is stored, and the
//!   raster its thumbnail is made from.
//! - [`iiif_image`]: the IIIF Image API at level 1 over a stored image.
//! - [`image_metadata`]: the EXIF, IPTC and XMP tags and pixel size of an
//!   image, for the archive import's tags.
//! - [`data`]: the typed `media.data` payloads.
//! - [`oembed`]: the oEmbed service (`Stdlib/Oembed.php`).
//! - [`kind`]: the per-kind table: what an annotation may point at, what the
//!   IIIF canvas paints, and the size and duration a canvas needs.
//! - [`access`]: the access-copy table: whether a current browser shows a stored
//!   original as it is, and when it does not, which copy and which optional
//!   tool make it. D3 and D4 run those copies; this module never changes the
//!   original.
//! - [`av_probe`]: `ffprobe`'s answer for an audio or video original, which is
//!   the duration and frame size a time-based canvas needs, and the container
//!   and codecs an access rule reads.
//! - [`tool`]: the external commands a still or an access copy needs (`ffmpeg`,
//!   `pdftoppm`, `ffprobe`, `exiftool`, libraw, `vips`, `gs`), optional at
//!   runtime and bounded in time and output.
//! - [`sample`]: the first and last megabyte of a stored original at their true
//!   offsets, so a header read never copies a large file whole.
//!
//! # The one network seam
//!
//! `Url`, `Youtube`, `IIIF`, `IiifPresentation` and `OEmbed` fetch over HTTP
//! upstream. The port routes every fetch through [`http::HttpFetch`]. In
//! production that is [`HttpClient`], the blocking rustls client in
//! [`http_client`], which admits every hop through the URL guard in
//! [`http_guard`]; in tests it is [`testing::FakeHttp`], so no test touches the
//! network (DESIGN.md "Exclusions: live external services").
//!
//! # Boundaries
//!
//! `media.data` JSON, request bodies and options parse at the edge into
//! [`data::MediaData`], [`ingest::IngestRequest`] and
//! [`render::RenderOptions`]. Inside, every type is trusted and every renderer
//! is a pure function from a [`render::MediaView`] to an HTML fragment.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod access;
pub mod av_probe;
pub mod data;
pub mod derivative;
pub mod file_render;
pub mod http;
pub mod http_client;
pub mod http_guard;
pub mod iiif_image;
pub mod image_metadata;
pub mod ingest;
pub mod kind;
pub mod media_type;
pub mod oembed;
pub mod render;
pub mod sample;
pub mod store;
pub mod svg;
pub mod testing;
pub mod text;
pub mod thumbnailer;
pub mod tool;
pub mod transcode;
pub mod uri;
pub mod validation;

pub use file_render::{FileRendererRegistry, ThumbnailType};
pub use http_client::HttpClient;
pub use image_metadata::image_metadata;
pub use ingest::IngesterRegistry;
pub use render::RendererRegistry;
pub use thumbnailer::{ImageThumbnailer, ThumbnailOutcome, ThumbnailSkip, generate_thumbnails};
