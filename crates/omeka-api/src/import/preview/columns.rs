//! The `columnMatches` section: for each column of each plan sheet, how
//! many cells equal a group key and how many equal a file's name, so the
//! person picks the join column by a count (`docs/import-v2.md`, "Column
//! matches"). Cells and keys are compared as the join compares them before
//! its optional folds (`join::normalize`): trimmed and NFC, nothing more.

use std::borrow::Cow;
use std::collections::HashSet;

use serde::Serialize;

use super::Derived;
use unicode_normalization::{UnicodeNormalization, is_nfc};

use crate::import::join::cell;
use crate::import::model::Origin;
use crate::import::plan::{RecordKind, SheetId};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetMatches {
    pub sheet: SheetId,
    pub rows: usize,
    pub columns: Vec<ColumnMatches>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMatches {
    pub index: u32,
    pub header: String,
    pub group_keys: usize,
    /// Cells equal to a file's name, or to its path, as a `fileName` join
    /// finds a file by either.
    pub file_names: usize,
    pub empty: usize,
}

/// Trimmed and NFC, borrowed when it already is: group keys and paths are
/// stored NFC, and most cells are ASCII.
fn exact(text: &str) -> Cow<'_, str> {
    let text = text.trim();
    if text.is_ascii() || is_nfc(text) {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(text.nfc().collect())
    }
}

/// Every plan sheet that read; a sheet that did not is in `sheets`.
pub(super) fn column_matches(derived: &Derived) -> Vec<SheetMatches> {
    let keys: HashSet<Cow<'_, str>> = derived
        .model
        .records
        .iter()
        .filter(|record| record.kind == RecordKind::Item)
        .filter_map(|record| match &record.origin {
            Origin::Group(key) => Some(exact(key.text())),
            _ => None,
        })
        .collect();
    let mut names: HashSet<Cow<'_, str>> = HashSet::new();
    for entry in derived
        .stored
        .index
        .sources
        .iter()
        .flat_map(|source| &source.walk.entries)
    {
        names.insert(exact(entry.name()));
        names.insert(exact(entry.path.as_str()));
    }
    derived
        .plan
        .sheets
        .iter()
        .filter_map(|plan| {
            let sheet = derived.sheets.get(&plan.id)?.sheet.as_ref().ok()?;
            let columns = sheet
                .columns
                .iter()
                .map(|column| {
                    let mut counts = ColumnMatches {
                        index: column.index,
                        header: column.header.clone(),
                        group_keys: 0,
                        file_names: 0,
                        empty: 0,
                    };
                    for row in &sheet.rows {
                        let text = exact(cell(row, column.index));
                        if text.is_empty() {
                            counts.empty += 1;
                            continue;
                        }
                        counts.group_keys += usize::from(keys.contains(&text));
                        counts.file_names += usize::from(names.contains(&text));
                    }
                    counts
                })
                .collect();
            Some(SheetMatches {
                sheet: plan.id.clone(),
                rows: sheet.rows.len(),
                columns,
            })
        })
        .collect()
}
