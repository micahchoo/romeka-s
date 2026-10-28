//! What the server answers about checks: a check's counts, the latest check, a new check, and the
//! `romeka_fixity` filter on `GET /api/media`.
//!
//! A check is a `job` row, so it is read under the jobs grant: the four roles that may read jobs
//! (global and site administrators, editors and reviewers) see results, and the two administrator
//! roles, which alone may write a job, start one. A caller without the grant gets an empty list
//! from the media filter, the same answer a check that does not exist gets.

use omeka_db::ids::JobId;
use omeka_db::repos::{JobFilter, JobRepo, Paging};
use omeka_db::rows::{JobRow, NewJob};
use omeka_db::with_db;
use serde::Serialize;

use super::CHECK_JOB;
use super::record::{CheckRecord, Outcome, Shown};
use crate::errors::ApiError;
use crate::messages;
use crate::read::{Api, Params, db_error, iso8601};
use crate::routes::Operation;

/// The media a `romeka_fixity` filter keeps.
#[derive(Debug, Clone)]
pub struct MediaShown {
    /// `None` when the filter names no check the caller may read, which keeps nothing.
    record: Option<CheckRecord>,
    shown: Shown,
}

impl MediaShown {
    #[must_use]
    pub fn holds(&self, id: u64) -> bool {
        self.record
            .as_ref()
            .is_some_and(|record| record.shows(self.shown, id))
    }
}

/// `romeka_fixity=<checked|ok|differs|missing|unreadable>` (`word`) with
/// `romeka_fixity_job=<job id>` (`job`), or the latest check that has a result when the job is
/// left out. `None` when the search has no `romeka_fixity`. A word the filter does not know, a job
/// that is not a check with a result, and a caller who may not read jobs all keep nothing.
pub(crate) async fn media_filter(
    api: &Api<'_>,
    word: Option<&str>,
    job: Option<&str>,
) -> Result<Option<MediaShown>, ApiError> {
    let Some(word) = word else {
        return Ok(None);
    };
    let Some(shown) = Shown::parse(word) else {
        return Ok(Some(MediaShown {
            record: None,
            shown: Shown::Checked,
        }));
    };
    let record = if api.allow("jobs", Operation::Read).is_err() {
        None
    } else {
        match job.map(|raw| raw.trim().parse::<u64>()) {
            None => latest_ended(api).await?.map(|(_, record)| record),
            Some(Ok(id)) => check_row(api, JobId(id)).await?.map(|(_, record)| record),
            Some(Err(_)) => None,
        }
    };
    Ok(Some(MediaShown { record, shown }))
}

/// One check as the result screen reads it. Every count is the total of the Media list
/// `romeka_fixity=<name>&romeka_fixity_job=<job>` answers the same caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CheckView {
    pub job: u64,
    pub status: Option<String>,
    pub started: String,
    pub ended: Option<String>,
    pub checked: u64,
    pub ok: u64,
    pub differs: u64,
    pub missing: u64,
    pub unreadable: u64,
}

/// The latest check that has a result, and the check that is running now, if any.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Latest {
    pub check: Option<CheckView>,
    pub running: Option<u64>,
}

/// `GET /api/romeka/fixity/{job}`.
///
/// # Errors
/// 403 without the jobs read grant; 404 when the job is missing, is not a check, or has no
/// result yet.
pub async fn one(api: &Api<'_>, job: JobId) -> Result<CheckView, ApiError> {
    api.allow("jobs", Operation::Read)?;
    let Some((row, record)) = check_row(api, job).await? else {
        return Err(ApiError::not_found(messages::NOT_FOUND));
    };
    view_of(api, &row, &record).await
}

/// `GET /api/romeka/fixity`.
///
/// # Errors
/// 403 without the jobs read grant.
pub async fn latest(api: &Api<'_>) -> Result<Latest, ApiError> {
    api.allow("jobs", Operation::Read)?;
    let rows = check_rows(api).await?;
    let running = rows
        .iter()
        .rev()
        .find(|row| is_active(row))
        .map(|row| row.id.0);
    let check = match ended_check(rows) {
        Some((row, record)) => Some(view_of(api, &row, &record).await?),
        None => None,
    };
    Ok(Latest { check, running })
}

