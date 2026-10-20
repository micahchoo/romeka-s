//! The `sheets` and `tags` sections: how each plan sheet read, and the
//! tags the first files offer.

use serde::Serialize;

use super::{Derived, TAG_SAMPLES};
use crate::import::derive::FileTags;
use crate::import::evidence::FileEntry;
use crate::import::plan::{Delimiter, SheetId, SourceId, TagId};
use crate::import::sheet::{Column, EncodingRead, HeaderNote};
use crate::import::tags::TAGS;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetView {
    pub id: SheetId,
    pub source: SourceId,
    pub file: String,
    /// `false` when the file is not in its source.
    pub found: bool,
    pub error: Option<String>,
    pub read: Option<SheetRead>,
    pub guess: Option<SheetGuess>,
    /// The first lines as written.
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetRead {
    pub hash: String,
    pub encoding: EncodingRead,
    pub delimiter: Delimiter,
    pub delimiter_guessed: bool,
    pub header_line: u32,
    pub rows: usize,
    pub cells_count: usize,
    pub blank_lines: usize,
    pub cells_with_spaces: usize,
    pub cells_suspect: usize,
    pub columns: Vec<ColumnView>,
    pub notes: Vec<String>,
    /// Set for a workbook: the sheet read and every sheet it holds.
    pub workbook: Option<WorkbookView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookView {
    /// The worksheet the rows came from.
    pub sheet: String,
    /// Every worksheet, in workbook order: the choices of a sheet picker.
    pub sheets: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnView {
    pub index: u32,
    pub header: String,
    pub mappable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetGuess {
    pub encoding: Option<EncodingRead>,
    /// A detected encoding the reader cannot decode.
    pub unsupported: Option<String>,
    pub delimiter: Delimiter,
    pub header_line: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagView {
    pub tag: TagId,
    /// `exif`, `iptc` and `xmp` read inside the file; `file` is a fact about it.
    pub kind: &'static str,
    pub samples: Vec<TagSample>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagSample {
    pub source: SourceId,
    pub path: String,
    pub value: String,
}

pub(super) fn sheet_views(derived: &Derived) -> Vec<SheetView> {
    derived
        .plan
        .sheets
        .iter()
        .map(|plan| {
            let read = derived.sheets.get(&plan.id);
            let (error, sheet_read) = match read.map(|read| &read.sheet) {
                None => (None, None),
                Some(Err(error)) => (Some(error.to_string()), None),
                Some(Ok(sheet)) => (
                    None,
                    Some(SheetRead {
                        hash: sheet.hash.clone(),
                        encoding: sheet.encoding,
                        delimiter: sheet.delimiter,
                        delimiter_guessed: sheet.delimiter_guessed,
                        header_line: plan.read.header_line.get(),
                        rows: sheet.counts.rows,
                        cells_count: sheet.counts.cells_count,
                        blank_lines: sheet.counts.blank_lines,
                        cells_with_spaces: sheet.counts.cells_with_spaces,
                        cells_suspect: sheet.counts.cells_suspect,
                        columns: sheet.columns.iter().map(column_view).collect(),
                        notes: sheet.notes.iter().map(note_text).collect(),
                        workbook: sheet.workbook.as_ref().map(|book| WorkbookView {
                            sheet: book.sheet.clone(),
                            sheets: book.sheets.clone(),
                        }),
                    }),
                ),
            };
            SheetView {
                id: plan.id.clone(),
                source: plan.source.clone(),
                file: plan.file.to_string(),
                found: read.is_some(),
                error,
                read: sheet_read,
                guess: read.map(|read| SheetGuess {
                    encoding: read.guess.encoding.ok(),
                    unsupported: read.guess.encoding.err().map(str::to_string),
                    delimiter: read.guess.delimiter,
                    header_line: read.guess.header_line.get(),
                }),
                lines: read.map(|read| read.lines.clone()).unwrap_or_default(),
            }
        })
        .collect()
}

fn column_view(column: &Column) -> ColumnView {
    ColumnView {
        index: column.index,
        header: column.header.clone(),
        mappable: column.mappable,
    }
}

fn note_text(note: &HeaderNote) -> String {
    match note {
        HeaderNote::Duplicate { header, indexes } => format!(
            "duplicate header {header:?} at columns {}",
            indexes
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        HeaderNote::Empty { index } => format!("empty header at column {index}"),
    }
}

/// Each tag with its values on the first files that have a media type.
pub(super) fn tag_views(derived: &Derived) -> Vec<TagView> {
    let samples: Vec<(&SourceId, &FileEntry)> = derived
        .stored
        .index
        .sources
        .iter()
        .flat_map(|source| {
            source
                .walk
                .entries
                .iter()
                .map(move |entry| (&source.id, entry))
        })
        .filter(|(_, entry)| entry.sniffed.starts_with("image/"))
        .take(TAG_SAMPLES)
        .collect();
    TAGS.iter()
        .map(|(tag, kind)| TagView {
            tag: *tag,
            kind: kind.wire(),
            samples: samples
                .iter()
                .filter_map(|(source, entry)| {
                    Some(TagSample {
                        source: (*source).clone(),
                        path: entry.path.to_string(),
                        value: derived.tags.tag(source, entry, *tag)?.values.join("; "),
                    })
                })
                .collect(),
        })
        .collect()
}
