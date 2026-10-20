//! The dry run: which rows a run creates, and what else it would make.
//!
//! The run and the Check tab read the same [`Model::to_create`], so what the
//! tab promises is what the run writes. A row whose identifier an item
//! already holds is counted as `existing` and nowhere else: it is not
//! created, not skipped, and its problems are not counted.

use std::collections::BTreeMap;

use serde::Serialize;

use super::catalog::{Catalog, OpenUndo};
use super::derive::{Model, Row, TypeSource};
use super::plan::{ImportPlan, ProblemRows};
use super::problems::KindKey;

const UNMATCHED_SAMPLE: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub create: usize,
    pub files: usize,
    pub new_sets: Vec<String>,
    pub existing_sets: Vec<String>,
    pub new_types: Vec<String>,
    pub new_fields: Vec<String>,
    pub with_problems: usize,
    pub skipped: usize,
    pub existing: usize,
    pub unmatched_files: usize,
    pub unmatched_sample: Vec<String>,
    pub can_run: bool,
    pub open_undo: Option<OpenUndo>,
}

impl Model {
    /// Each kind of problem with the number of rows that have it, skipped
    /// and existing rows left out.
    #[must_use]
    pub fn kinds(&self) -> Vec<(KindKey, usize)> {
        let mut counts: BTreeMap<KindKey, usize> = BTreeMap::new();
        for row in self.rows.iter().filter(|row| row.open()) {
            for kind in row.kinds() {
                *counts.entry(kind).or_default() += 1;
            }
        }
        counts.into_iter().collect()
    }

    /// Rows with problems that the run may create.
    #[must_use]
    pub fn with_problems(&self) -> usize {
        self.rows
            .iter()
            .filter(|row| row.open() && row.problems() > 0)
            .count()
    }

    /// The rows a run creates: every row not skipped and not existing, less
    /// those with problems when the plan leaves them out.
    #[must_use]
    pub fn to_create(&self, plan: &ImportPlan) -> Vec<&Row> {
        self.rows
            .iter()
            .filter(|row| row.open())
            .filter(|row| plan.problem_rows == ProblemRows::Fix || row.problems() == 0)
            .collect()
    }

    #[must_use]
    pub fn check(&self, plan: &ImportPlan, catalog: &Catalog) -> Check {
        let create = self.to_create(plan);
        let with_problems = self.with_problems();
        let mut new_sets = Vec::new();
        let mut existing_sets = Vec::new();
        for set in self.sets.iter().filter(|set| {
            create
                .iter()
                .any(|row| row.item.set_keys.contains(&set.key))
        }) {
            let list = if set.existing {
                &mut existing_sets
            } else {
                &mut new_sets
            };
            if !list.contains(&set.name) {
                list.push(set.name.clone());
            }
        }
        let mut new_types: Vec<String> = Vec::new();
        for row in &create {
            if let TypeSource::New(new) = &row.item_type.source
                && !new_types.contains(&new.name)
            {
                new_types.push(new.name.clone());
            }
        }
        let unmatched: Vec<&str> = self
            .files
            .iter()
            .filter(|file| file.groups.is_none())
            .map(|file| file.file.path.as_str())
            .collect();
        let fixed = plan.problem_rows == ProblemRows::Leave || with_problems == 0;
        Check {
            create: create.len(),
            files: create.iter().map(|row| row.item.files.len()).sum(),
            new_sets,
            existing_sets,
            new_types,
            new_fields: self
                .new_fields
                .iter()
                .map(|(_, label)| label.clone())
                .collect(),
            with_problems,
            skipped: self
                .rows
                .iter()
                .filter(|row| row.skipped && row.existing.is_none())
                .count(),
            existing: self
                .rows
                .iter()
                .filter(|row| row.existing.is_some())
                .count(),
            unmatched_files: unmatched.len(),
            unmatched_sample: unmatched
                .iter()
                .take(UNMATCHED_SAMPLE)
                .map(|path| (*path).to_string())
                .collect(),
            can_run: self.issues.is_empty() && !create.is_empty() && fixed,
            open_undo: catalog.open_undo.clone(),
        }
    }
}
