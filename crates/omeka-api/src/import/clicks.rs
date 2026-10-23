//! The click meter: the fewest offered clicks that make an archive's wanted placement, and the
//! differences no offered click removes. A click is one edit; an offered
//! click is one whose choice the proposal shows with its counts. The meter
//! never reads an archive's pinned edits: it finds its clicks greedily, each
//! round taking the offered edit that leaves the fewest differences. A
//! column on another level than the wanted one then takes one click when
//! the proposal offers that level as the column's rival segment; the
//! segment moves a slot rule's source, not the tree, so no edit records it.
//!
//! A second phase then offers the proposal's corrections
//! (`docs/import-v2.md`, "Decisions"). It starts where today's
//! loop ends and takes a correction only when the differences fall, so on
//! every archive it leaves at most the differences the first phase left.

use std::collections::BTreeMap;

use super::placement::Difference;
use super::plan::{ColumnRef, CutEdit, Edit, ListFile};
use super::preview::proposal::{ItemCut, NodeEvidence, Proposal, RollUp};

/// More clicks than any archive of the tests needs: the meter stops there.
const MOST_CLICKS: usize = 8;

/// More corrections than any archive of the tests needs: the second phase
/// stops there.
const MOST_CORRECTIONS: usize = 8;

/// What the meter found on one archive.
#[derive(Debug)]
pub struct Metered {
    /// The differences of the archive as found, with no click.
    pub as_found: usize,
    /// The offered clicks, in the order the meter took them.
    pub clicks: Vec<Click>,
    /// The differences that remain after them: the gaps.
    pub left: Vec<Difference>,
    /// The corrections the second phase took, in order, each followed by
    /// the instances it unticked.
    pub corrections: Vec<Click>,
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

    /// The archive's line in the held-out table.
    #[must_use]
    pub fn line(&self) -> String {
        let gaps: Vec<String> = self
            .gaps()
            .into_iter()
            .map(|(kind, count)| format!("{kind} {count}"))
            .collect();
        format!(
            "{} offered clicks ({} of them corrections), {} gaps{}{}",
            self.clicks.len() + self.corrections.len(),
            self.corrections.len(),
            self.left.len(),
            if gaps.is_empty() { "" } else { ": " },
            gaps.join(", ")
        )
    }
}

/// One offered click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Click {
    /// An edit of the tree.
    Edit(Edit),
    /// The rival segment of a column's roll-up.
    Column {
        list: ListFile,
        column: ColumnRef,
        to: RollUp,
    },
    /// Untick one instance of the correction taken before it, by its place
    /// in the offer.
    Untick { at: usize },
}

pub(crate) fn kind(difference: &Difference) -> &'static str {
    match difference {
        Difference::FilesSplit { .. } => "filesSplit",
        Difference::FilesMerged { .. } => "filesMerged",
        Difference::RowItem { .. } => "rowItem",
        Difference::Set { .. } => "set",
        Difference::List { .. } => "list",
        Difference::Row { .. } => "row",
        Difference::LeftOut { .. } => "leftOut",
        Difference::Column { .. } => "column",
    }
}

