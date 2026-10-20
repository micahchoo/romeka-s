//! The `flags` section: each flag kind, slot and policy with its count and
//! sample records (`docs/import-v2.md`, "Cause samples").

use std::collections::HashMap;

use serde::Serialize;

use super::facts::SAMPLES;
use super::query::record_of;
use super::sample::sample;
use crate::import::decisions::EntryRef;
use crate::import::model::{Flag, Model, RecordId, Subject};
use crate::import::plan::{FlagKind, FlagPolicy, SlotName};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlagCount {
    pub kind: FlagKind,
    pub slot: Option<SlotName>,
    pub policy: Option<FlagPolicy>,
    pub count: usize,
    /// At most three flags of this count, drawn by the request's seed.
    pub samples: Vec<FlagSample>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlagSample {
    pub subject: Subject,
    /// The record the subject is, or the record its entry lands in.
    pub record: Option<RecordId>,
    pub detail: Option<String>,
}

type CountKey<'m> = (FlagKind, Option<&'m SlotName>, Option<FlagPolicy>);

pub(super) fn flag_counts(model: &Model, seed: u64) -> Vec<FlagCount> {
    let mut by: HashMap<CountKey<'_>, Vec<(u64, &Flag)>> = HashMap::new();
    for (at, flag) in (0_u64..).zip(&model.flags) {
        by.entry((flag.kind, flag.slot.as_ref(), flag.policy))
            .or_default()
            .push((at, flag));
    }
    let mut entries: Option<HashMap<&EntryRef, Option<RecordId>>> = None;
    let mut counts: Vec<FlagCount> = by
        .into_iter()
        .map(|((kind, slot, policy), flags)| {
            let count = flags.len();
            let samples = sample(flags.into_iter(), seed, SAMPLES)
                .into_iter()
                .map(|flag| {
                    let entries = entries.get_or_insert_with(|| {
                        model
                            .entries
                            .iter()
                            .map(|state| (&state.entry, state.record))
                            .collect()
                    });
                    FlagSample {
                        subject: flag.subject.clone(),
                        record: record_of(&flag.subject, entries),
                        detail: flag.detail.clone(),
                    }
                })
                .collect();
            FlagCount {
                kind,
                slot: slot.cloned(),
                policy,
                count,
                samples,
            }
        })
        .collect();
    counts.sort_by(|a, b| {
        (
            a.kind,
            a.slot.as_ref().map(SlotName::as_str),
            a.policy.map(|p| p as u8),
        )
            .cmp(&(
                b.kind,
                b.slot.as_ref().map(SlotName::as_str),
                b.policy.map(|p| p as u8),
            ))
    });
    counts
}
