//! The preview: the sections of the model a tab asks for, a page at a time.
//!
//! [`PreviewRequest`] is the parse boundary for `POST .../preview`. [`build`]
//! answers only the sections the request names; rows and files come back as
//! a filtered page, so the browser never holds the whole folder.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::catalog::Catalog;
use super::check::Check;
use super::csv::sheet::{self, Detected};
use super::derive::{Column, Model, Row};
use super::grouping::SetEntry;
use super::inputs::Inputs;
use super::plan::{Delimiter, ImportPlan};
use super::problems::{ProblemKind, RowProblem};
use super::tags::{TAGS, TagCache, TagKind};
use super::values::Source;

/// The most rows one page holds.
pub const ROWS_LIMIT: usize = 200;
/// The most files one page holds.
pub const FILES_LIMIT: usize = 1000;
const DEFAULT_LIMIT: usize = 50;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewRequest {
    pub include: BTreeSet<Section>,
    #[serde(default)]
    pub files: FilesQuery,
    #[serde(default)]
    pub rows: RowsQuery,
    /// A plan to derive from instead of the stored one: edits the admin has
    /// not saved yet, or a candidate pattern. It passes the same boundary as
    /// a saved plan.
    #[serde(default, deserialize_with = "plan_override")]
    pub plan: Option<ImportPlan>,
}

