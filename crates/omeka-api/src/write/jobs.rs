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
use omeka_db::repos::JobRepo;

use crate::errors::ApiError;
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

/// `AbstractEntityAdapter::delete` for a job: find, authorize, remove.
pub async fn delete(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(JOB_CLASS, raw_id)?;
    let repo = JobRepo::new(api.pool());
    let Some(row) = repo.get(JobId(id)).await.map_err(db_error)? else {
        return Err(not_found(JOB_CLASS, raw_id));
    };
    let answer = api.read_member("jobs", raw_id).await?;
    repo.delete(row.id).await.map_err(db_error)?;
    Ok(answer)
}
