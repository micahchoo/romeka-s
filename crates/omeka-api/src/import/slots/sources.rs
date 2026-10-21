//! What one source ref yields for one record, before the slot's checks.

use super::super::join::{RowAt, cell};
use super::super::plan::{EachFile, FolderLevel, RecordKind, SourceRef, TagFiles};
use super::super::tags::file_fact;
use super::super::values::{Value, ValueType, uri_and_label};
use super::{Ctx, Raw};
use crate::import::derive::records::Draft;
use crate::import::grouping::pattern::PART;

fn literal(text: impl Into<String>) -> Value {
    Value {
        value_type: ValueType::Literal,
        text: text.into(),
        lang: None,
        label: None,
        is_public: true,
    }
}

/// The values `source` gives `draft`, empty ones dropped.
pub(super) fn values(ctx: &Ctx<'_, '_>, draft: &Draft, source: &SourceRef) -> Vec<Raw> {
    let files = || draft.files.iter().map(|file| &ctx.grouping.files[*file]);
    let mut out: Vec<Raw> = match source {
        SourceRef::Group { name } => files()
            .filter_map(|file| file.group(name))
            .map(|text| Raw::from(literal(text)))
            .collect(),
        SourceRef::Folder { level } => files()
            .filter_map(|file| {
                let folder = file.entry.folder();
                match level {
                    FolderLevel::Path(_) => Some(folder),
                    FolderLevel::Level(level) => folder
                        .split('/')
                        .nth(usize::try_from(level.get() - 1).ok()?),
                }
            })
            .map(|text| Raw::from(literal(text)))
            .collect(),
        SourceRef::Column { sheet, column } => {
            let of_sheet = |row: &&RowAt| &ctx.sheets[row.sheet].plan.id == sheet;
            let own: Vec<&RowAt> = draft.rows.iter().filter(of_sheet).collect();
            // A record with its own row in the sheet is described by it; the
            // rows of its media speak only for an item the sheet has no row for.
            let rows = if own.is_empty() {
                draft.child_rows.iter().filter(of_sheet).collect()
            } else {
                own
            };
            rows.into_iter()
                .flat_map(|row| {
                    let plan = ctx.sheets[row.sheet].plan;
                    let text = cell(row.row(ctx.sheets), column.index);
                    column_values(
                        plan.column(column.index).map(|column| &column.options),
                        text,
                    )
                })
                .map(Raw::from)
                .collect()
        }
        SourceRef::Tag { tag, files: which } => {
            let chosen: Vec<_> = match (draft.kind, which) {
                (RecordKind::Item, TagFiles::Each(EachFile::First)) => files().take(1).collect(),
                (_, TagFiles::Part(part)) => files()
                    .filter(|file| file.group(PART) == Some(part.part.as_str()))
                    .collect(),
                _ => files().collect(),
            };
            chosen
                .into_iter()
                .filter_map(|file| ctx.tags.tag(file.source, file.entry, *tag))
                .flat_map(|found| {
                    let suspect = found.windows_1252;
                    found.values.into_iter().map(move |text| Raw {
                        value: literal(text),
                        suspect,
                    })
                })
                .collect()
        }
        SourceRef::File { fact } => files()
            .map(|file| Raw::from(literal(file_fact(file.entry, *fact))))
            .collect(),
        SourceRef::Constant { value } => vec![Raw::from(literal(value.clone()))],
    };
    out.retain(|raw| !raw.value.text.trim().is_empty());
    out
}

/// A cell read with its column's options: split, type, language, private.
fn column_values(options: Option<&crate::import::plan::ColumnOptions>, text: &str) -> Vec<Value> {
    let Some(options) = options else {
        return vec![literal(text)];
    };
    let pieces: Vec<&str> = match options.split.as_deref() {
        Some(separator) if !separator.is_empty() => text.split(separator).map(str::trim).collect(),
        _ => vec![text],
    };
    pieces
        .into_iter()
        .map(|piece| {
            let (text, label) = match options.value_type {
                ValueType::Uri => {
                    let (uri, label) = uri_and_label(piece);
                    (uri.to_string(), label.map(str::to_string))
                }
                _ => (piece.to_string(), None),
            };
            Value {
                value_type: options.value_type.clone(),
                text,
                lang: options.lang.clone().filter(|lang| !lang.trim().is_empty()),
                label,
                is_public: !options.private,
            }
        })
        .collect()
}
