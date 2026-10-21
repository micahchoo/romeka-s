//! The IIIF Image API 3.0 at compliance level 1, over one stored image.
//!
//! A Romeka addition: upstream serves no Image API (the IIIF Server module
//! does). An image request is `{region}/{size}/{rotation}/{quality}.{format}`.
//! [`ImageRequest::parse`] is the parse boundary for those four path segments,
//! [`ImageRequest::plan`] resolves it against the image's pixel size and the
//! server's [`Limits`], and [`render`] does the pixel work. The parse and the
//! plan are pure, so every rule of the specification is a unit test here, and
//! the caller only reads bytes and writes the answer.
//!
//! What level 1 asks for, plus two level 2 sizes that cost nothing more:
//!
//! - regions `full`, `square` and `x,y,w,h` in pixels;
//! - sizes `max`, `w,`, `,h`, `w,h`, and the extras `pct:n` and `!w,h`;
//! - rotation `0` and quality `default` only;
//! - formats `jpg`, and `png` as an extra format.
//!
//! Upscaling (`^`) is not offered, so a size larger than the region is a 400,
//! as the specification requires of a server without `sizeUpscaling`.

use std::io::Cursor;

use image::DynamicImage;
use image::ImageDecoder as _;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::metadata::Orientation;
use serde_json::{Value, json};
use thiserror::Error;

/// The largest edge, in pixels, any answer may have. `info.json` publishes it
/// as `maxWidth` and `maxHeight`, so a client never asks for more.
pub const MAX_EDGE: u32 = 4096;

/// The tile edge `info.json` offers, and the scale factors a tiled client may
/// request at. A service offers factors down to the one where the whole image
/// fits in one tile.
const TILE: u32 = 512;
const SCALE_FACTORS: [u32; 7] = [1, 2, 4, 8, 16, 32, 64];

/// Why a request has no image. Every variant is the client's mistake, so the
/// caller answers 400.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ImageRequestError {
    /// A path segment is not one this server reads.
    #[error("the {segment} \"{value}\" is not supported")]
    Unsupported {
        /// `region`, `size`, `rotation`, `quality` or `format`.
        segment: &'static str,
        /// The segment as requested.
        value: String,
    },
    /// The region lies outside the image or has no area.
    #[error("the region lies outside the image")]
    EmptyRegion,
    /// The size is zero, larger than the region, or larger than [`MAX_EDGE`].
    #[error("the size {width}x{height} is not available")]
    SizeOutOfRange {
        /// The width the request resolves to.
        width: u32,
        /// The height the request resolves to.
        height: u32,
    },
}

/// The part of the image a request names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Region {
    Full,
    Square,
    Pixels { x: u32, y: u32, w: u32, h: u32 },
}

/// The size the region is scaled to.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Size {
    Max,
    Width(u32),
    Height(u32),
    Exact(u32, u32),
    Confined(u32, u32),
    Percent(f64),
}

/// The encoding of the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// `jpg`, `image/jpeg`.
    Jpg,
    /// `png`, `image/png`.
    Png,
}

impl ImageFormat {
    /// The media type the answer carries.
    #[must_use]
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Jpg => "image/jpeg",
            Self::Png => "image/png",
        }
    }
}

/// One parsed image request.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageRequest {
    region: Region,
    size: Size,
    /// The answer's encoding.
    pub format: ImageFormat,
}

/// The pixel work one request resolves to: the crop in source pixels and the
/// output size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    /// The crop's left edge.
    pub x: u32,
    /// The crop's top edge.
    pub y: u32,
    /// The crop's width.
    pub crop_width: u32,
    /// The crop's height.
    pub crop_height: u32,
    /// The answer's width.
    pub width: u32,
    /// The answer's height.
    pub height: u32,
}

