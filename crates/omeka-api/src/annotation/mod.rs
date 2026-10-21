//! Media annotations: one W3C Web Annotation per row, on a media, read and
//! written through one service.
//!
//! The service hides the wire (`wadm`), the canonical model (`model`), the
//! access rule and the switch (`access`), and the row mapping. A caller that
//! holds an [`Annotations`] cannot build a selector the media's surface does
//! not allow, cannot see a note its rights hide, and cannot overwrite a newer
//! revision.
//!
//! The container, the page and the HTTP status codes are the server's
//! annotations routes; this module answers in domain values and
//! [`AnnotationError`]. Captions share the rows: a caption is a
//! `supplementing` row with a time range, and its import is a later unit.

mod access;
mod model;
mod search;
pub mod tracks;
mod undo;
mod wadm;

pub use access::search_scope;
pub use model::{
    Annotation, AnnotationId, Body, Draft, Extensions, IfMatch, MediaRef, Motivation, Origin,
    Problem, Revision, Selector, TextFormat,
};
pub use tracks::{TrackError, TrackFormat};
pub use undo::{
    KeptCue, KeptEdition, KeptEditions, KeptSummary, NoKeptEditions, TrackUndo, decode, encode,
    none as no_kept, storage_path,
};
pub use wadm::{Iris, iso, render};

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDateTime;
use omeka_db::DbError;
use omeka_db::ids::MediaId;
use omeka_db::repos::MediaRepo;
use omeka_db::repos::{
    ChangeOutcome, Cue, InsertOutcome, MediaAnnotationRepo, MediaAnnotationRow,
    MediaAnnotationWrite, RestoredCue, TrackOutcome, UndoOutcome,
};
use omeka_db::rows::MediaRow;
use omeka_db::{DbPool, with_db};
use omeka_media::kind::{Extent, KindInput, MediaKind, Surface};

use crate::auth::Actor;
use crate::read::Api;
use crate::settings;

use tracks::{ExistingCue, ParsedCue};
use wadm::Stored;

/// The most annotations one page of a container holds.
const PAGE_SIZE: u32 = 1000;

/// Size and duration of a media.
///
/// Implemented once, by the server's IIIF extents (image headers through the
/// store, audio and video through the extent table and ffprobe). A test supplies
/// a fake, so this unit needs no file store and no probe.
pub trait ExtentSource: Send + Sync {
    fn extent<'a>(
        &'a self,
        media: &'a MediaRow,
    ) -> Pin<Box<dyn Future<Output = Extent> + Send + 'a>>;
}

/// What a caller may do with one media's annotations, decided once per request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rights {
    /// The switch is off and the caller holds no write grant: only captions.
    ReadCaptionsOnly,
    /// The caller may read the media and every note on it.
    Read,
    /// The caller holds the media-update grant.
    ReadWrite,
}

/// One keyset page of a container.
#[derive(Debug, Clone)]
pub struct Page {
    pub media: MediaRef,
    pub rights: Rights,
    pub total: u32,
    pub items: Vec<Annotation>,
    /// The cursor this page started after.
    pub after: Option<AnnotationId>,
    /// `Some` when another page exists.
    pub next: Option<AnnotationId>,
    /// Which rows this page lists; its `next` link keeps the filter.
    pub origins: OriginFilter,
}

/// What a public page shows without JavaScript: the total and the first N.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub total: u32,
    pub first: Vec<Annotation>,
    pub extent: Extent,
    pub tracks: Vec<TrackRef>,
}

/// One caption language of a media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackRef {
    pub lang: Option<String>,
    pub cues: u32,
}

/// Which rows a notes view reads.
///
/// The notes container leaves a caption import's rows out by default: a video
/// with two five thousand cue tracks would otherwise load ten thousand rows
/// into a note list. `All` is for a caller that asks for every row explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OriginFilter {
    /// What a person wrote.
    #[default]
    Author,
    /// Every row, captions included.
    All,
}

/// The answer to a caption import (contract 6.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackImport {
    pub lang: Option<String>,
    pub cues: u32,
    /// The import replaced existing track rows of the language.
    pub replaced: bool,
    /// The ids of the language's track rows, for `Romeka-Track-Ids`. Not on the
    /// wire.
    pub ids: Vec<i64>,
}

