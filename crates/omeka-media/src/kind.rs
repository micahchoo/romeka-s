//! What each media kind is, what an annotation may point at on it, and what its
//! IIIF canvas paints. The manifest, the annotation parser, the public page and
//! (through the container's `Romeka-Surface` header) the admin all ask this
//! module. None of them keeps a list of kinds.
//!
//! The table is one `match` per question, keyed on [`MediaKind`]. A caller that
//! needs a rule about a kind adds a variant here; it does not test the media
//! type or the renderer name itself.

/// The stored types whose derivatives are a still image for display: every
/// type the thumbnailer decodes, plus the types a JPEG access master makes
/// displayable (the camera RAW formats, PSD, the dynamic-range formats, HEIC
/// and the PostScript page formats). This is the one list [`MediaKind::of_file`]
/// and the SQL mirror in `omeka-db` both read.
///
/// Membership is not enough on its own: a row is a local image only once
/// `has_thumbnails` says the files exist, so a PSD on an install without `vips`
/// stays an other file and a type is never promised a canvas this install
/// cannot paint.
pub const DISPLAY_IMAGE_MEDIA_TYPES: [&str; 29] = [
    "application/illustrator",
    "application/postscript",
    "image/avif",
    "image/bmp",
    "image/gif",
    "image/heic",
    "image/heif",
    "image/jp2",
    "image/jpeg",
    "image/png",
    "image/svg+xml",
    "image/tiff",
    "image/vnd.adobe.photoshop",
    "image/vnd.radiance",
    "image/webp",
    "image/x-adobe-dng",
    "image/x-canon-cr2",
    "image/x-canon-cr3",
    "image/x-eps",
    "image/x-exr",
    "image/x-fuji-raf",
    "image/x-nikon-nef",
    "image/x-olympus-orf",
    "image/x-panasonic-rw2",
    "image/x-pentax-pef",
    "image/x-photoshop",
    "image/x-samsung-srw",
    "image/x-sigma-x3f",
    "image/x-sony-arw",
];

/// The stored types an office document carries: the word processing,
/// spreadsheet and presentation formats a curator uploads. OpenDocument's three
/// types are here for a server that recorded them; the bytes of an `.odt`,
/// `.ods` or `.odp` are a zip, so [`MediaKind::of`] also reads those extensions.
pub const OFFICE_DOCUMENT_MEDIA_TYPES: [&str; 10] = [
    "application/msword",
    "application/rtf",
    "application/vnd.ms-excel",
    "application/vnd.ms-powerpoint",
    "application/vnd.oasis.opendocument.presentation",
    "application/vnd.oasis.opendocument.spreadsheet",
    "application/vnd.oasis.opendocument.text",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
];