/// Every click the proposal offers: on each series the rival and the other
/// choices of each question, leaving it out, no list, dropping its set, its
/// splits and its pairing offers; the sets over every item; keeping each
/// folder outside. A list not used and a typed title are hand edits, not
/// offered clicks.
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
        let cuts = series.items.rival.iter().chain(&series.items.others);
        for cut in cuts.filter_map(|pick| cut(&pick.value)) {
            out.push(Edit::Items {
                scope: scope.clone(),
                cut,
            });
        }
        for list in series.list.rival.iter().chain(&series.list.others) {
            out.push(Edit::List {
                scope: scope.clone(),
                list: list.value.clone(),
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
                    description: None,
                });
            }
        }
        for split in &series.splits {
            out.push(Edit::AddSet {
                from: split.value.clone(),
            });
        }
        for from in series
            .set
            .others
            .iter()
            .filter_map(|other| other.value.clone())
        {
            out.push(Edit::AddSet { from });
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

/// Whether `proposal` offers `to` as the rival segment of `column`'s
/// roll-up in `list`.
fn offers_segment(proposal: &Proposal, list: &ListFile, column: &ColumnRef, to: RollUp) -> bool {
    proposal.nodes.iter().any(|node| {
        let NodeEvidence::Series(series) = node else {
            return false;
        };
        series.list.chosen.value.as_ref() == Some(list)
            && series.columns.iter().any(|roll_up| {
                roll_up.index == column.index
                    && roll_up.header == column.header
                    && roll_up
                        .pick
                        .rival
                        .as_ref()
                        .is_some_and(|rival| rival.value == to)
            })
    })
}

/// The meter on one archive. `judge` gives the proposal after some edits
/// and how its placement differs from the wanted one.
pub fn meter(judge: impl Fn(&[Edit]) -> (Proposal, Vec<Difference>)) -> Metered {
    meter_phases(judge, Phases::Both)
}

/// The meter with its first phase alone, as it read every archive before
/// corrections were offered: a held-out read compares with the reads
/// before it.
pub fn meter_first_phase(judge: impl Fn(&[Edit]) -> (Proposal, Vec<Difference>)) -> Metered {
    meter_phases(judge, Phases::First)
}

/// Which phases the meter runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phases {
    /// Today's loop alone.
    First,
    Both,
}