impl ImageRequest {
    /// Parse the four path segments after the identifier.
    ///
    /// # Errors
    /// [`ImageRequestError::Unsupported`] for any segment this level does not
    /// read.
    pub fn parse(
        region: &str,
        size: &str,
        rotation: &str,
        file: &str,
    ) -> Result<Self, ImageRequestError> {
        let unsupported = |segment: &'static str, value: &str| ImageRequestError::Unsupported {
            segment,
            value: value.to_string(),
        };
        if rotation != "0" {
            return Err(unsupported("rotation", rotation));
        }
        let (quality, format) = file
            .rsplit_once('.')
            .ok_or_else(|| unsupported("format", file))?;
        if quality != "default" {
            return Err(unsupported("quality", quality));
        }
        let format = match format {
            "jpg" => ImageFormat::Jpg,
            "png" => ImageFormat::Png,
            other => return Err(unsupported("format", other)),
        };
        Ok(Self {
            region: parse_region(region).ok_or_else(|| unsupported("region", region))?,
            size: parse_size(size).ok_or_else(|| unsupported("size", size))?,
            format,
        })
    }

    /// Resolve the request against an image of `width` by `height` pixels.
    ///
    /// # Errors
    /// [`ImageRequestError::EmptyRegion`] for a region with no pixels in the
    /// image, and [`ImageRequestError::SizeOutOfRange`] for a size of zero, one
    /// larger than the region, or one past [`MAX_EDGE`].
    pub fn plan(&self, width: u32, height: u32) -> Result<Plan, ImageRequestError> {
        let (x, y, crop_width, crop_height) = match self.region {
            Region::Full => (0, 0, width, height),
            Region::Square => {
                let side = width.min(height);
                ((width - side) / 2, (height - side) / 2, side, side)
            }
            Region::Pixels { x, y, w, h } => {
                if x >= width || y >= height {
                    return Err(ImageRequestError::EmptyRegion);
                }
                (x, y, w.min(width - x), h.min(height - y))
            }
        };
        if crop_width == 0 || crop_height == 0 {
            return Err(ImageRequestError::EmptyRegion);
        }
        let (out_width, out_height) = scaled_size(self.size, crop_width, crop_height);
        let fits = out_width > 0
            && out_height > 0
            && out_width <= crop_width
            && out_height <= crop_height
            && out_width <= MAX_EDGE
            && out_height <= MAX_EDGE;
        if !fits {
            return Err(ImageRequestError::SizeOutOfRange {
                width: out_width,
                height: out_height,
            });
        }
        Ok(Plan {
            x,
            y,
            crop_width,
            crop_height,
            width: out_width,
            height: out_height,
        })
    }
}

fn parse_region(region: &str) -> Option<Region> {
    match region {
        "full" => Some(Region::Full),
        "square" => Some(Region::Square),
        _ => {
            let [x, y, w, h] = numbers(region)?;
            (w > 0 && h > 0).then_some(Region::Pixels { x, y, w, h })
        }
    }
}

fn parse_size(size: &str) -> Option<Size> {
    if size == "max" {
        return Some(Size::Max);
    }
    if let Some(percent) = size.strip_prefix("pct:") {
        let value: f64 = percent.parse().ok()?;
        return (value > 0.0 && value <= 100.0).then_some(Size::Percent(value));
    }
    if let Some(confined) = size.strip_prefix('!') {
        let (w, h) = confined.split_once(',')?;
        return Some(Size::Confined(w.parse().ok()?, h.parse().ok()?));
    }
    match size.split_once(',')? {
        (w, "") => Some(Size::Width(w.parse().ok()?)),
        ("", h) => Some(Size::Height(h.parse().ok()?)),
        (w, h) => Some(Size::Exact(w.parse().ok()?, h.parse().ok()?)),
    }
}

/// Four comma-separated whole numbers.
fn numbers(text: &str) -> Option<[u32; 4]> {
    let mut parts = text.split(',').map(|part| part.parse::<u32>().ok());
    let out = [
        parts.next()??,
        parts.next()??,
        parts.next()??,
        parts.next()??,
    ];
    parts.next().is_none().then_some(out)
}

/// The output size of a crop. A scaled edge is rounded to the nearest pixel,
/// and never below one.
fn scaled_size(size: Size, width: u32, height: u32) -> (u32, u32) {
    let scale = |edge: u32, factor: f64| ((f64::from(edge) * factor).round() as u32).max(1);
    match size {
        Size::Max => {
            let factor = (f64::from(MAX_EDGE) / f64::from(width.max(height))).min(1.0);
            (scale(width, factor), scale(height, factor))
        }
        Size::Width(w) => (w, scale(height, f64::from(w) / f64::from(width))),
        Size::Height(h) => (scale(width, f64::from(h) / f64::from(height)), h),
        Size::Exact(w, h) => (w, h),
        Size::Confined(w, h) => {
            let factor = (f64::from(w) / f64::from(width)).min(f64::from(h) / f64::from(height));
            (scale(width, factor), scale(height, factor))
        }
        Size::Percent(percent) => (
            scale(width, percent / 100.0),
            scale(height, percent / 100.0),
        ),
    }
}

/// Why an image could not be produced from a stored file.
#[derive(Debug, Error)]
pub enum RenderError {
    /// The stored bytes did not decode as an image.
    #[error("the image did not decode: {0}")]
    Decode(String),
    /// The answer did not encode.
    #[error("the image did not encode: {0}")]
    Encode(String),
}