/// `POST /api/romeka/fixity`: compose a check, or answer the one that is already composed or
/// running, so a second click never reads the store twice at once. `true` when it composed one.
/// A check whose process is gone is ended first ([`crate::dead_jobs`]), so it never stands for a
/// running one.
///
/// # Errors
/// 403 for a caller who may not write jobs.
pub async fn start(api: &Api<'_>) -> Result<(JobId, bool), ApiError> {
    crate::write::access::adapter(api.actor(), "jobs", Operation::Create)?;
    crate::dead_jobs::settle(
        api.pool(),
        Some(CHECK_JOB),
        crate::dead_jobs::Unstarted::Kept,
    )
    .await
    .map_err(db_error)?;
    if let Some(row) = check_rows(api).await?.into_iter().rev().find(is_active) {
        return Ok((row.id, false));
    }
    let row = with_db!(api.pool(), |db| JobRepo::new(db)
        .create(&NewJob {
            owner_id: api.actor().user(),
            pid: None,
            status: Some("starting".to_string()),
            class: CHECK_JOB.to_string(),
            args: Some(serde_json::json!({})),
            log: None,
            started: None,
        })
        .await)
    .map_err(db_error)?;
    Ok((row.id, true))
}

async fn view_of(api: &Api<'_>, row: &JobRow, record: &CheckRecord) -> Result<CheckView, ApiError> {
    let params = Params::from_decoded(vec![
        ("romeka_fixity".to_string(), "checked".to_string()),
        ("romeka_fixity_job".to_string(), row.id.0.to_string()),
    ]);
    let (media, _) = crate::read::listed_media(api, &params).await?;
    let mut view = CheckView {
        job: row.id.0,
        status: row.status.clone(),
        started: iso8601(row.started),
        ended: row.ended.map(iso8601),
        checked: 0,
        ok: 0,
        differs: 0,
        missing: 0,
        unreadable: 0,
    };
    for media in &media {
        let Some(outcome) = record.outcome_of(media.id().0) else {
            continue;
        };
        view.checked += 1;
        *match outcome {
            Outcome::Ok => &mut view.ok,
            Outcome::Differs => &mut view.differs,
            Outcome::Missing => &mut view.missing,
            Outcome::Unreadable => &mut view.unreadable,
        } += 1;
    }
    Ok(view)
}

fn is_active(row: &JobRow) -> bool {
    matches!(
        row.status.as_deref(),
        Some("starting" | "in_progress" | "stopping")
    )
}

/// Every check job, oldest first.
async fn check_rows(api: &Api<'_>) -> Result<Vec<JobRow>, ApiError> {
    let filter = JobFilter {
        class: Some(CHECK_JOB.to_string()),
        ..JobFilter::default()
    };
    with_db!(api.pool(), |db| JobRepo::new(db)
        .list(&filter, &Paging::default())
        .await)
    .map_err(db_error)
}

fn ended_check(rows: Vec<JobRow>) -> Option<(JobRow, CheckRecord)> {
    rows.into_iter().rev().find_map(|row| {
        let record = CheckRecord::from_args(row.args.as_ref())?;
        Some((row, record))
    })
}

async fn latest_ended(api: &Api<'_>) -> Result<Option<(JobRow, CheckRecord)>, ApiError> {
    Ok(ended_check(check_rows(api).await?))
}

async fn check_row(api: &Api<'_>, job: JobId) -> Result<Option<(JobRow, CheckRecord)>, ApiError> {
    let row = with_db!(api.pool(), |db| JobRepo::new(db).get(job).await).map_err(db_error)?;
    Ok(row
        .filter(|row| row.class == CHECK_JOB)
        .and_then(|row| CheckRecord::from_args(row.args.as_ref()).map(|record| (row, record))))
}
