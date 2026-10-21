//! The pairs list round trip (`docs/import-v2.md`, "Pairs list round trip"):
//! the unpaired rows as a list to fill in, and the `pair` decisions that
//! the filled list gives back.
//!
//! The list has the column `Row [sheet:key]` (the row's address), the row's
//! own cells, and an empty `Files` column. The person writes a file's name,
//! or its path, in `Files`. A cell is a pair only when its text names
//! exactly one file that is in an item of this import (a pair with a file in
//! no item joins nothing); any other cell is reported with the reason and
//! never guessed.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

use super::unpaired::unpaired_rows;
use super::{Derived, ReadSheet};
use crate::import::decisions::{Decision, EntryRef, FileRef, RowKey, RowRef};
use crate::import::plan::SheetId;
use crate::import::sheet::Sheet;
use crate::import::template::{FILES, bracketed, guard, push_line, unguard};
use crate::import::values::fold_title;

/// The header of the column that addresses the row.
const ROW: &str = "Row [sheet:key]";
/// The bracketed word that marks the address column.
const ROW_MARK: &str = "sheet:key";
/// The most cells a report names; the total is always counted.
pub const MAX_REPORTED: usize = 200;
/// Past this many unpaired rows the admin offers the list instead of one
/// pair at a time.
pub const BY_LIST: usize = 30;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PairsError {
    /// An upload without the `Row [sheet:key]` or the `Files` column.
    #[error("the list needs the Row and Files columns")]
    NoColumns,
}

/// Why a filled `Files` cell made no pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Why {
    /// The row cell names no row of this import.
    NoRow,
    /// The text is no file name or path of this import.
    NoFile,
    /// The text is a name that several files of this import have.
    ManyFiles,
    /// The file is in no item, so a pair would join nothing: its name did
    /// not fit the example, and no answer made it an item of its own.
    NoItem,
}

/// A filled cell that made no pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Unmatched {
    /// The row cell as the list gave it.
    pub row: String,
    /// The `Files` cell as the list gave it.
    pub text: String,
    pub why: Why,
}

/// What a filled list gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairsRead {
    /// New decisions, in list order.
    pub decisions: Vec<Decision>,
    /// Cells whose pair the plan already holds.
    pub already: usize,
    /// The first [`MAX_REPORTED`] cells that made no pair.
    pub unmatched: Vec<Unmatched>,
    /// All the cells that made no pair.
    pub unmatched_total: usize,
}

/// The list to fill in: a byte order mark, the header and one CRLF-ended
/// line per unpaired row, sorted as the Pair screen sorts them. The own
/// cells of rows from several lists share a column when their headers agree.
#[must_use]
pub fn pairs_csv(derived: &Derived) -> Vec<u8> {
    let mut rows: Vec<(String, &RowRef)> = unpaired_rows(&derived.model)
        .map(|(row, _)| (fold_title(row.key.as_str()), row))
        .collect();
    rows.sort_by(|(a_key, a), (b_key, b)| a_key.cmp(b_key).then_with(|| a.cmp(b)));

    let sheets: BTreeMap<&SheetId, &Sheet> = derived
        .sheets
        .iter()
        .filter_map(|(id, read): (&SheetId, &std::sync::Arc<ReadSheet>)| {
            Some((id, read.sheet.as_ref().ok()?))
        })
        .collect();
    let mut headers: Vec<&str> = Vec::new();
    let mut by_key: HashMap<&SheetId, HashMap<&RowKey, &[String]>> = HashMap::new();
    for (_, sheet) in derived.plan.structure.lists() {
        let Some(read) = sheets.get(&sheet.id) else {
            continue;
        };
        if !rows.iter().any(|(_, row)| row.sheet == sheet.id) {
            continue;
        }
        for column in &read.columns {
            if !column.header.eq_ignore_ascii_case(FILES)
                && !headers.contains(&column.header.as_str())
            {
                headers.push(&column.header);
            }
        }
        by_key.insert(
            &sheet.id,
            read.rows
                .iter()
                .map(|row| (&row.key, row.cells.as_slice()))
                .collect(),
        );
    }

    let mut out = String::from('\u{feff}');
    let mut line = vec![ROW.to_string()];
    line.extend(headers.iter().map(|header| guard(header)));
    line.push(FILES.to_string());
    push_line(&mut out, &line);
    for (_, row) in rows {
        let mut line = vec![guard(&format!("{}:{}", row.sheet.0, row.key.as_str()))];
        let cells = by_key.get(&row.sheet).and_then(|rows| rows.get(&row.key));
        let sheet = sheets.get(&row.sheet);
        for header in &headers {
            let cell = sheet
                .zip(cells)
                .and_then(|(sheet, cells)| {
                    let column = sheet
                        .columns
                        .iter()
                        .find(|column| column.header == *header)?;
                    cells.get(column.index as usize)
                })
                .map_or("", String::as_str);
            line.push(guard(cell));
        }
        line.push(String::new());
        push_line(&mut out, &line);
    }
    out.into_bytes()
}

