//! Job rows whose process is gone.
//!
//! A job process records its pid when it starts (`perform-job.php`, `JobRepo::start`) and clears
//! it when it writes its ending. A process that dies first (a KILL, an out-of-memory end, a
//! container restart) leaves its row `in_progress` with the pid of a process that no longer
//! exists. Every "one at a time" rule reads such a row as a job still running: a fixity start
//! would answer that dead check for good, and no derivative worker would start again.
//!
//! [`settle`] ends those rows as `error`, with a log line that says why. The evidence is the pid:
//! a row whose recorded process is gone, or has exited and waits to be reaped, cannot finish. A
//! row with no pid is judged only at server start ([`Unstarted`]): while a server runs, its own
//! dispatcher may hold that job in its queue.
//!
//! Upstream has no such step; its rows stay as they are until someone stops them by hand.

use chrono::NaiveDateTime;
use omeka_db::ids::JobId;
use omeka_db::repos::JobRepo;
use omeka_db::{DbError, DbPool, with_db};

/// Which `starting` rows with no process count as dead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unstarted {
    /// None: a running server's dispatcher may still start them.
    Kept,
    /// Those composed before this time. At server start no queue holds a job, so a row composed
    /// before the server began never starts; the margin before that leaves alone a row another
    /// process composed and is about to run.
    ComposedBefore(NaiveDateTime),
}

/// End as `error` every active job of `class` (every class when `None`) whose process is gone, and
/// the unstarted ones `unstarted` names. Answers the jobs it ended. Safe to run at any time and any
/// number of times: a row that ended or was taken up since it was read is left alone.
///
/// # Errors
/// A database error reading or writing the rows.
pub async fn settle(
    pool: &DbPool,
    class: Option<&str>,
    unstarted: Unstarted,
) -> Result<Vec<JobId>, DbError> {
    let rows = with_db!(pool, |db| JobRepo::new(db).active(class).await)?;
    let mut settled = Vec::new();
    for row in rows {
        let why = match row.pid.as_deref() {
            Some(pid) if process_gone(pid) => {
                format!("the job's process {pid} ended before the job did")
            }
            Some(_) => continue,
            None => match unstarted {
                Unstarted::ComposedBefore(at) if row.started < at => {
                    "the server stopped before the job started".to_string()
                }
                _ => continue,
            },
        };
        let pid = row.pid.as_deref();
        if with_db!(pool, |db| JobRepo::new(db).settle(row.id, pid, &why).await)? {
            settled.push(row.id);
        }
    }
    Ok(settled)
}

/// Whether no live process has this pid. Only Linux's `/proc` answers that here; anywhere else,
/// and for a pid that is not a number, the job is taken to be alive.
fn process_gone(pid: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        let Ok(pid) = pid.trim().parse::<u32>() else {
            return false;
        };
        if !std::path::Path::new("/proc/self/stat").exists() {
            return false;
        }
        match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            // The state follows the command name, which may itself hold ") ".
            Ok(stat) => stat
                .rsplit_once(") ")
                .is_some_and(|(_, rest)| rest.starts_with('Z') || rest.starts_with('X')),
            Err(error) => error.kind() == std::io::ErrorKind::NotFound,
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        false
    }
}