/// One caption language as the editor's list shows it (G3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackSummary {
    pub lang: Option<String>,
    pub cues: u32,
    /// When the language's track rows were imported; `None` when it holds only
    /// hand-written `supplementing` notes.
    pub imported: Option<NaiveDateTime>,
    /// Track rows changed after their import (`revision > 1`).
    pub hand_edited: u32,
    /// The edition a replace kept, when one undo would bring it back (G6).
    pub undo: Option<KeptSummary>,
}

/// What a caption file would change, in milliseconds, before it is stored (G5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackPreview {
    pub cues: u32,
    pub start_ms: u64,
    pub end_ms: u64,
    /// The existing track rows the import would replace.
    pub replaces: u32,
    /// Hand-edited rows whose words the import would change or drop.
    pub hand_edited: u32,
}

/// The answer to a create: a new row, or the row a retried key names.
#[derive(Debug, Clone)]
pub enum Created {
    New(Annotation),
    Replayed(Annotation),
}

/// A client-chosen create key: 1 to 64 characters of `[A-Za-z0-9._:-]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    /// `None` for a key outside the alphabet or the length bound.
    #[must_use]
    pub fn parse(header: &str) -> Option<Self> {
        let key = header.trim();
        let shaped = !key.is_empty()
            && key.len() <= 64
            && key.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | ':' | '-')
            });
        shaped.then(|| Self(key.to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// The service's failures, before the HTTP layer names a status for each.
#[derive(Debug, thiserror::Error)]
pub enum AnnotationError {
    /// Also a media the caller may not read: hidden is absent.
    #[error("the annotation or its media is not there")]
    NotFound,
    #[error("the caller may read this media but may not update it")]
    Forbidden,
    #[error("{0}")]
    Invalid(Problem),
    #[error("the annotation document is larger than 64 KiB")]
    TooLarge,
    #[error("the request body is a type this route does not serve")]
    UnsupportedType,
    /// The current note, for the editor's conflict view.
    #[error("the annotation changed since it was read")]
    Stale(Box<Annotation>),
    #[error("If-Match is required")]
    PreconditionRequired,
    #[error("the idempotency key was used with a different draft")]
    KeyReused,
    /// A caption file the parser refused, with the parser's own sentence and
    /// the status its kind earns (413 for a bound, 400 for the grammar).
    #[error(transparent)]
    Track(#[from] TrackError),
    /// An undo of a replace with no kept edition to bring back, or with a live
    /// edition the kept one did not replace.
    #[error(
        "There is no caption edition to bring back. An undo works once, until the next import of these captions."
    )]
    NothingToUndo,
    #[error(transparent)]
    Db(#[from] DbError),
}

impl From<Problem> for AnnotationError {
    fn from(problem: Problem) -> Self {
        Self::Invalid(problem)
    }
}

/// The annotation service for one caller. Every method checks access itself.
pub struct Annotations<'a> {
    pool: &'a DbPool,
    actor: &'a Actor,
    extents: &'a dyn ExtentSource,
    kept: &'a dyn KeptEditions,
}

impl<'a> Annotations<'a> {
    /// A service that keeps no caption edition, so no undo is on offer. The
    /// readers want this; the caption routes pass [`Self::keeping`].
    pub fn new(api: &'a Api<'a>, extents: &'a dyn ExtentSource) -> Self {
        Self {
            pool: api.pool(),
            actor: api.actor(),
            extents,
            kept: undo::none(),
        }
    }

    /// A service that keeps one caption edition per language for an undo.
    pub fn keeping(
        api: &'a Api<'a>,
        extents: &'a dyn ExtentSource,
        kept: &'a dyn KeptEditions,
    ) -> Self {
        Self {
            pool: api.pool(),
            actor: api.actor(),
            extents,
            kept,
        }
    }

    /// The same service for a caller the public site resolved itself: the site
    /// layer holds the pool and the request's actor but no [`Api`], and building
    /// one would load a second term catalogue for every page.
    pub fn for_reader(pool: &'a DbPool, actor: &'a Actor, extents: &'a dyn ExtentSource) -> Self {
        Self {
            pool,
            actor,
            extents,
            kept: undo::none(),
        }
    }

    /// The media as parsing and rendering need it, with the caller's rights.
    /// `NotFound` when the caller may not read the media.
    pub async fn rights(&self, media: MediaId) -> Result<(MediaRef, Rights), AnnotationError> {
        self.readable_one(media).await
    }

    /// One keyset page, in id order, every row of the media.
    pub async fn page(
        &self,
        media: MediaId,
        after: Option<AnnotationId>,
    ) -> Result<Page, AnnotationError> {
        self.page_of(media, after, OriginFilter::All).await
    }

    /// One keyset page under an origin filter. The notes container asks for
    /// [`OriginFilter::Author`] so its list is notes, not captions.
    pub async fn page_of(
        &self,
        media: MediaId,
        after: Option<AnnotationId>,
        origins: OriginFilter,
    ) -> Result<Page, AnnotationError> {
        self.page_sized(media, after, origins, PAGE_SIZE).await
    }

    /// One keyset page of the notes a reader sees, at most `limit` rows: a
    /// caption cue is a row but not a note, so a public list leaves the cues
    /// out. `next` is the cursor of the last row on this page.
    pub async fn notes_page(
        &self,
        media: MediaId,
        after: Option<AnnotationId>,
        limit: u32,
    ) -> Result<Page, AnnotationError> {
        self.page_sized(media, after, OriginFilter::Author, limit)
            .await
    }

    async fn page_sized(
        &self,
        media: MediaId,
        after: Option<AnnotationId>,
        origins: OriginFilter,
        limit: u32,
    ) -> Result<Page, AnnotationError> {
        let (media_ref, rights) = self.readable_one(media).await?;
        let motivations = access::motivations(rights);
        let size = usize::try_from(limit).unwrap_or(usize::MAX);
        let mut rows = with_db!(self.pool, |db| {
            let repo = match origins {
                OriginFilter::Author => MediaAnnotationRepo::new(db).authored(),
                OriginFilter::All => MediaAnnotationRepo::new(db),
            };
            repo.page(media, after.map_or(0, |id| id.0), limit, motivations)
                .await
        })?;
        let next = if rows.len() > size {
            rows.get(size - 1).map(|row| AnnotationId(row.id))
        } else {
            None
        };
        rows.truncate(size);
        let total = with_db!(self.pool, |db| {
            let repo = match origins {
                OriginFilter::Author => MediaAnnotationRepo::new(db).authored(),
                OriginFilter::All => MediaAnnotationRepo::new(db),
            };
            repo.total(media, motivations).await
        })?;
        let items = annotations(media_ref, &rows)?;
        Ok(Page {
            media: media_ref,
            rights,
            total,
            items,
            after,
            next,
            origins,
        })
    }

    /// One query for the notes of every readable media on a public page; media
    /// the caller may not read are absent.
    pub async fn summaries(
        &self,
        media: &[MediaId],
        first: u32,
    ) -> Result<HashMap<MediaId, Summary>, AnnotationError> {
        let readable = self.readable(media).await?;
        if readable.is_empty() {
            return Ok(HashMap::new());
        }
        let (open, hidden) = split(&readable);
        let mut rows = Vec::new();
        let mut totals = HashMap::new();
        if !open.is_empty() {
            rows = with_db!(self.pool, |db| MediaAnnotationRepo::new(db)
                .authored()
                .firsts(&open, first, None)
                .await)?;
            totals = with_db!(self.pool, |db| MediaAnnotationRepo::new(db)
                .authored()
                .counts(&open, None)
                .await)?;
        }
        if !hidden.is_empty() {
            rows.extend(with_db!(self.pool, |db| MediaAnnotationRepo::new(db)
                .authored()
                .firsts(&hidden, first, Some(&["supplementing"]))
                .await)?);
            totals.extend(with_db!(self.pool, |db| MediaAnnotationRepo::new(db)
                .authored()
                .counts(&hidden, Some(&["supplementing"]))
                .await)?);
        }
        let ids: Vec<MediaId> = readable.iter().map(|(media, _)| media.id).collect();
        let mut tracks: HashMap<MediaId, Vec<TrackRef>> = HashMap::new();
        for (media, lang, cues) in with_db!(self.pool, |db| MediaAnnotationRepo::new(db)
            .track_langs(&ids)
            .await)?
        {
            tracks
                .entry(media)
                .or_default()
                .push(TrackRef { lang, cues });
        }
        let by_id: HashMap<MediaId, MediaRef> = readable
            .iter()
            .map(|(media, _)| (media.id, *media))
            .collect();
        let mut grouped: HashMap<MediaId, Vec<Annotation>> = HashMap::new();
        for row in &rows {
            let Some(media) = by_id.get(&row.media_id).copied() else {
                continue;
            };
            grouped
                .entry(row.media_id)
                .or_default()
                .push(Annotation::from_row(media, row)?);
        }
        let mut summaries = HashMap::with_capacity(readable.len());
        for (media, _) in readable {
            summaries.insert(
                media.id,
                Summary {
                    total: totals.get(&media.id).copied().unwrap_or(0),
                    first: grouped.remove(&media.id).unwrap_or_default(),
                    extent: media.extent,
                    tracks: tracks.remove(&media.id).unwrap_or_default(),
                },
            );
        }
        Ok(summaries)
    }

    /// The note count of every readable media, under the caller's rights.
    pub async fn counts(
        &self,
        media: &[MediaId],
    ) -> Result<HashMap<MediaId, u32>, AnnotationError> {
        let readable = self.readable(media).await?;
        let (open, hidden) = split(&readable);
        let mut totals = HashMap::new();
        if !open.is_empty() {
            totals = with_db!(self.pool, |db| MediaAnnotationRepo::new(db)
                .authored()
                .counts(&open, None)
                .await)?;
        }
        if !hidden.is_empty() {
            totals.extend(with_db!(self.pool, |db| MediaAnnotationRepo::new(db)
                .authored()
                .counts(&hidden, Some(&["supplementing"]))
                .await)?);
        }
        for (media, _) in &readable {
            totals.entry(media.id).or_insert(0);
        }
        Ok(totals)
    }

    pub async fn get(&self, id: AnnotationId) -> Result<Annotation, AnnotationError> {
        let Some(row) = self.row_of(id).await? else {
            return Err(AnnotationError::NotFound);
        };
        let (media, rights) = self.readable_one(row.media_id).await?;
        if !access::holds(rights, &row.motivation) {
            return Err(AnnotationError::NotFound);
        }
        Ok(Annotation::from_row(media, &row)?)
    }

    /// A key that matches a row of this media with an equal draft answers
    /// [`Created::Replayed`]; with a different draft it is `KeyReused`.
    pub async fn create(
        &self,
        media: MediaId,
        doc: &[u8],
        iris: &Iris,
        key: Option<&IdempotencyKey>,
    ) -> Result<Created, AnnotationError> {
        let (media_ref, rights) = self.readable_one(media).await?;
        if rights != Rights::ReadWrite {
            return Err(AnnotationError::Forbidden);
        }
        let draft = wadm::parse(doc, &media_ref, iris)?;
        let stored = wadm::stored(&draft);
        let write = write_of(&draft, &stored);
        let outcome = with_db!(self.pool, |db| MediaAnnotationRepo::new(db)
            .insert(
                media,
                self.actor.user(),
                key.map(IdempotencyKey::as_str),
                &write
            )
            .await)?;
        match outcome {
            InsertOutcome::Inserted(row) => {
                Ok(Created::New(Annotation::from_row(media_ref, &row)?))
            }
            InsertOutcome::KeyTaken(row) => {
                let existing = Annotation::from_row(media_ref, &row)?;
                if existing.draft == draft {
                    Ok(Created::Replayed(existing))
                } else {
                    Err(AnnotationError::KeyReused)
                }
            }
        }
    }

    /// An equal canonical draft returns the row and bumps no revision.
    pub async fn replace(
        &self,
        id: AnnotationId,
        if_match: &IfMatch,
        doc: &[u8],
        iris: &Iris,
    ) -> Result<Annotation, AnnotationError> {
        let current = self.writable(id, if_match).await?;
        let draft = wadm::parse(doc, &current.media, iris)?;
        if draft == current.draft {
            return Ok(current);
        }
        let stored = wadm::stored(&draft);
        let write = write_of(&draft, &stored);
        let outcome = with_db!(self.pool, |db| MediaAnnotationRepo::new(db)
            .replace(id.0, current.revision.value(), &write)
            .await)?;
        match outcome {
            ChangeOutcome::Changed { .. } => {
                let Some(row) = self.row_of(id).await? else {
                    return Err(AnnotationError::NotFound);
                };
                Ok(Annotation::from_row(current.media, &row)?)
            }
            ChangeOutcome::Stale { current: row } => Err(AnnotationError::Stale(Box::new(
                Annotation::from_row(current.media, &row)?,
            ))),
            ChangeOutcome::Missing => Err(AnnotationError::NotFound),
        }
    }

    pub async fn delete(
        &self,
        id: AnnotationId,
        if_match: &IfMatch,
    ) -> Result<(), AnnotationError> {
        let current = self.writable(id, if_match).await?;
        let outcome = with_db!(self.pool, |db| MediaAnnotationRepo::new(db)
            .delete(id.0, current.revision.value())
            .await)?;
        match outcome {
            ChangeOutcome::Changed { .. } => Ok(()),
            ChangeOutcome::Stale { current: row } => Err(AnnotationError::Stale(Box::new(
                Annotation::from_row(current.media, &row)?,
            ))),
            ChangeOutcome::Missing => Err(AnnotationError::NotFound),
        }
    }

    /// Replace the track rows of `media` in `lang` with the cues of `body`
    /// (contract 6.5). The server is the only cue parser.
    ///
    /// The rows this import replaces are kept as the edition [`Self::undo_track`]
    /// brings back (G6). They are kept after the rows commit: a store that
    /// cannot write them costs the undo, never the import.
    pub async fn import_track(
        &self,
        media: MediaId,
        lang: Option<&str>,
        body: &[u8],
        format: TrackFormat,
    ) -> Result<TrackImport, AnnotationError> {
        let (media_ref, rights) = self.readable_one(media).await?;
        if rights != Rights::ReadWrite {
            return Err(AnnotationError::Forbidden);
        }
        if media_ref.kind.surface() != Surface::Time {
            return Err(Problem::NoTimeline.into());
        }
        let lang = language(lang)?;
        let cues = parse_cues(body, format)?;
        let bodies: Vec<String> = cues
            .iter()
            .map(|cue| cue_body(&cue.text, lang.as_deref()))
            .collect();
        let rows: Vec<Cue<'_>> = cues
            .iter()
            .zip(&bodies)
            .map(|(cue, body_json)| Cue {
                start_ms: i64::try_from(cue.start_ms).unwrap_or(i64::MAX),
                end_ms: i64::try_from(cue.end_ms).unwrap_or(i64::MAX),
                body_json,
                body_text: &cue.text,
            })
            .collect();
        let outcome = with_db!(self.pool, |db| MediaAnnotationRepo::new(db)
            .replace_track(media, lang.as_deref(), &rows)
            .await)?;
        let (replaced, kept, live_max) = match outcome {
            TrackOutcome::Replaced { kept, live_max, .. } => (!kept.is_empty(), kept, live_max),
            TrackOutcome::Unchanged { .. } => (false, Vec::new(), 0),
        };
        let stored = self.cue_rows(media, lang.as_deref()).await?;
        let ids = if stored.len() > PAGE_SIZE as usize {
            Vec::new()
        } else {
            stored
                .iter()
                .filter(|row| row.origin == "track")
                .map(|row| row.id)
                .collect()
        };
        if replaced {
            self.keep_edition(media, lang.as_deref(), &kept, live_max)
                .await;
        }
        Ok(TrackImport {
            lang,
            cues: u32::try_from(cues.len()).unwrap_or(u32::MAX),
            replaced,
            ids,
        })
    }

    /// Keep the rows an import replaced as the edition one undo brings back, or
    /// drop the previous edition when the language held no track row.
    async fn keep_edition(
        &self,
        media: MediaId,
        lang: Option<&str>,
        kept: &[MediaAnnotationRow],
        replaced_by: i64,
    ) {
        match KeptEdition::of(media, lang, replaced_by, kept) {
            Some(edition) if !edition.cues.is_empty() => {
                self.kept.keep(media, lang, &edition).await;
            }
            _ => self.kept.forget(media, lang).await,
        }
    }

    /// Bring back the edition the last import of `lang` replaced, in one
    /// transaction, and drop it so a second undo finds nothing (G6).
    pub async fn undo_track(
        &self,
        media: MediaId,
        lang: Option<&str>,
    ) -> Result<TrackUndo, AnnotationError> {
        let (_, rights) = self.readable_one(media).await?;
        if rights != Rights::ReadWrite {
            return Err(AnnotationError::Forbidden);
        }
        let lang = language(lang)?;
        let Some(edition) = self.kept.kept(media, lang.as_deref()).await else {
            return Err(AnnotationError::NothingToUndo);
        };
        let texts: Vec<String> = edition
            .cues
            .iter()
            .map(|cue| undo::cue_text(&cue.body_json))
            .collect::<Result<_, _>>()?;
        let restored: Vec<RestoredCue<'_>> = edition
            .cues
            .iter()
            .zip(&texts)
            .map(|(cue, body_text)| RestoredCue {
                id: cue.id,
                revision: cue.revision,
                created: cue.created,
                modified: cue.modified,
                start_ms: cue.start_ms,
                end_ms: cue.end_ms,
                body_json: &cue.body_json,
                body_text,
            })
            .collect();
        let outcome = with_db!(self.pool, |db| MediaAnnotationRepo::new(db)
            .restore_track(media, lang.as_deref(), &restored, edition.replaced_by)
            .await)?;
        match outcome {
            UndoOutcome::Restored { cues } => {
                self.kept.forget(media, lang.as_deref()).await;
                Ok(TrackUndo {
                    lang,
                    cues,
                    imported: edition.imported,
                })
            }
            UndoOutcome::Nothing => Err(AnnotationError::NothingToUndo),
        }
    }

    /// What [`Self::import_track`] would change, with no row written (G5).
    ///
    /// Both halves read the same parser, so a preview cannot disagree with the
    /// commit it previews.
    pub async fn preview_track(
        &self,
        media: MediaId,
        lang: Option<&str>,
        body: &[u8],
        format: TrackFormat,
    ) -> Result<TrackPreview, AnnotationError> {
        let (media_ref, rights) = self.readable_one(media).await?;
        if rights != Rights::ReadWrite {
            return Err(AnnotationError::Forbidden);
        }
        if media_ref.kind.surface() != Surface::Time {
            return Err(Problem::NoTimeline.into());
        }
        let lang = language(lang)?;
        let cues = parse_cues(body, format)?;
        let rows = self.cue_rows(media, lang.as_deref()).await?;
        let mut existing_rows: Vec<(u64, u64, String, bool)> = Vec::new();
        for row in rows.iter().filter(|row| row.origin == "track") {
            existing_rows.push((
                u64::try_from(row.t_start_ms.unwrap_or_default()).unwrap_or_default(),
                u64::try_from(row.t_end_ms.unwrap_or_default()).unwrap_or_default(),
                row_text(row)?,
                row.revision > 1,
            ));
        }
        let existing: Vec<ExistingCue<'_>> = existing_rows
            .iter()
            .map(|(start_ms, end_ms, text, hand_edited)| ExistingCue {
                start_ms: *start_ms,
                end_ms: *end_ms,
                text,
                hand_edited: *hand_edited,
            })
            .collect();
        let diff = tracks::diff(&existing, &cues);
        Ok(TrackPreview {
            cues: diff.cues,
            start_ms: diff.start_ms,
            end_ms: diff.end_ms,
            replaces: diff.replaces,
            hand_edited: diff.hand_edited_lost,
        })
    }

    /// The caption languages of `media` and their cue counts (G3), each with the
    /// edition an undo would bring back (G6).
    pub async fn tracks(&self, media: MediaId) -> Result<Vec<TrackSummary>, AnnotationError> {
        self.readable_one(media).await?;
        let langs = with_db!(self.pool, |db| MediaAnnotationRepo::new(db)
            .track_langs(&[media])
            .await)?;
        let mut summaries = Vec::with_capacity(langs.len());
        for (_, lang, cues) in langs {
            let rows = self.cue_rows(media, lang.as_deref()).await?;
            let track: Vec<&MediaAnnotationRow> =
                rows.iter().filter(|row| row.origin == "track").collect();
            let imported = track.iter().map(|row| row.created).max();
            let hand_edited = track.iter().filter(|row| row.revision > 1).count();
            let live_max = track.iter().map(|row| row.id).max().unwrap_or(0);
            let undo = self
                .kept
                .kept(media, lang.as_deref())
                .await
                .filter(|edition| edition.replaced_by == live_max)
                .map(|edition| KeptSummary {
                    cues: count(edition.cues.len()),
                    imported: edition.imported,
                });
            summaries.push(TrackSummary {
                lang,
                cues,
                imported,
                hand_edited: count(hand_edited),
                undo,
            });
        }
        Ok(summaries)
    }

    /// Remove the track rows of one language (G4). Hand-written notes, a
    /// hand-written `supplementing` note included, stay; `NotFound` when the
    /// language holds no track row. A removal is not undoable: the kept edition
    /// goes with it.
    pub async fn remove_track(
        &self,
        media: MediaId,
        lang: Option<&str>,
    ) -> Result<(), AnnotationError> {
        let (_, rights) = self.readable_one(media).await?;
        if rights != Rights::ReadWrite {
            return Err(AnnotationError::Forbidden);
        }
        let lang = language(lang)?;
        let outcome = with_db!(self.pool, |db| MediaAnnotationRepo::new(db)
            .replace_track(media, lang.as_deref(), &[])
            .await)?;
        match outcome {
            TrackOutcome::Replaced { .. } => {
                self.kept.forget(media, lang.as_deref()).await;
                Ok(())
            }
            TrackOutcome::Unchanged { .. } => Err(AnnotationError::NotFound),
        }
    }

    /// Every visible `supplementing` row of `media` in `lang`, as WebVTT
    /// (contract 6.5). Without a language, the one with the most cues.
    pub async fn captions_vtt(
        &self,
        media: MediaId,
        lang: Option<&str>,
    ) -> Result<Option<String>, AnnotationError> {
        self.readable_one(media).await?;
        let lang = match language(lang)? {
            Some(lang) => Some(lang),
            None => self.default_lang(media).await?,
        };
        let rows = self.cue_rows(media, lang.as_deref()).await?;
        if rows.is_empty() {
            return Ok(None);
        }
        let mut timed: Vec<(u64, u64, String)> = Vec::with_capacity(rows.len());
        for row in &rows {
            timed.push((
                u64::try_from(row.t_start_ms.unwrap_or_default()).unwrap_or_default(),
                u64::try_from(row.t_end_ms.unwrap_or_default()).unwrap_or_default(),
                row_text(row)?,
            ));
        }
        let cues: Vec<(u64, u64, &str)> = timed
            .iter()
            .map(|(start, end, text)| (*start, *end, text.as_str()))
            .collect();
        Ok(Some(tracks::render_vtt(&cues)))
    }

    /// The language with the most cues; a named language beats an untagged one
    /// at an equal count, and the alphabetically first tag breaks a tie.
    async fn default_lang(&self, media: MediaId) -> Result<Option<String>, AnnotationError> {
        let langs = with_db!(self.pool, |db| MediaAnnotationRepo::new(db)
            .track_langs(&[media])
            .await)?;
        let mut best: Option<(Option<String>, u32)> = None;
        for (_, lang, cues) in langs {
            let wins = match &best {
                None => true,
                Some((current, count)) => {
                    cues > *count
                        || (cues == *count && named_first(lang.as_deref(), current.as_deref()))
                }
            };
            if wins {
                best = Some((lang, cues));
            }
        }
        Ok(best.and_then(|(lang, _)| lang))
    }

    /// Every `supplementing` row of `media` in `lang`, in time order.
    async fn cue_rows(
        &self,
        media: MediaId,
        lang: Option<&str>,
    ) -> Result<Vec<MediaAnnotationRow>, AnnotationError> {
        Ok(with_db!(self.pool, |db| MediaAnnotationRepo::new(db)
            .cues(media, lang)
            .await)?)
    }

    /// The current row, once the caller's write right and `If-Match` hold.
    async fn writable(
        &self,
        id: AnnotationId,
        if_match: &IfMatch,
    ) -> Result<Annotation, AnnotationError> {
        let Some(row) = self.row_of(id).await? else {
            return Err(AnnotationError::NotFound);
        };
        let (media, rights) = self.readable_one(row.media_id).await?;
        if rights != Rights::ReadWrite {
            return Err(AnnotationError::Forbidden);
        }
        let current = Annotation::from_row(media, &row)?;
        if if_match.matches(current.revision) {
            Ok(current)
        } else {
            Err(AnnotationError::Stale(Box::new(current)))
        }
    }

    async fn row_of(
        &self,
        id: AnnotationId,
    ) -> Result<Option<MediaAnnotationRow>, AnnotationError> {
        Ok(with_db!(self.pool, |db| MediaAnnotationRepo::new(db)
            .get(id.0)
            .await)?)
    }

    async fn readable_one(&self, media: MediaId) -> Result<(MediaRef, Rights), AnnotationError> {
        self.readable(&[media])
            .await?
            .into_iter()
            .next()
            .ok_or(AnnotationError::NotFound)
    }

    /// The media the caller may read, with its rights. Media the caller may not
    /// read are absent, which is the 404 every route answers for them.
    async fn readable(
        &self,
        media: &[MediaId],
    ) -> Result<Vec<(MediaRef, Rights)>, AnnotationError> {
        let annotations_public = settings::annotations_public(self.pool).await?;
        let mut seen = HashSet::new();
        let mut readable = Vec::with_capacity(media.len());
        for id in media {
            if !seen.insert(*id) {
                continue;
            }
            let Some(row) = self.readable_row(*id).await? else {
                continue;
            };
            let rights = access::level(self.actor, &row, annotations_public);
            let extent = self.extents.extent(&row).await;
            readable.push((media_ref(&row, extent), rights));
        }
        Ok(readable)
    }

    async fn readable_row(&self, media: MediaId) -> Result<Option<MediaRow>, AnnotationError> {
        Ok(
            with_db!(self.pool, |db| MediaRepo::new(db).get(media).await)?.filter(|row| {
                self.actor
                    .sees_resource(row.resource.is_public, row.resource.owner_id)
            }),
        )
    }
}

/// The media as parsing and rendering need it, from a row and its extent.
fn media_ref(row: &MediaRow, extent: Extent) -> MediaRef {
    MediaRef {
        id: row.id(),
        item: row.item_id,
        kind: MediaKind::of(&KindInput {
            renderer: &row.renderer,
            ingester: &row.ingester,
            media_type: row.media_type.as_deref(),
            extension: row.extension.as_deref(),
            has_thumbnails: row.has_thumbnails,
        }),
        extent,
    }
}

fn write_of<'s>(draft: &'s Draft, stored: &'s Stored) -> MediaAnnotationWrite<'s> {
    MediaAnnotationWrite {
        motivation: draft.motivation.as_str(),
        selector_kind: stored.selector_kind,
        selector: stored.selector.as_deref(),
        time_ms: stored.time_ms,
        body_json: &stored.body_json,
        body_text: &stored.body_text,
        lang: stored.lang.as_deref(),
        extensions: stored.extensions.as_deref(),
    }
}

fn annotations(
    media: MediaRef,
    rows: &[MediaAnnotationRow],
) -> Result<Vec<Annotation>, AnnotationError> {
    rows.iter()
        .map(|row| Annotation::from_row(media, row).map_err(AnnotationError::from))
        .collect()
}

/// A caption language tag, or none for an absent or empty value.
///
/// The shape is the one the `<track srclang>` and the `lang` column accept:
/// ASCII letters, digits and hyphens, starting with a letter.
fn language(raw: Option<&str>) -> Result<Option<String>, Problem> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let tag = raw.trim();
    if tag.is_empty() {
        return Ok(None);
    }
    let shaped = tag.len() <= 35
        && tag.starts_with(char::is_alphabetic)
        && !tag.ends_with('-')
        && !tag.contains("--")
        && tag
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    if shaped {
        Ok(Some(tag.to_string()))
    } else {
        Err(Problem::BadLanguage)
    }
}

