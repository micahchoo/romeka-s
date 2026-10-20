//! The `leave`, `set`, `match` and `flag` decisions, a `many` as one per
//! entry: which record each one targets, and which are orphans
//! (`docs/import-v2.md`, "Decisions").
//!
//! A decision targets the record of its kind that holds its entry: for a
//! file, its media record or its group's item record; for a row, the record
//! the row made or joined. `leave` has no kind: a file leaves its item, a
//! row the record it made or joined.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use super::super::decisions::{Act, EntryRef, RowRef, Single, singles};
use super::super::grouping::Grouping;
use super::super::join::{SheetIn, line_keys_changed};
use super::super::model::{Anomaly, LeftBy, Orphan, OrphanReason, RecordId};
use super::super::plan::{ImportPlanV2, RecordKind, SheetId, SlotName};
use super::super::slots::SetDecisions;
use super::records::Draft;

pub(crate) struct Decided {
    pub sets: SetDecisions,
    pub matches: HashMap<RecordId, Option<i64>>,
}

/// Every row of every sheet read, for the orphan test.
fn rows_read(sheets: &[SheetIn<'_>]) -> HashSet<RowRef> {
    sheets
        .iter()
        .filter_map(|sheet| Some((&sheet.plan.id, sheet.sheet?)))
        .flat_map(|(id, sheet)| {
            sheet.rows.iter().map(|row| RowRef {
                sheet: id.clone(),
                key: row.key.clone(),
            })
        })
        .collect()
}

/// `entries` holds each draft's entries, by the draft's place.
pub(crate) fn apply(
    plan: &ImportPlanV2,
    grouping: &Grouping<'_>,
    sheets: &[SheetIn<'_>],
    drafts: &mut [Draft],
    entries: &[Vec<EntryRef>],
    orphans: &mut Vec<Orphan>,
    anomalies: &mut Vec<Anomaly>,
) -> Decided {
    let wanted: HashSet<&EntryRef> = singles(&plan.decisions)
        .map(|single| single.entry)
        .collect();
    let mut holders: HashMap<&EntryRef, Vec<RecordId>> = HashMap::new();
    if !wanted.is_empty() {
        for (at, held) in entries.iter().enumerate() {
            let id = RecordId(u32::try_from(at).unwrap_or(u32::MAX));
            for entry in held.iter().filter(|entry| wanted.contains(entry)) {
                holders.entry(entry).or_default().push(id);
            }
        }
    }
    let rows = rows_read(sheets);
    let changed: BTreeSet<&SheetId> = sheets
        .iter()
        .filter(|sheet| line_keys_changed(sheet))
        .map(|sheet| &sheet.plan.id)
        .collect();
    let mut decided = Decided {
        sets: HashMap::new(),
        matches: HashMap::new(),
    };
    let mut set_by: BTreeMap<(RecordId, SlotName), Vec<(usize, &EntryRef)>> = BTreeMap::new();

    for Single {
        decision,
        entry,
        act,
    } in singles(&plan.decisions)
    {
        let orphan = |reason| Orphan {
            decision,
            entry: entry.clone(),
            reason,
        };
        let present = match entry {
            EntryRef::File(file) => grouping.by_ref.contains_key(file),
            EntryRef::Row(row) => rows.contains(row),
        };
        if !present {
            orphans.push(orphan(OrphanReason::MissingEntry));
            continue;
        }
        if let EntryRef::Row(row) = entry
            && changed.contains(&row.sheet)
        {
            orphans.push(orphan(OrphanReason::LineKeyChanged));
            continue;
        }
        let held = holders.get(entry).map(Vec::as_slice).unwrap_or_default();
        let of_kind = |kind: RecordKind| {
            held.iter()
                .copied()
                .find(|id| drafts[id.index()].kind == kind)
        };
        let target = match act {
            Act::Leave => match entry {
                EntryRef::File(_) => {
                    of_kind(RecordKind::Item).or_else(|| of_kind(RecordKind::Media))
                }
                EntryRef::Row(_) => held.first().copied(),
            },
            Act::Set { target, .. } | Act::Match { target, .. } => of_kind(target),
            Act::Flag { .. } => continue,
        };
        let Some(target) = target else {
            orphans.push(orphan(OrphanReason::WrongTarget));
            continue;
        };
        match act {
            Act::Leave => drafts[target.index()].left = Some(LeftBy::Decision(decision)),
            Act::Set { slot, .. } => {
                let by = decided
                    .sets
                    .entry(target)
                    .or_default()
                    .entry(slot.clone())
                    .or_default();
                // A `many` on two entries of one record is one decision.
                if by.last() != Some(&decision) {
                    by.push(decision);
                }
                set_by
                    .entry((target, slot.clone()))
                    .or_default()
                    .push((decision, entry));
            }
            Act::Match { resource, .. } => {
                decided.matches.insert(target, resource);
            }
            Act::Flag { .. } => {}
        }
    }

    // Two decisions on two entries: a `many` on two entries of one record,
    // or two decisions on one entry, is no overlap.
    for ((record, slot), by) in set_by {
        let entries: BTreeSet<&EntryRef> = by.iter().map(|(_, entry)| *entry).collect();
        let decisions: BTreeSet<usize> = by.iter().map(|(decision, _)| *decision).collect();
        if entries.len() > 1 && decisions.len() > 1 {
            anomalies.push(Anomaly::DecisionOverlap {
                record,
                slot,
                decisions: decisions.into_iter().collect(),
            });
        }
    }
    decided
}
