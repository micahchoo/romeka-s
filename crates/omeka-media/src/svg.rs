//! SVG files: the one place an uploaded SVG is made safe, and the raster the
//! thumbnailer draws from it.
//!
//! SVG is a document, not a picture: it can carry script, event handlers,
//! `javascript:` links, a foreign HTML subtree and references to another
//! origin. [`sanitize`] runs `svg-hush`, a parser-based allow list of SVG
//! elements and attributes, and writes the filtered document back out. The
//! ingest stores those bytes as the original, so every reader, download and
//! cache serves the safe document, not the one that was uploaded.
//!
//! The filter drops:
//!
//! - `<script>`, `<foreignObject>` and every element outside the SVG
//!   namespace; the subtree goes with it;
//! - every attribute it does not name, which is every `on*` handler;
//! - a `javascript:` or `blob:` URL, and every URL with a host: an external
//!   image, `use` or `feImage` becomes a same-origin path or is dropped;
//! - a `data:` URL that is not a raster image. `image/png`, `image/jpeg` and
//!   `image/gif` are kept, nested SVG and HTML are not;
//! - an external `url()` in a style element or attribute, and an `@import`,
//!   which is left as inert text a browser discards (it is no longer a URL
//!   function, so nothing is fetched).
//!
//! [`rasterized`] draws the sanitized document with `resvg`, so an SVG gets the
//! same three JPEG derivatives as any other image and a collection card can
//! show it. It renders at the file's own size up to the longest derivative, so
//! a huge viewBox never allocates a huge pixmap.

use svg_hush::data_url_filter::allow_standard_images;
use svg_hush::{FError, Filter};

/// Why an SVG could not be made safe or drawn.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SvgError {
    /// The bytes are not a well-formed SVG document the filter reads.
    #[error("the SVG did not parse: {0}")]
    Malformed(String),
    /// The document parsed but has no finite, positive size to draw.
    #[error("the SVG has no drawable size")]
    NothingDrawable,
    /// The raster could not be allocated or built.
    #[error("the SVG did not render: {0}")]
    Render(String),
}

impl From<FError> for SvgError {
    fn from(error: FError) -> Self {
        Self::Malformed(error.to_string())
    }
}

/// The SVG with every scripting and cross-origin feature removed.
///
/// A document the filter cannot read is [`SvgError::Malformed`]; the caller
/// refuses the file rather than storing bytes it could not examine.
///
/// # Errors
/// [`SvgError::Malformed`] for bytes that are not a well-formed SVG document.
pub fn sanitize(bytes: &[u8]) -> Result<Vec<u8>, SvgError> {
    let mut filter = Filter::new();
    filter.set_data_url_filter(allow_standard_images);
    let mut safe = Vec::with_capacity(bytes.len());
    filter.filter(bytes, &mut safe)?;
    Ok(safe)
}

/// The sanitized SVG drawn as pixels, its long side at most `max_side`.
///
/// An image smaller than `max_side` keeps its own pixel size, as the
/// thumbnailer does not enlarge. The pixmap is straight-alpha RGBA, ready for
/// the thumbnailer's JPEG encode onto white.
///
/// # Errors
/// [`SvgError::Malformed`] when the document does not parse,
/// [`SvgError::NothingDrawable`] when it has no size, and
/// [`SvgError::Render`] when no pixmap can be made.
pub fn rasterized(bytes: &[u8], max_side: u32) -> Result<image::DynamicImage, SvgError> {
    let tree = resvg::usvg::Tree::from_data(bytes, &resvg::usvg::Options::default())
        .map_err(|error| SvgError::Malformed(error.to_string()))?;
    let size = tree.size();
    let (width, height) = (size.width(), size.height());
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err(SvgError::NothingDrawable);
    }
    let scale = (max_side as f32 / width.max(height)).min(1.0);
    let pixel_width = pixel_size(width * scale)?;
    let pixel_height = pixel_size(height * scale)?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(pixel_width, pixel_height)
        .ok_or_else(|| SvgError::Render(format!("no {pixel_width} by {pixel_height} pixmap")))?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let mut rgba = Vec::with_capacity(pixmap.data().len());
    for pixel in pixmap.pixels() {
        let pixel = pixel.demultiply();
        rgba.extend_from_slice(&[pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]);
    }
    let image = image::RgbaImage::from_raw(pixel_width, pixel_height, rgba)
        .ok_or_else(|| SvgError::Render("the pixmap was not RGBA".to_string()))?;
    Ok(image::DynamicImage::ImageRgba8(image))
}

