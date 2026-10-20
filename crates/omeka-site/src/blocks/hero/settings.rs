//! The hero's typed settings, parsed from `o:layout_data.romeka.settings`.
//!
//! Every member reads its absence as the value a hero stored before the member
//! existed, so an old hero parses to the banner it always rendered: an asset
//! background, a medium height, a black overlay, an `<h2>` and one button.

use omeka_core::ids::{AssetId, MediaId, PageId};
use serde_json::Value as JsonValue;

use super::super::setting::{Button, text as text_of, whole as id_of};
use super::super::style::Colour;

/// The block's typed settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HeroSettings {
    pub(super) heading: String,
    pub(super) heading_level: HeadingLevel,
    pub(super) subheading: String,
    pub(super) body: String,
    pub(super) background: Background,
    pub(super) focal: Option<Focal>,
    pub(super) height: Height,
    pub(super) overlay: Overlay,
    pub(super) overlay_color: Option<Colour>,
    pub(super) alignment: Alignment,
    /// The filled button first, then the outlined one.
    pub(super) buttons: [Button; 2],
}

/// Where the background image comes from. The source choice decides which
/// stored id counts, so a hidden id left by a switch is never read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Background {
    None,
    Asset(AssetId),
    Media(MediaId),
}

/// The point of the image that stays in view, in whole percents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Focal {
    pub(super) x: u8,
    pub(super) y: u8,
}

impl Focal {
    /// The stored point, `None` when neither percent is stored, so an old hero
    /// keeps the browser's centred crop. A stored value outside 0 to 100 is
    /// clamped.
    fn parse(settings: &JsonValue) -> Option<Self> {
        let percent = |key: &str| {
            settings
                .get(key)
                .and_then(id_of)
                .map(|value| u8::try_from(value.min(100)).unwrap_or(100))
        };
        match (percent("focal_x"), percent("focal_y")) {
            (None, None) => None,
            (x, y) => Some(Self {
                x: x.unwrap_or(50),
                y: y.unwrap_or(50),
            }),
        }
    }

    /// The CSS `object-position` value.
    pub(super) fn css(self) -> String {
        format!("object-position: {}% {}%", self.x, self.y)
    }
}

/// The banner's height, a named step the theme sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Height {
    Short,
    Medium,
    Tall,
    Full,
}

impl Height {
    fn parse(value: Option<&str>) -> Self {
        match value {
            Some("short") => Self::Short,
            Some("tall") => Self::Tall,
            Some("full") => Self::Full,
            _ => Self::Medium,
        }
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Short => "short",
            Self::Medium => "medium",
            Self::Tall => "tall",
            Self::Full => "full",
        }
    }
}

/// The heading's element: `h1` for a page whose hero names it, else `h2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HeadingLevel {
    H1,
    H2,
}

impl HeadingLevel {
    pub(super) fn tag(self) -> &'static str {
        match self {
            Self::H1 => "h1",
            Self::H2 => "h2",
        }
    }
}

/// The overlay strength: how much the overlay colour covers the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Overlay {
    None,
    Light,
    Medium,
    Strong,
}

impl Overlay {
    fn parse(value: Option<&str>) -> Self {
        match value {
            Some("none") => Self::None,
            Some("light") => Self::Light,
            Some("strong") => Self::Strong,
            _ => Self::Medium,
        }
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Light => "light",
            Self::Medium => "medium",
            Self::Strong => "strong",
        }
    }
}

/// Where the banner's content sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Alignment {
    Left,
    Center,
    Right,
}

impl Alignment {
    fn parse(value: Option<&str>) -> Self {
        match value {
            Some("center") => Self::Center,
            Some("right") => Self::Right,
            _ => Self::Left,
        }
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }
}

impl HeroSettings {
    pub(super) fn parse(settings: &JsonValue) -> Self {
        let choice = |key: &str| settings.get(key).and_then(JsonValue::as_str);
        Self {
            heading: text_of(settings, "heading"),
            heading_level: match choice("heading_level") {
                Some("h1") => HeadingLevel::H1,
                _ => HeadingLevel::H2,
            },
            subheading: text_of(settings, "subheading"),
            body: text_of(settings, "body"),
            background: Background::parse(settings),
            focal: Focal::parse(settings),
            height: Height::parse(choice("height")),
            overlay: Overlay::parse(choice("overlay")),
            overlay_color: choice("overlay_color").and_then(Colour::parse),
            alignment: Alignment::parse(choice("alignment")),
            buttons: [
                Button::parse(settings, "cta"),
                Button::parse(settings, "cta2"),
            ],
        }
    }

    /// The pages the buttons name.
    pub(super) fn page_refs(&self) -> Vec<PageId> {
        self.buttons
            .iter()
            .filter_map(|button| button.page)
            .collect()
    }
}

impl Background {
    fn parse(settings: &JsonValue) -> Self {
        let source = settings
            .get("background_source")
            .and_then(JsonValue::as_str);
        if source == Some("media") {
            return settings
                .get("background_media")
                .and_then(id_of)
                .map_or(Self::None, |id| Self::Media(MediaId::new(id)));
        }
        settings
            .get("background")
            .and_then(id_of)
            .map_or(Self::None, |id| Self::Asset(AssetId::new(id)))
    }
}
