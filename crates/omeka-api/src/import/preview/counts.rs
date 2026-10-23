//! The `values` section: the distinct values of a slot, each with the count
//! of records that hold it (`docs/import-v2.md`, "Values with counts").
//!
//! Asked for one `slot`, it counts that slot rule's values, any slot,
//! **before** its value map, so a mapped value stays visible under its own
//! text, and says what the map makes each one (`to`). It also offers near
//! spellings as joins (`near`), never applied until the person confirms
//! one. Asked for no slot, it counts the three slots that name things (item
//! set, item type, class) as the run writes them, after the map: what the
//! Simple and Advanced screens read until they ask for one slot.
//!
//! Two values are one value only when they are equal after trim, NFC and
//! lower-casing ([`fold_title`]). Nothing else joins them: "Letters" and
//! "पत्र" stay apart from "Letter", and the person joins them with the
//! slot's value map.

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use super::Derived;
use super::query::{Page, page};
use crate::import::derive::{Sheets, derive};
use crate::import::model::{Model, Record};
use crate::import::plan::{
    EachValue, ImportPlan, LevelId, RecordKind, SeriesIx, SetSource, SlotName,
};
use crate::import::values::fold_title;

/// The slots the section counts when the query names none.
pub const COUNTED: [SlotName; 3] = [SlotName::Sets, SlotName::Template, SlotName::Class];

/// Above this many distinct values no near spelling is offered: every pair
/// is compared, and AF `Keywords`' 471 values already make about 110,000
/// pairs. A guess at the cost limit.
pub const MAX_NEAR_VALUES: usize = 2000;

