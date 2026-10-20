//! The rows an import leaves out, as a CSV file to fix and import again: the
//! rows whose identifier an item already holds, the rows the plan skips, the
//! rows it leaves out for their problems, and the rows whose run failed, each
//! with its reason in the last column.
//!
//! The caller loads the catalog with [`super::catalog::load_for_import`], so
//! the items this import made are not `exists` rows.
//!
//! The rows come from the folder and the plan as they are now, so a folder
//! that changed after the run gives today's rows. A failed row the folder no
//! longer makes still gets a line with its identifier and reason.

use std::collections::BTreeSet;

use super::archive::{ArchiveError, ArchiveFolder};
use super::catalog::Catalog;
use super::csv::render;
use super::derive::{self, Model, Row};
use super::inputs::Inputs;
use super::plan::{ImportPlan, ProblemRows};
use super::problems::KindKey;
use super::run::Failure;
use super::tags::{TagCache, TagReader};
use super::values::IDENTIFIER_TERM;

/// The header of the reason column. The `#` marks it as a note for people,
/// so `romeka import csv` ignores it and the fixed file imports as it is.
pub const REASON: &str = "#reason";

/// Read the folder and render the left-out rows of `plan`.
///
/// # Errors
/// [`ArchiveError`] when the folder cannot be walked.
pub fn left_out_csv(
    folder: &ArchiveFolder,
    plan: &ImportPlan,
    catalog: &Catalog,
    failures: &[Failure],
    reader: &dyn TagReader,
) -> Result<String, ArchiveError> {
    let inputs = Inputs::load(folder, plan)?;
    let tags = TagCache::new(reader);
    let model = derive::derive(plan, &inputs, &tags, catalog);
    Ok(render(&left_out_rows(&model, plan, failures)))
}

/// The header (the identifier, the grid's terms, the reason), then one line
/// per left-out row in the grid's order.
fn left_out_rows(model: &Model, plan: &ImportPlan, failures: &[Failure]) -> Vec<Vec<String>> {
    let terms: Vec<&str> = model
        .columns
        .iter()
        .map(|column| column.term.as_str())
        .filter(|term| *term != IDENTIFIER_TERM)
        .collect();
    let mut lines = vec![
        std::iter::once(IDENTIFIER_TERM)
            .chain(terms.iter().copied())
            .chain(std::iter::once(REASON))
            .map(str::to_string)
            .collect(),
    ];
    let mut listed = BTreeSet::new();
    for row in &model.rows {
        if let Some(reason) = reason(row, plan, failures) {
            listed.insert(row.item.identifier.as_str());
            let mut line = vec![row.item.identifier.clone()];
            line.extend(terms.iter().map(|term| {
                row.cells
                    .iter()
                    .find(|cell| cell.term == *term)
                    .map(|cell| cell.value.value.clone())
                    .unwrap_or_default()
            }));
            line.push(reason);
            lines.push(line);
        }
    }
    for failure in failures {
        if !listed.contains(failure.identifier.as_str()) {
            let mut line = vec![failure.identifier.clone()];
            line.extend(terms.iter().map(|_| String::new()));
            line.push(failed(failure));
            lines.push(line);
        }
    }
    lines
}

/// Why `row` is left out, or `None` when the run creates it.
fn reason(row: &Row, plan: &ImportPlan, failures: &[Failure]) -> Option<String> {
    if row.existing.is_some() {
        return Some("exists".to_string());
    }
    if row.skipped {
        return Some("skipped".to_string());
    }
    if plan.problem_rows == ProblemRows::Leave && row.problems() > 0 {
        let kinds: Vec<String> = row.kinds().iter().map(kind_text).collect();
        return Some(format!("problems: {}", kinds.join(", ")));
    }
    failures
        .iter()
        .find(|failure| failure.identifier == row.item.identifier)
        .map(failed)
}

fn failed(failure: &Failure) -> String {
    format!("failed: {}", failure.reason)
}

/// `date (dcterms:date)` for a cell problem, `nofile` for a row problem.
fn kind_text(key: &KindKey) -> String {
    let kind = serde_json::to_value(key.kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default();
    match &key.term {
        Some(term) => format!("{kind} ({term})"),
        None => kind,
    }
}
