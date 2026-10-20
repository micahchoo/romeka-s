//! `PUT`, `PATCH` and `DELETE` for `jobs`.
//!
//! `JobAdapter::hydrate` is empty, so a job's columns come from the dispatcher,
//! never from a request body. An update finds the row, passes the `update`
//! privilege and answers its representation unchanged; a delete removes the row
//! and answers the representation it removed.
//!
//! `POST /api/jobs` is not built. `job.class` is `NOT NULL` and the empty
//! hydrate never sets it (`Entity/Job.php:56`, `JobAdapter::hydrate`), so
//! upstream's create fails at flush with a database error. The port leaves the
//! endpoint at 501 rather than reproduce a 500 for every body. Reported as a
//! gap.

use omeka_db::ids::JobId;
use omeka_db::repos::{ImportRepo, JobRepo};

use crate::errors::ApiError;
use crate::messages;
use crate::read::{Api, ApiResponse, db_error, not_found, parse_id};

const JOB_CLASS: &str = "Omeka\\Entity\\Job";

/// `AbstractEntityAdapter::update` with the empty `JobAdapter::hydrate`: find,
/// authorize, write nothing.
///
/// The adapter grant keeps this to the two admin roles
/// (`AclFactory` grants no other role a write on `JobAdapter`), and
/// `Entity\Job` carries no assertion for them.
pub async fn update(
    api: &Api<'_>,
    raw_id: &str,
    _payload: &crate::write::body::Payload,
    _partial: bool,
) -> Result<ApiResponse, ApiError> {
    let id = parse_id(JOB_CLASS, raw_id)?;
    if JobRepo::new(api.pool())
        .get(JobId(id))
        .await
        .map_err(db_error)?
        .is_none()
    {
        return Err(not_found(JOB_CLASS, raw_id));
    }
    api.read_member("jobs", raw_id).await
}

/// `AbstractEntityAdapter::delete` for a job: find, authorize, remove. Upstream deletes any job.
/// Romeka refuses with 409 while an archive import that runs or undoes holds it: the delete would
/// null the import's `job_id`, and its worker would stop at the next heartbeat.
pub async fn delete(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(JOB_CLASS, raw_id)?;
    let repo = JobRepo::new(api.pool());
    let Some(row) = repo.get(JobId(id)).await.map_err(db_error)? else {
        return Err(not_found(JOB_CLASS, raw_id));
    };
    // A worker that died leaves its import `running` until a reader settles it; settle first so
    // the refusal is only for a job that can still write.
    ImportRepo::new(api.pool())
        .settle_dead()
        .await
        .map_err(db_error)?;
    let answer = api.read_member("jobs", raw_id).await?;
    if !repo.delete_unless_held(row.id).await.map_err(db_error)? {
        return Err(ApiError::Conflict {
            message: messages::JOB_HELD_BY_IMPORT.to_string(),
        });
    }
    Ok(answer)
}
