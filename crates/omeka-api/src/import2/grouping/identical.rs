//! `anomalies.identical: keepOne`: of each set of files with the same bytes,
//! the file a list row names stays, else the first by path, and the others
//! leave every group.

use std::collections::{HashMap, HashSet};

use super::File;
use crate::import2::decisions::{Decision, FileRef, pairs};
use crate::import2::derive::Index;
use crate::import2::join::{SheetIn, cell, normalize};
use crate::import2::plan::{JoinNormalize, JoinOn, SourceId};

/// The files to leave out, by their place in `files`.
pub(super) fn dropped(
    index: &Index,
    files: &[File<'_>],
    by_ref: &HashMap<FileRef, usize>,
    sheets: &[SheetIn<'_>],
    decisions: &[Decision],
) -> Vec<usize> {
    let has_sets = index
        .sources
        .iter()
        .any(|source| !source.walk.anomalies.identical.is_empty());
    if !has_sets {
        return Vec::new();
    }
    let at: HashMap<(&SourceId, &str), usize> = files
        .iter()
        .enumerate()
        .map(|(place, file)| ((file.source, file.entry.path.as_str()), place))
        .collect();
    let named = Named::new(sheets, decisions, by_ref);
    let mut dropped = Vec::new();
    for source in &index.sources {
        for set in &source.walk.anomalies.identical {
            let mut places: Vec<usize> = set
                .paths
                .iter()
                .filter_map(|path| at.get(&(&source.id, path.as_str())).copied())
                .collect();
            places.sort_unstable();
            let keep = places
                .iter()
                .copied()
                .find(|place| named.names(*place, &files[*place]))
                .or_else(|| places.first().copied());
            dropped.extend(places.into_iter().filter(|place| Some(*place) != keep));
        }
    }
    dropped
}

/// The files list rows name: by a `fileName` join cell, or by a `pair`.
struct Named {
    cells: Vec<(JoinNormalize, HashSet<String>)>,
    paired: HashSet<usize>,
}

impl Named {
    fn new(
        sheets: &[SheetIn<'_>],
        decisions: &[Decision],
        by_ref: &HashMap<FileRef, usize>,
    ) -> Self {
        let cells = sheets
            .iter()
            .filter_map(|sheet| {
                let join = sheet.plan.join.as_ref()?;
                let read = sheet.sheet.filter(|_| join.on == JoinOn::FileName)?;
                let keys = read
                    .rows
                    .iter()
                    .map(|row| normalize(cell(row, join.column.index), join.normalize))
                    .filter(|key| !key.is_empty())
                    .collect();
                Some((join.normalize, keys))
            })
            .collect();
        let paired = pairs(decisions)
            .filter_map(|(_, _, file)| by_ref.get(file).copied())
            .collect();
        Self { cells, paired }
    }

    fn names(&self, place: usize, file: &File<'_>) -> bool {
        self.paired.contains(&place)
            || self.cells.iter().any(|(how, keys)| {
                keys.contains(&normalize(file.entry.name(), *how))
                    || keys.contains(&normalize(file.entry.path.as_str(), *how))
            })
    }
}
