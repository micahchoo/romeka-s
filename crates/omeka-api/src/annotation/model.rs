//! The domain of one media annotation: the canonical selector, the body and
//! the draft a client may set. No I/O and no wire key names; [`super::wadm`]
//! owns the JSON.
//!
//! A [`Selector`] exists only after the parser checked it against the media's
//! `Surface` and `Extent`, so nothing downstream re-checks geometry. The values
//! here are the ones the database stores: integer pixels, milliseconds and a
//! body list, so a read and a write meet in the same canonical draft.

use chrono::NaiveDateTime;
use omeka_db::DbError;
use omeka_db::ids::{ItemId, MediaId, UserId};
use omeka_db::repos::MediaAnnotationRow;
use omeka_media::kind::{Extent, MediaKind};
use serde_json::Map;

use super::wadm;

/// The most characters one annotation's text may hold (the table's CHECK).
pub(crate) const MAX_TEXT_CHARS: usize = 16_000;

/// The most bodies one draft may hold.
pub(crate) const MAX_BODIES: usize = 20;

/// The most bytes the unknown extension properties may serialize to.
pub(crate) const MAX_EXTENSION_BYTES: usize = 8 * 1024;

/// One annotation's row id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AnnotationId(pub i64);

/// The strong ETag `"r<revision>"`.
///
/// `If-Match` is compared strongly (RFC 9110 13.1.1): a weak tag never
/// matches, which is why a weak tag parses to a list that names no revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Revision(u32);

impl Revision {
    pub(crate) fn value(self) -> u32 {
        self.0
    }

    /// The ETag header value: `"r1"`.
    #[must_use]
    pub fn etag(self) -> String {
        format!("\"r{}\"", self.0)
    }
}

/// An `If-Match` header, which a replace and a delete require.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IfMatch {
    /// `*`: any current revision.
    Any,
    /// The revisions named by strong tags. A weak tag is dropped, so a header
    /// holding only weak tags matches nothing.
    Tags(Vec<Revision>),
}

impl IfMatch {
    /// Parse the header. `None` when it is empty or one of its strong tags is
    /// malformed.
    #[must_use]
    pub fn parse(header: &str) -> Option<Self> {
        let header = header.trim();
        if header == "*" {
            return Some(Self::Any);
        }
        let mut tags = Vec::new();
        let mut named = false;
        for part in header.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            named = true;
            if part.starts_with("W/") {
                continue;
            }
            let quoted = part.strip_prefix('"')?.strip_suffix('"')?;
            let digits = quoted.strip_prefix('r')?;
            if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            tags.push(Revision(digits.parse().ok()?));
        }
        named.then_some(Self::Tags(tags))
    }

    pub(crate) fn matches(&self, revision: Revision) -> bool {
        match self {
            Self::Any => true,
            Self::Tags(tags) => tags.contains(&revision),
        }
    }
}

/// The ten motivations the table accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motivation {
    Commenting,
    Tagging,
    Describing,
    Identifying,
    Highlighting,
    Linking,
    Classifying,
    Questioning,
    Replying,
    Supplementing,
}

impl Motivation {
    pub(crate) fn of(text: &str) -> Option<Self> {
        Some(match text {
            "commenting" => Self::Commenting,
            "tagging" => Self::Tagging,
            "describing" => Self::Describing,
            "identifying" => Self::Identifying,
            "highlighting" => Self::Highlighting,
            "linking" => Self::Linking,
            "classifying" => Self::Classifying,
            "questioning" => Self::Questioning,
            "replying" => Self::Replying,
            "supplementing" => Self::Supplementing,
            _ => return None,
        })
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Commenting => "commenting",
            Self::Tagging => "tagging",
            Self::Describing => "describing",
            Self::Identifying => "identifying",
            Self::Highlighting => "highlighting",
            Self::Linking => "linking",
            Self::Classifying => "classifying",
            Self::Questioning => "questioning",
            Self::Replying => "replying",
            Self::Supplementing => "supplementing",
        }
    }
}

