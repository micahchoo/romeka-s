//! Steps 1 and 2 of the derive: files grouped by group key, then the plan's
//! answers to unmatched and identical files, then the `move` decisions, a
//! `many` move as one move per file (`docs/import-v2.md`, "The derive").
//! A decision comes last so that it wins over an answer.
//!
//! The group key is the text of the pattern's `identifier` group, NFC, and
//! case-folded when `grouping.caseFold` is on. It is not the record's
//! `dcterms:identifier`: that slot is resolved later and only defaults to
//! the group key.

mod identical;
pub mod pattern;

use std::collections::{BTreeMap, HashMap};

use unicode_normalization::UnicodeNormalization;

use super::decisions::{EntryRef, FileRef, MoveAlone, MoveTo, Moved, moves};
use super::derive::Index;
use super::evidence::FileEntry;
use super::join::SheetIn;
use super::model::{GroupKey, LeftBy, Orphan, OrphanReason, PatternState};
use super::plan::{IdenticalAnswer, ImportPlanV2, SourceId, UnmatchedFilesAnswer};
use super::values::fold_title;
use pattern::{FilePattern, Groups, IDENTIFIER, PatternError};

/// One file of the evidence index.
pub(crate) struct File<'i> {
    pub source: &'i SourceId,
    pub entry: &'i FileEntry,
    pub file: FileRef,
    /// The pattern's groups, `None` when the pattern does not match.
    pub groups: Option<Groups>,
}

impl File<'_> {
    pub(crate) fn group(&self, name: &str) -> Option<&str> {
        self.groups
            .as_ref()?
            .get(name)
            .map(|group| group.text.as_str())
    }
}

pub(crate) struct Grouping<'i> {
    /// Sorted by source, then path.
    pub files: Vec<File<'i>>,
    pub by_ref: HashMap<FileRef, usize>,
    /// Each group's files, in path order.
    pub groups: BTreeMap<GroupKey, Vec<usize>>,
    /// Why a file is out of every group: a `move … leave` decision, or
    /// `identical: keepOne`. An unmatched file is not here.
    pub left_out: BTreeMap<usize, LeftBy>,
    pub pattern: PatternState,
    pub orphans: Vec<Orphan>,
}