fn plan_override<'de, D>(deserializer: D) -> Result<Option<ImportPlan>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    value
        .map(|value| ImportPlan::from_json(&value).map_err(serde::de::Error::custom))
        .transpose()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Section {
    Files,
    Sets,
    Csv,
    Tags,
    Rows,
    Check,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileFilter {
    #[default]
    All,
    Match,
    Miss,
}

/// Files of items with this many files; 3 means three or more.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "u8")]
pub struct PerItem(usize);

impl TryFrom<u8> for PerItem {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, String> {
        match value {
            1..=3 => Ok(Self(usize::from(value))),
            _ => Err(format!("perItem is 1, 2 or 3, not {value}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct FilesQuery {
    pub filter: FileFilter,
    pub per_item: Option<PerItem>,
    pub offset: usize,
    pub limit: usize,
}

impl Default for FilesQuery {
    fn default() -> Self {
        Self {
            filter: FileFilter::All,
            per_item: None,
            offset: 0,
            limit: DEFAULT_LIMIT,
        }
    }
}

/// `kind` alone matches a kind in any field; with `term`, in that field only.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct RowsQuery {
    pub problems_only: bool,
    /// Only the rows whose identifier an item already holds.
    pub existing_only: bool,
    pub kind: Option<ProblemKind>,
    pub term: Option<String>,
    pub query: Option<String>,
    pub offset: usize,
    pub limit: usize,
}

impl Default for RowsQuery {
    fn default() -> Self {
        Self {
            problems_only: false,
            existing_only: false,
            kind: None,
            term: None,
            query: None,
            offset: 0,
            limit: DEFAULT_LIMIT,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Preview {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<PreviewError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<FilesSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sets: Option<Vec<SetEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub csv: Option<CsvSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<TagSection>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows: Option<RowsSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check: Option<Check>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreviewError {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilesSection {
    pub total: usize,
    pub matched: usize,
    pub items: usize,
    pub group_names: Vec<String>,
    pub buckets: BTreeMap<String, usize>,
    pub biggest: Option<Biggest>,
    pub filtered: usize,
    pub page: Vec<FileLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Biggest {
    pub identifier: String,
    pub files: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileLine {
    pub path: String,
    pub matched: bool,
    /// The item's 1-based number among the items the files make.
    pub item: Option<usize>,
    pub item_files: usize,
    pub groups: BTreeMap<String, [usize; 2]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CsvSection {
    pub files: Vec<String>,
    pub head: Vec<String>,
    pub raw_lines: Vec<String>,
    pub detected: Detected,
    #[serde(rename = "cellsLookLike1252")]
    pub cells_look_like_1252: usize,
    pub cells_with_spaces: usize,
    /// `delimiter` or `headerRow` when the chosen reading gives the wrong
    /// number of cells.
    pub row_count_error: Option<&'static str>,
    pub joined: usize,
    pub rows_without_file: usize,
    pub items_without_row: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagSection {
    pub tag: &'static str,
    pub kind: TagKind,
    pub found: usize,
    pub sample: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowsSection {
    pub columns: Vec<Column>,
    pub unmapped: Vec<String>,
    pub total: usize,
    pub with_problems: usize,
    /// Rows whose identifier an item already holds.
    pub existing: usize,
    pub filtered: usize,
    pub kinds: Vec<KindCount>,
    pub page: Vec<RowLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KindCount {
    pub kind: ProblemKind,
    pub term: Option<String>,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowLine {
    pub identifier: String,
    pub sets: Vec<String>,
    #[serde(rename = "type")]
    pub item_type: String,
    pub files: Vec<String>,
    pub cells: Vec<CellLine>,
    pub extra: Vec<String>,
    pub row_problems: Vec<RowProblem>,
    pub skipped: bool,
    pub problems: usize,
    pub existing: Option<ExistingItem>,
}

/// The item that already holds a row's identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExistingItem {
    pub item_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CellLine {
    pub term: String,
    pub value: String,
    pub source: Source,
    pub problems: Vec<ProblemKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict: Option<Conflict>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Conflict {
    pub csv: String,
    pub files: String,
}

/// Everything the preview reads, gathered by the caller.
pub struct PreviewContext<'a> {
    pub plan: &'a ImportPlan,
    pub inputs: &'a Inputs,
    pub model: &'a Model,
    pub tags: &'a TagCache<'a>,
    pub catalog: &'a Catalog,
}

/// The sections `request` names.
#[must_use]
pub fn build(context: &PreviewContext<'_>, request: &PreviewRequest) -> Preview {
    let model = context.model;
    let wants = |section| request.include.contains(&section);
    Preview {
        error: model.issues.first().map(|issue| PreviewError {
            key: issue.key().to_string(),
            detail: issue.detail(),
        }),
        files: wants(Section::Files).then(|| files_section(model, &request.files)),
        sets: wants(Section::Sets).then(|| model.sets.clone()),
        csv: wants(Section::Csv).then(|| csv_section(context)),
        tags: wants(Section::Tags).then(|| tags_section(model, context.tags)),
        rows: wants(Section::Rows).then(|| rows_section(model, &request.rows)),
        check: wants(Section::Check).then(|| model.check(context.plan, context.catalog)),
    }
}

fn files_section(model: &Model, query: &FilesQuery) -> FilesSection {
    let item_files =
        |item: Option<usize>| item.map_or(0, |index| model.file_items[index].files.len());
    let mut buckets = BTreeMap::new();
    for item in &model.file_items {
        *buckets
            .entry(item.files.len().min(3).to_string())
            .or_insert(0) += 1;
    }
    let biggest = model
        .file_items
        .iter()
        .fold(None::<&super::grouping::FileItem>, |top, item| match top {
            Some(top) if top.files.len() >= item.files.len() => Some(top),
            _ => Some(item),
        })
        .map(|item| Biggest {
            identifier: item.identifier.clone(),
            files: item.files.len(),
        });
    let selected: Vec<_> = model
        .files
        .iter()
        .filter(|file| match (query.per_item, query.filter) {
            (Some(PerItem(count)), _) => {
                file.item.is_some() && item_files(file.item).min(3) == count
            }
            (None, FileFilter::All) => true,
            (None, FileFilter::Match) => file.groups.is_some(),
            (None, FileFilter::Miss) => file.groups.is_none(),
        })
        .collect();
    let page = selected
        .iter()
        .skip(query.offset)
        .take(query.limit.min(FILES_LIMIT))
        .map(|file| FileLine {
            path: file.file.path.clone(),
            matched: file.groups.is_some(),
            item: file.item.map(|index| index + 1),
            item_files: item_files(file.item),
            groups: file
                .groups
                .iter()
                .flatten()
                .map(|(name, group)| (name.clone(), [group.utf16.0, group.utf16.1]))
                .collect(),
        })
        .collect();
    FilesSection {
        total: model.files.len(),
        matched: model
            .files
            .iter()
            .filter(|file| file.groups.is_some())
            .count(),
        items: model.file_items.len(),
        group_names: model.group_names.clone(),
        buckets,
        biggest,
        filtered: selected.len(),
        page,
    }
}

fn csv_section(context: &PreviewContext<'_>) -> CsvSection {
    let model = context.model;
    let inspection = match (&context.plan.csv, &context.inputs.csv) {
        (Some(plan), Some(Ok(bytes))) => Some(sheet::inspect(bytes, plan)),
        _ => None,
    };
    let row_count_error = model
        .sheet
        .as_ref()
        .and_then(|sheet| sheet.as_ref().err())
        .filter(|error| error.is_row_count())
        .map(|error| error.key());
    let (head, raw_lines, detected, look_1252, spaces) = match inspection {
        Some(seen) => (
            seen.head,
            seen.raw_lines,
            seen.detected,
            seen.cells_look_like_1252,
            seen.cells_with_spaces,
        ),
        None => (
            Vec::new(),
            Vec::new(),
            Detected {
                delimiter: Delimiter::Comma,
                header_row: 1,
            },
            0,
            0,
        ),
    };
    CsvSection {
        files: context.inputs.csv_files.clone(),
        head,
        raw_lines,
        detected,
        cells_look_like_1252: look_1252,
        cells_with_spaces: spaces,
        row_count_error,
        joined: model.join.joined,
        rows_without_file: model.join.rows_without_file,
        items_without_row: model.join.items_without_row,
    }
}

/// Each tag over the matched files: how many have it, and the first value.
fn tags_section(model: &Model, tags: &TagCache<'_>) -> Vec<TagSection> {
    let matched: Vec<_> = model
        .files
        .iter()
        .filter(|file| file.groups.is_some())
        .collect();
    TAGS.iter()
        .map(|(tag, kind)| {
            let values: Vec<String> = matched
                .iter()
                .map(|file| tags.value(&file.file, tag))
                .filter(|v| !v.is_empty())
                .collect();
            TagSection {
                tag,
                kind: *kind,
                found: values.len(),
                sample: values.into_iter().next().unwrap_or_default(),
            }
        })
        .collect()
}

fn rows_section(model: &Model, query: &RowsQuery) -> RowsSection {
    let needle = query
        .query
        .as_deref()
        .map(|q| q.trim().to_lowercase())
        .filter(|q| !q.is_empty());
    let selected: Vec<&Row> = model
        .rows
        .iter()
        .filter(|row| !query.problems_only || (row.existing.is_none() && row.problems() > 0))
        .filter(|row| !query.existing_only || row.existing.is_some())
        .filter(|row| {
            query.kind.is_none_or(|kind| {
                row.existing.is_none()
                    && row.kinds().iter().any(|key| {
                        key.kind == kind
                            && query
                                .term
                                .as_ref()
                                .is_none_or(|term| key.term.as_ref() == Some(term))
                    })
            })
        })
        .filter(|row| {
            needle
                .as_ref()
                .is_none_or(|needle| row.item.identifier.to_lowercase().contains(needle))
        })
        .collect();
    let page = selected
        .iter()
        .skip(query.offset)
        .take(query.limit.min(ROWS_LIMIT))
        .map(|row| row_line(model, row))
        .collect();
    RowsSection {
        columns: model.columns.clone(),
        unmapped: model.unmapped.clone(),
        total: model.rows.len(),
        with_problems: model.with_problems(),
        existing: model
            .rows
            .iter()
            .filter(|row| row.existing.is_some())
            .count(),
        filtered: selected.len(),
        kinds: model
            .kinds()
            .into_iter()
            .map(|(key, count)| KindCount {
                kind: key.kind,
                term: key.term,
                count,
            })
            .collect(),
        page,
    }
}

fn row_line(model: &Model, row: &Row) -> RowLine {
    let set_name = |key: &String| {
        model
            .sets
            .iter()
            .find(|set| set.key == *key)
            .map_or_else(|| key.clone(), |set| set.name.clone())
    };
    RowLine {
        identifier: row.item.identifier.clone(),
        sets: row.item.set_keys.iter().map(set_name).collect(),
        item_type: row.item_type.name().to_string(),
        files: row
            .item
            .files
            .iter()
            .map(|&index| model.files[index].file.path.clone())
            .collect(),
        cells: row
            .cells
            .iter()
            .map(|cell| CellLine {
                term: cell.term.clone(),
                value: cell.value.value.clone(),
                source: cell.value.source,
                problems: cell.problems.clone(),
                conflict: (!cell.value.csv.is_empty()
                    && !cell.value.files.is_empty()
                    && cell.value.csv != cell.value.files)
                    .then(|| Conflict {
                        csv: cell.value.csv.clone(),
                        files: cell.value.files.clone(),
                    }),
            })
            .collect(),
        extra: row.extra.clone(),
        row_problems: row.row_problems.clone(),
        skipped: row.skipped,
        problems: row.problems(),
        existing: row.existing.map(|item_id| ExistingItem { item_id }),
    }
}
