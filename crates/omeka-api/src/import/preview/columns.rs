//! The `columnMatches` section: for each column of each plan sheet, how
//! many cells equal a group key and how many equal a file's name, so the
//! person picks the join column by a count (`docs/import-v2.md`, "Column
//! matches"). Cells and keys are compared as the join compares them before
//! its optional folds (`join::normalize`): trimmed and NFC, nothing more.
//! [`Targets`] and [`column_counts`] are that comparison; `listCandidates`
//! counts with them too, once exact and once with every fold.

use std::borrow::Cow;
use std::collections::HashMap;

use serde::Serialize;

use super::Derived;
use unicode_normalization::{UnicodeNormalization, is_nfc};

use crate::import::evidence::FileEntry;
use crate::import::join::{cell, normalize};
use crate::import::model::{GroupKey, Origin};
use crate::import::plan::{JoinNormalize, RecordKind, SheetId, SourceId};
use crate::import::sheet::{Column, Sheet};

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

const EVERY_FOLD: JoinNormalize = JoinNormalize {
    case_fold: true,
    leading_zeros: true,
    separators: true,
};

fn folded(text: &str) -> Cow<'_, str> {
    Cow::Owned(normalize(text, EVERY_FOLD))
}

/// Every file of the index, in walk order: a file's place here is the
/// place [`Found::file`] gives.
pub(super) fn walk(derived: &Derived) -> impl Iterator<Item = (&SourceId, &FileEntry)> {
    derived.stored.index.sources.iter().flat_map(|source| {
        source
            .walk
            .entries
            .iter()
            .map(move |entry| (&source.id, entry))
    })
}

/// What a cell can equal, compared after one fold: the item groups' keys,
/// and every file's name and path. A text two keys or two files share
/// finds the first, as the join does.
pub(super) struct Targets<'d> {
    fold: fn(&str) -> Cow<'_, str>,
    keys: HashMap<Cow<'d, str>, &'d GroupKey>,
    files: HashMap<Cow<'d, str>, usize>,
}

/// What one non-empty cell found: a group, and a file by its place in
/// [`walk`].
pub(super) struct Found<'d> {
    pub key: Option<&'d GroupKey>,
    pub file: Option<usize>,
}

impl<'d> Targets<'d> {
    /// Compared trimmed and NFC.
    pub(super) fn exact(derived: &'d Derived) -> Self {
        Self::new(derived, exact)
    }

    /// Compared with every [`JoinNormalize`] fold on.
    pub(super) fn folded(derived: &'d Derived) -> Self {
        Self::new(derived, folded)
    }

    fn new(derived: &'d Derived, fold: fn(&str) -> Cow<'_, str>) -> Self {
        let mut keys = HashMap::new();
        for record in &derived.model.records {
            if let (RecordKind::Item, Origin::Group(key)) = (record.kind, &record.origin) {
                keys.entry(fold(key.text())).or_insert(key);
            }
        }
        let mut files = HashMap::new();
        for (at, (_, entry)) in walk(derived).enumerate() {
            files.entry(fold(entry.name())).or_insert(at);
            files.entry(fold(entry.path.as_str())).or_insert(at);
        }
        Self { fold, keys, files }
    }

    /// What each row's cell in `column` finds; `None` for an empty cell.
    pub(super) fn cells<'t>(
        &'t self,
        sheet: &'t Sheet,
        column: u32,
    ) -> impl Iterator<Item = Option<Found<'d>>> + 't {
        sheet.rows.iter().map(move |row| {
            let text = (self.fold)(cell(row, column));
            (!text.is_empty()).then(|| Found {
                key: self.keys.get(text.as_ref()).copied(),
                file: self.files.get(text.as_ref()).copied(),
            })
        })
    }
}

/// How many cells of `column` are empty, equal a group key, or name a file.
pub(super) fn column_counts(
    sheet: &Sheet,
    column: &Column,
    targets: &Targets<'_>,
) -> ColumnMatches {
    let mut counts = ColumnMatches {
        index: column.index,
        header: column.header.clone(),
        group_keys: 0,
        file_names: 0,
        empty: 0,
    };
    for found in targets.cells(sheet, column.index) {
        let Some(found) = found else {
            counts.empty += 1;
            continue;
        };
        counts.group_keys += usize::from(found.key.is_some());
        counts.file_names += usize::from(found.file.is_some());
    }
    counts
}

/// Every plan sheet that read; a sheet that did not is in `sheets`.
pub(super) fn column_matches(derived: &Derived) -> Vec<SheetMatches> {
    let targets = Targets::exact(derived);
    derived
        .plan
        .structure
        .lists()
        .filter_map(|(_, plan)| {
            let sheet = derived.sheets.get(&plan.id)?.sheet.as_ref().ok()?;
            Some(SheetMatches {
                sheet: plan.id.clone(),
                rows: sheet.rows.len(),
                columns: sheet
                    .columns
                    .iter()
                    .map(|column| column_counts(sheet, column, &targets))
                    .collect(),
            })
        })
        .collect()
}
