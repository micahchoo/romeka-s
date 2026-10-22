//! The click meter (`briefs/import-p4-design.md`, section 4): the fewest
//! offered clicks that make an archive's wanted placement, and the
//! differences no offered click removes. A click is one edit; an offered
//! click is one whose choice the proposal shows with its counts. The meter
//! never reads an archive's pinned edits: it finds its clicks greedily, each
//! round taking the offered edit that leaves the fewest differences.

use std::collections::BTreeMap;

use super::placement::Difference;
use super::plan::{CutEdit, Edit};
use super::preview::proposal::{ItemCut, NodeEvidence, Proposal};

/// More clicks than any archive of the tests needs: the meter stops there.
const MOST_CLICKS: usize = 8;

/// What the meter found on one archive.
#[derive(Debug)]
pub struct Metered {
    /// The differences of the archive as found, with no click.
    pub as_found: usize,
    /// The offered clicks, in the order the meter took them.
    pub clicks: Vec<Edit>,
    /// The differences that remain after them: the gaps.
    pub left: Vec<Difference>,
}

impl Metered {
    /// The gaps by the kind of difference, which names the question that
    /// lacks a choice.
    #[must_use]
    pub fn gaps(&self) -> BTreeMap<&'static str, usize> {
        let mut kinds = BTreeMap::new();
        for difference in &self.left {
            *kinds.entry(kind(difference)).or_default() += 1;
        }
        kinds
    }
}

fn kind(difference: &Difference) -> &'static str {
    match difference {
        Difference::FilesSplit { .. } => "filesSplit",
        Difference::FilesMerged { .. } => "filesMerged",
        Difference::RowItem { .. } => "rowItem",
        Difference::Set { .. } => "set",
        Difference::List { .. } => "list",
        Difference::Row { .. } => "row",
        Difference::LeftOut { .. } => "leftOut",
    }
}

/// Every click the proposal offers: on each series the rival of each
/// question, leaving it out, no list, dropping its set, its splits and its
/// pairing offers; the sets over every item; keeping each folder outside.
#[must_use]
pub fn offered(proposal: &Proposal) -> Vec<Edit> {
    let mut out = Vec::new();
    for node in &proposal.nodes {
        let NodeEvidence::Series(series) = node else {
            continue;
        };
        let scope = &series.scope.scope;
        out.push(Edit::LeaveOut {
            scope: scope.clone(),
        });
        if let Some(cut) = series
            .items
            .rival
            .as_ref()
            .and_then(|rival| cut(&rival.value))
        {
            out.push(Edit::Items {
                scope: scope.clone(),
                cut,
            });
        }
        if let Some(rival) = &series.list.rival {
            out.push(Edit::List {
                scope: scope.clone(),
                list: rival.value.clone(),
            });
        }
        if series.list.chosen.value.is_some() {
            out.push(Edit::List {
                scope: scope.clone(),
                list: None,
            });
        }
        if let (Some(grain), Some(list)) = (&series.grain, &series.list.chosen.value)
            && let Some(rival) = &grain.rival
        {
            out.push(Edit::Grain {
                list: list.clone(),
                describes: rival.value,
            });
        }
        if let Some(from) = &series.set.chosen.value {
            out.push(Edit::DropSet { from: from.clone() });
        }
        if let Some(rival) = &series.set.rival {
            if let Some(from) = &rival.value {
                out.push(Edit::AddSet { from: from.clone() });
            }
            if let (Some(chosen), Some((name, _))) =
                (&series.set.chosen.value, rival.counts.sets.first())
                && let Some(title) = serde_json::to_value(name)
                    .ok()
                    .and_then(|name| name.as_str().map(str::to_owned))
            {
                out.push(Edit::Title {
                    from: chosen.clone(),
                    title,
                });
            }
        }
        for split in &series.splits {
            out.push(Edit::AddSet {
                from: split.value.clone(),
            });
        }
        if let Some(list) = &series.list.chosen.value {
            for pair in &series.pairing {
                out.push(Edit::Pair {
                    list: list.clone(),
                    rule: pair.value.clone(),
                });
            }
        }
    }
    for top in &proposal.top_sets {
        out.push(Edit::AddSet {
            from: top.value.clone(),
        });
    }
    for outside in &proposal.outside {
        out.push(Edit::Keep {
            scope: outside.counts.scope.clone(),
        });
    }
    out
}

/// The `items` edit that asks for a cut; a cut made from rows has none.
fn cut(item: &ItemCut) -> Option<CutEdit> {
    match item {
        ItemCut::Whole => Some(CutEdit::Whole),
        ItemCut::Places { places, .. } => Some(CutEdit::Places { places: *places }),
        ItemCut::Folder { depth } => Some(CutEdit::Folder { depth: *depth }),
        ItemCut::Rows { .. } => None,
    }
}

/// The meter on one archive. `judge` gives the proposal after some edits
/// and how its placement differs from the wanted one.
pub fn meter(judge: impl Fn(&[Edit]) -> (Proposal, Vec<Difference>)) -> Metered {
    let mut clicks: Vec<Edit> = Vec::new();
    let (mut proposal, mut left) = judge(&clicks);
    let as_found = left.len();
    while !left.is_empty() && clicks.len() < MOST_CLICKS {
        let mut best: Option<(Edit, Proposal, Vec<Difference>)> = None;
        for edit in offered(&proposal) {
            let mut tried = clicks.clone();
            tried.push(edit.clone());
            let (after, found) = judge(&tried);
            let took = after.edits.last().is_some_and(|result| result.applied);
            let fewest = best.as_ref().map_or(left.len(), |(_, _, best)| best.len());
            if took && found.len() < fewest {
                best = Some((edit, after, found));
            }
        }
        let Some((edit, after, found)) = best else {
            break;
        };
        clicks.push(edit);
        proposal = after;
        left = found;
    }
    Metered {
        as_found,
        clicks,
        left,
    }
}
