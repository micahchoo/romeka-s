//! Undo: remove what one import made, from what its `romeka_import_resource`
//! rows say, so a rerun after a crash at any point converges.
//!
//! Each item goes through the API item delete (its media files and rows
//! first, then values, the item and its index rows), and its row is forgotten
//! after the target is gone. A target that is already gone only loses its row.
//! Then the import's new item sets with no items, new templates no resource
//! uses, and new fields no value or template uses are deleted. One still in
//! use stays, keeps its row, and is named in the report, which the import's
//! summary keeps as `undo`. The caller closes the undo window.

use std::collections::HashSet;
use std::sync::Arc;

use omeka_db::DbError;
use omeka_db::ids::ImportId;
use omeka_db::repos::{ImportRepo, ImportResourceRepo, ImportWriteRepo, MediaRepo};
use omeka_db::rows::{ImportKind, ImportTarget};
use omeka_media::store::Store;
use serde::Serialize;
use serde_json::{Map, Value};
use sqlx::SqlitePool;

use super::run::Progress;
use crate::errors::ApiError;
use crate::jsonld::BaseUrl;
use crate::read::Api;
use crate::write::{item_sets, items, media, templates, vocab};

/// The summary member that holds the undo's own `{ done, total }`. The run's `progress` stays as
/// the run left it, so the ended screens keep its numbers.
const UNDO_PROGRESS: &str = "undoProgress";

/// What an undo removed, and what it kept because something else uses it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoReport {
    pub items: usize,
    pub media: usize,
    pub item_sets: usize,
    pub templates: usize,
    pub fields: usize,
    pub kept: Vec<Kept>,
}

/// A set, template or field the import made that is still in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Kept {
    pub kind: KeptKind,
    pub id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum KeptKind {
    ItemSet,
    Template,
    Field,
}

#[derive(Debug, thiserror::Error)]
pub enum UndoError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error(transparent)]
    Api(#[from] ApiError),
}

/// Undo an import.
///
/// # Errors
/// [`UndoError`] when the database or the store fails; what was removed so
/// far stays removed, and a rerun goes on from there.
pub async fn undo<F>(
    pool: &SqlitePool,
    store: &Arc<dyn Store>,
    import_id: ImportId,
    mut progress: F,
) -> Result<UndoReport, UndoError>
where
    F: FnMut(Progress<'_>) + Send,
{
    let base = BaseUrl::parse("http://localhost").map_err(ApiError::internal)?;
    let api = Api::load_with_store(pool, base, super::system_actor(), Some(store.clone())).await?;
    let records = ImportResourceRepo::new(pool);
    let writer = ImportWriteRepo::new(pool, import_id);
    let targets = records.list(import_id, None).await?;
    let total = targets.len();
    let mut report = UndoReport::default();
    // The item delete removes an item's media first, so the media pass would
    // find them gone; remember which existed so they still count as removed.
    let mut present = HashSet::new();
    for row in &targets {
        if let ImportTarget::Media(id) = row.target
            && MediaRepo::new(pool).get(id).await?.is_some()
        {
            present.insert(id.0);
        }
    }
    let order = [
        ImportKind::Item,
        ImportKind::Media,
        ImportKind::ItemSet,
        ImportKind::ResourceTemplate,
        ImportKind::Property,
    ];
    let mut done = 0;
    for kind in order {
        for row in targets.iter().filter(|row| row.target.kind() == kind) {
            let line = match remove(&api, &writer, row.target, &present, &mut report).await? {
                Some(line) => {
                    records.delete(import_id, row.target).await?;
                    line
                }
                None => format!("kept {} {}: still in use", kind.as_str(), raw(row.target)),
            };
            done += 1;
            keep_member(
                pool,
                import_id,
                UNDO_PROGRESS,
                serde_json::json!({ "done": done, "total": total }),
            )
            .await?;
            progress(Progress {
                done,
                total,
                line: &line,
            });
        }
    }
    keep_report(pool, import_id, &report).await?;
    Ok(report)
}

/// Write `report` into the import's summary as `undo`, beside what the run wrote.
async fn keep_report(
    pool: &SqlitePool,
    import_id: ImportId,
    report: &UndoReport,
) -> Result<(), UndoError> {
    let report = serde_json::to_value(report).map_err(ApiError::internal)?;
    keep_member(pool, import_id, "undo", report).await
}

/// Set one member of the import's summary and keep the rest. The write is also the heartbeat.
async fn keep_member(
    pool: &SqlitePool,
    import_id: ImportId,
    key: &str,
    value: Value,
) -> Result<(), UndoError> {
    let repo = ImportRepo::new(pool);
    let stored = repo.get(import_id).await?.and_then(|row| row.summary);
    let mut summary = match stored {
        Some(Value::Object(members)) => members,
        _ => Map::new(),
    };
    summary.insert(key.to_string(), value);
    repo.set_summary(import_id, &Value::Object(summary)).await?;
    Ok(())
}

/// Remove one target. `None` when it is in use and stays.
async fn remove(
    api: &Api<'_>,
    writer: &ImportWriteRepo<'_>,
    target: ImportTarget,
    present: &HashSet<u64>,
    report: &mut UndoReport,
) -> Result<Option<String>, UndoError> {
    let id = raw(target).to_string();
    let gone = match target {
        ImportTarget::Item(_) => {
            let gone = gone(items::delete(api, &id).await)?;
            report.items += usize::from(gone);
            gone
        }
        ImportTarget::Media(media_id) => {
            let row = MediaRepo::new(api.pool()).get(media_id).await?;
            if let Some(row) = &row {
                media::remove(api, row).await?;
            }
            let removed_with_item = row.is_none() && present.contains(&media_id.0);
            report.media += usize::from(row.is_some() || removed_with_item);
            row.is_some() || removed_with_item
        }
        ImportTarget::ItemSet(set) => {
            if writer.item_set_has_items(set).await? {
                report.kept.push(Kept {
                    kind: KeptKind::ItemSet,
                    id: raw(target),
                });
                return Ok(None);
            }
            let gone = gone(item_sets::delete(api, &id).await)?;
            report.item_sets += usize::from(gone);
            gone
        }
        ImportTarget::ResourceTemplate(template) => {
            if writer.template_in_use(template).await? {
                report.kept.push(Kept {
                    kind: KeptKind::Template,
                    id: raw(target),
                });
                return Ok(None);
            }
            let gone = gone(templates::delete(api, &id).await)?;
            report.templates += usize::from(gone);
            gone
        }
        ImportTarget::Property(property) => {
            if writer.property_in_use(property).await? {
                report.kept.push(Kept {
                    kind: KeptKind::Field,
                    id: raw(target),
                });
                return Ok(None);
            }
            let gone = gone(vocab::delete_property(api, &id).await)?;
            report.fields += usize::from(gone);
            gone
        }
    };
    let verb = if gone { "removed" } else { "already gone:" };
    Ok(Some(format!("{verb} {} {id}", target.kind().as_str())))
}

/// True when the delete removed the target, false when it was already gone.
fn gone<T>(result: Result<T, ApiError>) -> Result<bool, ApiError> {
    match result {
        Ok(_) => Ok(true),
        Err(ApiError::NotFound { .. }) => Ok(false),
        Err(other) => Err(other),
    }
}

fn raw(target: ImportTarget) -> u64 {
    match target {
        ImportTarget::Item(id) => id.0,
        ImportTarget::Media(id) => id.0,
        ImportTarget::ItemSet(id) => id.0,
        ImportTarget::ResourceTemplate(id) => id.0,
        ImportTarget::Property(id) => id.0,
    }
}
