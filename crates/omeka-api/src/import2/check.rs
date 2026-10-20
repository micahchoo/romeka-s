//! Check: what a run of the model would do, and whether it may run
//! (`docs/import-v2.md`, "Wire", the `check` section, and "Lifecycle and
//! the ledger", "Digest").
//!
//! [`check`] is pure. The run route calls it on a fresh derive and refuses
//! to start unless the digest the admin saw is the digest it finds,
//! [`Check::can_run`] holds, and a delete is confirmed with
//! [`Check::deletes`].

use std::collections::BTreeMap;

use serde::Serialize;

use super::model::{Fate, Flag, Model, NewSet, OwnerRef};
use super::plan::{Action, FlagPolicy, ImportPlanV2, NewTypeFrom, RecordKind, Term};

/// What a record does when the run reaches it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Does {
    /// It writes a new resource.
    Create,
    /// It applies the plan's action to the resource it matched.
    Act(Action),
    /// It is left out.
    Leave,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Count {
    pub kind: RecordKind,
    pub does: Does,
    pub count: usize,
}

/// A template the run creates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTypeCheck {
    pub label: String,
    pub from: NewTypeFrom,
    /// The fields from the mapping; empty for a starter, whose fields the
    /// starter table lists.
    pub fields: Vec<Term>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewFieldCheck {
    pub label: String,
    pub term: Option<Term>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub action: Action,
    pub counts: Vec<Count>,
    pub new_sets: Vec<NewSet>,
    pub new_types: Vec<NewTypeCheck>,
    pub new_fields: Vec<NewFieldCheck>,
    /// How many flags each policy applies.
    pub flags: BTreeMap<String, usize>,
    /// The flags that stop the run.
    pub blocking: Vec<Flag>,
    /// Records the run writes with no template: no default, no "Base
    /// Resource".
    pub without_template: usize,
    /// Records whose owner names no user, which get the default owner.
    pub default_owner: usize,
    /// The resources a delete removes; the run needs this as `confirm`.
    pub deletes: usize,
    pub can_run: bool,
    pub digest: String,
}

#[must_use]
pub fn check(plan: &ImportPlanV2, model: &Model) -> Check {
    let action = plan.action.action;
    let mut counts: Vec<Count> = Vec::new();
    let mut without_template = 0;
    let mut default_owner = 0;
    for record in &model.records {
        let does = match record.fate {
            Fate::Create => Does::Create,
            Fate::Act(_) => Does::Act(action),
            Fate::Left(_) => Does::Leave,
        };
        match counts
            .iter_mut()
            .find(|count| count.kind == record.kind && count.does == does)
        {
            Some(count) => count.count += 1,
            None => counts.push(Count {
                kind: record.kind,
                does,
                count: 1,
            }),
        }
        if does != Does::Leave && action != Action::Delete {
            without_template += usize::from(record.resolved.template.is_none());
            default_owner += usize::from(matches!(record.resolved.owner, OwnerRef::Unknown(_)));
        }
    }
    let deletes = if action == Action::Delete {
        model
            .records
            .iter()
            .filter(|record| matches!(record.fate, Fate::Act(_)))
            .count()
    } else {
        0
    };
    let mut flags = BTreeMap::new();
    let mut blocking = Vec::new();
    for flag in &model.flags {
        match flag.policy {
            Some(policy) => *flags.entry(policy_name(policy).to_string()).or_default() += 1,
            None => blocking.push(flag.clone()),
        }
    }
    let new_types = plan
        .new_types
        .iter()
        .enumerate()
        .map(|(at, new_type)| NewTypeCheck {
            label: new_type.label.clone(),
            from: new_type.from.clone(),
            fields: model
                .new_types
                .iter()
                .find(|fields| fields.new_type == at)
                .map(|fields| fields.fields.clone())
                .unwrap_or_default(),
        })
        .collect();
    let new_fields = plan
        .new_terms
        .iter()
        .map(|new_term| NewFieldCheck {
            label: new_term.label.clone(),
            term: Term::local(&new_term.label),
        })
        .collect();
    Check {
        action,
        counts: {
            counts.sort_by_key(|count| (kind_order(count.kind), does_order(count.does)));
            counts
        },
        new_sets: model.new_sets.clone(),
        new_types,
        new_fields,
        flags,
        can_run: blocking.is_empty() && model.kept().next().is_some(),
        blocking,
        without_template,
        default_owner,
        deletes,
        digest: model.digest.clone(),
    }
}

