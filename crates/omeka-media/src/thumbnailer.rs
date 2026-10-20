//! The image thumbnailer (`File/Thumbnailer/Gd.php`).
//!
//! Upstream makes three derivatives per media file, one per entry of
//! `thumbnails.types` (`application/config/module.config.php:222`): `large`
//! with constraint 800, `medium` with 400, and `square` with 200 and gravity
//! `center`. Each is stored as a JPEG under its own prefix, so the file is
//! `files/<type>/<storage_id>.jpg` (`File/TempFile.php:280`).
//!
//! The port decodes and resizes with the `image` crate and keeps GD's two
//! strategies. [`ImageThumbnailer::derivative_bytes`] is the pure transform
//! from source bytes to the three derivative byte strings;
//! [`generate_thumbnails`] writes them through [`crate::store::Store`] and
//! is idempotent.
//!
//! The formats that reach the decoder are the four GD decodes (gif, jpeg, png,
//! webp), tiff and JPEG 2000. TIFF and JPEG 2000 are deliberate additions for
//! archive masters: GD has neither decoder, so upstream makes no thumbnail for
//! a `.tif` or a `.jp2` unless ImageMagick is configured. Anything else is a
//! typed [`ThumbnailSkip`], never a panic.
//!
//! An original over 64 MB is never held whole: [`ImageThumbnailer::
//! derivatives_from_reader`] makes a reduced copy of a TIFF or a JPEG with
//! bounded memory (`large`), then the derivatives from the copy.

mod jp2;
mod large;

use std::io::{Read, Seek};

use image::DynamicImage;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use thiserror::Error;

use crate::file_render::ThumbnailType;
use crate::store::{Store, StoreError, StorePrefix, Thumbnailer, storage_path};

/// The source media types the thumbnailer decodes: the four GD decodes
/// (`Gd::setSource`) plus `image/tiff` and `image/jp2`, which GD cannot read
/// (see the module docs). Any other declared type raises
/// `CannotCreateThumbnailException` upstream.
pub const SOURCE_MEDIA_TYPES: [&str; 6] = [
    "image/gif",
    "image/jpeg",
    "image/jp2",
    "image/png",
    "image/tiff",
    "image/webp",
];

/// The source media types [`ImageThumbnailer::derivatives_from_reader`]
/// reads with bounded memory.
pub const LARGE_SOURCE_MEDIA_TYPES: [&str; 2] = ["image/jpeg", "image/tiff"];

/// Why no thumbnail was made. Upstream throws
/// `CannotCreateThumbnailException` and `TempFile::storeThumbnails` turns it
/// into `false`; the port keeps the reason.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ThumbnailSkip {
    /// The declared media type is one GD does not decode.
    #[error("cannot create a thumbnail for media type \"{0}\"")]
    UnsupportedType(String),
    /// The source is a listed type but its bytes did not decode.
    #[error("the source did not decode as an image: {0}")]
    Decode(String),
    /// A resized image did not encode as JPEG.
    #[error("the thumbnail did not encode as JPEG: {0}")]
    Encode(String),
}

/// What [`generate_thumbnails`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThumbnailOutcome {
    /// The three files were already present; nothing was written.
    Existing,
    /// The three files were written.
    Generated,
    /// No thumbnails were made.
    Skipped(ThumbnailSkip),
}

/// The image thumbnailer: a pure transform from source bytes to derivative
/// bytes.
#[derive(Debug, Clone, Copy, Default)]
pub struct ImageThumbnailer;

impl ImageThumbnailer {
    /// A stateless thumbnailer.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// The three derivatives, one per [`ThumbnailType::ALL`] entry.
    ///
    /// # Errors
    /// A [`ThumbnailSkip`] when the media type is not one GD decodes, when the
    /// bytes do not decode as an image, or when a derivative does not encode.
    pub fn derivative_bytes(
        &self,
        original: &[u8],
        media_type: Option<&str>,
    ) -> Result<Vec<(ThumbnailType, Vec<u8>)>, ThumbnailSkip> {
        let media_type = media_type.unwrap_or_default();
        if !SOURCE_MEDIA_TYPES.contains(&media_type) {
            return Err(ThumbnailSkip::UnsupportedType(media_type.to_string()));
        }
        let source = if media_type == "image/jp2" {
            jp2::decode(original)?
        } else {
            image::load_from_memory(original)
                .map_err(|error| ThumbnailSkip::Decode(error.to_string()))?
        };
        derivatives(&source)
    }

