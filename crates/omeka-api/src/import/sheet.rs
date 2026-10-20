//! The sheet reader: a CSV or TSV file, or a workbook sheet, read as rows
//! with the plan's read options (`docs/import-v2.md`, "Sheets").
//!
//! [`read`] decodes, finds the delimiter when the plan says `auto`, takes
//! the header from any line, and gives every row its key. It never fails on
//! a row: a short, long or broken row is kept with its raw line and flags
//! `cells-count`. It fails only on the sheet: an encoding it cannot decode
//! (`encoding-unsupported`) or a header line past the end. [`guess`] is what
//! the Sources tab offers before the plan chooses. A workbook (.xlsx, .xls,
//! .ods, told by its bytes) gives its rows through [`workbook`] and then
//! takes the same header, key and count rules as a CSV.

mod decode;
mod records;
#[cfg(test)]
mod tests;
pub mod workbook;
#[cfg(test)]
mod workbook_tests;

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::Path;

use encoding_rs::WINDOWS_1252;
use unicode_normalization::UnicodeNormalization;

pub use decode::EncodingRead;
use records::{Syntax, line_at, line_start, record_at, split_plain};
pub use workbook::{Workbook, WorkbookKind};

use super::decisions::RowKey;
use super::plan::{ColumnRef, Delimiter, Encoding, EncodingChoice, Quote, ReadOptions, SheetPlan};

/// No reader loads a larger file whole (`docs/import-v2.md`, "Bytes").
pub const MAX_SHEET_BYTES: u64 = 64 * 1024 * 1024;

/// The delimiters `auto` tries, the more likely first; a tie keeps the
/// earlier one.
const DELIMITERS: [Delimiter; 6] = [
    Delimiter::Comma,
    Delimiter::Semicolon,
    Delimiter::Tab,
    Delimiter::Pipe,
    Delimiter::Colon,
    Delimiter::Space,
];

/// How many records the delimiter guess reads.
const GUESS_RECORDS: usize = 200;

#[derive(Debug, Clone, PartialEq)]
pub struct Sheet {
    /// SHA-256 of the bytes, lower-case hex: the plan's `hash`.
    pub hash: String,
    /// `None` for a CSV or TSV.
    pub workbook: Option<Workbook>,
    /// UTF-8, not guessed, for a workbook.
    pub encoding: EncodingRead,
    /// Never [`Delimiter::Auto`]; for a workbook, the plan's or a comma.
    pub delimiter: Delimiter,
    pub delimiter_guessed: bool,
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    pub notes: Vec<HeaderNote>,
    pub counts: Counts,
}

/// A column, keyed by position; the header is shown and checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    /// 0-based.
    pub index: u32,
    /// Trimmed and NFC.
    pub header: String,
    /// `false` for a header starting with `#`: such a column is never mapped.
    pub mappable: bool,
}

