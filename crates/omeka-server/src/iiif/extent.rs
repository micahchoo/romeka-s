//! The extents of stored media, which a canvas needs and `media.data` must not
//! hold: the upright pixel size of an image, and the duration and frame size of
//! an audio or video original.
//!
//! An image's size is read from its header through the store, upright as its
//! EXIF orientation says, and remembered by storage path for the life of the
//! process. A stored file never changes under its path, so nothing invalidates
//! an entry. A document the image crate cannot read (an SVG) is read by
//! [`omeka_media::svg`], which is the code the Image API renders it with, so a
//! canvas and its image are one space.
//!
//! An audio or video extent comes from `romeka_media_extent` (`MediaExtentRepo`).
//! A miss, or a row whose storage path no longer matches the media's, is probed
//! once with `ffprobe` and recorded. At most [`PROBES`] run at once, and a probe
//! copies the original to a temporary file first, because `ffprobe` opens a
//! file and the original may live in an object store.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use omeka_db::ids::MediaId;
use omeka_db::repos::{ExtentRow, MediaExtentRepo};
use omeka_db::rows::MediaRow;
use omeka_db::{DbPool, with_db};
use omeka_media::av_probe;
use omeka_media::file_render::ThumbnailType;
use omeka_media::iiif_image::source_size;
use omeka_media::image_metadata;
use omeka_media::kind::{Extent, KindInput, MediaKind};
use omeka_media::media_type::is_raw_media_type;
use omeka_media::store::{Store, StorePrefix, storage_path};
use omeka_media::tool::Tools;
use tokio::sync::Semaphore;

use super::manifest::Size;

/// How much of a file the first read takes. A JPEG's frame header follows its
/// metadata segments, which this covers for any file a camera writes.
const HEAD_BYTES: u64 = 256 * 1024;

/// How many `ffprobe` runs may be in flight at once.
const PROBES: usize = 4;

/// The extents of stored media, shared by every request.
pub(crate) struct Extents {
    /// Image sizes by storage path, read once.
    sizes: Mutex<HashMap<String, Size>>,
    pool: DbPool,
    store: Arc<dyn Store>,
    tools: Tools,
    probing: Semaphore,
}

impl Extents {
    pub(crate) fn new(pool: DbPool, store: Arc<dyn Store>) -> Self {
        Self {
            sizes: Mutex::new(HashMap::new()),
            pool,
            store,
            tools: Tools::on_path().clone(),
            probing: Semaphore::new(PROBES),
        }
    }

    /// The upright size of the stored image at `storage_path`, `None` when the
    /// file is missing or its header names no size.
    pub(crate) async fn image(&self, storage_path: &str) -> Option<Size> {
        if let Some(size) = self.remembered(storage_path) {
            return Some(size);
        }
        let store = Arc::clone(&self.store);
        let path = storage_path.to_string();
        let size = tokio::task::spawn_blocking(move || read_size(store.as_ref(), &path))
            .await
            .ok()
            .flatten();
        if let Some(size) = size
            && let Ok(mut known) = self.sizes.lock()
        {
            known.insert(storage_path.to_string(), size);
        }
        size
    }

    /// The duration and frame size of `media`'s audio or video original. An
    /// absent extent means the probe answered nothing, and the media gets no
    /// canvas.
    pub(crate) async fn av(
        &self,
        media: MediaId,
        original_path: &str,
        extension: Option<&str>,
    ) -> Extent {
        if let Some(recorded) = self.recorded(media, original_path).await {
            return recorded;
        }
        let Ok(_permit) = self.probing.acquire().await else {
            return Extent::default();
        };
        let store = Arc::clone(&self.store);
        let tools = self.tools.clone();
        let path = original_path.to_string();
        let extension = extension.map(str::to_string);
        let probed = tokio::task::spawn_blocking(move || {
            av_probe::probe_stored(store.as_ref(), &tools, &path, extension.as_deref())
        })
        .await
        .ok()
        .flatten();
        let Some(extent) = probed else {
            return Extent::default();
        };
        self.record(media, original_path, &extent).await;
        extent
    }

    /// The extent of one media row: the size of a local image, or an audio or
    /// video original's duration and frame size. Every other kind answers the
    /// default, because it has nothing this module can measure.
    ///
    /// The annotation service asks this once per media through `ExtentSource`.
    pub(crate) async fn of(&self, media: &MediaRow) -> Extent {
        match self.kind(media) {
            MediaKind::LocalImage => Extent {
                size: self.image(&display_path(media)).await,
                duration_ms: None,
            },
            MediaKind::Audio | MediaKind::Video => {
                self.av(
                    media.id(),
                    &original_path(media),
                    media.extension.as_deref(),
                )
                .await
            }
            _ => Extent::default(),
        }
    }

