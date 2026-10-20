//! The import v2 plan: the rule and the decisions, stored as JSON on the
//! draft and sent by the admin (`docs/import-v2.md`, "The plan").
//!
//! [`ImportPlanV2::from_json`] is the one parse boundary. Types refuse what
//! serde can see (unknown members, closed string sets, path and term
//! shapes); [`check`] refuses what needs the whole plan (ids, refs,
//! combine, delete, new types and terms). Inside the crate the plan is
//! trusted. A check that needs the install, such as a new type whose label
//! a template already has, is [`ImportPlanV2::refuse_existing_labels`],
//! which the caller feeds; the plan itself holds no database or files.

mod action;
mod check;
mod flags;
mod names;
mod path;
mod slots;
mod source;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub use action::{
    Action, ActionPlan, MatchKey, MatchPlan, MatchRule, NamedScope, Scope, SetsScope, Unmatched,
};
pub use flags::{FlagKind, FlagPolicy, PolicyDefault};
pub use names::{RecordKind, SheetId, SlotName, SourceId, Term, local_term};
pub use path::{RelPath, RelPathError};
pub use slots::{
    Combine, EachFile, FileFact, FolderLevel, SlotRule, SlotRules, Slots, SourceRef, TagFiles,
    TagId, TagPart, WholePath,
};
pub use source::{
    Auto, ColumnOptions, ColumnPlan, ColumnRef, Delimiter, Encoding, EncodingChoice, Join,
    JoinNormalize, JoinOn, KindColumn, Quote, ReadOptions, RowKind, SheetPlan, SourcePlan,
};

use super::decisions::Decision;
use super::values::Visibility;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportPlanV2 {
    pub version: Version2,
    pub sources: Vec<SourcePlan>,
    pub grouping: Grouping,
    /// The admin's mark-an-example state. The server keeps it and never
    /// reads it, so its members are the admin's to define.
    #[serde(deserialize_with = "super::nullable")]
    pub example: Option<serde_json::Map<String, serde_json::Value>>,
    pub sheets: Vec<SheetPlan>,
    pub slots: Slots,
    pub action: ActionPlan,
    pub defaults: Defaults,
    /// Policies the plan sets; a kind it leaves out has its default.
    pub flags: indexmap::IndexMap<FlagKind, FlagPolicy>,
    pub decisions: Vec<Decision>,
    pub new_types: Vec<NewType>,
    pub new_terms: Vec<NewTerm>,
}

/// The member `version: 2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Version2;

impl Serialize for Version2 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(2)
    }
}

impl<'de> Deserialize<'de> for Version2 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match u64::deserialize(deserializer)? {
            2 => Ok(Self),
            other => Err(serde::de::Error::custom(format!(
                "version {other} is not 2"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Grouping {
    /// Kept as text: a draft may hold a pattern the admin is still typing,
    /// and the preview reports its error.
    #[serde(deserialize_with = "super::nullable")]
    pub pattern: Option<String>,
    pub case_fold: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Defaults {
    pub visibility: Visibility,
    pub owner: Owner,
    pub sites: Vec<i64>,
    #[serde(deserialize_with = "super::nullable")]
    pub template: Option<i64>,
}

/// The user who owns what the import writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Owner {
    /// The user who presses Run (on the CLI, `--as`).
    Runner,
    User(i64),
}

impl Serialize for Owner {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Runner => serializer.serialize_str("runner"),
            Self::User(id) => serializer.serialize_i64(*id),
        }
    }
}

impl<'de> Deserialize<'de> for Owner {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Id(i64),
            Name(String),
        }
        match Wire::deserialize(deserializer)? {
            Wire::Id(id) => Ok(Self::User(id)),
            Wire::Name(name) if name == "runner" => Ok(Self::Runner),
            Wire::Name(name) => Err(serde::de::Error::custom(format!(
                "owner {name:?} is neither \"runner\" nor a user id"
            ))),
        }
    }
}

/// A template the run creates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewType {
    pub label: String,
    pub from: NewTypeFrom,
    #[serde(deserialize_with = "super::nullable")]
    pub class: Option<Term>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NewTypeFrom {
    /// One field per term a record using the type writes.
    Mapping(FromMapping),
    Starter(FromStarter),
}

/// The text `mapping`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FromMapping {
    Mapping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FromStarter {
    pub starter: StarterId,
}

/// The starter templates the server offers (`GET /api/romeka/starters`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StarterId {
    Letter,
    Photograph,
    Postcard,
    Diary,
    Person,
    Other,
}

/// A property the run creates in the `local` vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewTerm {
    pub label: String,
}

