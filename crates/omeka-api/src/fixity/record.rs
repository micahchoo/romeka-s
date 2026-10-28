//! What one check found, in the shape it is stored in `job.args.result`.

use serde::{Deserialize, Serialize};

/// What the check found for one media's original.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Outcome {
    /// The file was read to its end and its SHA-256 equals `media.sha256`.
    Ok,
    /// The file was read to its end and its SHA-256 is another one.
    Differs,
    /// The store has no file at the original's path.
    Missing,
    /// The store has a file there, or could not say, and could not read it to its end.
    Unreadable,
}

impl Outcome {
    pub const ALL: [Self; 4] = [Self::Ok, Self::Differs, Self::Missing, Self::Unreadable];

    /// The word the record, the filter and the log use.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Differs => "differs",
            Self::Missing => "missing",
            Self::Unreadable => "unreadable",
        }
    }
}

/// What a `romeka_fixity` filter asks for: every media the check read, or those of one outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    Checked,
    Outcome(Outcome),
}

impl Shown {
    /// The filter's word, or `None` for a word it does not know.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        if word == "checked" {
            return Some(Self::Checked);
        }
        Outcome::ALL
            .into_iter()
            .find(|outcome| outcome.as_str() == word)
            .map(Self::Outcome)
    }
}

/// A set of ids kept as sorted runs of consecutive ids, `[first, last]` each.
///
/// An install's originals are mostly consecutive media ids, so the check of 4,230 files stores a
/// few runs instead of 4,230 numbers in a job row every Jobs page reads.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IdRuns(Vec<[u64; 2]>);

impl IdRuns {
    /// Add `id`, which is greater than every id already added.
    pub fn push(&mut self, id: u64) {
        match self.0.last_mut() {
            Some(run) if run[1].checked_add(1) == Some(id) => run[1] = id,
            _ => self.0.push([id, id]),
        }
    }

    #[must_use]
    pub fn contains(&self, id: u64) -> bool {
        let at = self.0.partition_point(|run| run[1] < id);
        self.0.get(at).is_some_and(|run| run[0] <= id)
    }

    /// How many ids the runs hold.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.0
            .iter()
            .map(|run| run[1].saturating_sub(run[0]) + 1)
            .sum()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Every id, ascending.
    pub fn ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.0.iter().flat_map(|run| run[0]..=run[1])
    }
}

/// The media ids of each outcome of one check.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckRecord {
    pub ok: IdRuns,
    pub differs: IdRuns,
    pub missing: IdRuns,
    pub unreadable: IdRuns,
}

impl CheckRecord {
    /// Record `id`'s outcome. Ids arrive in ascending order.
    pub fn push(&mut self, id: u64, outcome: Outcome) {
        self.runs_mut(outcome).push(id);
    }

    #[must_use]
    pub fn runs(&self, outcome: Outcome) -> &IdRuns {
        match outcome {
            Outcome::Ok => &self.ok,
            Outcome::Differs => &self.differs,
            Outcome::Missing => &self.missing,
            Outcome::Unreadable => &self.unreadable,
        }
    }

    fn runs_mut(&mut self, outcome: Outcome) -> &mut IdRuns {
        match outcome {
            Outcome::Ok => &mut self.ok,
            Outcome::Differs => &mut self.differs,
            Outcome::Missing => &mut self.missing,
            Outcome::Unreadable => &mut self.unreadable,
        }
    }

    /// The outcome recorded for `id`, `None` when the check did not read it.
    #[must_use]
    pub fn outcome_of(&self, id: u64) -> Option<Outcome> {
        Outcome::ALL
            .into_iter()
            .find(|outcome| self.runs(*outcome).contains(id))
    }

    /// Whether `id` is one of the media `shown` names.
    #[must_use]
    pub fn shows(&self, shown: Shown, id: u64) -> bool {
        match shown {
            Shown::Checked => self.outcome_of(id).is_some(),
            Shown::Outcome(outcome) => self.runs(outcome).contains(id),
        }
    }

    /// The record a job's `args` holds, `None` when it holds none (the check has not ended, or the
    /// job is another kind).
    #[must_use]
    pub fn from_args(args: Option<&serde_json::Value>) -> Option<Self> {
        let result = args?.get(super::RESULT_ARG)?;
        serde_json::from_value(result.clone()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_join_consecutive_ids_and_answer_membership() {
        let mut runs = IdRuns::default();
        for id in [1, 2, 3, 5, 9, 10] {
            runs.push(id);
        }
        assert_eq!(runs, IdRuns(vec![[1, 3], [5, 5], [9, 10]]));
        assert_eq!(runs.len(), 6);
        let held: Vec<u64> = (0..12).filter(|id| runs.contains(*id)).collect();
        assert_eq!(held, [1, 2, 3, 5, 9, 10]);
    }

    #[test]
    fn a_record_round_trips_through_job_args() {
        let mut record = CheckRecord::default();
        record.push(1, Outcome::Ok);
        record.push(2, Outcome::Differs);
        record.push(3, Outcome::Ok);
        record.push(4, Outcome::Missing);
        let args = serde_json::json!({ "result": record });
        assert_eq!(
            args["result"],
            serde_json::json!({
                "ok": [[1, 1], [3, 3]],
                "differs": [[2, 2]],
                "missing": [[4, 4]],
                "unreadable": [],
            })
        );
        let back = CheckRecord::from_args(Some(&args)).unwrap_or_default();
        assert_eq!(back, record);
        assert!(back.shows(Shown::Checked, 4));
        assert!(!back.shows(Shown::Checked, 5));
        assert_eq!(back.outcome_of(2), Some(Outcome::Differs));
        assert_eq!(CheckRecord::from_args(Some(&serde_json::json!({}))), None);
    }

    #[test]
    fn the_filter_words_are_checked_and_the_four_outcomes() {
        assert_eq!(Shown::parse("checked"), Some(Shown::Checked));
        assert_eq!(
            Shown::parse("differs"),
            Some(Shown::Outcome(Outcome::Differs))
        );
        assert_eq!(Shown::parse("Differs"), None);
    }
}