/// Read a filled list against the import's rows and files.
///
/// # Errors
/// [`PairsError::NoColumns`] when the list lacks the row or the `Files` column.
pub fn read_pairs(derived: &Derived, list: &Sheet) -> Result<PairsRead, PairsError> {
    let row_column = list
        .columns
        .iter()
        .find(|column| bracketed(&column.header) == Some(ROW_MARK))
        .ok_or(PairsError::NoColumns)?
        .index as usize;
    let files_column = list
        .columns
        .iter()
        .find(|column| column.header.eq_ignore_ascii_case(FILES))
        .ok_or(PairsError::NoColumns)?
        .index as usize;

    let mut rows: HashSet<&RowRef> = HashSet::new();
    let mut by_path: HashMap<&str, Vec<(&FileRef, bool)>> = HashMap::new();
    let mut by_name: HashMap<&str, Vec<(&FileRef, bool)>> = HashMap::new();
    for state in &derived.model.entries {
        match &state.entry {
            EntryRef::Row(row) => {
                rows.insert(row);
            }
            EntryRef::File(file) => {
                let path = file.path.as_str();
                let in_item = state.record.is_some();
                by_path.entry(path).or_default().push((file, in_item));
                let name = path.rsplit_once('/').map_or(path, |(_, name)| name);
                by_name.entry(name).or_default().push((file, in_item));
            }
        }
    }
    let mut held: HashSet<(&RowRef, &FileRef)> = derived
        .plan
        .decisions
        .iter()
        .filter_map(|decision| match decision {
            Decision::Pair { row, file } => Some((row, file)),
            _ => None,
        })
        .collect();

    let mut read = PairsRead {
        decisions: Vec::new(),
        already: 0,
        unmatched: Vec::new(),
        unmatched_total: 0,
    };
    let mut miss = |row: &str, text: &str, why: Why| {
        read.unmatched_total += 1;
        if read.unmatched.len() < MAX_REPORTED {
            read.unmatched.push(Unmatched {
                row: row.to_string(),
                text: text.to_string(),
                why,
            });
        }
    };
    let mut fresh: Vec<(&RowRef, &FileRef)> = Vec::new();
    for line in &list.rows {
        let text = line.cells.get(files_column).map_or("", |cell| cell.trim());
        if text.is_empty() {
            continue;
        }
        let address = line.cells.get(row_column).map_or("", |cell| cell.trim());
        let row = unguard(address)
            .split_once(':')
            .map(|(sheet, key)| RowRef {
                sheet: SheetId(sheet.to_string()),
                key: RowKey::new(key),
            })
            .and_then(|row| rows.get(&row).copied());
        let Some(row) = row else {
            miss(address, text, Why::NoRow);
            continue;
        };
        // A path names one file per source, a bare name every file called so.
        let mut named: Vec<(&FileRef, bool)> = by_path.get(text).cloned().unwrap_or_default();
        for found in by_name.get(text).into_iter().flatten() {
            if !named.contains(found) {
                named.push(*found);
            }
        }
        let pairable: Vec<&FileRef> = named
            .iter()
            .filter_map(|(file, in_item)| in_item.then_some(*file))
            .collect();
        match (named.is_empty(), pairable.as_slice()) {
            (true, _) => miss(address, text, Why::NoFile),
            (false, []) => miss(address, text, Why::NoItem),
            (false, [file]) => {
                if held.insert((row, file)) {
                    fresh.push((row, file));
                } else {
                    read.already += 1;
                }
            }
            (false, _) => miss(address, text, Why::ManyFiles),
        }
    }
    read.decisions = fresh
        .into_iter()
        .map(|(row, file)| Decision::Pair {
            row: row.clone(),
            file: file.clone(),
        })
        .collect();
    Ok(read)
}
