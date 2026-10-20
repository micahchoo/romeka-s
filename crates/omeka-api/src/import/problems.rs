//! The problems a grid row can have, and the checks that find them.
//!
//! A cell problem belongs to one term; a row problem belongs to the row. A
//! [`KindKey`] names a problem across rows, so the grid can count them and
//! filter by them.

use serde::{Deserialize, Serialize};

use super::csv::windows1252;
use super::values::Resolved;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProblemKind {
    /// A required field with no value.
    Empty,
    /// A date field whose value is not `YYYY`, `YYYY-MM` or `YYYY-MM-DD`.
    Date,
    /// Spaces at the start or end.
    Space,
    /// Text that looks read with the wrong encoding.
    Encoding,
    /// A CSV line with more or fewer cells than the header.
    Cells,
    /// A CSV row that no file matches.
    Nofile,
}

/// A problem of the row rather than of one cell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum RowProblem {
    Cells { line: usize, cells: usize },
    Nofile,
}

impl RowProblem {
    #[must_use]
    pub fn kind(&self) -> ProblemKind {
        match self {
            Self::Cells { .. } => ProblemKind::Cells,
            Self::Nofile => ProblemKind::Nofile,
        }
    }
}

/// One kind of problem: a cell kind with its term, or a row kind.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct KindKey {
    pub kind: ProblemKind,
    pub term: Option<String>,
}

/// The Dublin Core date terms. A value in one must be an ISO date.
const DATE_TERMS: [&str; 9] = [
    "dcterms:date",
    "dcterms:created",
    "dcterms:issued",
    "dcterms:modified",
    "dcterms:available",
    "dcterms:dateAccepted",
    "dcterms:dateCopyrighted",
    "dcterms:dateSubmitted",
    "dcterms:valid",
];

#[must_use]
pub fn is_date_term(term: &str) -> bool {
    DATE_TERMS.contains(&term)
}

/// `YYYY`, `YYYY-MM` or `YYYY-MM-DD`, and a real calendar day.
#[must_use]
pub fn valid_date(value: &str) -> bool {
    let parts: Vec<&str> = value.split('-').collect();
    let digits =
        |part: &str, width: usize| part.len() == width && part.bytes().all(|b| b.is_ascii_digit());
    let number = |part: &str| part.parse::<u32>().unwrap_or(0);
    match parts.as_slice() {
        [year] => digits(year, 4),
        [year, month] => digits(year, 4) && digits(month, 2) && (1..=12).contains(&number(month)),
        [year, month, day] => {
            digits(year, 4)
                && digits(month, 2)
                && digits(day, 2)
                && year
                    .parse::<i32>()
                    .ok()
                    .and_then(|y| chrono::NaiveDate::from_ymd_opt(y, number(month), number(day)))
                    .is_some()
        }
        _ => false,
    }
}

/// The problems of one cell. A date is checked per value the run writes,
/// because the grid joins several file values into one text.
#[must_use]
pub fn cell_problems(term: &str, resolved: &Resolved, required: bool) -> Vec<ProblemKind> {
    let value = resolved.value.as_str();
    let mut found = Vec::new();
    if required && value.trim().is_empty() {
        found.push(ProblemKind::Empty);
    }
    if !value.is_empty() && is_date_term(term) && !resolved.written().into_iter().all(valid_date) {
        found.push(ProblemKind::Date);
    }
    if !value.is_empty() && value.trim() != value {
        found.push(ProblemKind::Space);
    }
    if windows1252::looks_misdecoded(value) {
        found.push(ProblemKind::Encoding);
    }
    found
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use crate::import::values::Source;

    fn typed(value: &str) -> Resolved {
        Resolved {
            value: value.to_string(),
            source: Source::Csv,
            csv: value.to_string(),
            files: String::new(),
            file_values: Vec::new(),
        }
    }

    #[test]
    fn dates_are_iso_and_real() {
        for good in ["1890", "1890-12", "1890-12-31", "2024-02-29"] {
            assert!(valid_date(good), "{good}");
        }
        for bad in [
            "1890-13-01",
            "1890-13",
            "2023-02-29",
            "90",
            "12/03/2024",
            "1890-1-1",
            "",
        ] {
            assert!(!valid_date(bad), "{bad}");
        }
    }

    #[test]
    fn a_cell_reports_each_of_its_problems() {
        assert_eq!(
            cell_problems("dcterms:title", &typed(""), true),
            [ProblemKind::Empty]
        );
        assert_eq!(
            cell_problems("dcterms:date", &typed("1890-13-01"), false),
            [ProblemKind::Date]
        );
        assert_eq!(
            cell_problems("dcterms:title", &typed("  Harbour "), false),
            [ProblemKind::Space]
        );
        assert_eq!(
            cell_problems("dcterms:title", &typed("CafÃ© sign"), false),
            [ProblemKind::Encoding]
        );
        assert_eq!(
            cell_problems("dcterms:description", &typed("1890-13-01"), false),
            []
        );
        assert_eq!(cell_problems("dcterms:title", &typed(""), false), []);
    }

    #[test]
    fn a_row_problem_serializes_with_its_kind() {
        let cells = serde_json::to_value(RowProblem::Cells { line: 5, cells: 4 }).expect("json");
        assert_eq!(
            cells,
            serde_json::json!({ "kind": "cells", "line": 5, "cells": 4 })
        );
        let nofile = serde_json::to_value(RowProblem::Nofile).expect("json");
        assert_eq!(nofile, serde_json::json!({ "kind": "nofile" }));
    }
}
