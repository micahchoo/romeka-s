//! The import plan: the rule and the decisions, stored as JSON on the
//! draft and sent by the admin (`docs/import-v2.md`, "The plan"). It comes
//! in as version 2 ([`v2`], lifted to one series) or version 3 ([`v3`], the
//! tree of `briefs/import-plan-v3.md`), and is written back in the version
//! it came in as.
//!
//! [`ImportPlan::from_json`] is the one parse boundary. Types refuse what
//! serde can see (unknown members, closed string sets, path and term
//! shapes); [`check`] refuses what needs the whole plan (ids, refs,
//! combine, delete, new types and terms). Inside the crate the plan is
//! trusted. A check that needs the install, such as a new type whose label
//! a template already has, is [`ImportPlan::refuse_existing_labels`],
//! which the caller feeds; the plan itself holds no database or files.

mod action;
mod anomalies;
#[cfg(test)]
mod anomalies_tests;
mod check;
mod flags;
mod names;
mod path;
mod sets;
mod slots;
mod source;
mod structure;
#[cfg(test)]
mod tests;
#[cfg(doctest)]
mod tests {
    /// The engine reads the plan's pattern, lists and slot rules only
    /// through [`Structure`](super::Structure): the members sit in private
    /// fields, so a read from outside `plan/` does not compile.
    ///
    /// ```compile_fail,E0616
    /// fn read(plan: &omeka_api::import::plan::ImportPlan) -> usize {
    ///     plan.structure.levels.len()
    /// }
    /// ```
    ///
    /// The v2 member is not on the plan at all:
    ///
    /// ```compile_fail,E0609
    /// fn read(plan: &omeka_api::import::plan::ImportPlan) -> usize {
    ///     plan.sheets.len()
    /// }
    /// ```
    ///
    /// The calls are how the engine reads them:
    ///
    /// ```
    /// fn read(plan: &omeka_api::import::plan::ImportPlan) -> usize {
    ///     plan.structure.lists().count()
    /// }
    /// ```
    fn engine_reads_the_plan_through_the_structure() {}
}
mod v2;
mod v3;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub use action::{
    Action, ActionPlan, MatchKey, MatchPlan, MatchRule, NamedScope, Scope, SetsScope, Unmatched,
};
pub use anomalies::{AnomalyAnswers, IdenticalAnswer, ItemsWithoutRowAnswer, UnmatchedFilesAnswer};
pub use flags::{FlagKind, FlagOn, FlagPolicy, PolicyDefault};
pub use names::{RecordKind, SheetId, SlotName, SourceId, Term, local_term};
pub use path::{RelPath, RelPathError};
pub use sets::{EachSet, EachValue, SetById, SetByTitle, SetNames, SetSource};
pub use slots::{
    Combine, EachFile, FileFact, FolderLevel, SlotRule, SlotRules, Slots, SourceRef, TagFiles,
    TagId, TagPart, WholePath,
};
pub use source::{
    Auto, ColumnOptions, ColumnPlan, ColumnRef, Delimiter, Encoding, EncodingChoice, Join,
    JoinNormalize, JoinOn, KindColumn, Quote, ReadOptions, RowKind, SheetPlan, SourcePlan,
};
pub use structure::{
    AllFiles, FolderScope, Level, LevelId, Placement, Series, SeriesIx, SeriesScope, SetLevel,
    Structure,
};

use super::decisions::Decision;
use super::values::Visibility;

/// The plan the engine reads. Its pattern, lists and slot rules sit in
/// [`Structure`]; its JSON is [`v2::PlanV2`] or [`v3::PlanV3`], by
/// [`Self::version`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPlan {
    /// The version the plan came in as, which it is written back in, so a
    /// v2 admin gets a v2 plan back. P4 deletes it with the lower.
    pub version: PlanVersion,
    pub sources: Vec<SourcePlan>,
    pub structure: Structure,
    pub action: ActionPlan,
    pub defaults: Defaults,
    /// Policies the plan sets; a kind it leaves out has its default.
    pub flags: indexmap::IndexMap<FlagKind, FlagPolicy>,
    pub decisions: Vec<Decision>,
    pub new_types: Vec<NewType>,
    pub new_terms: Vec<NewTerm>,
    pub anomalies: AnomalyAnswers,
    /// The admin's Simple-screen state (the answers a person gave). Kept and
    /// never read, like the series' example.
    pub simple: Option<serde_json::Map<String, serde_json::Value>>,
}

/// The wire version a plan came in as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlanVersion {
    V2,
    V3,
}

/// The member `version: N` of a wire plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct WireVersion<const N: u64>;

impl<const N: u64> Serialize for WireVersion<N> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(N)
    }
}

impl<'de, const N: u64> Deserialize<'de> for WireVersion<N> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match u64::deserialize(deserializer)? {
            n if n == N => Ok(Self),
            other => Err(serde::de::Error::custom(format!(
                "version {other} is not {N}"
            ))),
        }
    }
}