/// A side length as whole pixels, at least one, never past `u32::MAX`.
fn pixel_size(side: f32) -> Result<u32, SvgError> {
    let pixels = side.ceil();
    if !pixels.is_finite() || pixels > u32::MAX as f32 {
        return Err(SvgError::NothingDrawable);
    }
    Ok((pixels as u32).max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the filter wrote, as text.
    fn safe(svg: &str) -> String {
        String::from_utf8(sanitize(svg.as_bytes()).expect("a well-formed SVG"))
            .expect("the filter writes UTF-8")
    }

    const ROOT: &str = "http://www.w3.org/2000/svg";

    /// A script element is gone, and the drawing beside it stays.
    #[test]
    fn a_script_element_is_removed() {
        let out = safe(&format!(
            "<svg xmlns=\"{ROOT}\"><script>alert('script')</script>\
             <rect width=\"10\" height=\"10\"/></svg>"
        ));
        assert!(!out.to_lowercase().contains("<script"), "{out}");
        assert!(!out.contains("alert"), "{out}");
        assert!(out.contains("<rect"), "{out}");
    }

    /// Every `on*` event handler attribute is gone.
    #[test]
    fn event_handler_attributes_are_removed() {
        let out = safe(&format!(
            "<svg xmlns=\"{ROOT}\" onload=\"alert('onload')\">\
             <rect width=\"10\" height=\"10\" onclick=\"alert('click')\"/></svg>"
        ));
        let lower = out.to_lowercase();
        assert!(!lower.contains("onload"), "{out}");
        assert!(!lower.contains("onclick"), "{out}");
        assert!(!out.contains("alert"), "{out}");
    }

    /// A `javascript:` href is gone, on a link and on a filter reference.
    #[test]
    fn javascript_hrefs_are_removed() {
        let out = safe(&format!(
            "<svg xmlns=\"{ROOT}\"><a href=\"javascript:alert('href')\">\
             <rect width=\"10\" height=\"10\"/></a>\
             <filter id=\"f\"><feImage href=\"java&#115;cript:alert(1)\"/></filter></svg>"
        ));
        assert!(!out.to_lowercase().contains("javascript"), "{out}");
        assert!(!out.contains("alert"), "{out}");
    }

    /// A `data:` href is kept only when it carries a raster image.
    #[test]
    fn data_urls_are_kept_only_for_raster_images() {
        let raster = safe(&format!(
            "<svg xmlns=\"{ROOT}\"><image href=\"data:image/png;base64,iVBORw0KGgo=\" \
             width=\"10\" height=\"10\"/></svg>"
        ));
        assert!(raster.contains("data:image/png"), "{raster}");

        for payload in [
            "data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==",
            "data:image/svg+xml;base64,PHN2Zy8+",
        ] {
            let out = safe(&format!(
                "<svg xmlns=\"{ROOT}\"><image href=\"{payload}\" width=\"10\" height=\"10\"/></svg>"
            ));
            assert!(!out.contains(payload), "{out}");
            assert!(!out.contains("data:"), "{out}");
        }
    }

    /// A `foreignObject` subtree is removed whole.
    #[test]
    fn foreign_object_is_removed() {
        let out = safe(&format!(
            "<svg xmlns=\"{ROOT}\"><foreignObject x=\"0\" y=\"0\" width=\"10\" height=\"10\">\
             <div xmlns=\"http://www.w3.org/1999/xhtml\">foreign</div>\
             </foreignObject><rect width=\"10\" height=\"10\"/></svg>"
        ));
        assert!(!out.to_lowercase().contains("foreignobject"), "{out}");
        assert!(!out.contains("foreign"), "{out}");
    }

    /// A reference to another origin keeps no host: `svg-hush` turns a URL
    /// with a host into a same-origin path, so the browser fetches nothing
    /// from the other site. The namespace is the only `http` left.
    #[test]
    fn external_references_keep_no_host() {
        let out = safe(&format!(
            "<svg xmlns=\"{ROOT}\" xmlns:xlink=\"http://www.w3.org/1999/xlink\">\
             <image href=\"https://evil.example/a.png\" width=\"10\" height=\"10\"/>\
             <use xlink:href=\"//evil.example/sprite.svg#mark\"/>\
             <feImage href=\"http://evil.example/b.png\"/></svg>"
        ));
        assert!(!out.contains("evil.example"), "{out}");
        assert!(!out.contains("://evil.example"), "{out}");
    }

    /// An external stylesheet import and an external `url()` reach no host.
    /// The `url()` is rewritten to a same-origin path. svg-hush reduces the
    /// `@import` to the text inside its parentheses, which a browser discards
    /// as an invalid declaration: the literal host stays as text, and no URL
    /// function carries it, so nothing is fetched.
    #[test]
    fn external_stylesheet_references_are_dropped() {
        let out = safe(&format!(
            "<svg xmlns=\"{ROOT}\"><style>\
             @import url(https://evil.example/x.css);\
             .a {{ fill: url(https://evil.example/b.png) }}</style>\
             <rect class=\"a\" width=\"10\" height=\"10\"/></svg>"
        ));
        assert!(!out.to_lowercase().contains("@import"), "{out}");
        assert!(!out.contains("url(http"), "{out}");
        assert!(!out.contains("url(https"), "{out}");
        assert!(out.contains("url(/b.png)"), "{out}");
    }

    /// A benign drawing keeps its markup, its colours and its accessible name.
    #[test]
    fn a_benign_drawing_is_kept() {
        let out = safe(&format!(
            "<svg xmlns=\"{ROOT}\" width=\"120\" height=\"80\" viewBox=\"0 0 120 80\" \
             role=\"img\" aria-label=\"A red square\">\
             <title>A red square</title>\
             <rect width=\"47\" height=\"39\" fill=\"#c0392b\"/>\
             <path d=\"M0 0 L10 10\" stroke=\"#2c3e50\"/></svg>"
        ));
        assert!(out.contains("#c0392b"), "{out}");
        assert!(out.contains("<path"), "{out}");
        assert!(out.contains("A red square"), "{out}");
        assert!(out.contains("aria-label"), "{out}");
    }

    /// Bytes that are not a well-formed SVG are refused, never stored as
    /// though they were one.
    #[test]
    fn malformed_bytes_are_refused() {
        assert!(matches!(
            sanitize(b"<svg xmlns=\"http://www.w3.org/2000/svg\"><rect"),
            Err(SvgError::Malformed(_))
        ));
        assert!(matches!(
            sanitize(b"not an svg at all"),
            Err(SvgError::Malformed(_))
        ));
    }

    /// The raster keeps the drawing's own size and never grows past the
    /// longest derivative.
    #[test]
    fn a_drawing_rasterizes_at_its_own_size_bounded() {
        let small = format!(
            "<svg xmlns=\"{ROOT}\" width=\"120\" height=\"80\">\
             <rect width=\"120\" height=\"80\" fill=\"#f4f0e8\"/></svg>"
        );
        let image = rasterized(small.as_bytes(), 800).expect("a raster");
        assert_eq!((image.width(), image.height()), (120, 80));

        let large = format!(
            "<svg xmlns=\"{ROOT}\" width=\"1600\" height=\"900\">\
             <rect width=\"1600\" height=\"900\" fill=\"#c0392b\"/></svg>"
        );
        let image = rasterized(large.as_bytes(), 800).expect("a raster");
        assert_eq!((image.width(), image.height()), (800, 450));
        let pixel = image.to_rgba8().get_pixel(400, 225).0;
        assert!(pixel[0] > 150 && pixel[1] < 100, "{pixel:?}");
    }
}