    fn kind(&self, media: &MediaRow) -> MediaKind {
        MediaKind::of(&KindInput {
            renderer: &media.renderer,
            ingester: &media.ingester,
            media_type: media.media_type.as_deref(),
            extension: media.extension.as_deref(),
            has_thumbnails: media.has_thumbnails,
        })
    }

    fn remembered(&self, storage_path: &str) -> Option<Size> {
        self.sizes.lock().ok()?.get(storage_path).copied()
    }

    /// The recorded extent, when it was taken from the original at this path.
    async fn recorded(&self, media: MediaId, original_path: &str) -> Option<Extent> {
        let row = with_db!(&self.pool, |db| MediaExtentRepo::new(db)
            .get(media, original_path)
            .await)
        .ok()
        .flatten()?;
        Some(Extent {
            size: row.width.zip(row.height),
            duration_ms: row.duration_ms,
        })
    }

    /// Record a probe's answer. A write that fails costs the next request one
    /// more probe, and never the answer the caller is waiting for.
    async fn record(&self, media: MediaId, original_path: &str, extent: &Extent) {
        let row = ExtentRow {
            duration_ms: extent.duration_ms,
            width: extent.size.map(|size| size.0),
            height: extent.size.map(|size| size.1),
        };
        if let Err(error) = with_db!(&self.pool, |db| MediaExtentRepo::new(db)
            .put(media, original_path, &row)
            .await)
        {
            tracing::warn!(%error, media = media.0, "the extent was not recorded");
        }
    }
}

impl omeka_api::annotation::ExtentSource for Extents {
    fn extent<'a>(
        &'a self,
        media: &'a MediaRow,
    ) -> Pin<Box<dyn Future<Output = Extent> + Send + 'a>> {
        Box::pin(self.of(media))
    }
}

/// The image the media page draws: the original, or for a camera RAW, whose
/// sensor data no decoder here reads, the preview-derived large derivative
/// (`iiif::image::StoredImage::of` makes the same choice for the manifest and
/// the Image API). Notes address the pixels the reader sees, so their canvas is
/// this image.
fn display_path(media: &MediaRow) -> String {
    if media.media_type.as_deref().is_some_and(is_raw_media_type) {
        return match media.storage_id.as_deref() {
            Some(storage_id) => storage_path(
                StorePrefix::Thumbnail(ThumbnailType::Large),
                storage_id,
                Some("jpg"),
            ),
            None => String::new(),
        };
    }
    original_path(media)
}

/// The storage path of a media row's original.
fn original_path(media: &MediaRow) -> String {
    match &media.storage_id {
        Some(storage_id) => storage_path(
            StorePrefix::Original,
            storage_id,
            media.extension.as_deref(),
        ),
        None => String::new(),
    }
}

fn read_size(store: &dyn Store, storage_path: &str) -> Option<Size> {
    let head = store.read_range(storage_path, 0..HEAD_BYTES).ok()?;
    size_of(&head).or_else(|| size_of(&store.read(storage_path).ok()?))
}

/// The upright size of an image header. The size the Image API renders from is
/// the size a canvas reports, so both read it here.
fn size_of(bytes: &[u8]) -> Option<Size> {
    source_size(bytes).or_else(|| {
        let tags = image_metadata(&mut std::io::Cursor::new(bytes));
        let width = tags.get("Width")?.values.first()?.parse().ok()?;
        let height = tags.get("Height")?.values.first()?.parse().ok()?;
        Some((width, height))
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::path::Path;

    use super::*;

    fn corpus(name: &str) -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/media-kinds")
            .join(name);
        std::fs::read(&path).unwrap_or_else(|error| panic!("{name}: {error}"))
    }

    /// image.jpg is 120 by 80 with EXIF Orientation 6: the canvas is the
    /// upright 80 by 120, the size its derived large image is.
    #[test]
    fn an_oriented_jpeg_is_upright() {
        assert_eq!(size_of(&corpus("image.jpg")), Some((80, 120)));
        assert_eq!(size_of(&corpus("image.png")), Some((120, 80)));
    }

    /// An SVG's size is the raster the thumbnailer draws: its own 120 by 80.
    #[test]
    fn an_svg_is_its_drawn_size() {
        assert_eq!(size_of(&corpus("image.svg")), Some((120, 80)));
        assert_eq!(size_of(&corpus("hostile.svg")), Some((120, 80)));
    }

    /// Bytes that are not an image at all name no size.
    #[test]
    fn other_bytes_name_no_size() {
        assert_eq!(size_of(b"not an image"), None);
        assert_eq!(size_of(&[]), None);
    }
}
