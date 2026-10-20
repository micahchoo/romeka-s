//! The `values` section: the distinct values of the slots that name things
//! (item set, item type, class), each with the count of records that hold it
//! (`docs/import-v2.md`, "Values with counts").
//!
//! Two values are one value only when they are equal after trim, NFC and
//! lower-casing ([`fold_title`]). Nothing else joins them: "Letters" and
//! "पत्र" stay apart from "Letter", and the person joins them with the
//! slot's value map.

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use super::query::{Page, page};
use crate::import2::model::Model;
use crate::import2::plan::{RecordKind, SlotName};
use crate::import2::values::fold_title;

/// The slots the section counts.
pub const COUNTED: [SlotName; 3] = [SlotName::Sets, SlotName::Template, SlotName::Class];

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValuesQuery {
    /// `item` when left out.
    #[serde(default)]
    pub kind: Option<RecordKind>,
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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueCount {
    /// The value as the first record holding it writes it.
    pub text: String,
    /// Records the run writes that hold the value.
    pub count: usize,
    /// Every spelling that folds to the value, sorted.
    pub spellings: Vec<String>,
}

pub(super) fn values(model: &Model, query: &ValuesQuery) -> Vec<SlotValues> {
    let kind = query.kind.unwrap_or(RecordKind::Item);
    COUNTED
        .iter()
        .map(|slot| {
            let mut by: HashMap<String, (usize, ValueCount)> = HashMap::new();
            for record in model.kept().filter(|record| record.kind == kind) {
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
                    let (_, count) = by.entry(folded.clone()).or_insert_with(|| {
                        (
                            first_seen,
                            ValueCount {
                                text: text.clone(),
                                count: 0,
                                spellings: Vec::new(),
                            },
                        )
                    });
                    if !count.spellings.contains(text) {
                        count.spellings.push(text.clone());
                    }
                    if seen.insert(folded) {
                        count.count += 1;
                    }
                }
            }
            let mut counted: Vec<(usize, ValueCount)> = by.into_values().collect();
            counted.sort_by(|(a_at, a), (b_at, b)| b.count.cmp(&a.count).then(a_at.cmp(b_at)));
            SlotValues {
                slot: slot.clone(),
                values: page(
                    counted.into_iter(),
                    query.page,
                    query.per_page,
                    |(_, mut count)| {
                        count.spellings.sort();
                        count
                    },
                ),
            }
        })
        .collect()
}
