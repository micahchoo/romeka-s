//! The reads: one run with its progress, the recent runs, and what an undo would keep.

use omeka_db::repos::{BulkRepo, Presence, RunsOf};
use omeka_db::rows::{BulkRow, Described, FailReason, TargetCounts, TargetState};
use omeka_db::with_db;
use serde::Serialize;
use serde_json::Value;

use super::BulkError;
use super::access::load;
use crate::messages;
use crate::read::{Api, iso8601};

/// How many failed items a run shows.
const FAILURES: i64 = 10;

/// How many runs the recent list shows.
const RECENT: i64 = 25;

/// How many items of each kind the undo preview names.
const NAMED: usize = 50;

/// An item's describing fields on the wire.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fields {
    pub is_public: bool,
    pub owner: Option<u64>,
    pub class: Option<u64>,
    pub template: Option<u64>,
    pub item_sets: Vec<u64>,
    pub sites: Vec<u64>,
}

impl Fields {
    pub(super) fn of(fields: &Described) -> Self {
        Self {
            is_public: fields.is_public,
            owner: fields.owner.map(|id| id.0),
            class: fields.class.map(|id| id.0),
            template: fields.template.map(|id| id.0),
            item_sets: fields.item_sets.iter().map(|id| id.0).collect(),
            sites: fields.sites.iter().map(|id| id.0).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Counts {
    pub pending: u64,
    pub done: u64,
    pub unchanged: u64,
    pub failed: u64,
    pub restored: u64,
    pub kept: u64,
}

impl From<TargetCounts> for Counts {
    fn from(counts: TargetCounts) -> Self {
        Self {
            pending: counts.pending,
            done: counts.done,
            unchanged: counts.unchanged,
            failed: counts.failed,
            restored: counts.restored,
            kept: counts.kept,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Failure {
    pub item: u64,
    /// `gone`, `denied` or `templateRequires`.
    pub reason: Option<&'static str>,
    /// The reason in English, from the message registry, which the admin translates.
    pub message: Option<&'static str>,
}

fn reason_text(reason: FailReason) -> &'static str {
    match reason {
        FailReason::Gone => messages::BULK_ITEM_GONE,
        FailReason::Denied => messages::BULK_ITEM_DENIED,
        FailReason::TemplateRequires => messages::BULK_ITEM_TEMPLATE_REQUIRES,
    }
}

/// One run as the routes answer it. `counts` and `failures` are read live from the targets;
/// the list leaves them out.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkView {
    pub id: u64,
    pub kind: &'static str,
    pub status: &'static str,
    pub owner: Option<u64>,
    pub query: Value,
    pub change: Value,
    pub total: u64,
    pub done: u64,
    pub failed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counts: Option<Counts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failures: Option<Vec<Failure>>,
    pub job_id: Option<u64>,
    pub error: Option<String>,
    pub created: String,
    pub started: Option<String>,
    pub ended: Option<String>,
}

impl BulkView {
    pub(super) fn of(row: &BulkRow) -> Self {
        Self {
            id: row.id.0,
            kind: row.kind.as_str(),
            status: row.status.as_str(),
            owner: row.owner_id.map(|id| id.0),
            query: row.query.clone(),
            change: row.change.clone(),
            total: row.total,
            done: row.done,
            failed: row.failed,
            counts: None,
            failures: None,
            job_id: row.job_id.map(|id| id.0),
            error: row.error.clone(),
            created: iso8601(row.created),
            started: row.started.map(iso8601),
            ended: row.ended.map(iso8601),
        }
    }
}

/// `GET /api/romeka/bulk/{id}`.
///
/// # Errors
/// 404 for no such run, 403 for another person's run.
pub async fn show(api: &Api<'_>, raw: &str) -> Result<BulkView, BulkError> {
    let row = load(api, raw, false).await?;
    let counts = with_db!(api.pool(), |db| BulkRepo::new(db).counts(row.id).await)?;
    let failed = with_db!(api.pool(), |db| BulkRepo::new(db)
        .targets(row.id, TargetState::Failed, FAILURES)
        .await)?;
    let mut view = BulkView::of(&row);
    view.counts = Some(counts.into());
    view.failures = Some(
        failed
            .iter()
            .map(|target| Failure {
                item: target.item_id.0,
                reason: target.error.map(|reason| reason.as_str()),
                message: target.error.map(reason_text),
            })
            .collect(),
    );
    Ok(view)
}

/// `GET /api/romeka/bulk`: the caller's recent runs, newest first; an administrator's lists
/// everyone's.
///
/// # Errors
/// A storage failure.
pub async fn list(api: &Api<'_>) -> Result<Vec<BulkView>, BulkError> {
    let whose = if api.actor().is_admin() {
        RunsOf::Everyone
    } else if let Some(user) = api.actor().user() {
        RunsOf::Owner(user)
    } else {
        return Ok(Vec::new());
    };
    with_db!(api.pool(), |db| BulkRepo::new(db).settle_dead().await)?;
    let rows = with_db!(api.pool(), |db| BulkRepo::new(db)
        .recent(whose, RECENT)
        .await)?;
    Ok(rows.iter().map(BulkView::of).collect())
}

/// What an undo of the run would do now.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoPreview {
    /// Items undo restores.
    pub restore: usize,
    /// Items edited since the run wrote them, which undo keeps unless forced.
    pub edited: usize,
    /// Items that are gone, which nothing restores.
    pub gone: usize,
    /// The first edited items, by id.
    pub edited_items: Vec<u64>,
    pub gone_items: Vec<u64>,
}

/// `POST /api/romeka/bulk/{id}/undo/preview`.
///
/// # Errors
/// 404 for no such run, 403 for another person's run.
pub async fn undo_preview(api: &Api<'_>, raw: &str) -> Result<UndoPreview, BulkError> {
    let row = load(api, raw, false).await?;
    let candidates = with_db!(api.pool(), |db| BulkRepo::new(db)
        .undo_candidates(row.id)
        .await)?;
    let mut preview = UndoPreview {
        restore: 0,
        edited: 0,
        gone: 0,
        edited_items: Vec::new(),
        gone_items: Vec::new(),
    };
    for candidate in candidates {
        match candidate.now {
            Presence::Gone => {
                preview.gone += 1;
                if preview.gone_items.len() < NAMED {
                    preview.gone_items.push(candidate.item.0);
                }
            }
            Presence::Here { modified } if modified != candidate.written => {
                preview.edited += 1;
                if preview.edited_items.len() < NAMED {
                    preview.edited_items.push(candidate.item.0);
                }
            }
            Presence::Here { .. } => preview.restore += 1,
        }
    }
    Ok(preview)
}