fn kind_order(kind: RecordKind) -> u8 {
    match kind {
        RecordKind::Item => 0,
        RecordKind::Media => 1,
        RecordKind::ItemSet => 2,
    }
}

fn does_order(does: Does) -> u8 {
    match does {
        Does::Create => 0,
        Does::Act(_) => 1,
        Does::Leave => 2,
    }
}

fn policy_name(policy: FlagPolicy) -> &'static str {
    match policy {
        FlagPolicy::Keep => "keep",
        FlagPolicy::Repair => "repair",
        FlagPolicy::Leave => "leave",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use serde_json::json;

    use super::*;
    use crate::import2::catalog::Catalog;
    use crate::import2::derive::{Index, IndexedSource, NoTags, Sheets, derive};
    use crate::import2::evidence::{FileEntry, Walk};
    use crate::import2::plan::{RelPath, SourceId, SourcePlan, default_plan};

    fn index(paths: &[&str]) -> Index {
        Index {
            sources: vec![IndexedSource {
                id: SourceId("s1".into()),
                walk: Walk {
                    entries: paths
                        .iter()
                        .map(|path| FileEntry {
                            path: RelPath::parse(path).expect("path"),
                            size: 1,
                            modified: None,
                            sniffed: "image/jpeg".into(),
                            decoded: None,
                        })
                        .collect(),
                    ..Walk::default()
                },
            }],
        }
    }

    fn plan(edit: impl FnOnce(&mut serde_json::Value)) -> ImportPlanV2 {
        let plan = default_plan(vec![SourcePlan::Folder {
            id: SourceId("s1".into()),
            root: "archive".into(),
            path: RelPath::parse("").expect("root"),
        }])
        .expect("plan");
        let mut value = serde_json::to_value(plan).expect("json");
        edit(&mut value);
        ImportPlanV2::from_json(&value.to_string()).expect("valid")
    }

    #[test]
    fn check_counts_records_by_what_they_do_and_carries_the_digest() {
        let plan = plan(|plan| {
            plan["grouping"]["pattern"] = json!(r"^(?<identifier>[A-Z]\d)\.jpg$");
        });
        let model = derive(
            &index(&["A1.jpg", "B2.jpg", "notes.jpg"]),
            &Sheets::new(),
            &plan,
            &Catalog::default(),
            &NoTags,
        );
        let check = check(&plan, &model);
        let items = check
            .counts
            .iter()
            .find(|count| count.kind == RecordKind::Item && count.does == Does::Create)
            .expect("items");
        assert_eq!(items.count, 2);
        assert!(check.can_run);
        assert_eq!(check.digest, model.digest);
        assert_eq!(
            check.without_template, 4,
            "no base template: items and media"
        );
        assert!(check.blocking.is_empty());
    }

    #[test]
    fn a_blocking_flag_or_an_empty_model_cannot_run() {
        let empty = plan(|_| {});
        let model = derive(
            &index(&[]),
            &Sheets::new(),
            &empty,
            &Catalog::default(),
            &NoTags,
        );
        assert!(!check(&empty, &model).can_run, "nothing to write");

        let unknown = plan(|plan| {
            plan["grouping"]["pattern"] = json!(r"^(?<identifier>[A-Z]\d)\.jpg$");
            plan["slots"]["item"]["dcterms:nothing"] =
                json!({ "sources": [{ "from": "constant", "value": "x" }], "combine": "first" });
        });
        let model = derive(
            &index(&["A1.jpg"]),
            &Sheets::new(),
            &unknown,
            &Catalog::default(),
            &NoTags,
        );
        let check = check(&unknown, &model);
        assert!(!check.can_run);
        assert_eq!(check.blocking.len(), 1, "{:?}", check.blocking);
    }
}