/// The letters the shorter of two near spellings must hold: below it, one
/// letter apart is most of the word (`ink`, `inks`; `cap`, `cup`).
const MIN_NEAR_LETTERS: usize = 4;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValuesQuery {
    /// `item` when left out.
    #[serde(default)]
    pub kind: Option<RecordKind>,
    /// With `slot`: only the records of this series. Left out, every
    /// series' records, each read through its own series' map.
    #[serde(default)]
    pub series: Option<LevelId>,
    /// The one slot to count, before its value map. Left out, the
    /// [`COUNTED`] slots after it.
    #[serde(default)]
    pub slot: Option<SlotName>,
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub per_page: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotValues {
    pub slot: SlotName,
    /// The most records first.
    pub values: Page<ValueCount>,
    /// Offered joins, the most records first. Never applied until
    /// confirmed; empty with no `slot` in the query.
    pub near: Vec<NearSpelling>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueCount {
    /// The value as the first record holding it writes it.
    pub text: String,
    /// Records that hold the value.
    pub count: usize,
    /// Every spelling that folds to the value, sorted.
    pub spellings: Vec<String>,
    /// What the map makes it: `None` unmapped, `Some("")` left out. Always
    /// `None` with no `slot` in the query, whose values are the map's own.
    pub to: Option<String>,
}

/// Two values one letter apart: the rarer, `from`, would become `to`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NearSpelling {
    pub from: String,
    pub from_records: usize,
    pub to: String,
    pub to_records: usize,
}

pub(super) fn values(derived: &Derived, query: &ValuesQuery) -> Vec<SlotValues> {
    let kind = query.kind.unwrap_or(RecordKind::Item);
    let Some(slot) = &query.slot else {
        return COUNTED
            .iter()
            .map(|slot| SlotValues {
                slot: slot.clone(),
                values: paged(counted(&derived.model, slot, kind, |_| true), query),
                near: Vec::new(),
            })
            .collect();
    };
    let plan = &derived.plan;
    let series: Option<Option<SeriesIx>> = query.series.as_ref().map(|id| {
        plan.structure
            .all_series()
            .find(|(_, series)| series.id() == id)
            .map(|(at, _)| at)
    });
    let sheets: Sheets = derived
        .sheets
        .iter()
        .map(|(id, read)| (id.clone(), read.sheet.clone()))
        .collect();
    let model = derive(
        &derived.stored.index,
        &sheets,
        &without_maps(plan, slot),
        &derived.catalog,
        &derived.tags,
    );
    let maps = Maps::new(plan, kind, slot);
    let mut values = counted(&model, slot, kind, |record| {
        series.is_none_or(|at| at == Some(record.series))
    });
    for (_, value) in &mut values {
        value.count.to = maps.to(value.series, &value.folded);
    }
    let near = near(&values);
    vec![SlotValues {
        slot: slot.clone(),
        values: paged(values, query),
        near,
    }]
}

/// One distinct value: the series of the first record that holds it, its
/// folded text and its count.
struct Counted {
    series: SeriesIx,
    folded: String,
    count: ValueCount,
}

/// The distinct values of `slot` in the kept records of `kind` that `keep`
/// keeps, the most records first, a tie in the order first seen.
fn counted(
    model: &Model,
    slot: &SlotName,
    kind: RecordKind,
    keep: impl Fn(&Record) -> bool,
) -> Vec<(usize, Counted)> {
    let mut by: HashMap<String, (usize, Counted)> = HashMap::new();
    for record in model
        .kept()
        .filter(|record| record.kind == kind && keep(record))
    {
        let Some(values) = record.slots.get(slot) else {
            continue;
        };
        let mut seen = BTreeSet::new();
        for value in values {
            let text = &value.value.text;
            let folded = fold_title(text);
            if folded.is_empty() {
                continue;
            }
            let first_seen = by.len();
            let (_, one) = by.entry(folded.clone()).or_insert_with(|| {
                (
                    first_seen,
                    Counted {
                        series: record.series,
                        folded: folded.clone(),
                        count: ValueCount {
                            text: text.clone(),
                            count: 0,
                            spellings: Vec::new(),
                            to: None,
                        },
                    },
                )
            });
            if !one.count.spellings.contains(text) {
                one.count.spellings.push(text.clone());
            }
            if seen.insert(folded) {
                one.count.count += 1;
            }
        }
    }
    let mut counted: Vec<(usize, Counted)> = by.into_values().collect();
    counted.sort_by(|(a_at, a), (b_at, b)| b.count.count.cmp(&a.count.count).then(a_at.cmp(b_at)));
    counted
}

fn paged(values: Vec<(usize, Counted)>, query: &ValuesQuery) -> Page<ValueCount> {
    page(
        values.into_iter(),
        query.page,
        query.per_page,
        |(_, mut one)| {
            one.count.spellings.sort();
            one.count
        },
    )
}

/// `plan` with no value map on `slot`: its rule's in every series, and for
/// `sets` every `each` set source's too, since those name an item's sets.
fn without_maps(plan: &ImportPlan, slot: &SlotName) -> ImportPlan {
    let mut plan = plan.clone();
    let every: Vec<SeriesIx> = plan.structure.all_series().map(|(at, _)| at).collect();
    for at in every {
        for kind in [RecordKind::Item, RecordKind::Media, RecordKind::ItemSet] {
            if let Some(rule) = plan.structure.rules_mut(at, kind).get_mut(slot) {
                rule.map.clear();
            }
        }
    }
    if *slot == SlotName::Sets {
        for map in plan.structure.each_maps_mut() {
            map.clear();
        }
    }
    plan
}

/// Each series' value maps of one slot, their keys folded; the first key
/// wins when two fold alike, as the run reads them.
struct Maps(Vec<Vec<HashMap<String, String>>>);

impl Maps {
    fn new(plan: &ImportPlan, kind: RecordKind, slot: &SlotName) -> Self {
        let folded = |map: &indexmap::IndexMap<String, String>| {
            let mut folded = HashMap::new();
            for (key, to) in map {
                folded.entry(fold_title(key)).or_insert_with(|| to.clone());
            }
            folded
        };
        Self(
            plan.structure
                .all_series()
                .map(|(at, series)| {
                    let mut maps: Vec<HashMap<String, String>> = series
                        .rules(kind)
                        .get(slot)
                        .map(|rule| folded(&rule.map))
                        .into_iter()
                        .collect();
                    if *slot == SlotName::Sets && kind == RecordKind::Item {
                        let each =
                            plan.structure
                                .memberships(at)
                                .into_iter()
                                .filter_map(|source| match source {
                                    SetSource::Each(EachValue { each }) => Some(folded(&each.map)),
                                    _ => None,
                                });
                        maps.extend(each);
                    }
                    maps
                })
                .collect(),
        )
    }

    fn to(&self, series: SeriesIx, folded: &str) -> Option<String> {
        self.0
            .get(series.index())?
            .iter()
            .find_map(|map| map.get(folded).cloned())
    }
}

/// Every pair of unmapped values one edit apart (a letter added, dropped,
/// changed, or two neighbours swapped), with no digit in either and at
/// least [`MIN_NEAR_LETTERS`] letters in the shorter, the rarer to the
/// commoner; a tie goes to the one seen first. None above
/// [`MAX_NEAR_VALUES`].
fn near(values: &[(usize, Counted)]) -> Vec<NearSpelling> {
    if values.len() > MAX_NEAR_VALUES {
        return Vec::new();
    }
    let open: Vec<(Vec<char>, &Counted)> = values
        .iter()
        .map(|(_, one)| one)
        .filter(|one| one.count.to.is_none() && !one.folded.chars().any(char::is_numeric))
        .map(|one| (one.folded.chars().collect(), one))
        .collect();
    let mut out = Vec::new();
    for (at, (a, commoner)) in open.iter().enumerate() {
        for (b, rarer) in &open[at + 1..] {
            let shorter = if a.len() <= b.len() { a } else { b };
            if shorter.iter().filter(|c| c.is_alphabetic()).count() < MIN_NEAR_LETTERS
                || !one_edit(a, b)
            {
                continue;
            }
            // `values` holds the most records first.
            out.push(NearSpelling {
                from: rarer.count.text.clone(),
                from_records: rarer.count.count,
                to: commoner.count.text.clone(),
                to_records: commoner.count.count,
            });
        }
    }
    out.sort_by(|a, b| {
        (b.from_records + b.to_records)
            .cmp(&(a.from_records + a.to_records))
            .then_with(|| a.from.cmp(&b.from))
    });
    out
}

/// Whether `a` and `b` differ by one letter added, dropped or changed, or
/// two neighbours swapped.
fn one_edit(a: &[char], b: &[char]) -> bool {
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let start = short.iter().zip(long).take_while(|(x, y)| x == y).count();
    match long.len() - short.len() {
        0 if start == short.len() => false,
        0 => {
            short[start + 1..] == long[start + 1..]
                || (start + 1 < short.len()
                    && short[start] == long[start + 1]
                    && short[start + 1] == long[start]
                    && short[start + 2..] == long[start + 2..])
        }
        1 => short[start..] == long[start + 1..],
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::one_edit;

    fn edit(a: &str, b: &str) -> bool {
        one_edit(
            &a.chars().collect::<Vec<_>>(),
            &b.chars().collect::<Vec<_>>(),
        )
    }

    #[test]
    fn one_edit_is_a_letter_added_dropped_changed_or_two_swapped() {
        assert!(edit("dilp patil", "dilip patil"), "added");
        assert!(edit("dilip patil", "dilp patil"), "dropped");
        assert!(edit("seylon", "ceylon"), "changed");
        assert!(edit("balck", "black"), "swapped");
        assert!(!edit("black", "black"), "the same");
        assert!(!edit("bakcl", "black"), "two apart");
        assert!(!edit("dilp", "dilipp"), "two added");
    }
}
