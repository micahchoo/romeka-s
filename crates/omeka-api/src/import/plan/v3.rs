//! The v3 plan as the wire and the database hold it: the v2 members less
//! `grouping`, `example`, `sheets` and `slots`, which move into the series
//! of `structure`.

use serde::{Deserialize, Serialize};

use super::structure::Structure;
use super::{
    ActionPlan, AnomalyAnswers, Defaults, FlagKind, FlagPolicy, ImportPlan, NewTerm, NewType,
    PlanError, PlanVersion, SourcePlan, WireVersion, check,
};
use crate::import::decisions::Decision;

/// Named as the plan, as [`super::v2::PlanV2`] is.
#[derive(Serialize, Deserialize)]
#[serde(rename = "ImportPlan", rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct PlanV3 {
    version: WireVersion<3>,
    sources: Vec<SourcePlan>,
    structure: Structure,
    action: ActionPlan,
    defaults: Defaults,
    flags: indexmap::IndexMap<FlagKind, FlagPolicy>,
    decisions: Vec<Decision>,
    new_types: Vec<NewType>,
    new_terms: Vec<NewTerm>,
    #[serde(default, skip_serializing_if = "AnomalyAnswers::is_default")]
    anomalies: AnomalyAnswers,
    /// The admin's Simple state, keyed by level id. Kept, never read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    simple: Option<serde_json::Map<String, serde_json::Value>>,
}

/// The v3 plan as the engine reads it. Refused when a series holds an
/// `item.sets` rule: in memory that place is the folded free set's
/// ([`Structure::fold`]), so the rule is checked on the wire.
impl TryFrom<PlanV3> for ImportPlan {
    type Error = PlanError;

    fn try_from(wire: PlanV3) -> Result<Self, PlanError> {
        check::no_series_sets(&wire.structure)?;
        let mut structure = wire.structure;
        structure.fold();
        Ok(Self {
            version: PlanVersion::V3,
            sources: wire.sources,
            structure,
            action: wire.action,
            defaults: wire.defaults,
            flags: wire.flags,
            decisions: wire.decisions,
            new_types: wire.new_types,
            new_terms: wire.new_terms,
            anomalies: wire.anomalies,
            simple: wire.simple,
        })
    }
}

impl From<ImportPlan> for PlanV3 {
    fn from(plan: ImportPlan) -> Self {
        let mut structure = plan.structure;
        structure.unfold();
        Self {
            version: WireVersion,
            sources: plan.sources,
            structure,
            action: plan.action,
            defaults: plan.defaults,
            flags: plan.flags,
            decisions: plan.decisions,
            new_types: plan.new_types,
            new_terms: plan.new_terms,
            anomalies: plan.anomalies,
            simple: plan.simple,
        }
    }
}
