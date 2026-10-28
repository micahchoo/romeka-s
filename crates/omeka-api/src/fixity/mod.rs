//! Fixity: whether each stored original still has the bytes its `media.sha256` names.
//!
//! Upstream records the digest at ingest (`TempFile::getSha256`) and never reads it again. The job
//! `Romeka\Job\FixityCheck` reads every original through the [`Store`](omeka_media::store::Store)
//! in bounded ranges, hashes it, and records one [`Outcome`] per media. The CLI's
//! `romeka fixity check` runs the same job in its own process, so a cron can call it.
//!
//! The result lives in the job's own `args` JSON under `result`, as [`CheckRecord`]: the media ids
//! of each outcome, as runs of consecutive ids. No table holds it, so deleting the job deletes its
//! result, and the time of every outcome is the job's own `started` and `ended`.
//!
//! A count the result screen shows and the Media list it opens are one read: both are the media
//! the caller may see whose id the record holds for that outcome (`romeka_fixity` on
//! `GET /api/media`, [`view`]).

mod check;
mod record;
pub mod view;

pub use check::{CHUNK, check_all, check_file};
pub use record::{CheckRecord, IdRuns, Outcome, Shown};

/// The `job.class` of the check.
pub const CHECK_JOB: &str = "Romeka\\Job\\FixityCheck";

/// The `args` member the check writes its [`CheckRecord`] into.
pub const RESULT_ARG: &str = "result";
