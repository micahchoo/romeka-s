//! The import plan: what the admin decided, stored as JSON on the draft.
//!
//! [`ImportPlan::from_json`] is the parse boundary. It refuses unknown members
//! and the states the types cannot rule out on their own (a header row of 0,
//! an unknown tag, a new field with no letters). Inside the crate the plan is
//! trusted. The file-name pattern is kept as text: a draft may hold a pattern
//! the admin is still typing, and [`crate::import::pattern::FilePattern`]
//! reports its error in the preview.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::tags;

/// The whole plan. Field names are the contract's camelCase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportPlan {
    pub pattern: Option<String>,
    /// The admin's mark-an-example state. The server keeps it and never reads it.
    pub example: Option<Example>,
    pub set_level: SetLevel,
    /// Set choices by folder key.
    pub sets: BTreeMap<String, SetChoice>,
    /// The item type of the items in a set, by set key.
    pub type_for: BTreeMap<String, TypeChoice>,
    pub csv: Option<CsvPlan>,
    pub meta: Vec<MetaRule>,
    pub priority: Priority,
    /// Grid edits: identifier, then term, then the value typed.
    pub edits: BTreeMap<String, BTreeMap<String, String>>,
    pub skip: BTreeSet<String>,
    pub without_files: BTreeSet<String>,
    pub csv_row: BTreeMap<String, CsvRowChoice>,
    pub problem_rows: ProblemRows,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Example {
    pub path: String,
    pub labels: BTreeMap<String, Label>,
    pub optional: Vec<Label>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Label {
    Identifier,
    Part,
    Page,
    Set,
}

/// Which folders of a file's path become its item sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SetLevel {
    /// The top folder.
    Level1,
    /// The second folder, or the top one when there is none.
    Level2,
    /// Both the top and the second folder.
    Both,
    /// The whole folder path as one set.
    Path,
    /// The pattern's `set` group, or the top folder when it did not match.
    Group,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetChoice {
    #[serde(rename = "use")]
    pub use_set: bool,
    pub name: String,
}

/// An existing template by id, or a new one made from a starter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TypeChoice {
    Template(i64),
    New(NewType),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewType {
    pub starter: StarterId,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StarterId {
    Letter,
    Photograph,
    Postcard,
    Diary,
    Person,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CsvPlan {
    /// Relative to the import folder.
    pub file: String,
    pub delimiter: Delimiter,
    /// The 1-based line that holds the header.
    pub header_row: usize,
    pub windows1252: bool,
    pub trim: bool,
    pub join_column: String,
    /// Header to target.
    pub columns: BTreeMap<String, ColumnTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Delimiter {
    #[serde(rename = ",")]
    Comma,
    #[serde(rename = ";")]
    Semicolon,
    #[serde(rename = "\t")]
    Tab,
}

impl Delimiter {
    pub const ALL: [Self; 3] = [Self::Comma, Self::Semicolon, Self::Tab];

    #[must_use]
    pub fn as_char(self) -> char {
        match self {
            Self::Comma => ',',
            Self::Semicolon => ';',
            Self::Tab => '\t',
        }
    }
}

/// A CSV column goes to an existing term or to a new local field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ColumnTarget {
    Term(String),
    NewField(NewField),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewField {
    pub new_field: String,
}

impl ColumnTarget {
    /// The term the column writes. A new field is `local:` and its label in
    /// lower camel case: "Shelf mark" is `local:shelfMark`.
    #[must_use]
    pub fn term(&self) -> String {
        match self {
            Self::Term(term) => term.clone(),
            Self::NewField(field) => local_term(&field.new_field),
        }
    }
}

/// `local:` and the label in lower camel case, letters and digits of any script.
#[must_use]
pub fn local_term(label: &str) -> String {
    let mut out = String::from("local:");
    let words = label
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty());
    for (index, word) in words.enumerate() {
        let lower = word.to_lowercase();
        let mut chars = lower.chars();
        if index > 0
            && let Some(first) = chars.next()
        {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        } else {
            out.push_str(&lower);
        }
    }
    out
}

/// One tag mapped to a field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "MetaRuleWire", into = "MetaRuleWire")]
pub struct MetaRule {
    pub tag: String,
    pub term: String,
    pub on: MetaTarget,
    pub from: FileChoice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MetaTarget {
    Item,
    Media,
    Both,
}

impl MetaTarget {
    #[must_use]
    pub fn on_item(self) -> bool {
        matches!(self, Self::Item | Self::Both)
    }

    #[must_use]
    pub fn on_media(self) -> bool {
        matches!(self, Self::Media | Self::Both)
    }
}

/// Which files of an item give the item its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChoice {
    First,
    /// The file whose `part` group is this text, else the first file.
    Part(String),
    All,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MetaRuleWire {
    tag: String,
    term: String,
    on: MetaTarget,
    from: FromWire,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    part: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum FromWire {
    First,
    Part,
    All,
}

impl TryFrom<MetaRuleWire> for MetaRule {
    type Error = String;

    fn try_from(wire: MetaRuleWire) -> Result<Self, String> {
        let from = match (wire.from, wire.part) {
            (FromWire::First, None) => FileChoice::First,
            (FromWire::All, None) => FileChoice::All,
            (FromWire::Part, Some(part)) if !part.is_empty() => FileChoice::Part(part),
            (FromWire::Part, _) => return Err("from \"part\" needs a part".to_string()),
            (_, Some(_)) => return Err("part is only for from \"part\"".to_string()),
        };
        Ok(Self {
            tag: wire.tag,
            term: wire.term,
            on: wire.on,
            from,
        })
    }
}

impl From<MetaRule> for MetaRuleWire {
    fn from(rule: MetaRule) -> Self {
        let (from, part) = match rule.from {
            FileChoice::First => (FromWire::First, None),
            FileChoice::Part(part) => (FromWire::Part, Some(part)),
            FileChoice::All => (FromWire::All, None),
        };
        Self {
            tag: rule.tag,
            term: rule.term,
            on: rule.on,
            from,
            part,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Priority {
    Csv,
    Files,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CsvRowChoice {
    Keep,
    Ignore,
}

/// What a run does with rows that still have problems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProblemRows {
    /// Leave them out of the run.
    Leave,
    /// Refuse to run until they are fixed.
    Fix,
}

/// A plan the boundary refuses.
#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    #[error("the plan is not an import plan: {0}")]
    Shape(#[from] serde_json::Error),
    #[error("the CSV header row is 1-based, so 0 is not a line")]
    HeaderRowZero,
    #[error("{0:?} is not a tag the import reads")]
    UnknownTag(String),
    #[error("the new field {0:?} has no letters or digits")]
    EmptyFieldLabel(String),
}

impl ImportPlan {
    /// The plan of a new draft: nothing chosen yet.
    #[must_use]
    pub fn new_draft() -> Self {
        Self {
            pattern: None,
            example: None,
            set_level: SetLevel::Level1,
            sets: BTreeMap::new(),
            type_for: BTreeMap::new(),
            csv: None,
            meta: Vec::new(),
            priority: Priority::Csv,
            edits: BTreeMap::new(),
            skip: BTreeSet::new(),
            without_files: BTreeSet::new(),
            csv_row: BTreeMap::new(),
            problem_rows: ProblemRows::Leave,
        }
    }

    /// Parse a plan sent by the admin or read from the database.
    ///
    /// # Errors
    /// A [`PlanError`] for an unknown member, a wrong type, or a state the
    /// types allow but the import cannot use.
    pub fn from_json(value: &serde_json::Value) -> Result<Self, PlanError> {
        let plan = Self::deserialize(value)?;
        if let Some(csv) = &plan.csv {
            if csv.header_row == 0 {
                return Err(PlanError::HeaderRowZero);
            }
            for target in csv.columns.values() {
                if let ColumnTarget::NewField(field) = target
                    && !field.new_field.chars().any(char::is_alphanumeric)
                {
                    return Err(PlanError::EmptyFieldLabel(field.new_field.clone()));
                }
            }
        }
        if let Some(rule) = plan
            .meta
            .iter()
            .find(|rule| tags::kind_of(&rule.tag).is_none())
        {
            return Err(PlanError::UnknownTag(rule.tag.clone()));
        }
        Ok(plan)
    }

    /// The plan as stored JSON.
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

impl Default for ImportPlan {
    fn default() -> Self {
        Self::new_draft()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use serde_json::json;

    fn full() -> serde_json::Value {
        json!({
            "pattern": "^(?<identifier>[^/]+)\\.jpg$",
            "example": { "path": "A/x.jpg", "labels": { "n0": "identifier" }, "optional": ["part"] },
            "setLevel": "both",
            "sets": { "Artifacts": { "use": true, "name": "Objects" } },
            "typeFor": { "Artifacts": 3, "Photographs": { "starter": "photograph", "name": "Photo" } },
            "csv": {
                "file": "register.csv", "delimiter": ";", "headerRow": 2, "windows1252": true,
                "trim": true, "joinColumn": "Accession No",
                "columns": { "Title": "dcterms:title", "Shelf mark": { "newField": "Shelf mark" } }
            },
            "meta": [
                { "tag": "DateTimeOriginal", "term": "dcterms:created", "on": "item", "from": "first" },
                { "tag": "Artist", "term": "dcterms:creator", "on": "both", "from": "part", "part": "front" }
            ],
            "priority": "files",
            "edits": { "BHC_006_AF_001": { "dcterms:title": "Lamp" } },
            "skip": ["BHC_006_AF_002"],
            "withoutFiles": ["BHC_006_PH_999"],
            "csvRow": { "BHC_006_AF_003": "keep" },
            "problemRows": "fix"
        })
    }

    #[test]
    fn a_full_plan_parses_and_writes_back_the_same_json() {
        let plan = ImportPlan::from_json(&full()).expect("plan");
        assert_eq!(plan.type_for["Artifacts"], TypeChoice::Template(3));
        assert_eq!(plan.meta[1].from, FileChoice::Part("front".to_string()));
        let csv = plan.csv.as_ref().expect("csv");
        assert_eq!(csv.delimiter, Delimiter::Semicolon);
        assert_eq!(csv.columns["Shelf mark"].term(), "local:shelfMark");
        assert_eq!(plan.to_json(), full());
    }

    #[test]
    fn the_new_draft_round_trips() {
        let draft = ImportPlan::new_draft();
        assert_eq!(
            ImportPlan::from_json(&draft.to_json()).expect("plan"),
            draft
        );
    }

    #[test]
    fn unknown_members_and_bad_states_are_refused() {
        let mut extra = full();
        extra["colour"] = json!("red");
        assert!(matches!(
            ImportPlan::from_json(&extra),
            Err(PlanError::Shape(_))
        ));

        let mut nested = full();
        nested["sets"]["Artifacts"]["colour"] = json!(1);
        assert!(matches!(
            ImportPlan::from_json(&nested),
            Err(PlanError::Shape(_))
        ));

        let mut zero = full();
        zero["csv"]["headerRow"] = json!(0);
        assert!(matches!(
            ImportPlan::from_json(&zero),
            Err(PlanError::HeaderRowZero)
        ));

        let mut tag = full();
        tag["meta"][0]["tag"] = json!("ShutterSpeed");
        assert!(matches!(
            ImportPlan::from_json(&tag),
            Err(PlanError::UnknownTag(_))
        ));

        let mut part = full();
        part["meta"][1]["part"] = json!(null);
        assert!(matches!(
            ImportPlan::from_json(&part),
            Err(PlanError::Shape(_))
        ));

        let mut label = full();
        label["csv"]["columns"]["Shelf mark"] = json!({ "newField": " - " });
        assert!(matches!(
            ImportPlan::from_json(&label),
            Err(PlanError::EmptyFieldLabel(_))
        ));
    }

    #[test]
    fn a_local_term_keeps_letters_of_any_script() {
        assert_eq!(local_term("Shelf mark"), "local:shelfMark");
        assert_eq!(local_term("  OLD accession-no. "), "local:oldAccessionNo");
        assert_eq!(local_term("दिनांक"), "local:दिनांक");
    }
}