pub(crate) fn group<'i>(
    index: &'i Index,
    plan: &ImportPlanV2,
    sheets: &[SheetIn<'_>],
) -> Grouping<'i> {
    let compiled = FilePattern::compile(plan.grouping.pattern.as_deref());
    let pattern = match &compiled {
        Ok(pattern) => PatternState::Groups(pattern.group_names().to_vec()),
        Err(PatternError::Missing) => PatternState::Missing,
        Err(PatternError::Syntax(message)) => PatternState::Syntax(message.clone()),
        Err(PatternError::NoIdentifier) => PatternState::NoIdentifier,
    };
    let mut files: Vec<File<'i>> = index
        .sources
        .iter()
        .flat_map(|source| {
            source.walk.entries.iter().map(|entry| File {
                source: &source.id,
                entry,
                file: FileRef {
                    source: source.id.clone(),
                    path: entry.path.clone(),
                },
                groups: compiled
                    .as_ref()
                    .ok()
                    .and_then(|pattern| pattern.groups(entry.path.as_str())),
            })
        })
        .collect();
    files.sort_by(|a, b| a.file.cmp(&b.file));
    let by_ref: HashMap<FileRef, usize> = files
        .iter()
        .enumerate()
        .map(|(at, file)| (file.file.clone(), at))
        .collect();

    // With no pattern yet, no file is one a pattern does not fit.
    let own_unmatched =
        compiled.is_ok() && plan.anomalies.unmatched_files == UnmatchedFilesAnswer::Own;
    let mut group_of: Vec<Option<GroupKey>> = files
        .iter()
        .map(|file| {
            let Some(text) = file.group(IDENTIFIER) else {
                return own_unmatched.then(|| GroupKey::Own(file.file.clone()));
            };
            let text: String = text.nfc().collect();
            Some(GroupKey::Pattern(if plan.grouping.case_fold {
                fold_title(&text)
            } else {
                text
            }))
        })
        .collect();

    let mut left_out = BTreeMap::new();
    if plan.anomalies.identical == IdenticalAnswer::KeepOne {
        for place in identical::dropped(index, &files, &by_ref, sheets, &plan.decisions) {
            group_of[place] = None;
            left_out.insert(place, LeftBy::Identical);
        }
    }
    let mut orphans = Vec::new();
    for Moved { decision, file, to } in moves(&plan.decisions) {
        let orphan = |file: &FileRef, reason| Orphan {
            decision,
            entry: EntryRef::File(file.clone()),
            reason,
        };
        let Some(&moved) = by_ref.get(file) else {
            orphans.push(orphan(file, OrphanReason::MissingEntry));
            continue;
        };
        match to {
            MoveTo::With(with) => {
                let with = with.with.file();
                let Some(&other) = by_ref.get(with) else {
                    orphans.push(orphan(with, OrphanReason::MissingEntry));
                    continue;
                };
                let Some(key) = group_of[other].clone() else {
                    orphans.push(orphan(with, OrphanReason::NoGroup));
                    continue;
                };
                group_of[moved] = Some(key);
                left_out.remove(&moved);
            }
            MoveTo::Alone(MoveAlone::Own) => {
                group_of[moved] = Some(GroupKey::Own(files[moved].file.clone()));
                left_out.remove(&moved);
            }
            MoveTo::Alone(MoveAlone::Leave) => {
                group_of[moved] = None;
                left_out.insert(moved, LeftBy::Decision(decision));
            }
        }
    }

    let mut groups: BTreeMap<GroupKey, Vec<usize>> = BTreeMap::new();
    for (at, key) in group_of.into_iter().enumerate() {
        if let Some(key) = key {
            groups.entry(key).or_default().push(at);
        }
    }
    Grouping {
        files,
        by_ref,
        groups,
        left_out,
        pattern,
        orphans,
    }
}

/// `p2` before `p10`: runs of ASCII digits compare as numbers, the rest as
/// text. Ties fall back to the plain order, so the order is total.
#[must_use]
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let (mut x, mut y) = (a, b);
    loop {
        match (x.is_empty(), y.is_empty()) {
            (true, true) => return a.cmp(b),
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            _ => {}
        }
        let digits = |text: &str| text.bytes().take_while(u8::is_ascii_digit).count();
        let (dx, dy) = (digits(x), digits(y));
        let order = if dx > 0 && dy > 0 {
            let (nx, ny) = (
                x[..dx].trim_start_matches('0'),
                y[..dy].trim_start_matches('0'),
            );
            nx.len().cmp(&ny.len()).then_with(|| nx.cmp(ny))
        } else {
            std::cmp::Ordering::Equal
        };
        if order.is_ne() {
            return order;
        }
        let (cx, cy) = if dx > 0 && dy > 0 {
            (dx, dy)
        } else {
            let len = |text: &str| text.chars().next().map_or(0, char::len_utf8);
            (len(x), len(y))
        };
        let order = if dx > 0 && dy > 0 {
            std::cmp::Ordering::Equal
        } else {
            x[..cx].cmp(&y[..cy])
        };
        if order.is_ne() {
            return order;
        }
        x = &x[cx..];
        y = &y[cy..];
    }
}

#[cfg(test)]
mod tests {
    use super::natural_cmp;

    #[test]
    fn a16_media_order_is_natural() {
        let mut names = ["p10.tif", "p2.tif", "p1.tif", "p02a.tif", "back.jpg"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            names,
            ["back.jpg", "p1.tif", "p2.tif", "p02a.tif", "p10.tif"]
        );
        assert!(natural_cmp("a", "a").is_eq());
        assert!(natural_cmp("p01", "p1").is_lt(), "ties fall back to text");
    }
}
