//! The v2 plan as the wire and the database hold it: the members flat.
//! A v2 plan is read by lifting it to one series ([`Structure::lifted`]) and
//! written by lowering that series back, so its JSON keeps the v2 members,
//! their order and their errors.

use serde::{Deserialize, Serialize};

use super::structure::{Series, Structure};
use super::{
    ActionPlan, AnomalyAnswers, Defaults, FlagKind, FlagPolicy, Grouping, ImportPlan, NewTerm,
    NewType, PlanError, PlanVersion, SheetPlan, Slots, SourcePlan, WireVersion,
};
use crate::import::decisions::Decision;

/// Named as the plan, so a shape error reads "expected struct ImportPlan"
/// as it did before this type.
#[derive(Serialize, Deserialize)]
#[serde(rename = "ImportPlan", rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct PlanV2 {
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
    /// Missing means the defaults, and the defaults are not written, so a
    /// plan stored before this member keeps its bytes.
    #[serde(default, skip_serializing_if = "AnomalyAnswers::is_default")]
    anomalies: AnomalyAnswers,
    /// Absent is not written, so a stored plan keeps its bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    simple: Option<serde_json::Map<String, serde_json::Value>>,
}

/// The lift.
impl From<PlanV2> for ImportPlan {
    fn from(wire: PlanV2) -> Self {
        Self {
            version: PlanVersion::V2,
            sources: wire.sources,
            structure: Structure::lifted(wire.grouping, wire.example, wire.sheets, wire.slots),
            action: wire.action,
            defaults: wire.defaults,
            flags: wire.flags,
            decisions: wire.decisions,
            new_types: wire.new_types,
            new_terms: wire.new_terms,
            anomalies: wire.anomalies,
            simple: wire.simple,
        }
    }
}

/// The lower: refused for a plan that is not one series `all` with no set,
/// taking every file, with at most the one `text` free set.
impl TryFrom<ImportPlan> for PlanV2 {
    type Error = PlanError;

    fn try_from(plan: ImportPlan) -> Result<Self, PlanError> {
        let Series {
            grouping,
            example,
            lists,
            slots,
            ..
        } = plan.structure.into_lifted().map_err(PlanError::NotV2)?;
        Ok(Self {
            version: WireVersion,
            sources: plan.sources,
            grouping,
            example,
            sheets: lists,
            slots,
            action: plan.action,
            defaults: plan.defaults,
            flags: plan.flags,
            decisions: plan.decisions,
            new_types: plan.new_types,
            new_terms: plan.new_terms,
            anomalies: plan.anomalies,
            simple: plan.simple,
        })
    }
}