/// The media formats a text body may store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextFormat {
    Plain,
    Markdown,
    /// Purified when the note is parsed, so a stored HTML body is already safe.
    Html,
}

impl TextFormat {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Plain => "text/plain",
            Self::Markdown => "text/markdown",
            Self::Html => "text/html",
        }
    }

    pub(crate) fn of(text: &str) -> Option<Self> {
        Some(match text {
            "text/plain" => Self::Plain,
            "text/markdown" => Self::Markdown,
            "text/html" => Self::Html,
            _ => return None,
        })
    }
}

/// One body of a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    /// A `TextualBody`.
    Text {
        value: String,
        format: TextFormat,
        language: Option<String>,
        purpose: Option<Motivation>,
    },
    /// `{ "id": "https://…", "purpose": "identifying" }`, an absolute http(s)
    /// IRI. `String` rather than a URL type because the workspace has no URL
    /// crate and the parser checks the scheme at the boundary.
    Link {
        id: String,
        purpose: Option<Motivation>,
    },
}

/// Unknown extension properties, kept verbatim under their compact-IRI keys.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extensions(Map<String, serde_json::Value>);

impl Extensions {
    pub(crate) fn of(map: Map<String, serde_json::Value>) -> Self {
        Self(map)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&String, &serde_json::Value)> {
        self.0.iter()
    }

    pub(crate) fn map(&self) -> &Map<String, serde_json::Value> {
        &self.0
    }
}

/// The half of a note a client may set. Everything else is the server's: the
/// id, the times, the revision, the owner, the origin and the target source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub motivation: Motivation,
    pub selector: Selector,
    pub extensions: Extensions,
    /// 0..=[`MAX_BODIES`]; a tagging or caption note needs one non-empty text
    /// body.
    pub bodies: Vec<Body>,
}

/// The canonical selector. `Rect` and `Polygon` are integer pixels in the
/// media canvas, upright; `Time` is milliseconds into the media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    Whole,
    /// Integer pixels, inside the canvas when its size is known.
    Rect {
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    },
    /// 3..=500 integer points. Rendered back as a fresh polygon, never the
    /// client's own SVG text.
    Polygon(Vec<(u32, u32)>),
    /// `start < end`, `end <= duration` when the duration is known.
    Time {
        start_ms: u64,
        end_ms: u64,
    },
}

/// Who wrote the row: a person, or a caption import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Author,
    Track,
}

impl Origin {
    fn of(text: &str) -> Option<Self> {
        match text {
            "author" => Some(Self::Author),
            "track" => Some(Self::Track),
            _ => None,
        }
    }
}

/// The media an annotation hangs on, as far as parsing and rendering need it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaRef {
    pub id: MediaId,
    pub item: ItemId,
    pub kind: MediaKind,
    pub extent: Extent,
}

/// One stored, well-formed annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    pub id: AnnotationId,
    pub media: MediaRef,
    pub revision: Revision,
    pub created: NaiveDateTime,
    pub modified: NaiveDateTime,
    pub owner: Option<UserId>,
    /// Not on the wire; a reader learns it from `Romeka-Track-Ids`.
    pub origin: Origin,
    pub draft: Draft,
}

impl Annotation {
    /// The only place a row becomes a domain value. A stored column the model
    /// cannot represent is a [`DbError::Column`], not a panic.
    pub(crate) fn from_row(media: MediaRef, row: &MediaAnnotationRow) -> Result<Self, DbError> {
        Ok(Self {
            id: AnnotationId(row.id),
            media,
            revision: Revision(row.revision),
            created: row.created,
            modified: row.modified,
            owner: row.owner_id,
            origin: Origin::of(&row.origin).ok_or_else(|| column("origin", &row.origin))?,
            draft: Draft {
                motivation: Motivation::of(&row.motivation)
                    .ok_or_else(|| column("motivation", &row.motivation))?,
                selector: selector_of_row(row)?,
                extensions: extensions_of_row(row.extensions.as_deref())?,
                bodies: wadm::bodies_of_row(&row.body_json)?,
            },
        })
    }
}

