//! The `job.status` state machine, from `Omeka\Entity\Job`.
//!
//! Upstream stores the status as a string on the entity and writes it as a
//! `STATUS_*` constant. The port parses that string at the storage boundary
//! into this enum, so the runner's transitions are a total function over the
//! states instead of string comparisons spread across call sites.

/// One `Omeka\Entity\Job` status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    /// `STATUS_STARTING`: dispatched, not yet handed to a process.
    Starting,
    /// `STATUS_IN_PROGRESS`: a process is running the job.
    InProgress,
    /// `STATUS_STOPPING`: a stop was requested and the job has not yet acted on it.
    Stopping,
    /// `STATUS_COMPLETED`: `perform` returned with no stop requested.
    Completed,
    /// `STATUS_STOPPED`: the job saw the stop request and returned early.
    Stopped,
    /// `STATUS_ERROR`: `perform` failed, or no process could be started.
    Error,
}

impl JobStatus {
    /// The value written to `job.status` (`Entity\Job::STATUS_*`).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::InProgress => "in_progress",
            Self::Stopping => "stopping",
            Self::Completed => "completed",
            Self::Stopped => "stopped",
            Self::Error => "error",
        }
    }

    /// Parse a stored `job.status`. `None` when the value is absent or is not a
    /// status this build knows, so the read layer passes it through unchanged.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "starting" => Some(Self::Starting),
            "in_progress" => Some(Self::InProgress),
            "stopping" => Some(Self::Stopping),
            "completed" => Some(Self::Completed),
            "stopped" => Some(Self::Stopped),
            "error" => Some(Self::Error),
            _ => None,
        }
    }

    /// Whether the job has ended, so the runner must not run it again.
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Stopped | Self::Error)
    }

    /// The status `perform` returning normally reaches from here.
    ///
    /// Upstream `Synchronous::send` reads the in-memory status, which
    /// `shouldStop` has updated from the database; the port reads the stored
    /// status at the same point, so a `stopping` job ends `stopped` and any
    /// other state ends `completed`.
    #[must_use]
    pub fn ending(self) -> Self {
        match self {
            Self::Stopping => Self::Stopped,
            _ => Self::Completed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::JobStatus;

    #[test]
    fn status_round_trips_through_its_stored_string() {
        let all = [
            JobStatus::Starting,
            JobStatus::InProgress,
            JobStatus::Stopping,
            JobStatus::Completed,
            JobStatus::Stopped,
            JobStatus::Error,
        ];
        for status in all {
            assert_eq!(JobStatus::parse(status.as_str()), Some(status));
        }
        assert_eq!(JobStatus::parse("paused"), None);
    }

    #[test]
    fn a_stopping_job_ends_stopped_and_a_running_one_ends_completed() {
        assert_eq!(JobStatus::Stopping.ending(), JobStatus::Stopped);
        assert_eq!(JobStatus::InProgress.ending(), JobStatus::Completed);
        assert_eq!(JobStatus::Starting.ending(), JobStatus::Completed);
    }
}
