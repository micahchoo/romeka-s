//! Where a field's value comes from: a grid edit, the spreadsheet, the files'
//! own tags, or the identifier itself.
//!
//! An edit wins. Otherwise the plan's priority picks between the spreadsheet
//! and the files, and the other fills in when the first is empty. Both are
//! kept beside the chosen value, so the grid can show a conflict.

use serde::Serialize;

use super::csv::sheet::Sheet;
use super::grouping::MatchedFile;
use super::pattern;
use super::plan::{CsvRowChoice, FileChoice, ImportPlan, MetaRule, Priority};
use super::problems::is_date_term;
use super::tags::{self, TagCache};

pub const IDENTIFIER_TERM: &str = "dcterms:identifier";

/// How the grid shows several file values in one cell.
const FILE_JOIN: &str = " ; ";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Edit,
    Csv,
    File,
    Name,
    #[serde(rename = "")]
    None,
}

/// The value of one field of one item, with both candidate values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// The value the grid shows; the files' values joined by ` ; ` when they
    /// gave it.
    pub value: String,
    pub source: Source,
    pub csv: String,
    /// The files' values joined by ` ; `, for the grid's conflict.
    pub files: String,
    /// The files' distinct values, in file order.
    pub file_values: Vec<String>,
}

impl Resolved {
    /// The values a run writes: one per distinct file value when the files
    /// gave the value, otherwise the value itself.
    #[must_use]
    pub fn written(&self) -> Vec<&str> {
        match self.source {
            Source::File => self.file_values.iter().map(String::as_str).collect(),
            _ => vec![self.value.as_str()],
        }
    }
}

/// One item as the value sources see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub identifier: String,
    /// Indices into the model's files.
    pub files: Vec<usize>,
    pub set_keys: Vec<String>,
    /// The index of its row in the sheet, when it joined one.
    pub csv_row: Option<usize>,
}

/// Everything a value can be read from.
pub struct Sources<'a> {
    pub plan: &'a ImportPlan,
    pub sheet: Option<&'a Sheet>,
    pub files: &'a [MatchedFile],
    pub tags: &'a TagCache<'a>,
}

impl Sources<'_> {
    /// The value of `term` for `item`.
    #[must_use]
    pub fn resolve(&self, item: &Item, term: &str) -> Resolved {
        if let Some(edit) = self
            .plan
            .edits
            .get(&item.identifier)
            .and_then(|edits| edits.get(term))
        {
            let file_values = self.file_values(item, term);
            return Resolved {
                value: edit.clone(),
                source: Source::Edit,
                csv: self.csv_value(item, term),
                files: file_values.join(FILE_JOIN),
                file_values,
            };
        }
        if term == IDENTIFIER_TERM {
            return Resolved {
                value: item.identifier.clone(),
                source: Source::Name,
                csv: String::new(),
                files: String::new(),
                file_values: Vec::new(),
            };
        }
        let csv = self.csv_value(item, term);
        let file_values = self.file_values(item, term);
        let files = file_values.join(FILE_JOIN);
        let order = match self.plan.priority {
            Priority::Csv => [(Source::Csv, &csv), (Source::File, &files)],
            Priority::Files => [(Source::File, &files), (Source::Csv, &csv)],
        };
        let (source, value) = order
            .into_iter()
            .find(|(_, value)| !value.is_empty())
            .map_or((Source::None, String::new()), |(source, value)| {
                (source, value.clone())
            });
        Resolved {
            value,
            source,
            csv,
            files,
            file_values,
        }
    }

    /// The first non-empty cell of a column mapped to `term`.
    fn csv_value(&self, item: &Item, term: &str) -> String {
        let (Some(sheet), Some(plan), Some(row)) = (self.sheet, &self.plan.csv, item.csv_row)
        else {
            return String::new();
        };
        if self.plan.csv_row.get(&item.identifier) == Some(&CsvRowChoice::Ignore) {
            return String::new();
        }
        let row = &sheet.rows[row];
        sheet
            .head
            .iter()
            .enumerate()
            .filter(|(index, header)| {
                *index != sheet.join
                    && plan
                        .columns
                        .get(*header)
                        .is_some_and(|target| target.term() == term)
            })
            .map(|(index, _)| row.cell(index))
            .find(|cell| !cell.is_empty())
            .unwrap_or("")
            .to_string()
    }

    /// The item-level values the tag rules give `term`, distinct, in file order.
    fn file_values(&self, item: &Item, term: &str) -> Vec<String> {
        let mut values: Vec<String> = Vec::new();
        for rule in self
            .plan
            .meta
            .iter()
            .filter(|rule| rule.term == term && rule.on.on_item())
        {
            for file in self.chosen_files(item, &rule.from) {
                let value = self.tag_value(file, rule);
                if !value.is_empty() && !values.contains(&value) {
                    values.push(value);
                }
            }
        }
        values
    }

    fn chosen_files(&self, item: &Item, from: &FileChoice) -> Vec<&MatchedFile> {
        let files: Vec<&MatchedFile> = item.files.iter().map(|&index| &self.files[index]).collect();
        match from {
            FileChoice::All => files,
            FileChoice::First => files.into_iter().take(1).collect(),
            FileChoice::Part(part) => files
                .iter()
                .find(|file| file.group(pattern::PART) == Some(part.as_str()))
                .or(files.first())
                .map(|file| vec![*file])
                .unwrap_or_default(),
        }
    }

    /// A tag's value for a field: an EXIF date in a date field becomes an ISO date.
    fn tag_value(&self, file: &MatchedFile, rule: &MetaRule) -> String {
        let raw = self.tags.value(&file.file, &rule.tag);
        if rule.tag == tags::DATE_TIME_ORIGINAL && is_date_term(&rule.term) && !raw.is_empty() {
            return match tags::exif_date_to_iso(&raw) {
                Some(iso) => iso,
                None if raw.starts_with("0000") => String::new(),
                None => raw,
            };
        }
        raw
    }

    /// The values the tag rules put on one media file: term and value.
    #[must_use]
    pub fn media_values(&self, file: &MatchedFile) -> Vec<(String, String)> {
        self.plan
            .meta
            .iter()
            .filter(|rule| rule.on.on_media())
            .map(|rule| (rule.term.clone(), self.tag_value(file, rule)))
            .filter(|(_, value)| !value.is_empty())
            .collect()
    }
}
