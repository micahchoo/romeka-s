//! `romeka_media_derivative`: what a background job is doing for one media.
//!
//! Not an upstream table. One row per media and kind of derivative, written when a media is
//! created outside an import and settled by the worker that makes the derivative. `kind` names
//! the work, `state` where it stands, and `reason` the typed skip or failure a reader shows.
//!
//! `DerivativeKind::is_preview` names the kinds the media document's
//! `o-module-romeka:preview` member answers from. A media has at most one preview kind, because
//! a video's poster and a PDF's first page are how the three thumbnails are made, not a second
//! set of files.

use chrono::NaiveDateTime;

use crate::DbError;
use crate::backend::Backend;
use crate::ids::MediaId;

/// One kind of derivative the table knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DerivativeKind {
    /// The three JPEG thumbnails, from a decodable image.
    Thumbnail,
    /// A video's poster, made by `ffmpeg`, stored as the three thumbnails.
    Poster,
    /// A PDF's first page, made by `pdftoppm`, stored as the three thumbnails.
    PdfPage,
    /// A copy a browser can play or show, made by a later unit.
    AccessCopy,
}

impl DerivativeKind {
    /// Every kind, in the order a media document reads them.
    pub const ALL: [Self; 4] = [
        Self::Thumbnail,
        Self::Poster,
        Self::PdfPage,
        Self::AccessCopy,
    ];

    /// The name stored in `kind`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Thumbnail => "thumbnail",
            Self::Poster => "poster",
            Self::PdfPage => "pdf_page",
            Self::AccessCopy => "access_copy",
        }
    }

    /// The kind a stored name holds, or `None` for a name a later build wrote.
    #[must_use]
    pub fn parse(stored: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == stored)
    }

    /// Whether this kind is the still the media's preview member reports.
    #[must_use]
    pub const fn is_preview(self) -> bool {
        !matches!(self, Self::AccessCopy)
    }
}

/// Where a derivative stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DerivativeState {
    /// Queued, not claimed.
    Pending,
    /// Claimed by a worker.
    Running,
    /// The file or files are in the store.
    Ready,
    /// The work refused; `reason` says why.
    Failed,
    /// This kind does not apply to this media, or its tool is not installed.
    None,
}

impl DerivativeState {
    /// Every state, in the order the table's `CHECK` lists them.
    pub const ALL: [Self; 5] = [
        Self::Pending,
        Self::Running,
        Self::Ready,
        Self::Failed,
        Self::None,
    ];

    /// The name stored in `state`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::None => "none",
        }
    }

    /// The state a stored name holds.
    ///
    /// # Errors
    /// A [`DbError::Column`] for a name the table's `CHECK` does not allow.
    pub fn parse(stored: &str) -> Result<Self, DbError> {
        Self::ALL
            .into_iter()
            .find(|state| state.name() == stored)
            .ok_or_else(|| DbError::Column {
                column: "state",
                reason: format!("`{stored}` is not a derivative state"),
            })
    }
}

/// One `romeka_media_derivative` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivativeRow {
    pub media_id: MediaId,
    pub kind: DerivativeKind,
    pub state: DerivativeState,
    /// The typed skip or failure, in words a reader can show.
    pub reason: Option<String>,
    /// 0-100 while a long derivative runs.
    pub progress: Option<u8>,
    /// The external command the work used or needed, for example `ffmpeg`.
    pub tool: Option<String>,
    pub queued: NaiveDateTime,
    pub started: Option<NaiveDateTime>,
    pub finished: Option<NaiveDateTime>,
    pub attempts: u32,
}

impl DerivativeRow {
    /// Parse one database row. A kind or state this build does not know is a column error, so a
    /// row written by a newer build is refused rather than guessed at.
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        let kind = DB::text(row, "kind")?;
        let state = DB::text(row, "state")?;
        Ok(Self {
            media_id: DB::id(row, "media_id")?,
            kind: DerivativeKind::parse(&kind).ok_or_else(|| DbError::Column {
                column: "kind",
                reason: format!("`{kind}` is not a derivative kind"),
            })?,
            state: DerivativeState::parse(&state)?,
            reason: DB::opt_text(row, "reason")?,
            progress: percent(DB::opt_int(row, "progress")?)?,
            tool: DB::opt_text(row, "tool")?,
            queued: DB::datetime(row, "queued")?,
            started: DB::opt_datetime(row, "started")?,
            finished: DB::opt_datetime(row, "finished")?,
            attempts: attempts(DB::opt_int(row, "attempts")?)?,
        })
    }
}

fn percent(value: Option<i64>) -> Result<Option<u8>, DbError> {
    value
        .map(|raw| {
            u8::try_from(raw)
                .ok()
                .filter(|percent| *percent <= 100)
                .ok_or_else(|| DbError::Column {
                    column: "progress",
                    reason: format!("{raw} is not a percentage"),
                })
        })
        .transpose()
}

fn attempts(value: Option<i64>) -> Result<u32, DbError> {
    let raw = value.unwrap_or(0);
    u32::try_from(raw).map_err(|_| DbError::Column {
        column: "attempts",
        reason: format!("{raw} is not a count"),
    })
}

/// Insert input for one queued derivative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDerivative {
    pub media_id: MediaId,
    pub kind: DerivativeKind,
}