/// Decode a stored image, upright as its EXIF orientation says. An SVG is
/// rasterised at its own size, capped at [`MAX_EDGE`], which is the size its
/// canvas reports.
///
/// A camera writes the pixels in the sensor's orientation and an EXIF tag
/// saying how to turn them; every derivative this server makes is upright
/// (`thumbnailer::decode_oriented`), so the Image API's pixels are too, and a
/// canvas and its image are one space.
///
/// Callers run this on a blocking thread: a large original takes a noticeable
/// time to decode.
///
/// # Errors
/// [`RenderError::Decode`] when the bytes are not an image.
pub fn decode(bytes: &[u8]) -> Result<DynamicImage, RenderError> {
    if is_svg(bytes) {
        return crate::svg::rasterized(bytes, MAX_EDGE)
            .map_err(|error| RenderError::Decode(error.to_string()));
    }
    let mut decoder = reader(bytes)?;
    // A malformed or absent EXIF block leaves the pixels as stored.
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut source = DynamicImage::from_decoder(decoder)
        .map_err(|error| RenderError::Decode(error.to_string()))?;
    source.apply_orientation(orientation);
    Ok(source)
}

/// The pixel size [`decode`] answers for `bytes`, `None` when they are not an
/// image: the stored size turned upright by its EXIF orientation, or an SVG's
/// drawn size. A canvas reports this size, so its space and the Image API's
/// pixels are the same one.
#[must_use]
pub fn source_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if is_svg(bytes) {
        return crate::svg::size(bytes, MAX_EDGE).ok();
    }
    let mut decoder = reader(bytes).ok()?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let (width, height) = decoder.dimensions();
    let quarter_turn = matches!(
        orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    );
    Some(if quarter_turn {
        (height, width)
    } else {
        (width, height)
    })
}

/// The decoder for `bytes`, guessed by content.
fn reader(bytes: &[u8]) -> Result<impl image::ImageDecoder + '_, RenderError> {
    image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| RenderError::Decode(error.to_string()))?
        .into_decoder()
        .map_err(|error| RenderError::Decode(error.to_string()))
}

/// Whether the bytes are an SVG document rather than a raster.
fn is_svg(bytes: &[u8]) -> bool {
    let text = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text = text.trim_ascii_start();
    text.starts_with(b"<svg") || (text.starts_with(b"<?xml") && find_svg(text))
}

/// Whether an XML prolog is followed by an SVG root, within the first
/// [`PROLOG_BYTES`] where a document's prolog lives.
fn find_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(PROLOG_BYTES)];
    head.windows(4).any(|window| window == b"<svg")
}

/// How far into an SVG the root element is looked for.
const PROLOG_BYTES: usize = 4 * 1024;

/// Crop, scale and encode one planned answer from a decoded image.
///
/// # Errors
/// [`RenderError::Encode`] when the answer does not encode.
pub fn render(
    source: &DynamicImage,
    plan: &Plan,
    format: ImageFormat,
) -> Result<Vec<u8>, RenderError> {
    let cropped = source.crop_imm(plan.x, plan.y, plan.crop_width, plan.crop_height);
    let scaled = if (plan.width, plan.height) == (plan.crop_width, plan.crop_height) {
        cropped
    } else {
        cropped.resize_exact(plan.width, plan.height, FilterType::Triangle)
    };
    let mut bytes = Vec::new();
    let encoded = match format {
        ImageFormat::Jpg => JpegEncoder::new_with_quality(&mut bytes, 85)
            .encode_image(&DynamicImage::ImageRgb8(scaled.to_rgb8())),
        ImageFormat::Png => scaled.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png),
    };
    encoded.map_err(|error| RenderError::Encode(error.to_string()))?;
    Ok(bytes)
}

