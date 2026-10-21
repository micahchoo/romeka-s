//! Text search over the notes of one item, for the Content Search 2.0 route.
//!
//! The repository's `search` owns the visibility predicate, the FTS match and
//! the keyset limit; this module turns its rows into domain annotations and
//! hands back the cursor the route pages with. A media the caller may not read
//! is absent from the readable set, and one the switch hides keeps only its
//! `supplementing` rows, so a private sibling's note is never a hit.

use std::collections::HashMap;

use omeka_db::ids::{ItemId, MediaId};
use omeka_db::repos::{FtsQuery, ItemRepo, MediaAnnotationRepo};
use omeka_db::with_db;

use super::{Annotation, AnnotationError, AnnotationId, Annotations, MediaRef, Rights};
use crate::settings;

impl Annotations<'_> {
    /// The notes of `item` whose text holds the words, in id order.
    ///
    /// `NotFound` when the caller may not read the item: a hidden item's notes
    /// are as absent as its manifest. The second value is the cursor of the
    /// next page, `None` when this is the last one.
    pub async fn search_item(
        &self,
        item: ItemId,
        words: &str,
        after: Option<AnnotationId>,
        limit: u32,
    ) -> Result<(Vec<Annotation>, Option<AnnotationId>), AnnotationError> {
        let Some(item_row) = with_db!(self.api.pool(), |db| ItemRepo::new(db).get(item).await)?
            .filter(|row| self.api.sees(row.resource.is_public, row.resource.owner_id))
        else {
            return Err(AnnotationError::NotFound);
        };
        let Some(query) = FtsQuery::from_words(words) else {
            return Ok((Vec::new(), None));
        };
        let limit = limit.clamp(1, super::PAGE_SIZE);
        let scope = super::search_scope(
            self.api.actor(),
            item_row.resource.owner_id,
            settings::annotations_public(self.api.pool()).await?,
        );
        let mut rows = with_db!(self.api.pool(), |db| MediaAnnotationRepo::new(db)
            .search(&scope, item, &query, after.map_or(0, |id| id.0), limit)
            .await)?;
        let next = if rows.len() > limit as usize {
            rows.get(limit as usize - 1).map(|row| AnnotationId(row.id))
        } else {
            None
        };
        rows.truncate(limit as usize);
        let ids: Vec<MediaId> = rows.iter().map(|row| row.media_id).collect();
        let readable: HashMap<MediaId, (MediaRef, Rights)> = self
            .readable(&ids)
            .await?
            .into_iter()
            .map(|(media, rights)| (media.id, (media, rights)))
            .collect();
        let mut items = Vec::with_capacity(rows.len());
        for row in &rows {
            let Some((media, rights)) = readable.get(&row.media_id) else {
                continue;
            };
            if !super::access::holds(*rights, &row.motivation) {
                continue;
            }
            items.push(Annotation::from_row(*media, row)?);
        }
        Ok((items, next))
    }
}