/// A plan the boundary refuses.
#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    #[error("the plan is not an import v2 plan: {0}")]
    Shape(#[from] serde_json::Error),
    /// The caller answers 409 `v1Plan`.
    #[error("the plan is a v1 plan")]
    V1Plan,
    #[error("the plan's version is {0}, not 2")]
    Version(String),
    #[error("two sources have the id {0:?}")]
    DuplicateSource(String),
    #[error("two sheets have the id {0:?}")]
    DuplicateSheet(String),
    #[error("sheet {sheet:?} is in source {missing:?}, which the plan does not have")]
    MissingSource { sheet: String, missing: String },
    #[error("a slot reads sheet {0:?}, which the plan does not have")]
    MissingSheet(String),
    #[error("a slot reads column {index} of sheet {sheet:?}, which the sheet does not map")]
    MissingColumn { sheet: String, index: u32 },
    #[error("sheet {sheet:?} maps column {index} twice")]
    DuplicateColumn { sheet: String, index: u32 },
    #[error("{kind:?} slot {slot} holds one value, so it cannot combine all")]
    CombineAll { kind: RecordKind, slot: String },
    #[error("a delete plan has no slot rules")]
    SlotRulesOnDelete,
    #[error("the new type {0:?} has no label")]
    EmptyTypeLabel(String),
    #[error("two new types have the label {0:?}")]
    DuplicateNewType(String),
    #[error("a template already has the label {0:?}")]
    TypeExists(String),
    #[error("the new field {0:?} has no letters or digits")]
    EmptyTermLabel(String),
    #[error("the new fields {first:?} and {second:?} both make {term}")]
    TermsCollide {
        first: String,
        second: String,
        term: String,
    },
}

impl ImportPlanV2 {
    /// Parse a plan sent by the admin or read from the database.
    ///
    /// # Errors
    /// [`PlanError::V1Plan`] for a v1 plan, [`PlanError::Version`] for
    /// another version, [`PlanError::Shape`] for anything the types refuse,
    /// and the other variants for what the contract refuses across members.
    pub fn from_json(text: &str) -> Result<Self, PlanError> {
        let value: serde_json::Value = serde_json::from_str(text)?;
        match value.get("version") {
            Some(version) if version.as_u64() == Some(2) => {}
            Some(version) => return Err(PlanError::Version(version.to_string())),
            // v1 plans have no version; `setLevel` is a member every v1 plan has.
            None if value.get("setLevel").is_some() => return Err(PlanError::V1Plan),
            None => {}
        }
        // Parsed again from the text so a shape error names its line and column.
        let plan: Self = serde_json::from_str(text)?;
        check::plan(&plan)?;
        Ok(plan)
    }

    /// The plan as stored JSON. Every map key is a string, so this cannot
    /// fail; an empty string would mean serde_json broke that rule.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// The policy a flag of this kind gets, or `None` for a blocking kind.
    #[must_use]
    pub fn policy(&self, kind: FlagKind) -> Option<FlagPolicy> {
        let default = kind.default_policy()?;
        Some(self.flags.get(&kind).copied().unwrap_or(match default {
            PolicyDefault::Is(policy) => policy,
            PolicyDefault::ActionUnmatched => self.action.unmatched.policy(),
        }))
    }

    /// Refuse a new type whose label, compared as a title, an existing
    /// template already has.
    ///
    /// # Errors
    /// [`PlanError::TypeExists`] with the new type's label.
    pub fn refuse_existing_labels<'a>(
        &self,
        templates: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), PlanError> {
        check::existing_labels(self, templates)
    }
}

/// The plan a new draft starts with: private, owned by the runner, the
/// `create` action, every kind matched on `dcterms:identifier` within this
/// import's sets, nothing mapped and nothing decided.
///
/// # Errors
/// What [`ImportPlanV2::from_json`] refuses about the sources: two with one id.
pub fn default_plan(sources: Vec<SourcePlan>) -> Result<ImportPlanV2, PlanError> {
    let by_identifier = || MatchRule {
        key: MatchKey::Term(Term::dcterms_identifier()),
        scope: Scope::Named(NamedScope::ImportSets),
    };
    let plan = ImportPlanV2 {
        version: Version2,
        sources,
        grouping: Grouping {
            pattern: None,
            case_fold: false,
        },
        example: None,
        sheets: Vec::new(),
        slots: Slots::default(),
        action: ActionPlan {
            action: Action::Create,
            match_by: MatchPlan {
                item: by_identifier(),
                media: by_identifier(),
                item_set: by_identifier(),
            },
            unmatched: Unmatched::Leave,
        },
        defaults: Defaults {
            visibility: Visibility::Private,
            owner: Owner::Runner,
            sites: Vec::new(),
            template: None,
        },
        flags: indexmap::IndexMap::new(),
        decisions: Vec::new(),
        new_types: Vec::new(),
        new_terms: Vec::new(),
    };
    check::plan(&plan)?;
    Ok(plan)
}