/// Why a document is a 400: one plain sentence the caller can show.
///
/// The message registry and the admin catalogue translate the API's own
/// messages; these carry no catalogue key yet, so the server sends the English
/// sentence as it stands (unit 5 owns the error document).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Problem {
    #[error("The request is not a Web Annotation.")]
    NotAnnotation,
    #[error("The note targets a canvas that is not this media's.")]
    ForeignTarget,
    #[error("This media takes notes about the whole file only.")]
    WholeFileOnly,
    #[error("A box on a video frame is not supported yet.")]
    BoxOnTimedMedia,
    #[error("A time range belongs on audio or video.")]
    TimeOnImage,
    #[error("The note's selector is not one this server accepts.")]
    BadSelector,
    #[error("The time range must end after it begins.")]
    EmptyTimeRange,
    #[error("The box or the polygon leaves the image.")]
    BoxOutsideCanvas,
    #[error("The time range runs past the media's length.")]
    PastDuration,
    #[error("The note's motivation is not one this server accepts.")]
    BadMotivation,
    #[error("The note's body is not one this server accepts.")]
    BadBody,
    #[error("A note may hold at most 20 bodies.")]
    TooManyBodies,
    #[error("A note's text may hold at most 16,000 characters.")]
    TextTooLong,
    #[error("A note with this motivation needs its text.")]
    NoTextBody,
    #[error("A caption note needs a time range.")]
    CaptionNeedsTime,
    #[error("This media has no timeline.")]
    NoTimeline,
    #[error("A caption language must be a tag such as en or pt-BR.")]
    BadLanguage,
    #[error("A note's extension properties may hold at most 8 KiB.")]
    ExtensionsTooLarge,
}

fn column(name: &'static str, value: &str) -> DbError {
    DbError::Column {
        column: name,
        reason: format!("`{value}` is not a value the annotation model holds"),
    }
}

fn selector_of_row(row: &MediaAnnotationRow) -> Result<Selector, DbError> {
    match row.selector_kind.as_str() {
        "whole" => Ok(Selector::Whole),
        "rect" => {
            let text = row
                .selector
                .as_deref()
                .ok_or_else(|| column("selector", "a rectangle without geometry"))?;
            let numbers: Vec<u32> = text
                .split(',')
                .map(parse_u32)
                .collect::<Option<_>>()
                .ok_or_else(|| column("selector", text))?;
            let [x, y, w, h] = numbers.as_slice() else {
                return Err(column("selector", text));
            };
            Ok(Selector::Rect {
                x: *x,
                y: *y,
                w: *w,
                h: *h,
            })
        }
        "polygon" => {
            let text = row
                .selector
                .as_deref()
                .ok_or_else(|| column("selector", "a polygon without geometry"))?;
            let mut points = Vec::new();
            for pair in text.split(' ') {
                let (x, y) = pair
                    .split_once(',')
                    .ok_or_else(|| column("selector", text))?;
                let x = parse_u32(x).ok_or_else(|| column("selector", text))?;
                let y = parse_u32(y).ok_or_else(|| column("selector", text))?;
                points.push((x, y));
            }
            Ok(Selector::Polygon(points))
        }
        "time" => {
            let start = row
                .t_start_ms
                .and_then(|ms| u64::try_from(ms).ok())
                .ok_or_else(|| column("t_start_ms", "a time range without a start"))?;
            let end = row
                .t_end_ms
                .and_then(|ms| u64::try_from(ms).ok())
                .ok_or_else(|| column("t_end_ms", "a time range without an end"))?;
            Ok(Selector::Time {
                start_ms: start,
                end_ms: end,
            })
        }
        other => Err(column("selector_kind", other)),
    }
}

fn extensions_of_row(text: Option<&str>) -> Result<Extensions, DbError> {
    let Some(text) = text else {
        return Ok(Extensions::default());
    };
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| column("extensions", &error.to_string()))?;
    let serde_json::Value::Object(map) = value else {
        return Err(column("extensions", "a value that is not an object"));
    };
    Ok(Extensions::of(map))
}

fn parse_u32(text: &str) -> Option<u32> {
    text.trim().parse().ok()
}
