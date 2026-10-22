//! The `columnMatches` section: for each column of each plan sheet, how
//! many cells equal a group key and how many equal a file's name, so the
//! person picks the join column by a count (`docs/import-v2.md`, "Column
//! matches"). Cells and keys are compared as the join compares them before
//! its optional folds (`join::normalize`): trimmed and NFC, nothing more.
//! [`Targets`] and [`column_counts`] are that comparison; `listCandidates`
//! counts with them too, once exact and once with every fold.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use serde::Serialize;

use super::Derived;
use unicode_normalization::{UnicodeNormalization, is_nfc};

use crate::import::derive::Index;
use crate::import::evidence::FileEntry;
use crate::import::join::{cell, normalize};
use crate::import::model::{GroupKey, Model, Origin};
use crate::import::plan::{
    JoinNormalize, Placement, RecordKind, SeriesIx, SheetId, SourceId, Structure,
};
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

/// No fold: trimmed and NFC, as the join compares before its folds.
pub(super) const NO_FOLD: JoinNormalize = JoinNormalize {
    case_fold: false,
    leading_zeros: false,
    separators: false,
};

/// Every fold the plan can state.
pub(super) const EVERY_FOLD: JoinNormalize = JoinNormalize {
    case_fold: true,
    leading_zeros: true,
    separators: true,
};

/// A text after the fold `how`, borrowed when no fold applies.
fn fold(text: &str, how: JoinNormalize) -> Cow<'_, str> {
    if how == NO_FOLD {
        exact(text)
    } else {
        Cow::Owned(normalize(text, how))
    }
}

/// Every file of the index, in walk order: a file's place here is the
/// place [`Found::file`] gives.
pub(super) fn walk(index: &Index) -> impl Iterator<Item = (&SourceId, &FileEntry)> {
    index.sources.iter().flat_map(|source| {
        source
            .walk
            .entries
            .iter()
            .map(move |entry| (&source.id, entry))
    })
}

/// One series of a structure: the targets of a list that describes it.
#[derive(Clone, Copy)]
pub(super) struct Within<'d> {
    pub structure: &'d Structure,
    pub at: SeriesIx,
}

impl Within<'_> {
    fn takes(&self, source: &SourceId, entry: &FileEntry) -> bool {
        matches!(
            self.structure.place(source, &entry.path),
            Placement::Series { at, .. } if at == self.at
        )
    }
}

/// What a cell can equal, compared after one fold: the item groups' keys,
/// and every file's name and path. A text two keys or two files share
/// finds the first, as the join does, and is marked shared.
pub(super) struct Targets<'d> {
    how: JoinNormalize,
    keys: HashMap<Cow<'d, str>, &'d GroupKey>,
    files: HashMap<Cow<'d, str>, usize>,
    shared_keys: HashSet<Cow<'d, str>>,
    shared_files: HashSet<Cow<'d, str>>,
}

/// What one non-empty cell found: a group, and a file by its place in
/// [`walk`]. `shared` says another group or file has the same text.
pub(super) struct Found<'d> {
    pub key: Option<&'d GroupKey>,
    pub file: Option<usize>,
    pub shared_key: bool,
    pub shared_file: bool,
}

impl<'d> Targets<'d> {
    /// Compared trimmed and NFC.
    pub(super) fn exact(derived: &'d Derived) -> Self {
        Self::of(&derived.model, &derived.stored.index, None, NO_FOLD)
    }

    /// Compared with every [`JoinNormalize`] fold on.
    pub(super) fn folded(derived: &'d Derived) -> Self {
        Self::of(&derived.model, &derived.stored.index, None, EVERY_FOLD)
    }

    /// The groups and files of one series (every series' with `None`),
    /// compared after the fold `how`.
    pub(super) fn of(
        model: &'d Model,
        index: &'d Index,
        within: Option<Within<'_>>,
        how: JoinNormalize,
    ) -> Self {
        let mut targets = Self {
            how,
            keys: HashMap::new(),
            files: HashMap::new(),
            shared_keys: HashSet::new(),
            shared_files: HashSet::new(),
        };
        for record in &model.records {
            if let (RecordKind::Item, Origin::Group(key)) = (record.kind, &record.origin)
                && within.is_none_or(|within| record.series == within.at)
            {
                let text = fold(key.text(), how);
                match targets.keys.get(&text) {
                    Some(first) if *first != key => {
                        targets.shared_keys.insert(text);
                    }
                    Some(_) => {}
                    None => {
                        targets.keys.insert(text, key);
                    }
                }
            }
        }
        for (at, (source, entry)) in walk(index).enumerate() {
            if within.is_some_and(|within| !within.takes(source, entry)) {
                continue;
            }
            for text in [fold(entry.name(), how), fold(entry.path.as_str(), how)] {
                match targets.files.get(&text) {
                    Some(first) if *first != at => {
                        targets.shared_files.insert(text);
                    }
                    Some(_) => {}
                    None => {
                        targets.files.insert(text, at);
                    }
                }
            }
        }
        targets
    }

    /// What each row's cell in `column` finds; `None` for an empty cell.
    pub(super) fn cells<'t>(
        &'t self,
        sheet: &'t Sheet,
        column: u32,
    ) -> impl Iterator<Item = Option<Found<'d>>> + 't {
        sheet.rows.iter().map(move |row| {
            let text = fold(cell(row, column), self.how);
            (!text.is_empty()).then(|| Found {
                key: self.keys.get(text.as_ref()).copied(),
                file: self.files.get(text.as_ref()).copied(),
                shared_key: self.shared_keys.contains(text.as_ref()),
                shared_file: self.shared_files.contains(text.as_ref()),
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