/// A header the sheet reports. The column stays mappable by index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderNote {
    Duplicate { header: String, indexes: Vec<u32> },
    Empty { index: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The 1-based line the row starts on.
    pub line: u32,
    pub key: RowKey,
    /// One cell per column; a short row's missing cells are empty.
    pub cells: Vec<String>,
    /// Cells past the last column: kept, never mapped.
    pub extra: Vec<String>,
    /// Set when the row flags `cells-count`.
    pub cells_count: Option<CellsCount>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellsCount {
    pub problem: CellsProblem,
    /// The row as written, line ends included.
    pub raw: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellsProblem {
    Short {
        found: usize,
    },
    Long {
        found: usize,
    },
    /// A quote that does not close, or text after a closing quote. The row
    /// is its first line cut at each delimiter.
    Quote,
}

/// What the Sources tab shows, after the repairs the plan turns on, over
/// the lines below the header.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub rows: usize,
    pub cells_count: usize,
    /// Lines with no text, or only delimiters: not rows.
    pub blank_lines: usize,
    /// Cells with space at either end (`space-at-ends`).
    pub cells_with_spaces: usize,
    /// Cells with U+FFFD or UTF-8 read as 1252 (`encoding-suspect`).
    pub cells_suspect: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SheetError {
    #[error("the sheet is in {0}, which the reader cannot decode")]
    EncodingUnsupported(&'static str),
    #[error("the header line {header_line} is past the sheet's {lines} lines")]
    NoHeaderLine { header_line: u32, lines: u32 },
    #[error("the sheet is {0} bytes; the reader takes at most {MAX_SHEET_BYTES}")]
    TooLarge(u64),
    #[error("the sheet cannot be read: {0}")]
    Io(String),
    #[error("the workbook cannot be read: {0}")]
    Workbook(String),
    #[error("the workbook has no worksheet named \"{0}\"")]
    NoSheet(String),
    #[error(
        "the workbook unpacks to more than {} bytes",
        workbook::MAX_UNPACKED_BYTES
    )]
    Unpacked(u64),
}

/// The plan's key column: its join column, else its item key column.
#[must_use]
pub fn key_column(plan: &SheetPlan) -> Option<&ColumnRef> {
    plan.join
        .as_ref()
        .map(|join| &join.column)
        .or(plan.item_key.as_ref())
}

/// Read the file at `path` as `plan` says.
///
/// # Errors
/// [`SheetError::TooLarge`] past [`MAX_SHEET_BYTES`], [`SheetError::Io`],
/// or what [`read`] refuses.
pub fn read_file(path: &Path, plan: &SheetPlan) -> Result<Sheet, SheetError> {
    let io = |error: std::io::Error| SheetError::Io(error.to_string());
    let size = std::fs::metadata(path).map_err(io)?.len();
    if size > MAX_SHEET_BYTES {
        return Err(SheetError::TooLarge(size));
    }
    read(
        &std::fs::read(path).map_err(io)?,
        &plan.read,
        key_column(plan),
    )
}

/// Read `bytes` with `options`, keying rows by the `key` column's value, or
/// by line when there is none.
///
/// # Errors
/// [`SheetError::EncodingUnsupported`] and [`SheetError::NoHeaderLine`].
pub fn read(
    bytes: &[u8],
    options: &ReadOptions,
    key: Option<&ColumnRef>,
) -> Result<Sheet, SheetError> {
    if let Some(kind) = WorkbookKind::sniff(bytes) {
        let (book, records) = workbook::rows(
            bytes,
            kind,
            options.sheet_name.as_deref(),
            options.header_line.get(),
        )?;
        let mut records = records.into_iter();
        let decoded = Decoded {
            workbook: Some(book),
            encoding: EncodingRead {
                encoding: Encoding::Utf8,
                confidence: None,
            },
            delimiter: match options.delimiter {
                Delimiter::Auto => Delimiter::Comma,
                chosen => chosen,
            },
            delimiter_guessed: false,
        };
        return Ok(rows_of(bytes, options, key, decoded, |_| records.next()));
    }
    if workbook::is_packed(bytes) {
        return Err(SheetError::Workbook(
            "it is packed like a workbook but holds no .xlsx, .xls or .ods sheet".to_string(),
        ));
    }
    let (text, encoding) =
        decode::decode(bytes, options.encoding).map_err(SheetError::EncodingUnsupported)?;
    let header_line = options.header_line.get();
    let start = line_start(&text, header_line).ok_or(SheetError::NoHeaderLine {
        header_line,
        lines: line_count(&text),
    })?;
    let quote = quote_char(options.quote);
    let (delimiter, delimiter_guessed) = match options.delimiter {
        Delimiter::Auto => (best_delimiter(&text[start..], quote).0, true),
        chosen => (chosen, false),
    };
    let syntax = Syntax {
        delimiter: delimiter.as_char().unwrap_or(','),
        quote,
    };
    let mut parsed = Records::new(&text, start, header_line, syntax);
    let decoded = Decoded {
        workbook: None,
        encoding,
        delimiter,
        delimiter_guessed,
    };
    Ok(rows_of(bytes, options, key, decoded, |width| {
        parsed.next_record(width)
    }))
}

/// How the bytes were read, before any row.
struct Decoded {
    workbook: Option<Workbook>,
    encoding: EncodingRead,
    delimiter: Delimiter,
    delimiter_guessed: bool,
}

/// The sheet whose header is the first record `next` gives and whose rows
/// are the rest. `next` takes the header's width.
fn rows_of(
    bytes: &[u8],
    options: &ReadOptions,
    key: Option<&ColumnRef>,
    decoded: Decoded,
    mut next: impl FnMut(usize) -> Option<Parsed>,
) -> Sheet {
    let head = next(0).map(|record| record.fields).unwrap_or_default();
    let columns: Vec<Column> = (0_u32..)
        .zip(head)
        .map(|(index, header)| {
            let header: String = header.trim().nfc().collect();
            Column {
                index,
                mappable: !header.starts_with('#'),
                header,
            }
        })
        .collect();
    let notes = header_notes(&columns);

    let mut rows = Vec::new();
    let mut counts = Counts::default();
    let width = columns.len();
    while let Some(record) = next(width) {
        let tidy = |cell: String| {
            if options.trim {
                cell.trim().to_string()
            } else {
                cell
            }
        };
        let mut cells: Vec<String> = record.fields.into_iter().map(tidy).collect();
        if cells.iter().all(|cell| cell.trim().is_empty()) {
            counts.blank_lines += 1;
            continue;
        }
        let found = cells.len();
        let mut extra = cells.split_off(width.min(found));
        if extra.iter().all(|cell| cell.trim().is_empty()) {
            extra.clear();
        }
        cells.resize(width, String::new());
        let problem = if record.broken {
            Some(CellsProblem::Quote)
        } else if found < width {
            Some(CellsProblem::Short { found })
        } else if !extra.is_empty() {
            Some(CellsProblem::Long { found })
        } else {
            None
        };
        for cell in cells.iter().chain(&extra) {
            counts.cells_with_spaces += usize::from(cell.trim() != cell);
            counts.cells_suspect += usize::from(looks_misdecoded(cell));
        }
        counts.cells_count += usize::from(problem.is_some());
        rows.push(Row {
            line: record.line,
            key: RowKey::new(&record.line.to_string()),
            cells,
            extra,
            cells_count: problem.map(|problem| CellsCount {
                problem,
                raw: record.raw,
            }),
        });
    }
    counts.rows = rows.len();
    if let Some(column) = key {
        key_rows(&mut rows, column);
    }
    Sheet {
        hash: omeka_media::store::sha256_hex(bytes),
        workbook: decoded.workbook,
        encoding: decoded.encoding,
        delimiter: decoded.delimiter,
        delimiter_guessed: decoded.delimiter_guessed,
        columns,
        rows,
        notes,
        counts,
    }
}

/// What the Sources tab offers before the plan chooses.
#[derive(Debug, Clone, PartialEq)]
pub struct Guess {
    /// The guessed encoding, or the name of one the reader cannot decode.
    pub encoding: Result<EncodingRead, &'static str>,
    pub delimiter: Delimiter,
    /// The first line as wide as most lines: lines above it are a title.
    pub header_line: NonZeroU32,
}

#[must_use]
pub fn guess(bytes: &[u8]) -> Guess {
    if workbook::is_packed(bytes) {
        return Guess {
            encoding: Ok(EncodingRead {
                encoding: Encoding::Utf8,
                confidence: None,
            }),
            delimiter: Delimiter::Comma,
            header_line: NonZeroU32::MIN,
        };
    }
    let decoded = decode::decode(bytes, EncodingChoice::Auto(super::plan::Auto::Auto));
    let text = match &decoded {
        Ok((text, _)) => text.clone(),
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    };
    let quote = Some('"');
    let (delimiter, width) = best_delimiter(&text, quote);
    let syntax = Syntax {
        delimiter: delimiter.as_char().unwrap_or(','),
        quote,
    };
    let mut parsed = Records::new(&text, 0, 1, syntax);
    let mut header_line = 1;
    while let Some(record) = parsed.next_record(0) {
        if width.is_none_or(|width| record.fields.len() == width) {
            header_line = record.line;
            break;
        }
    }
    Guess {
        encoding: decoded.map(|(_, read)| read),
        delimiter,
        header_line: NonZeroU32::new(header_line).unwrap_or(NonZeroU32::MIN),
    }
}

/// Whether the start of a `.txt` file reads as a delimited sheet: most of
/// its lines one width of two or more by a comma, semicolon, tab or pipe.
#[must_use]
pub fn looks_delimited(head: &[u8]) -> bool {
    let text = String::from_utf8_lossy(head);
    let mut parsed = Records::new(
        &text,
        0,
        1,
        Syntax {
            delimiter: ',',
            quote: Some('"'),
        },
    );
    let mut lines = 0;
    while parsed.next_record(0).is_some() {
        lines += 1;
    }
    DELIMITERS[..4].iter().any(|delimiter| {
        let (score, width) = score(&text, Some('"'), *delimiter);
        width.is_some() && score >= 2 && score * 2 >= lines
    })
}

/// The delimiter that gives the most records of one width of two or more,
/// and that width; a comma and `None` when no delimiter splits anything.
/// Colon and space also split prose and times, so they are tried only when
/// no other delimiter splits two records alike.
fn best_delimiter(text: &str, quote: Option<char>) -> (Delimiter, Option<usize>) {
    let mut best = (Delimiter::Comma, None, 0);
    for (tried, delimiter) in DELIMITERS.into_iter().enumerate() {
        if tried == 4 && best.2 >= 2 {
            break;
        }
        let (score, width) = score(text, quote, delimiter);
        if width.is_some() && score > best.2 {
            best = (delimiter, width, score);
        }
    }
    (best.0, best.1)
}

/// How many of the first records have the most common width of two or
/// more (the wider on a tie), and that width.
fn score(text: &str, quote: Option<char>, delimiter: Delimiter) -> (usize, Option<usize>) {
    let syntax = Syntax {
        delimiter: delimiter.as_char().unwrap_or(','),
        quote,
    };
    let mut parsed = Records::new(text, 0, 1, syntax);
    let mut widths: HashMap<usize, usize> = HashMap::new();
    for _ in 0..GUESS_RECORDS {
        let Some(record) = parsed.next_record(0) else {
            break;
        };
        if record.fields.len() > 1 && !record.broken {
            *widths.entry(record.fields.len()).or_default() += 1;
        }
    }
    widths
        .into_iter()
        .max_by_key(|&(width, count)| (count, width))
        .map_or((0, None), |(width, count)| (count, Some(width)))
}

/// A record as the sheet keeps it.
struct Parsed {
    line: u32,
    fields: Vec<String>,
    raw: String,
    broken: bool,
}

/// Records from a start byte, a broken record read as its first line.
struct Records<'t> {
    text: &'t str,
    at: usize,
    line: u32,
    syntax: Syntax,
}