/// The cue parser, with its bounds. A body over the size limit is answered
/// before the text decode, so a hostile binary file gets the bound it earned.
fn parse_cues(body: &[u8], format: TrackFormat) -> Result<Vec<ParsedCue>, AnnotationError> {
    if body.len() > tracks::MAX_BYTES {
        return Err(TrackError::TooLarge.into());
    }
    let text = std::str::from_utf8(body).map_err(|_| TrackError::NotText)?;
    Ok(tracks::parse(text, format)?)
}

/// One cue's canonical body: a plain `TextualBody`, the language when it has one.
fn cue_body(text: &str, lang: Option<&str>) -> String {
    let mut body = serde_json::json!({
        "type": "TextualBody",
        "value": text,
        "format": "text/plain",
    });
    if let Some(lang) = lang {
        body["language"] = serde_json::json!(lang);
    }
    serde_json::Value::Array(vec![body]).to_string()
}

/// The text of one stored row, from its canonical body JSON.
fn row_text(row: &MediaAnnotationRow) -> Result<String, AnnotationError> {
    wadm::text_of_body(&row.body_json)
}

/// A count as the wire carries it.
fn count(items: usize) -> u32 {
    u32::try_from(items).unwrap_or(u32::MAX)
}

/// Whether `candidate` is the better default between two equally long tracks:
/// a named language before an untagged one, then the smaller tag.
fn named_first(candidate: Option<&str>, current: Option<&str>) -> bool {
    match (candidate, current) {
        (Some(candidate), Some(current)) => candidate < current,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

/// The readable media split by the switch: captions only, or everything.
fn split(readable: &[(MediaRef, Rights)]) -> (Vec<MediaId>, Vec<MediaId>) {
    let mut open = Vec::new();
    let mut hidden = Vec::new();
    for (media, rights) in readable {
        match rights {
            Rights::ReadCaptionsOnly => hidden.push(media.id),
            Rights::Read | Rights::ReadWrite => open.push(media.id),
        }
    }
    (open, hidden)
}
