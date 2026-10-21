//! What each media kind is, what an annotation may point at on it, and what its
//! IIIF canvas paints. The manifest, the annotation parser, the public page and
//! (through the container's `Romeka-Surface` header) the admin all ask this
//! module. None of them keeps a list of kinds.
//!
//! The table is one `match` per question, keyed on [`MediaKind`]. A caller that
//! needs a rule about a kind adds a variant here; it does not test the media
//! type or the renderer name itself.

/// The stored types whose derivatives are a still image for display: every
/// type the thumbnailer decodes, plus the camera RAW formats, whose embedded
/// preview makes the derivatives. This is the one list [`MediaKind::of_file`]
/// and the SQL mirror in `omeka-db` both read.
pub const DISPLAY_IMAGE_MEDIA_TYPES: [&str; 18] = [
    "image/gif",
    "image/jpeg",
    "image/jp2",
    "image/png",
    "image/svg+xml",
    "image/tiff",
    "image/webp",
    "image/x-adobe-dng",
    "image/x-canon-cr2",
    "image/x-canon-cr3",
    "image/x-fuji-raf",
    "image/x-nikon-nef",
    "image/x-olympus-orf",
    "image/x-panasonic-rw2",
    "image/x-pentax-pef",
    "image/x-samsung-srw",
    "image/x-sigma-x3f",
    "image/x-sony-arw",
];

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
    /// Every kind, in the order the wire and the admin read them.
    pub const ALL: [Self; 10] = [
        Self::LocalImage,
        Self::IiifImage,
        Self::Audio,
        Self::Video,
        Self::Pdf { first_page: false },
        Self::Model3d,
        Self::Captions,
        Self::Embed,
        Self::IiifPresentation,
        Self::OtherFile,
    ];

    /// Every kind's [`MediaKind::name`], in [`MediaKind::ALL`] order.
    #[must_use]
    pub fn names() -> Vec<&'static str> {
        Self::ALL.iter().map(|kind| kind.name()).collect()
    }

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
        if has_thumbnails && DISPLAY_IMAGE_MEDIA_TYPES.contains(&media_type) {
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

    /// The stable name the wire and the query parameters use for this kind, one
    /// name per variant. `Pdf` carries its derivative state in `first_page`,
    /// which is not part of the name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::LocalImage => "local-image",
            Self::IiifImage => "iiif-image",
            Self::Audio => "audio",
            Self::Video => "video",
            Self::Pdf { .. } => "pdf",
            Self::Model3d => "model-3d",
            Self::Captions => "captions",
            Self::Embed => "embed",
            Self::IiifPresentation => "iiif-presentation",
            Self::OtherFile => "other-file",
        }
    }

    /// The kind a name from a query parameter or a client names, or `None` for a
    /// name this server does not know. A value with no variant is not a guess:
    /// it matches no row.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "local-image" => Self::LocalImage,
            "iiif-image" => Self::IiifImage,
            "audio" => Self::Audio,
            "video" => Self::Video,
            "pdf" => Self::Pdf { first_page: false },
            "model-3d" => Self::Model3d,
            "captions" => Self::Captions,
            "embed" => Self::Embed,
            "iiif-presentation" => Self::IiifPresentation,
            "other-file" => Self::OtherFile,
            _ => return None,
        })
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
    /// A canvas needs something to paint, so an audio or video canvas is
    /// emitted only when its extent (a duration) is known; `canvas_body` says
    /// what the body would be, not that one can be made. A PDF paints its
    /// first page once the derivative exists.
    #[must_use]
    pub fn canvas_body(self) -> Option<CanvasBody> {
        match self {
            Self::LocalImage | Self::IiifImage => Some(CanvasBody::Image),
            Self::Pdf { first_page: true } => Some(CanvasBody::Still),
            Self::Audio => Some(CanvasBody::Sound),
            Self::Video => Some(CanvasBody::Video),
            Self::Pdf { first_page: false }
            | Self::Model3d
            | Self::Captions
            | Self::Embed
            | Self::IiifPresentation
            | Self::OtherFile => None,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::thumbnailer::SOURCE_MEDIA_TYPES;

    /// Every variant's name comes back from `from_name` as the same kind, and no
    /// two variants share a name.
    #[test]
    fn every_kind_has_one_stable_name() {
        let kinds = [
            MediaKind::LocalImage,
            MediaKind::IiifImage,
            MediaKind::Audio,
            MediaKind::Video,
            MediaKind::Pdf { first_page: false },
            MediaKind::Pdf { first_page: true },
            MediaKind::Model3d,
            MediaKind::Captions,
            MediaKind::Embed,
            MediaKind::IiifPresentation,
            MediaKind::OtherFile,
        ];
        let mut names = Vec::new();
        for kind in kinds {
            let name = kind.name();
            assert_eq!(
                MediaKind::from_name(name),
                Some(match kind {
                    MediaKind::Pdf { .. } => MediaKind::Pdf { first_page: false },
                    other => other,
                }),
                "name {name}"
            );
            names.push(name);
        }
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 10, "names: {names:?}");
    }

    /// A name this server does not know matches no kind.
    #[test]
    fn an_unknown_name_is_no_kind() {
        assert_eq!(MediaKind::from_name("AUDIO"), None);
        assert_eq!(MediaKind::from_name("document"), None);
        assert_eq!(MediaKind::from_name(""), None);
    }

    /// Every type the thumbnailer decodes is a display image, and a RAW file is
    /// one once its preview made the derivatives.
    #[test]
    fn a_raw_file_with_derivatives_is_a_display_image() {
        for media_type in SOURCE_MEDIA_TYPES {
            assert!(
                DISPLAY_IMAGE_MEDIA_TYPES.contains(&media_type),
                "{media_type}"
            );
        }
        let raw = |has_thumbnails| {
            MediaKind::of(&KindInput {
                renderer: "file",
                ingester: "upload",
                media_type: Some("image/x-nikon-nef"),
                has_thumbnails,
            })
        };
        assert_eq!(raw(true), MediaKind::LocalImage);
        assert_eq!(raw(false), MediaKind::OtherFile);
    }
}
