//! Raising flags, and the policy each one takes (`docs/import-v2.md`,
//! "Flags").
//!
//! A flag's policy is the latest `flag` decision on an entry of what it is
//! about (filtered by slot when the decision names one), else the plan's
//! policy for the kind, else the table's default. A blocking kind has no
//! policy. The caller acts on the policy [`FlagBook::raise`] returns, so the
//! flag listed and the effect applied cannot disagree.

use std::collections::HashMap;

use encoding_rs::WINDOWS_1252;

use super::decisions::{Decision, EntryRef};
use super::model::{Flag, Subject};
use super::plan::{FlagKind, FlagPolicy, ImportPlanV2, SlotName};

struct Choice {
    decision: usize,
    kind: FlagKind,
    slot: Option<SlotName>,
    policy: FlagPolicy,
}

pub(crate) struct FlagBook<'p> {
    plan: &'p ImportPlanV2,
    by_entry: HashMap<EntryRef, Vec<Choice>>,
    flags: Vec<Flag>,
}

impl<'p> FlagBook<'p> {
    pub(crate) fn new(plan: &'p ImportPlanV2) -> Self {
        let mut by_entry: HashMap<EntryRef, Vec<Choice>> = HashMap::new();
        for (decision, choice) in plan.decisions.iter().enumerate() {
            if let Decision::Flag {
                entry,
                flag,
                slot,
                policy,
            } = choice
            {
                by_entry.entry(entry.clone()).or_default().push(Choice {
                    decision,
                    kind: *flag,
                    slot: slot.clone(),
                    policy: *policy,
                });
            }
        }
        Self {
            plan,
            by_entry,
            flags: Vec::new(),
        }
    }

    /// The policy a flag of `kind` about these entries takes, without
    /// raising it. `None` for a blocking kind.
    pub(crate) fn policy(
        &self,
        kind: FlagKind,
        slot: Option<&SlotName>,
        entries: &[EntryRef],
    ) -> Option<FlagPolicy> {
        let planned = self.plan.policy(kind)?;
        let chosen = entries
            .iter()
            .filter_map(|entry| self.by_entry.get(entry))
            .flatten()
            .filter(|choice| {
                choice.kind == kind
                    && choice
                        .slot
                        .as_ref()
                        .is_none_or(|wanted| Some(wanted) == slot)
            })
            .max_by_key(|choice| choice.decision);
        Some(chosen.map_or(planned, |choice| choice.policy))
    }

    /// List a flag and return the policy it takes.
    pub(crate) fn raise(
        &mut self,
        kind: FlagKind,
        subject: Subject,
        slot: Option<SlotName>,
        detail: Option<String>,
        entries: &[EntryRef],
    ) -> Option<FlagPolicy> {
        let policy = self.policy(kind, slot.as_ref(), entries);
        self.flags.push(Flag {
            kind,
            subject,
            slot,
            policy,
            detail,
        });
        policy
    }

    pub(crate) fn into_flags(self) -> Vec<Flag> {
        self.flags
    }
}

/// U+FFFD, or UTF-8 that passed once through a 1252 reader (`CafÃ©`): the
/// sheet reader's `encoding-suspect` test, applied to every value.
pub(crate) fn looks_misdecoded(text: &str) -> bool {
    if text.contains('\u{FFFD}') {
        return true;
    }
    if text.is_ascii() {
        return false;
    }
    let (bytes, _, unmappable) = WINDOWS_1252.encode(text);
    !unmappable && std::str::from_utf8(&bytes).is_ok_and(|repaired| repaired != text)
}

/// The media types an extension names, for `format-mismatch`. Only
/// extensions whose type the sniffer can tell are listed: a file it cannot
/// tell (`application/octet-stream`) is never a mismatch.
pub(crate) fn expected_types(extension: &str) -> Option<&'static [&'static str]> {
    Some(match extension {
        "jpg" | "jpeg" | "jpe" => &["image/jpeg"],
        "png" => &["image/png"],
        "gif" => &["image/gif"],
        "webp" => &["image/webp"],
        "tif" | "tiff" => &["image/tiff"],
        "bmp" => &["image/bmp"],
        "pdf" => &["application/pdf"],
        "txt" | "csv" | "tsv" | "md" => &["text/plain", "text/csv"],
        _ => return None,
    })
}