impl<'t> Records<'t> {
    fn new(text: &'t str, at: usize, line: u32, syntax: Syntax) -> Self {
        Self {
            text,
            at,
            line,
            syntax,
        }
    }

    /// The next record. With `width` above 0, a record that spans lines
    /// and does not have `width` cells is taken as broken too: a stray
    /// quote, not a cell with a line end in it.
    fn next_record(&mut self, width: usize) -> Option<Parsed> {
        if self.at >= self.text.len() {
            return None;
        }
        let record = record_at(self.text, self.at, self.syntax);
        let misfit = width > 0 && record.embedded > 0 && record.fields.len() != width;
        let line = self.line;
        if record.broken || misfit {
            let (first, next) = line_at(self.text, self.at);
            self.at = next;
            self.line += 1;
            return Some(Parsed {
                line,
                fields: split_plain(first, self.syntax.delimiter),
                raw: first.to_string(),
                broken: true,
            });
        }
        let raw = self.text[self.at..record.end].to_string();
        self.at = record.end;
        self.line += record.embedded + 1;
        Some(Parsed {
            line,
            fields: record.fields,
            raw,
            broken: false,
        })
    }
}

fn header_notes(columns: &[Column]) -> Vec<HeaderNote> {
    let mut notes = Vec::new();
    let mut by_header: Vec<(&str, Vec<u32>)> = Vec::new();
    for column in columns {
        if column.header.is_empty() {
            notes.push(HeaderNote::Empty {
                index: column.index,
            });
            continue;
        }
        match by_header
            .iter_mut()
            .find(|(header, _)| *header == column.header)
        {
            Some((_, indexes)) => indexes.push(column.index),
            None => by_header.push((&column.header, vec![column.index])),
        }
    }
    for (header, indexes) in by_header {
        if indexes.len() > 1 {
            notes.push(HeaderNote::Duplicate {
                header: header.to_string(),
                indexes,
            });
        }
    }
    notes
}