fn meter_phases(judge: impl Fn(&[Edit]) -> (Proposal, Vec<Difference>), phases: Phases) -> Metered {
    let mut edits: Vec<Edit> = Vec::new();
    let (mut proposal, mut left) = judge(&edits);
    let as_found = left.len();
    let tree_left = |left: &[Difference]| {
        left.iter()
            .any(|difference| !matches!(difference, Difference::Column { .. }))
    };
    while tree_left(&left) && edits.len() < MOST_CLICKS {
        let mut best: Option<(Edit, Proposal, Vec<Difference>)> = None;
        for edit in offered(&proposal) {
            let mut tried = edits.clone();
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
        edits.push(edit);
        proposal = after;
        left = found;
    }
    let mut clicks: Vec<Click> = edits.iter().cloned().map(Click::Edit).collect();
    let mut corrections = Vec::new();
    if phases == Phases::Both {
        let mut now = State {
            edits,
            proposal,
            left,
        };
        let mut taken = 0;
        while tree_left(&now.left) && taken < MOST_CORRECTIONS {
            let Some((steps, next)) = now.best_step(&judge) else {
                break;
            };
            let first = now.edits.len();
            taken += steps.len();
            now = next;
            for (at, step) in steps.into_iter().enumerate() {
                corrections.push(Click::Edit(step));
                corrections.extend(now.untick(&judge, first + at));
            }
        }
        proposal = now.proposal;
        left = now.left;
    }
    left.retain(|difference| {
        let Difference::Column { list, column, want } = difference else {
            return true;
        };
        if !offers_segment(&proposal, list, column, *want) {
            return true;
        }
        clicks.push(Click::Column {
            list: list.clone(),
            column: column.clone(),
            to: *want,
        });
        false
    });
    Metered {
        as_found,
        clicks,
        left,
        corrections,
    }
}

/// Where the second phase stands: the edits so far, their proposal and the
/// differences they leave.
#[derive(Clone)]
struct State {
    edits: Vec<Edit>,
    proposal: Proposal,
    left: Vec<Difference>,
}

impl State {
    /// The state after one more edit, when the proposal applied it.
    fn with(
        &self,
        judge: &impl Fn(&[Edit]) -> (Proposal, Vec<Difference>),
        edit: Edit,
    ) -> Option<Self> {
        let mut edits = self.edits.clone();
        edits.push(edit);
        let (proposal, left) = judge(&edits);
        proposal.edits.last()?.applied.then_some(Self {
            edits,
            proposal,
            left,
        })
    }

    /// The correction that leaves the fewest differences, when it leaves
    /// fewer than now. When none does alone, the pair of a correction and
    /// one its proposal then offers: a join can leave a row that only then
    /// points at one file, and a person takes both.
    fn best_step(
        &self,
        judge: &impl Fn(&[Edit]) -> (Proposal, Vec<Difference>),
    ) -> Option<(Vec<Edit>, Self)> {
        let mut best: Option<(Vec<Edit>, Self)> = None;
        let keep = |best: &mut Option<(Vec<Edit>, Self)>, steps: Vec<Edit>, next: Self| {
            let fewest = best
                .as_ref()
                .map_or(self.left.len(), |(_, best)| best.left.len());
            if next.left.len() < fewest {
                *best = Some((steps, next));
            }
        };
        let firsts: Vec<(Edit, Self)> = corrections_offered(&self.proposal)
            .into_iter()
            .filter_map(|edit| Some((edit.clone(), self.with(judge, edit)?)))
            .collect();
        for (edit, next) in &firsts {
            keep(&mut best, vec![edit.clone()], next.clone());
        }
        if best.is_some() {
            return best;
        }
        for (first, after) in firsts {
            for second in corrections_offered(&after.proposal) {
                if let Some(next) = after.with(judge, second.clone()) {
                    keep(&mut best, vec![first.clone(), second], next);
                }
            }
        }
        best
    }

    /// Untick each instance of the correction at `at` that is wrong for the
    /// archive: a drop is kept only when the differences fall, and each is
    /// one more click.
    fn untick(
        &mut self,
        judge: &impl Fn(&[Edit]) -> (Proposal, Vec<Difference>),
        at: usize,
    ) -> Vec<Click> {
        let mut clicks = Vec::new();
        let offered = instances(&self.edits[at]);
        let mut dropped = 0;
        for instance in 0..offered {
            let Some(fewer) = without(&self.edits[at], instance - dropped) else {
                continue;
            };
            let mut tried = self.edits.clone();
            tried[at] = fewer;
            let (proposal, left) = judge(&tried);
            let applied = proposal.edits.get(at).is_some_and(|result| result.applied);
            if applied && left.len() < self.left.len() {
                clicks.push(Click::Untick { at: instance });
                dropped += 1;
                *self = Self {
                    edits: tried,
                    proposal,
                    left,
                };
            }
        }
        clicks
    }
}

/// The edit of each correction the proposal offers, every instance ticked.
#[must_use]
pub fn corrections_offered(proposal: &Proposal) -> Vec<Edit> {
    proposal
        .nodes
        .iter()
        .filter_map(|node| match node {
            NodeEvidence::Series(series) => Some(series),
            _ => None,
        })
        .flat_map(|series| series.fixes.iter().map(|fix| fix.edit(&series.scope.scope)))
        .collect()
}

/// The instances a correction edit holds.
fn instances(edit: &Edit) -> usize {
    match edit {
        Edit::OneItem { joins, .. } => joins.len(),
        Edit::RowIs { pairs, .. } => pairs.len(),
        Edit::DescribesNothing { rows, .. } => rows.len(),
        _ => 0,
    }
}

/// The correction without its instance at `at`; `None` when that would
/// leave it empty, which is not taking it.
fn without(edit: &Edit, at: usize) -> Option<Edit> {
    if instances(edit) <= 1 {
        return None;
    }
    let mut fewer = edit.clone();
    match &mut fewer {
        Edit::OneItem { joins, .. } => {
            joins.remove(at);
        }
        Edit::RowIs { pairs, .. } => {
            pairs.remove(at);
        }
        Edit::DescribesNothing { rows, .. } => {
            rows.remove(at);
        }
        _ => return None,
    }
    Some(fewer)
}
