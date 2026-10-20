//! `left.csv`: the entries a run left out or failed to write, each with its
//! reason (`docs/import-v2.md`, "Lifecycle and the ledger").
//!
//! The file is written to the store at [`path`] when a run ends, so it no
//! longer depends on the folder staying put. Its columns name the entry
//! (`kind` is `file` or `row`; a row's `source` is its sheet and its `path`
//! its row key), the record it would have made, and `#reason`. The `#`
//! marks a note for people: a sheet column whose header starts with `#` is
//! never mapped, so the file can be fixed and read back as a sheet.

use std::collections::HashMap;

use omeka_db::ids::ImportId;

use super::decisions::EntryRef;
use super::ledger::{Failure, Written};
use super::model::{LeftBy, Model, Parent, RecordId};

/// The header of the reason column.
pub const REASON: &str = "#reason";

/// Where the run stores the file.
#[must_use]
pub fn path(import: ImportId) -> String {
    format!("imports/{}/left.csv", import.0)
}

/// The file's text: a header, then one line per entry the run did not
/// write, in the model's entry order. A record an earlier attempt wrote is
/// written, though a retry's derive leaves it out as `exists`.
#[must_use]
pub fn render(model: &Model, failures: &[Failure], written: &Written) -> String {
    let failed: HashMap<&str, &Failure> = failures
        .iter()
        .map(|failure| (failure.record.as_str(), failure))
        .collect();
    let failure_of = |record: RecordId| -> Option<&Failure> {
        let held = model.record(record);
        failed.get(held.origin.key().as_str()).copied().or_else(|| {
            let Some(Parent::Record(parent)) = held.parent else {
                return None;
            };
            failed
                .get(model.record(parent).origin.key().as_str())
                .copied()
        })
    };
    let mut lines = vec![line(&["kind", "source", "path", "record", REASON])];
    for state in &model.entries {
        let record_key = state
            .record
            .map(|record| model.record(record).origin.key())
            .unwrap_or_default();
        if !record_key.is_empty() && written.holds(&record_key) {
            continue;
        }
        let reason = match (state.left, state.record) {
            (Some(left), _) => left_reason(left),
            (None, None) if state.unmatched => {
                "The file name does not match the pattern.".to_string()
            }
            (None, None) => "No record uses this entry.".to_string(),
            (None, Some(record)) => match failure_of(record) {
                Some(failure) => failure.detail.clone(),
                None => continue,
            },
        };
        let (kind, source, path) = match &state.entry {
            EntryRef::File(file) => ("file", file.source.0.as_str(), file.path.as_str()),
            EntryRef::Row(row) => ("row", row.sheet.0.as_str(), row.key.as_str()),
        };
        lines.push(line(&[kind, source, path, &record_key, &reason]));
    }
    lines.concat()
}

fn left_reason(left: LeftBy) -> String {
    match left {
        LeftBy::Decision(at) => format!("Left out by correction {}.", at + 1),
        LeftBy::Flag(kind) => {
            let name = serde_json::to_value(kind)
                .ok()
                .and_then(|value| value.as_str().map(str::to_string))
                .unwrap_or_default();
            format!("Left out by the check \"{name}\".")
        }
        LeftBy::Parent => "Its item is left out.".to_string(),
        LeftBy::NoFiles => "Every file of this item is left out.".to_string(),
        LeftBy::Identical => "It has the same bytes as a file the import keeps.".to_string(),
        LeftBy::NoRow => "No row of the list joined this item.".to_string(),
    }
}

/// One CSV line, each cell quoted when it holds a comma, a quote or a line
/// end, with CRLF as RFC 4180 has it.
fn line(cells: &[&str]) -> String {
    let quoted: Vec<String> = cells
        .iter()
        .map(|cell| {
            if cell.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", cell.replace('"', "\"\""))
            } else {
                (*cell).to_string()
            }
        })
        .collect();
    format!("{}\r\n", quoted.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cell_with_a_comma_or_a_quote_is_quoted() {
        assert_eq!(
            line(&["a", "b,c", "say \"hi\""]),
            "a,\"b,c\",\"say \"\"hi\"\"\"\r\n"
        );
    }
}
