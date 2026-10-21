//! What each media kind is, what an annotation may point at on it, and what its
//! IIIF canvas paints. The manifest, the annotation parser, the public page and
//! (through the container's `Romeka-Surface` header) the admin all ask this
//! module. None of them keeps a list of kinds.
//!
//! The table is one `match` per question, keyed on [`MediaKind`]. A caller that
//! needs a rule about a kind adds a variant here; it does not test the media
//! type or the renderer name itself.

use crate::thumbnailer::SOURCE_MEDIA_TYPES;

/// A media as stored, reduced to what decides its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KindInput<'a> {
    /// `media.renderer`: `file`, `iiif`, `youtube`, `oembed`, `html` or
    /// `iiif_presentation`. An empty string when the row names none.
    pub renderer: &'a str,
    /// `media.ingester`: `upload`, `url`, `iiif`, `iiif_presentation`,
    /// `youtube`, `oembed` or `html`. An empty string when the row names none.
    pub ingester: &'a str,
    /// `media.media_type`, the type the ingester recorded.
    pub media_type: Option<&'a str>,
    /// Whether the three image derivatives exist.
    pub has_thumbnails: bool,
}

/// One row of the per-kind table (section 7 of the annotation design). The
/// manifest, the annotation parser, the public page and the admin read it
/// through the three methods below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    /// A stored raster image whose derivatives exist.
    LocalImage,
    /// A remote image with an Image API service.
    IiifImage,
    Audio,
    Video,
    /// A stored PDF; `first_page` says a first-page derivative exists.
    Pdf {
        first_page: bool,
    },
    /// A glTF binary or JSON model.
    Model3d,
    /// A WebVTT or SRT file, which is also a caption-import source.
    Captions,
    /// A YouTube, oEmbed or HTML embed.
    Embed,
    /// A remote Presentation 3 manifest.
    IiifPresentation,
    /// Any other stored file, including a raster whose derivatives failed.
    OtherFile,
}

/// What a selector on a media may address. `Image` takes rectangles and
/// polygons in canvas pixels, `Time` takes `t=start,end`, and every surface
/// also takes whole-object notes; on `Whole` any selector is a 400.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Image,
    Time,
    Whole,
}

/// What a media canvas paints. `None` from [`MediaKind::canvas_body`] means the
/// kind has no canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasBody {
    /// The original through an Image API service.
    Image,
    /// A still derivative with no service, such as a PDF's first page.
    Still,
    Sound,
    Video,
}

/// The facts a canvas needs and `media.data` must not hold. `size` is upright:
/// EXIF orientations 5 to 8 swap the header's width and height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Extent {
    pub size: Option<(u32, u32)>,
    pub duration_ms: Option<u64>,
}

impl MediaKind {
    /// The kind of one media row.
    #[must_use]
    pub fn of(input: &KindInput<'_>) -> Self {
        match input.ingester {
            "iiif" => return Self::IiifImage,
            "iiif_presentation" => return Self::IiifPresentation,
            _ => {}
        }
        match input.renderer {
            "youtube" | "oembed" | "html" => Self::Embed,
            "file" => Self::of_file(input.media_type, input.has_thumbnails),
            _ => Self::OtherFile,
        }
    }

    fn of_file(media_type: Option<&str>, has_thumbnails: bool) -> Self {
        let Some(media_type) = media_type else {
            return Self::OtherFile;
        };
        if has_thumbnails && SOURCE_MEDIA_TYPES.contains(&media_type) {
            return Self::LocalImage;
        }
        match media_type {
            "application/pdf" => Self::Pdf {
                first_page: has_thumbnails,
            },
            "model/gltf-binary" | "model/gltf+json" => Self::Model3d,
            "text/vtt" | "application/x-subrip" => Self::Captions,
            media_type if media_type.starts_with("audio/") => Self::Audio,
            media_type if media_type.starts_with("video/") => Self::Video,
            _ => Self::OtherFile,
        }
    }

    /// What a selector on this kind may address. This is the v1 table of the
    /// annotation design's section 7, and its single source of truth.
    #[must_use]
    pub fn surface(self) -> Surface {
        match self {
            Self::LocalImage | Self::IiifImage => Surface::Image,
            Self::Audio | Self::Video => Surface::Time,
            Self::Pdf { .. }
            | Self::Model3d
            | Self::Captions
            | Self::Embed
            | Self::IiifPresentation
            | Self::OtherFile => Surface::Whole,
        }
    }

    /// What this kind's canvas paints, or `None` when it has no canvas.
    ///
    /// This is the phase-one table: the two image kinds paint a canvas, which
    /// is exactly what `iiif::paints_canvas` accepted before this module
    /// existed. The AV and still canvases belong to the extents unit, which
    /// turns them on here for [`Self::Audio`], [`Self::Video`] and
    /// [`Self::Pdf`].
    #[must_use]
    pub fn canvas_body(self) -> Option<CanvasBody> {
        match self {
            Self::LocalImage | Self::IiifImage => Some(CanvasBody::Image),
            Self::Audio
            | Self::Video
            | Self::Pdf { .. }
            | Self::Model3d
            | Self::Captions
            | Self::Embed
            | Self::IiifPresentation
            | Self::OtherFile => None,
        }
    }
}
