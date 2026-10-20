//! Run progress on `GET /api/romeka/imports/{id}` (`docs/import-v2.md`,
//! "Run progress"): the records the run has done of its total, from the
//! summary the run writes (`progress: { done, total }`), and the seconds
//! left at the rate so far, counted from the job row's start. Times are
//! Unix seconds.

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Progress {
    pub done: u64,
    pub total: u64,
    /// `None` until a record is done, and once the run has ended.
    pub seconds_left: Option<u64>,
}

/// The progress `summary` holds; `started` is the job's start, set only
/// while the run goes on.
pub(crate) fn progress(
    summary: Option<&Value>,
    started: Option<i64>,
    now: i64,
) -> Option<Progress> {
    let stored = summary?.get("progress")?;
    let done = stored.get("done")?.as_u64()?;
    let total = stored.get("total")?.as_u64()?;
    let seconds_left = started.and_then(|started| {
        let elapsed = u64::try_from(now - started).ok()?;
        (done > 0).then(|| {
            let left = u128::from(total.saturating_sub(done));
            u64::try_from(u128::from(elapsed) * left / u128::from(done)).unwrap_or(u64::MAX)
        })
    });
    Some(Progress {
        done,
        total,
        seconds_left,
    })
}

/// Now, in Unix seconds.
pub(crate) fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use serde_json::json;

    use super::*;

    fn at(seconds: i64) -> i64 {
        1_790_000_000 + seconds
    }

    #[test]
    fn seconds_left_follow_the_rate_so_far() {
        let summary = json!({ "progress": { "done": 250, "total": 1000 } });
        assert_eq!(
            progress(Some(&summary), Some(at(0)), at(100)),
            Some(Progress {
                done: 250,
                total: 1000,
                seconds_left: Some(300)
            })
        );
        let none_done = json!({ "progress": { "done": 0, "total": 1000 } });
        assert_eq!(
            progress(Some(&none_done), Some(at(0)), at(5))
                .unwrap()
                .seconds_left,
            None
        );
        assert_eq!(
            progress(Some(&summary), None, at(100))
                .unwrap()
                .seconds_left,
            None,
            "ended"
        );
        assert_eq!(
            progress(Some(&json!({ "items": 2 })), Some(at(0)), at(1)),
            None
        );
        assert_eq!(progress(None, Some(at(0)), at(1)), None);
    }
}
