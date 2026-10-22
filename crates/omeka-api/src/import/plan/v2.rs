//! The v2 plan as the database still holds it for drafts and done imports
//! saved before the admin wrote v3: the members flat. It is only read,
//! lifted to one series ([`Structure::lifted`]); every plan is written as
//! v3.

use serde::Deserialize;

use super::structure::{LIFTED, Structure};
use super::{
    ActionPlan, AnomalyAnswers, Defaults, FlagKind, FlagPolicy, Grouping, ImportPlan, NewTerm,
    NewType, SheetPlan, Slots, SourcePlan, WireVersion,
};
use crate::import::decisions::Decision;

/// Named as the plan, so a shape error reads "expected struct ImportPlan"
/// as it did before this type.
#[derive(Deserialize)]
#[serde(rename = "ImportPlan", rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct PlanV2 {
    #[expect(dead_code, reason = "read only to refuse a version other than 2")]
    version: WireVersion<2>,
    sources: Vec<SourcePlan>,
    grouping: Grouping,
    /// The admin's mark-an-example state. The server keeps it and never
    /// reads it, so its members are the admin's to define.
    #[serde(deserialize_with = "crate::import::nullable")]
    example: Option<serde_json::Map<String, serde_json::Value>>,
    sheets: Vec<SheetPlan>,
    slots: Slots,
    action: ActionPlan,
    defaults: Defaults,
    flags: indexmap::IndexMap<FlagKind, FlagPolicy>,
    decisions: Vec<Decision>,
    new_types: Vec<NewType>,
    new_terms: Vec<NewTerm>,
    /// Missing means the defaults: a plan stored before this member.
    #[serde(default)]
    anomalies: AnomalyAnswers,
    #[serde(default)]
    simple: Option<serde_json::Map<String, serde_json::Value>>,
}

/// The lift: the v2 members into one series `all`, and Simple's flat
/// state under that series' id.
impl From<PlanV2> for ImportPlan {
    fn from(wire: PlanV2) -> Self {
        Self {
            sources: wire.sources,
            structure: Structure::lifted(wire.grouping, wire.example, wire.sheets, wire.slots),
            action: wire.action,
            defaults: wire.defaults,
            flags: wire.flags,
            decisions: wire.decisions,
            new_types: wire.new_types,
            new_terms: wire.new_terms,
            anomalies: wire.anomalies,
            simple: wire.simple.map(|simple| {
                serde_json::Map::from_iter([(LIFTED.to_owned(), serde_json::Value::Object(simple))])
            }),
            edits: Vec::new(),
        }
    }
}
