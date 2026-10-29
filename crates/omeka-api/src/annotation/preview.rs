//! A picture whose preview the derivative worker has not made yet is an other file until its
//! thumbnails exist, so its surface is the whole file. A region note on it then fails the
//! surface check, but only for now: [`Annotations::parse`] answers that refusal as
//! [`AnnotationError::PreviewPending`] when the preview is queued or being made.

use omeka_db::ids::MediaId;
use omeka_db::repos::MediaDerivativeRepo;
use omeka_db::with_db;
use omeka_media::kind::DISPLAY_IMAGE_MEDIA_TYPES;

use super::model::{Draft, MediaRef, Problem};
use super::wadm::{self, Iris};
use super::{AnnotationError, Annotations};
use crate::wire::media::PreviewState;

impl Annotations<'_> {
    /// [`wadm::parse`], with a whole-file refusal on a picture whose preview is coming answered
    /// as [`AnnotationError::PreviewPending`].
    pub(super) async fn parse(
        &self,
        doc: &[u8],
        media: &MediaRef,
        iris: &Iris,
    ) -> Result<Draft, AnnotationError> {
        match wadm::parse(doc, media, iris) {
            Err(AnnotationError::Invalid(Problem::WholeFileOnly))
                if self.preview_coming(media.id).await? =>
            {
                Err(AnnotationError::PreviewPending)
            }
            parsed => parsed,
        }
    }

    /// Whether `media` is a picture that becomes a local image once its thumbnails exist, and
    /// the media document's preview member says they are pending.
    async fn preview_coming(&self, media: MediaId) -> Result<bool, AnnotationError> {
        let Some(row) = self.media_row(media).await? else {
            return Ok(false);
        };
        let picture = row
            .media_type
            .as_deref()
            .is_some_and(|media_type| DISPLAY_IMAGE_MEDIA_TYPES.contains(&media_type));
        if !picture || row.has_thumbnails {
            return Ok(false);
        }
        let rows = with_db!(self.pool, |db| MediaDerivativeRepo::new(db)
            .for_media(media)
            .await)?;
        Ok(crate::derivatives::preview(&rows, row.has_thumbnails).state == PreviewState::Pending)
    }
}