/// The `info.json` of an image service at `id` over an image of `width` by
/// `height` pixels.
#[must_use]
pub fn info(id: &str, width: u32, height: u32) -> Value {
    let largest = width.max(height);
    let mut scale_factors = Vec::new();
    for factor in SCALE_FACTORS {
        scale_factors.push(factor);
        if largest.div_ceil(factor) <= TILE {
            break;
        }
    }
    json!({
        "@context": "http://iiif.io/api/image/3/context.json",
        "id": id,
        "type": "ImageService3",
        "protocol": "http://iiif.io/api/image",
        "profile": "level1",
        "width": width,
        "height": height,
        "maxWidth": MAX_EDGE,
        "maxHeight": MAX_EDGE,
        "tiles": [{"width": TILE, "scaleFactors": scale_factors}],
        "extraFormats": ["png"],
        "extraFeatures": ["sizeByPct", "sizeByConfinedWh"],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(region: &str, size: &str, width: u32, height: u32) -> Result<Plan, ImageRequestError> {
        ImageRequest::parse(region, size, "0", "default.jpg")?.plan(width, height)
    }

    fn out(region: &str, size: &str) -> (u32, u32) {
        let plan = plan(region, size, 1000, 800).expect("a plan");
        (plan.width, plan.height)
    }

    /// Image API 3.0 section 4.2: each size form against a 1000x800 image.
    #[test]
    fn sizes_resolve_as_the_specification_states() {
        assert_eq!(out("full", "max"), (1000, 800));
        assert_eq!(out("full", "500,"), (500, 400));
        assert_eq!(out("full", ",200"), (250, 200));
        assert_eq!(out("full", "300,300"), (300, 300));
        assert_eq!(out("full", "!500,500"), (500, 400));
        assert_eq!(out("full", "pct:25"), (250, 200));
    }

    /// Section 4.1: the square region is centred, and a pixel region past the
    /// edge is cut at the edge.
    #[test]
    fn regions_resolve_as_the_specification_states() {
        let square = plan("square", "max", 1000, 800).expect("a plan");
        assert_eq!(
            (square.x, square.y, square.crop_width, square.crop_height),
            (100, 0, 800, 800)
        );
        let edge = plan("900,700,512,512", "max", 1000, 800).expect("a plan");
        assert_eq!((edge.crop_width, edge.crop_height), (100, 100));
        assert_eq!(
            plan("1000,0,10,10", "max", 1000, 800),
            Err(ImageRequestError::EmptyRegion)
        );
    }

    /// No upscaling, no zero, and nothing past [`MAX_EDGE`].
    #[test]
    fn sizes_outside_the_limits_are_refused() {
        assert!(matches!(
            plan("full", "1001,", 1000, 800),
            Err(ImageRequestError::SizeOutOfRange { .. })
        ));
        assert!(matches!(
            plan("full", "0,", 1000, 800),
            Err(ImageRequestError::SizeOutOfRange { .. })
        ));
        assert!(matches!(
            plan("full", "5000,", 6000, 4000),
            Err(ImageRequestError::SizeOutOfRange { .. })
        ));
        let max = plan("full", "max", 6000, 4000).expect("max is confined");
        assert_eq!((max.width, max.height), (MAX_EDGE, 2731));
    }

    /// Only rotation 0, quality `default` and the two formats parse.
    #[test]
    fn segments_outside_level_one_are_unsupported() {
        for (region, size, rotation, file) in [
            ("full", "max", "90", "default.jpg"),
            ("full", "max", "0", "gray.jpg"),
            ("full", "max", "0", "default.webp"),
            ("pct:10,10,10,10", "max", "0", "default.jpg"),
            ("full", "^max", "0", "default.jpg"),
            ("full", "pct:0", "0", "default.jpg"),
            ("0,0,0,10", "max", "0", "default.jpg"),
        ] {
            assert!(
                matches!(
                    ImageRequest::parse(region, size, rotation, file),
                    Err(ImageRequestError::Unsupported { .. })
                ),
                "{region}/{size}/{rotation}/{file}"
            );
        }
        let png = ImageRequest::parse("full", "max", "0", "default.png").expect("png");
        assert_eq!(png.format.media_type(), "image/png");
    }

    /// The answer has the planned size and decodes in the requested format.
    #[test]
    fn render_crops_and_scales() {
        let source = DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            64,
            32,
            image::Rgb([200, 10, 10]),
        ));
        let plan = ImageRequest::parse("0,0,32,32", "16,", "0", "default.png")
            .expect("parse")
            .plan(64, 32)
            .expect("plan");
        let bytes = render(&source, &plan, ImageFormat::Png).expect("render");
        let answer = image::load_from_memory(&bytes).expect("a png");
        assert_eq!((answer.width(), answer.height()), (16, 16));
    }

    /// `info.json` names the level, the size and the limits, and its scale
    /// factors stop where the whole image fits in one tile.
    #[test]
    fn info_describes_a_level_one_service() {
        let info = info("https://example.org/iiif/image/3/7", 2000, 1000);
        assert_eq!(info["type"], "ImageService3");
        assert_eq!(info["profile"], "level1");
        assert_eq!(info["width"], 2000);
        assert_eq!(info["tiles"][0]["scaleFactors"], json!([1, 2, 4]));
    }

    fn corpus(name: &str) -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/media-kinds")
            .join(name);
        std::fs::read(&path).unwrap_or_else(|error| panic!("{name}: {error}"))
    }

    /// A JPEG whose EXIF Orientation is 6 decodes upright: the stored 120 by
    /// 80 landscape becomes the 80 by 120 portrait the canvas reports and the
    /// derivative is made in.
    #[test]
    fn an_exif_oriented_jpeg_decodes_upright() {
        let source = decode(&corpus("image.jpg")).expect("a jpeg");
        assert_eq!((source.width(), source.height()), (80, 120));
    }

    /// An SVG decodes at the size its canvas reports, which is the raster the
    /// thumbnailer made.
    #[test]
    fn an_svg_decodes_at_its_own_size() {
        let source = decode(&corpus("image.svg")).expect("an svg");
        assert_eq!((source.width(), source.height()), (120, 80));
    }
}