    /// The three derivatives of an original read from `reader` and never
    /// held whole: a TIFF chunk by chunk, a JPEG at a reduced IDCT scale.
    /// The memory is bounded by the reduced copy (at most 1600 pixels a
    /// side) and one TIFF chunk ([`large::MAX_CHUNK_BYTES`]).
    ///
    /// # Errors
    /// A [`ThumbnailSkip`] for a type not in [`LARGE_SOURCE_MEDIA_TYPES`], a
    /// layout the reduced path does not read (a TIFF strip over the chunk
    /// bound, separate colour planes, a palette; a large progressive JPEG),
    /// or bytes that do not decode.
    pub fn derivatives_from_reader<R: Read + Seek>(
        &self,
        reader: R,
        media_type: Option<&str>,
    ) -> Result<Vec<(ThumbnailType, Vec<u8>)>, ThumbnailSkip> {
        let source = match media_type.unwrap_or_default() {
            "image/tiff" => large::tiff(reader)?,
            "image/jpeg" => large::jpeg(reader)?,
            other => return Err(ThumbnailSkip::UnsupportedType(other.to_string())),
        };
        derivatives(&source)
    }
}

/// The three derivatives of a decoded image, one per [`ThumbnailType::ALL`].
fn derivatives(source: &DynamicImage) -> Result<Vec<(ThumbnailType, Vec<u8>)>, ThumbnailSkip> {
    if source.width() == 0 || source.height() == 0 {
        return Err(ThumbnailSkip::Decode("the image has no pixels".to_string()));
    }
    let mut derivatives = Vec::with_capacity(ThumbnailType::ALL.len());
    for kind in ThumbnailType::ALL {
        let derivative = match kind {
            ThumbnailType::Square => square(source, kind.constraint()),
            ThumbnailType::Large | ThumbnailType::Medium => scale_to_fit(source, kind.constraint()),
        };
        derivatives.push((kind, encode_jpeg(&derivative)?));
    }
    Ok(derivatives)
}

impl Thumbnailer for ImageThumbnailer {
    /// The ingest seam (`TempFile::storeThumbnails`): the three derivatives, or
    /// an empty list when GD could not make them, which upstream treats as a
    /// normal outcome.
    fn thumbnails(
        &self,
        original: &[u8],
        media_type: Option<&str>,
    ) -> Vec<(ThumbnailType, Vec<u8>)> {
        self.derivative_bytes(original, media_type)
            .unwrap_or_default()
    }
}

/// Generate and store the three thumbnails for one media file.
///
/// Idempotent. When all three files are already present the call writes nothing
/// and answers [`ThumbnailOutcome::Existing`]. Each file is written to a unique
/// sibling and renamed into place, so a crash between the two steps leaves a
/// temporary file, not a partial thumbnail; the next call finds the missing
/// target and writes it again.
///
/// # Errors
/// A [`StoreError`] when a thumbnail cannot be written. A source that is not an
/// image is not an error: it answers [`ThumbnailOutcome::Skipped`].
pub fn generate_thumbnails(
    store: &dyn Store,
    storage_id: &str,
    original: &[u8],
    media_type: Option<&str>,
) -> Result<ThumbnailOutcome, StoreError> {
    if thumbnails_stored(store, storage_id)? {
        return Ok(ThumbnailOutcome::Existing);
    }
    let derivatives = match ImageThumbnailer.derivative_bytes(original, media_type) {
        Ok(derivatives) => derivatives,
        Err(skip) => return Ok(ThumbnailOutcome::Skipped(skip)),
    };
    for (kind, bytes) in derivatives {
        store.put(&bytes, &thumbnail_path(storage_id, kind))?;
    }
    Ok(ThumbnailOutcome::Generated)
}

/// [`generate_thumbnails`] for an original read from `reader` and never held
/// whole ([`ImageThumbnailer::derivatives_from_reader`]).
///
/// # Errors
/// A [`StoreError`] when a thumbnail cannot be written.
pub fn generate_thumbnails_from_reader<R: Read + Seek>(
    store: &dyn Store,
    storage_id: &str,
    reader: R,
    media_type: Option<&str>,
) -> Result<ThumbnailOutcome, StoreError> {
    if thumbnails_stored(store, storage_id)? {
        return Ok(ThumbnailOutcome::Existing);
    }
    let derivatives = match ImageThumbnailer.derivatives_from_reader(reader, media_type) {
        Ok(derivatives) => derivatives,
        Err(skip) => return Ok(ThumbnailOutcome::Skipped(skip)),
    };
    for (kind, bytes) in derivatives {
        store.put(&bytes, &thumbnail_path(storage_id, kind))?;
    }
    Ok(ThumbnailOutcome::Generated)
}