/// The extensions an office document carries, lowercase. The type is the
/// authority; these name a document whose bytes read as their container, which
/// an OpenDocument file always does because its bytes are a zip.
pub const OFFICE_DOCUMENT_EXTENSIONS: [&str; 10] = [
    "doc", "docx", "odp", "ods", "odt", "ppt", "pptx", "rtf", "xls", "xlsx",
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
    /// `media.extension`, the stored extension without its dot. It names an
    /// office document whose type is only its zip container.
    pub extension: Option<&'a str>,
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
    /// A stored office document: word processing, spreadsheet or presentation.
    Document,
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
    pub const ALL: [Self; 11] = [
        Self::LocalImage,
        Self::IiifImage,
        Self::Audio,
        Self::Video,
        Self::Pdf { first_page: false },
        Self::Document,
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
            "file" => Self::of_file(input.media_type, input.extension, input.has_thumbnails),
            _ => Self::OtherFile,
        }
    }

    fn of_file(media_type: Option<&str>, extension: Option<&str>, has_thumbnails: bool) -> Self {
        if let Some(media_type) = media_type {
            if has_thumbnails && DISPLAY_IMAGE_MEDIA_TYPES.contains(&media_type) {
                return Self::LocalImage;
            }
            match media_type {
                "application/pdf" => {
                    return Self::Pdf {
                        first_page: has_thumbnails,
                    };
                }
                "model/gltf-binary" | "model/gltf+json" => return Self::Model3d,
                "text/vtt" | "application/x-subrip" => return Self::Captions,
                media_type if media_type.starts_with("audio/") => return Self::Audio,
                media_type if media_type.starts_with("video/") => return Self::Video,
                media_type if OFFICE_DOCUMENT_MEDIA_TYPES.contains(&media_type) => {
                    return Self::Document;
                }
                _ => {}
            }
        }
        // An OpenDocument file's bytes are a zip, so its type is the container
        // and only the name says it is a document.
        match extension {
            Some(extension) if Self::is_office_document(extension) => Self::Document,
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
            Self::Document => "document",
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
            "document" => Self::Document,
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
            | Self::Document
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
            | Self::Document
            | Self::Model3d
            | Self::Captions
            | Self::Embed
            | Self::IiifPresentation
            | Self::OtherFile => None,
        }
    }

    /// Whether an extension names an office document. Case is the caller's, so
    /// an older row that stored `DOC` reads the same.
    fn is_office_document(extension: &str) -> bool {
        OFFICE_DOCUMENT_EXTENSIONS
            .iter()
            .any(|known| known.eq_ignore_ascii_case(extension))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    /// `ALL` is written by hand, so a new variant can miss it (`Document` did).
    /// The match stops compiling when a variant is added, which points here.
    #[test]
    fn all_names_every_kind() {
        let listed = |kind: super::MediaKind| {
            assert!(
                super::MediaKind::ALL
                    .iter()
                    .any(|each| each.name() == kind.name()),
                "{} is missing from MediaKind::ALL",
                kind.name()
            );
        };
        for kind in super::MediaKind::ALL {
            match kind {
                super::MediaKind::LocalImage
                | super::MediaKind::IiifImage
                | super::MediaKind::Audio
                | super::MediaKind::Video
                | super::MediaKind::Pdf { .. }
                | super::MediaKind::Document
                | super::MediaKind::Model3d
                | super::MediaKind::Captions
                | super::MediaKind::Embed
                | super::MediaKind::IiifPresentation
                | super::MediaKind::OtherFile => {}
            }
        }
        for kind in [
            super::MediaKind::LocalImage,
            super::MediaKind::IiifImage,
            super::MediaKind::Audio,
            super::MediaKind::Video,
            super::MediaKind::Pdf { first_page: true },
            super::MediaKind::Document,
            super::MediaKind::Model3d,
            super::MediaKind::Captions,
            super::MediaKind::Embed,
            super::MediaKind::IiifPresentation,
            super::MediaKind::OtherFile,
        ] {
            listed(kind);
        }
    }

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
            MediaKind::Document,
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
        assert_eq!(names.len(), 11, "names: {names:?}");
    }

    /// A name this server does not know matches no kind.
    #[test]
    fn an_unknown_name_is_no_kind() {
        assert_eq!(MediaKind::from_name("AUDIO"), None);
        assert_eq!(MediaKind::from_name("DOCUMENT"), None);
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
                extension: Some("nef"),
                has_thumbnails,
            })
        };
        assert_eq!(raw(true), MediaKind::LocalImage);
        assert_eq!(raw(false), MediaKind::OtherFile);
    }

    /// A HEIC, an AVIF and a BMP are local images only once a tool has made
    /// their derivatives. Before that they are other files: a type is never
    /// promised a canvas this install cannot paint.
    #[test]
    fn an_avif_heic_or_bmp_is_a_local_image_only_with_derivatives() {
        for (media_type, extension) in [
            ("image/avif", "avif"),
            ("image/heic", "heic"),
            ("image/bmp", "bmp"),
        ] {
            let kind = |has_thumbnails| {
                MediaKind::of(&KindInput {
                    renderer: "file",
                    ingester: "upload",
                    media_type: Some(media_type),
                    extension: Some(extension),
                    has_thumbnails,
                })
            };
            assert_eq!(kind(true), MediaKind::LocalImage, "{media_type}");
            assert_eq!(kind(false), MediaKind::OtherFile, "{media_type}");
            assert!(
                DISPLAY_IMAGE_MEDIA_TYPES.contains(&media_type),
                "{media_type}"
            );
        }
    }

    /// Every type a JPEG access master makes displayable is a display image, so
    /// its media gets a canvas once the master and the derivatives exist.
    #[test]
    fn an_access_master_type_is_a_display_image() {
        for media_type in [
            "application/illustrator",
            "application/postscript",
            "image/heif",
            "image/vnd.adobe.photoshop",
            "image/vnd.radiance",
            "image/x-eps",
            "image/x-exr",
            "image/x-photoshop",
        ] {
            assert!(
                DISPLAY_IMAGE_MEDIA_TYPES.contains(&media_type),
                "{media_type}"
            );
        }
    }

    /// An office document is its own kind, by the type the sniffer recorded or,
    /// when the bytes are only a zip, by the extension the upload kept.
    #[test]
    fn an_office_document_is_a_document() {
        let kind = |media_type: Option<&str>, extension: Option<&str>| {
            MediaKind::of(&KindInput {
                renderer: "file",
                ingester: "upload",
                media_type,
                extension,
                has_thumbnails: false,
            })
        };
        for media_type in OFFICE_DOCUMENT_MEDIA_TYPES {
            assert_eq!(
                kind(Some(media_type), None),
                MediaKind::Document,
                "{media_type}"
            );
        }
        for extension in OFFICE_DOCUMENT_EXTENSIONS {
            assert_eq!(
                kind(Some("application/zip"), Some(extension)),
                MediaKind::Document,
                "{extension}"
            );
        }
        assert_eq!(
            kind(
                Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
                None
            ),
            MediaKind::Document
        );
        // An uppercase extension is the same document.
        assert_eq!(kind(None, Some("DOCX")), MediaKind::Document);
        // A plain archive and a plain file stay other files.
        assert_eq!(
            kind(Some("application/zip"), Some("zip")),
            MediaKind::OtherFile
        );
        assert_eq!(kind(None, None), MediaKind::OtherFile);
    }
}
