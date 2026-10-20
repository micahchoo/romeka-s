//! The gallery's typed settings, parsed from `o:layout_data.romeka.settings`.

use omeka_core::ids::{AssetId, ItemId, MediaId};
use serde_json::Value as JsonValue;

use super::super::setting::{flag, records, text, whole, whole_at};

/// The block's typed settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GallerySettings {
    pub(super) pictures: Pictures,
    pub(super) layout: Layout,
    pub(super) columns: u8,
    pub(super) captions: bool,
    pub(super) lightbox: bool,
}

/// Where the images come from. The source choice decides which stored list
/// counts, so a list hidden by a switch is never read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Pictures {
    Media(Vec<MediaRecord>),
    Assets(Vec<AssetRecord>),
}

/// One item of the media list: the item, the media chosen from it (its main
/// media when none is), and the caption html.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MediaRecord {
    pub(super) item: ItemId,
    pub(super) media: Option<MediaId>,
    pub(super) caption: String,
}

/// One asset of the asset list with its caption html.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AssetRecord {
    pub(super) asset: AssetId,
    pub(super) caption: String,
}

/// How the images line up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Layout {
    /// Equal cells, each image cropped to a square.
    Grid,
    /// Columns of whole images, each at its own height.
    Masonry,
    /// One row that scrolls sideways, a few images in view.
    Carousel,
}

impl Layout {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Grid => "grid",
            Self::Masonry => "masonry",
            Self::Carousel => "carousel",
        }
    }
}

/// The fewest and the most columns a gallery lays out, and the default.
pub(super) const COLUMNS: (u8, u8, u8) = (2, 6, 3);

impl GallerySettings {
    pub(super) fn parse(settings: &JsonValue) -> Self {
        let pictures = if text(settings, "source") == "assets" {
            Pictures::Assets(
                records(settings, "assets")
                    .filter_map(|record| {
                        Some(AssetRecord {
                            asset: AssetId::new(whole_at(record, "asset")?),
                            caption: text(record, "caption"),
                        })
                    })
                    .collect(),
            )
        } else {
            Pictures::Media(
                records(settings, "media")
                    .filter_map(|record| {
                        Some(MediaRecord {
                            item: ItemId::new(whole_at(record, "item")?),
                            media: record.get("media").and_then(whole).map(MediaId::new),
                            caption: text(record, "caption"),
                        })
                    })
                    .collect(),
            )
        };
        let (fewest, most, default) = COLUMNS;
        Self {
            pictures,
            layout: match text(settings, "layout").as_str() {
                "masonry" => Layout::Masonry,
                "carousel" => Layout::Carousel,
                _ => Layout::Grid,
            },
            columns: whole_at(settings, "columns")
                .and_then(|columns| u8::try_from(columns).ok())
                .map_or(default, |columns| columns.clamp(fewest, most)),
            captions: flag(settings, "captions", true),
            lightbox: flag(settings, "lightbox", true),
        }
    }
}