/// Key each row by its `column` value, trimmed and NFC, and `#n` for the
/// row's place among rows with that value. A caller that changes the key
/// column's cells after the read keys the rows again.
pub(crate) fn key_rows(rows: &mut [Row], column: &ColumnRef) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    for row in rows {
        let value: String = cell_at(&row.cells, &row.extra, column.index)
            .trim()
            .nfc()
            .collect();
        let n = seen.entry(value.clone()).or_default();
        *n += 1;
        row.key = RowKey::new(&format!("{value}#{n}"));
    }
}

fn cell_at<'c>(cells: &'c [String], extra: &'c [String], index: u32) -> &'c str {
    let index = usize::try_from(index).unwrap_or(usize::MAX);
    cells
        .iter()
        .chain(extra)
        .nth(index)
        .map_or("", String::as_str)
}

fn quote_char(quote: Quote) -> Option<char> {
    match quote {
        Quote::Double => Some('"'),
        Quote::Single => Some('\''),
        Quote::None => None,
    }
}

fn line_count(text: &str) -> u32 {
    let mut lines = 0;
    let mut at = 0;
    while at < text.len() {
        at = line_at(text, at).1;
        lines += 1;
    }
    lines
}

/// U+FFFD, or UTF-8 that passed once through a 1252 reader (`CafÃ©`).
fn looks_misdecoded(cell: &str) -> bool {
    if cell.contains('\u{FFFD}') {
        return true;
    }
    if cell.is_ascii() {
        return false;
    }
    let (bytes, _, unmappable) = WINDOWS_1252.encode(cell);
    !unmappable && std::str::from_utf8(&bytes).is_ok_and(|repaired| repaired != cell)
}