/// Whether all three thumbnails of `storage_id` are in the store.
///
/// # Errors
/// A [`StoreError`] when the store cannot answer.
pub fn thumbnails_stored(store: &dyn Store, storage_id: &str) -> Result<bool, StoreError> {
    for kind in ThumbnailType::ALL {
        if !store.exists(&thumbnail_path(storage_id, kind))? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The store path for one thumbnail: `<type>/<storage_id>.jpg`
/// (`File/TempFile.php:280`).
#[must_use]
pub fn thumbnail_path(storage_id: &str, kind: ThumbnailType) -> String {
    storage_path(StorePrefix::Thumbnail(kind), storage_id, Some("jpg"))
}

/// `Gd::createDefault`: fit the longer side to the constraint, and leave an
/// image that already fits inside it at its own size.
fn scale_to_fit(source: &DynamicImage, constraint: u32) -> DynamicImage {
    let (width, height) = (source.width(), source.height());
    let (target_width, target_height) = if width < constraint && height < constraint {
        (width, height)
    } else if width > height {
        (constraint, scaled(height, constraint, width))
    } else if width < height {
        (scaled(width, constraint, height), constraint)
    } else {
        (constraint, constraint)
    };
    source.resize_exact(target_width, target_height, FilterType::Triangle)
}

/// PHP `round($value * $constraint / $limit)`. GD computes this in floating
/// point and can reach zero for a very long thin image, which no encoder
/// accepts; the port keeps one pixel.
fn scaled(value: u32, constraint: u32, limit: u32) -> u32 {
    let scaled = (f64::from(value) * f64::from(constraint) / f64::from(limit)).round();
    (scaled as u32).max(1)
}

/// `Gd::createSquare` with gravity `center`: crop the largest centred square,
/// then resize it to the constraint.
fn square(source: &DynamicImage, constraint: u32) -> DynamicImage {
    let (width, height) = (source.width(), source.height());
    let size = width.min(height);
    let x = (width - size) / 2;
    let y = (height - size) / 2;
    source
        .crop_imm(x, y, size, size)
        .resize_exact(constraint, constraint, FilterType::Triangle)
}

/// `Gd::createTempImage` plus `imagejpeg`: the source is flattened onto white
/// so a transparent pixel becomes white, not black, then encoded at PHP's
/// default JPEG quality.
fn encode_jpeg(image: &DynamicImage) -> Result<Vec<u8>, ThumbnailSkip> {
    let flattened = flatten_on_white(image);
    let mut bytes = Vec::new();
    JpegEncoder::new(&mut bytes)
        .encode_image(&flattened)
        .map_err(|error| ThumbnailSkip::Encode(error.to_string()))?;
    Ok(bytes)
}

/// Replace transparent pixels with white, as `Gd::createTempImage` fills the
/// destination before `imagecopyresampled` blends the source over it.
fn flatten_on_white(image: &DynamicImage) -> image::RgbImage {
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    image::RgbImage::from_fn(width, height, |x, y| {
        let [red, green, blue, alpha] = rgba.get_pixel(x, y).0;
        let alpha = u32::from(alpha);
        let over_white = |channel: u8| -> u8 {
            ((u32::from(channel) * alpha + 255 * (255 - alpha)) / 255) as u8
        };
        image::Rgb([over_white(red), over_white(green), over_white(blue)])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A landscape RGB image of known size, full of one colour per pixel band.
    fn image_bytes(width: u32, height: u32) -> Vec<u8> {
        let image = image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
        });
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(image)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .expect("encode png");
        bytes
    }

    fn dimensions(bytes: &[u8]) -> (u32, u32) {
        let image = image::load_from_memory(bytes).expect("decode");
        (image.width(), image.height())
    }

    /// `Gd::createDefault` for a landscape image: the long side meets the
    /// constraint and the short side scales with it.
    #[test]
    fn default_strategy_scales_the_long_side_to_the_constraint() {
        let bytes = image_bytes(1000, 500);
        let derivatives = ImageThumbnailer
            .derivative_bytes(&bytes, Some("image/png"))
            .expect("thumbnails");
        let by_type: std::collections::BTreeMap<_, _> = derivatives
            .iter()
            .map(|(kind, bytes)| (*kind, dimensions(bytes)))
            .collect();
        assert_eq!(by_type[&ThumbnailType::Large], (800, 400));
        assert_eq!(by_type[&ThumbnailType::Medium], (400, 200));
    }

    /// `Gd::createDefault` leaves an image smaller than the constraint alone;
    /// `Gd::createSquare` still crops it to a square.
    #[test]
    fn a_small_image_is_not_enlarged_and_the_square_is_cropped() {
        let bytes = image_bytes(100, 60);
        let derivatives = ImageThumbnailer
            .derivative_bytes(&bytes, Some("image/png"))
            .expect("thumbnails");
        let by_type: std::collections::BTreeMap<_, _> = derivatives
            .iter()
            .map(|(kind, bytes)| (*kind, dimensions(bytes)))
            .collect();
        assert_eq!(by_type[&ThumbnailType::Large], (100, 60));
        assert_eq!(by_type[&ThumbnailType::Medium], (100, 60));
        assert_eq!(by_type[&ThumbnailType::Square], (200, 200));
    }

    /// `Gd::createSquare` on a landscape image crops a centred square before
    /// resizing, so the derivative is square.
    #[test]
    fn square_strategy_crops_the_centre() {
        let bytes = image_bytes(1000, 500);
        let derivatives = ImageThumbnailer
            .derivative_bytes(&bytes, Some("image/png"))
            .expect("thumbnails");
        let square = derivatives
            .iter()
            .find(|(kind, _)| *kind == ThumbnailType::Square)
            .map(|(_, bytes)| dimensions(bytes))
            .expect("square derivative");
        assert_eq!(square, (200, 200));
    }

    /// The thumbnailer accepts gif, jpeg, png, tiff and webp, and rejects every other
    /// declared type with a typed skip.
    #[test]
    fn unsupported_media_types_are_a_typed_skip() {
        let bytes = image_bytes(10, 10);
        assert_eq!(
            ImageThumbnailer.derivative_bytes(&bytes, Some("application/pdf")),
            Err(ThumbnailSkip::UnsupportedType(
                "application/pdf".to_string()
            ))
        );
        assert_eq!(
            ImageThumbnailer.derivative_bytes(&bytes, None),
            Err(ThumbnailSkip::UnsupportedType(String::new()))
        );
    }

    /// A listed image type whose bytes are not an image is a typed skip, not a
    /// panic.
    #[test]
    fn undecodable_bytes_are_a_typed_skip() {
        let result = ImageThumbnailer.derivative_bytes(b"not an image", Some("image/jpeg"));
        assert!(
            matches!(result, Err(ThumbnailSkip::Decode(_))),
            "{result:?}"
        );
    }

    /// `Gd::createTempImage`: a transparent pixel becomes white, not black.
    #[test]
    fn transparent_pixels_are_flattened_onto_white() {
        let image = image::RgbaImage::from_pixel(10, 10, image::Rgba([0, 0, 0, 0]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .expect("encode png");
        let derivatives = ImageThumbnailer
            .derivative_bytes(&bytes, Some("image/png"))
            .expect("thumbnails");
        let large = derivatives
            .iter()
            .find(|(kind, _)| *kind == ThumbnailType::Large)
            .map(|(_, bytes)| bytes)
            .expect("large derivative");
        let pixel = image::load_from_memory(large)
            .expect("decode")
            .to_rgb8()
            .get_pixel(5, 5)
            .0;
        assert!(pixel.iter().all(|channel| *channel >= 250), "{pixel:?}");
    }

    /// The ingest seam answers the three derivatives for an image and an empty
    /// list otherwise.
    #[test]
    fn the_store_seam_returns_three_derivatives_or_nothing() {
        let bytes = image_bytes(40, 20);
        let derivatives = ImageThumbnailer.thumbnails(&bytes, Some("image/png"));
        assert_eq!(
            derivatives
                .iter()
                .map(|(kind, _)| *kind)
                .collect::<Vec<_>>(),
            ThumbnailType::ALL
        );
        assert!(
            ImageThumbnailer
                .thumbnails(b"nope", Some("image/png"))
                .is_empty()
        );
        assert!(
            ImageThumbnailer
                .thumbnails(&bytes, Some("image/bmp"))
                .is_empty()
        );
    }

    /// A TIFF original gets the three JPEG derivatives like any other source.
    #[test]
    fn a_tiff_original_gets_three_thumbnails() {
        let image = image::RgbImage::from_fn(1000, 500, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 40])
        });
        let mut tiff = Vec::new();
        image::DynamicImage::ImageRgb8(image)
            .write_to(
                &mut std::io::Cursor::new(&mut tiff),
                image::ImageFormat::Tiff,
            )
            .expect("encode tiff");
        assert_eq!(crate::media_type::detected_media_type(&tiff), "image/tiff");
        let derivatives = ImageThumbnailer
            .derivative_bytes(&tiff, Some("image/tiff"))
            .expect("thumbnails");
        let sizes: Vec<_> = derivatives
            .iter()
            .map(|(kind, bytes)| (*kind, dimensions(bytes)))
            .collect();
        assert_eq!(
            sizes,
            [
                (ThumbnailType::Large, (800, 400)),
                (ThumbnailType::Medium, (400, 200)),
                (ThumbnailType::Square, (200, 200)),
            ]
        );
        assert!(
            derivatives
                .iter()
                .all(|(_, b)| b.starts_with(&[0xFF, 0xD8]))
        );
    }
}
