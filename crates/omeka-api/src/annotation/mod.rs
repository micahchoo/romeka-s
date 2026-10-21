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
pub mod tracks;
mod wadm;

pub use access::search_scope;
pub use model::{
    Annotation, AnnotationId, Body, Draft, Extensions, IfMatch, MediaRef, Motivation, Origin,
    Problem, Revision, Selector, TextFormat,
};
pub use wadm::{Iris, render};

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;

use omeka_db::DbError;
use omeka_db::ids::MediaId;
use omeka_db::repos::MediaRepo;
use omeka_db::repos::{
    ChangeOutcome, InsertOutcome, MediaAnnotationRepo, MediaAnnotationRow, MediaAnnotationWrite,
};
use omeka_db::rows::MediaRow;
use omeka_db::with_db;
use omeka_media::kind::{Extent, KindInput, MediaKind};

use crate::read::Api;
use crate::settings;

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
    api: &'a Api<'a>,
    extents: &'a dyn ExtentSource,
}

impl<'a> Annotations<'a> {
    pub fn new(api: &'a Api<'a>, extents: &'a dyn ExtentSource) -> Self {
        Self { api, extents }
    }

    /// The media as parsing and rendering need it, with the caller's rights.
    /// `NotFound` when the caller may not read the media.
    pub async fn rights(&self, media: MediaId) -> Result<(MediaRef, Rights), AnnotationError> {
        self.readable_one(media).await
    }

    /// One keyset page, in id order.
    pub async fn page(
        &self,
        media: MediaId,
        after: Option<AnnotationId>,
    ) -> Result<Page, AnnotationError> {
        let (media_ref, rights) = self.readable_one(media).await?;
        let motivations = access::motivations(rights);
        let mut rows = with_db!(self.api.pool(), |db| MediaAnnotationRepo::new(db)
            .page(media, after.map_or(0, |id| id.0), PAGE_SIZE, motivations)
            .await)?;
        let next = if rows.len() > PAGE_SIZE as usize {
            rows.get(PAGE_SIZE as usize - 1)
                .map(|row| AnnotationId(row.id))
        } else {
            None
        };
        rows.truncate(PAGE_SIZE as usize);
        let total = with_db!(self.api.pool(), |db| MediaAnnotationRepo::new(db)
            .total(media, motivations)
            .await)?;
        let items = annotations(media_ref, &rows)?;
        Ok(Page {
            media: media_ref,
            rights,
            total,
            items,
            after,
            next,
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
            rows = with_db!(self.api.pool(), |db| MediaAnnotationRepo::new(db)
                .firsts(&open, first, None)
                .await)?;
            totals = with_db!(self.api.pool(), |db| MediaAnnotationRepo::new(db)
                .counts(&open, None)
                .await)?;
        }
        if !hidden.is_empty() {
            rows.extend(with_db!(self.api.pool(), |db| MediaAnnotationRepo::new(
                db
            )
            .firsts(&hidden, first, Some(&["supplementing"]))
            .await)?);
            totals.extend(with_db!(self.api.pool(), |db| MediaAnnotationRepo::new(
                db
            )
            .counts(&hidden, Some(&["supplementing"]))
            .await)?);
        }
        let ids: Vec<MediaId> = readable.iter().map(|(media, _)| media.id).collect();
        let mut tracks: HashMap<MediaId, Vec<TrackRef>> = HashMap::new();
        for (media, lang, cues) in with_db!(self.api.pool(), |db| MediaAnnotationRepo::new(db)
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
            totals = with_db!(self.api.pool(), |db| MediaAnnotationRepo::new(db)
                .counts(&open, None)
                .await)?;
        }
        if !hidden.is_empty() {
            totals.extend(with_db!(self.api.pool(), |db| MediaAnnotationRepo::new(
                db
            )
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
        let outcome = with_db!(self.api.pool(), |db| MediaAnnotationRepo::new(db)
            .insert(
                media,
                self.api.actor().user(),
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
        let outcome = with_db!(self.api.pool(), |db| MediaAnnotationRepo::new(db)
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
        let outcome = with_db!(self.api.pool(), |db| MediaAnnotationRepo::new(db)
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
        Ok(with_db!(self.api.pool(), |db| MediaAnnotationRepo::new(
            db
        )
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
        let annotations_public = settings::annotations_public(self.api.pool()).await?;
        let mut seen = HashSet::new();
        let mut readable = Vec::with_capacity(media.len());
        for id in media {
            if !seen.insert(*id) {
                continue;
            }
            let Some(row) = self.readable_row(*id).await? else {
                continue;
            };
            let rights = access::level(self.api, &row, annotations_public);
            let extent = self.extents.extent(&row).await;
            readable.push((media_ref(&row, extent), rights));
        }
        Ok(readable)
    }

    async fn readable_row(&self, media: MediaId) -> Result<Option<MediaRow>, AnnotationError> {
        Ok(
            with_db!(self.api.pool(), |db| MediaRepo::new(db).get(media).await)?
                .filter(|row| self.api.sees(row.resource.is_public, row.resource.owner_id)),
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