/// Written in the version the plan came in as. A v2 plan the lower refuses
/// holds more than v2 can say, so it is written as v3 rather than cut.
impl Serialize for ImportPlan {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.version == PlanVersion::V2
            && let Ok(wire) = v2::PlanV2::try_from(self.clone())
        {
            return wire.serialize(serializer);
        }
        v3::PlanV3::from(self.clone()).serialize(serializer)
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
    #[error("the plan is not an import v3 plan: {0}")]
    ShapeV3(serde_json::Error),
    #[error("the plan's version is {0}, not 2 or 3")]
    Version(String),
    #[error("the plan cannot be written as version 2: {0}")]
    NotV2(&'static str),
    #[error("two levels have the id {0:?}")]
    DuplicateLevel(String),
    #[error("series {second:?} takes the same files as series {first:?}")]
    DuplicateScope { first: String, second: String },
    #[error("series {series:?} takes a folder of source {missing:?}, which the plan does not have")]
    ScopeSource { series: String, missing: String },
    #[error("a slot of series {series:?} reads list {sheet:?}, which is a list of another series")]
    ForeignList { series: String, sheet: String },
    #[error("series {0:?} has an item.sets rule; sets are set levels and free sets")]
    SeriesSets(String),
    #[error("a set named by title has an empty title ({0})")]
    EmptySetTitle(String),
    /// The temporary rule: what the engine cannot run until U4 and U5.
    #[error("this server cannot run this plan yet: {0}")]
    NotYet(NotYet),
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
    #[error("the {slot} value map has the keys {first:?} and {second:?}, which read as one")]
    MapKeysCollide {
        slot: String,
        first: String,
        second: String,
    },
    #[error("decision {0} changes many records but names no entry")]
    EmptyMany(usize),
    #[error("decision {0} moves many entries, and a row is not a file")]
    ManyMovesRow(usize),
    #[error("the new fields {first:?} and {second:?} both make {term}")]
    TermsCollide {
        first: String,
        second: String,
        term: String,
    },
}

/// What a plan the types accept asks of the engine that it does not do yet.
/// Each is a rule U4 (scopes and series) or U5 (sets) deletes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotYet {
    SeveralSeries(usize),
    SetLevel(String),
    SeriesSet(String),
    Scope(String),
    FreeSets,
}

impl std::fmt::Display for NotYet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SeveralSeries(n) => write!(f, "it reads one series only, and this plan has {n}"),
            Self::SetLevel(id) => write!(
                f,
                "it does not read set levels yet, and this plan has the set level {id:?}"
            ),
            Self::SeriesSet(id) => write!(
                f,
                "it does not read a series' own set yet, and series {id:?} has one"
            ),
            Self::Scope(id) => write!(
                f,
                "it reads only a series that takes every file, and series {id:?} takes some folders"
            ),
            Self::FreeSets => write!(f, "it reads free sets only as a version 2 plan's sets rule"),
        }
    }
}

impl ImportPlan {
    /// Parse a plan sent by the admin or read from the database.
    ///
    /// # Errors
    /// [`PlanError::V1Plan`] for a v1 plan, [`PlanError::Version`] for
    /// another version, [`PlanError::Shape`] (v2) or [`PlanError::ShapeV3`]
    /// for anything the types refuse, and the other variants for what the
    /// contract refuses across members.
    pub fn from_json(text: &str) -> Result<Self, PlanError> {
        let value: serde_json::Value = serde_json::from_str(text)?;
        // Each wire type parses the text again, so a shape error names its
        // line and column.
        let plan = match value.get("version") {
            Some(version) if version.as_u64() == Some(3) => {
                let wire: v3::PlanV3 = serde_json::from_str(text).map_err(PlanError::ShapeV3)?;
                Self::try_from(wire)?
            }
            Some(version) if version.as_u64() != Some(2) => {
                return Err(PlanError::Version(version.to_string()));
            }
            // v1 plans have no version; `setLevel` is a member every v1 plan has.
            None if value.get("setLevel").is_some() => return Err(PlanError::V1Plan),
            _ => Self::from(serde_json::from_str::<v2::PlanV2>(text)?),
        };
        check::plan(&plan)?;
        Ok(plan)
    }

    /// The plan as stored JSON, in the version it came in as. Every map key
    /// is a string, so this cannot fail; an empty string would mean
    /// serde_json broke that rule.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// The policy a flag of this kind gets, or `None` for a blocking kind.
    #[must_use]
    pub fn policy(&self, kind: FlagKind) -> Option<FlagPolicy> {
        let planned = self.flags.get(&kind).copied();
        Some(match kind.default_policy()? {
            PolicyDefault::Only(policy) => policy,
            PolicyDefault::Is(policy) => planned.unwrap_or(policy),
            PolicyDefault::ActionUnmatched => {
                planned.unwrap_or_else(|| self.action.unmatched.policy())
            }
        })
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

/// The pattern that groups by the name alone: it skips the folders and the
/// ending, so files whose names are equal but for the ending are one item.
/// The admin's `patternOf([])` writes the same text.
pub const NAME_ALONE: &str = r"^(?:[^/]*/)*(?<identifier>[^/]+?)(?:\.[^./]*)?$";

/// The plan a new draft starts with: one item per name ([`NAME_ALONE`]),
/// private, owned by the runner, the `create` action, every kind matched on
/// `dcterms:identifier` within this import's sets, nothing mapped and nothing
/// decided.
///
/// # Errors
/// What [`ImportPlan::from_json`] refuses about the sources: two with one id.
pub fn default_plan(sources: Vec<SourcePlan>) -> Result<ImportPlan, PlanError> {
    let by_identifier = || MatchRule {
        key: MatchKey::Term(Term::dcterms_identifier()),
        scope: Scope::Named(NamedScope::ImportSets),
    };
    let plan = ImportPlan {
        version: PlanVersion::V2,
        sources,
        structure: Structure::lifted(
            Grouping {
                pattern: Some(NAME_ALONE.to_owned()),
                case_fold: false,
            },
            None,
            Vec::new(),
            Slots::default(),
        ),
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
        anomalies: AnomalyAnswers::default(),
        simple: None,
    };
    check::plan(&plan)?;
    Ok(plan)
}
