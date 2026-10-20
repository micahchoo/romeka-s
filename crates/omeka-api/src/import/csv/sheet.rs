//! The import's spreadsheet: bytes from the import folder, read the way the
//! plan says, with the counts the Sources tab shows.
//!
//! [`detect`] guesses the delimiter and header line; [`Sheet::read`] applies
//! the plan's choices and refuses a reading that cannot be right (one cell per
//! line, a header shorter than the lines under it). [`inspect`] counts what
//! the repairs would change, on the text as written.

use serde::Serialize;

use super::reader::{CsvError, Record, parse_with};
use super::windows1252;
use crate::import::plan::{CsvPlan, Delimiter};

/// What the server guesses before the user chooses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detected {
    pub delimiter: Delimiter,
    pub header_row: usize,
}

/// The spreadsheet as the plan reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    pub head: Vec<String>,
    pub rows: Vec<SheetRow>,
    /// The index of the join column in `head`.
    pub join: usize,
}

/// One data line, cells repaired and trimmed as the plan says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetRow {
    pub line: usize,
    pub cells: Vec<String>,
}

impl SheetRow {
    /// The cell at `index`, or `""` past the end of a short line.
    #[must_use]
    pub fn cell(&self, index: usize) -> &str {
        self.cells.get(index).map_or("", String::as_str)
    }
}

/// Why the plan's reading of the spreadsheet cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SheetError {
    #[error(transparent)]
    Parse(#[from] CsvError),
    #[error("with this delimiter every line has one cell")]
    OneCell,
    #[error("the header line has fewer cells than the lines under it")]
    HeaderRow,
    #[error("the header has no column {0:?}")]
    JoinColumn(String),
    #[error("the spreadsheet cannot be read: {0}")]
    Unreadable(String),
}

impl SheetError {
    /// The key the admin translates.
    #[must_use]
    pub fn key(&self) -> &'static str {
        match self {
            Self::Parse(_) => "csvParse",
            Self::OneCell => "delimiter",
            Self::HeaderRow => "headerRow",
            Self::JoinColumn(_) => "joinColumn",
            Self::Unreadable(_) => "csvUnreadable",
        }
    }

    /// Whether the error is about the delimiter or header line the user chose,
    /// which the Sources tab shows beside those choices.
    #[must_use]
    pub fn is_row_count(&self) -> bool {
        matches!(self, Self::OneCell | Self::HeaderRow)
    }
}

/// What the Sources tab shows about the file as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspection {
    pub head: Vec<String>,
    pub raw_lines: Vec<String>,
    pub detected: Detected,
    pub cells_look_like_1252: usize,
    pub cells_with_spaces: usize,
}

const RAW_LINES: usize = 8;
const RAW_LINE_CHARS: usize = 300;

/// The text of the file: UTF-8 when it is UTF-8 (a byte order mark dropped),
/// else Windows-1252 when the plan says so, else UTF-8 with U+FFFD for the
/// bytes that are not. The flag says whether the bytes were UTF-8.
fn decode(bytes: &[u8], windows1252: bool) -> (String, bool) {
    match std::str::from_utf8(bytes) {
        Ok(text) => (
            text.strip_prefix('\u{FEFF}').unwrap_or(text).to_string(),
            true,
        ),
        Err(_) if windows1252 => (windows1252::decode(bytes), false),
        Err(_) => (String::from_utf8_lossy(bytes).into_owned(), false),
    }
}

/// The most common cell count of at least two, the larger on a tie.
fn modal_width(records: &[Record]) -> Option<usize> {
    let mut counts = std::collections::BTreeMap::<usize, usize>::new();
    for record in records.iter().filter(|record| record.fields.len() > 1) {
        *counts.entry(record.fields.len()).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|&(width, count)| (count, width))
        .map(|(width, _)| width)
}

/// The delimiter that gives the most lines of one width, and the first line
/// of that width as the header. A comma and line 1 when nothing fits.
#[must_use]
pub fn detect(text: &str) -> Detected {
    let mut best: Option<(usize, Detected)> = None;
    for delimiter in Delimiter::ALL {
        let Ok(records) = parse_with(text, delimiter.as_char()) else {
            continue;
        };
        let Some(width) = modal_width(&records) else {
            continue;
        };
        let score = records.iter().filter(|r| r.fields.len() == width).count();
        let header_row = records
            .iter()
            .find(|r| r.fields.len() == width)
            .map_or(1, |r| r.line);
        if best.is_none_or(|(top, _)| score > top) {
            best = Some((
                score,
                Detected {
                    delimiter,
                    header_row,
                },
            ));
        }
    }
    best.map_or(
        Detected {
            delimiter: Delimiter::Comma,
            header_row: 1,
        },
        |(_, detected)| detected,
    )
}

impl Sheet {
    /// Read `bytes` the way `plan` says.
    ///
    /// # Errors
    /// A [`SheetError`] when the text is not CSV, the delimiter gives one cell
    /// per line, the header line is shorter than the lines under it, or the
    /// header has no join column.
    pub fn read(bytes: &[u8], plan: &CsvPlan) -> Result<Self, SheetError> {
        let (text, was_utf8) = decode(bytes, plan.windows1252);
        let records = parse_with(&text, plan.delimiter.as_char())?;
        if records.iter().all(|record| record.fields.len() <= 1) {
            return Err(SheetError::OneCell);
        }
        let at = records
            .iter()
            .position(|record| record.line == plan.header_row)
            .ok_or(SheetError::HeaderRow)?;
        let below = &records[at + 1..];
        if modal_width(below).is_some_and(|width| records[at].fields.len() < width) {
            return Err(SheetError::HeaderRow);
        }
        let head: Vec<String> = records[at]
            .fields
            .iter()
            .map(|h| h.trim().to_string())
            .collect();
        let join = head
            .iter()
            .position(|h| *h == plan.join_column)
            .ok_or_else(|| SheetError::JoinColumn(plan.join_column.clone()))?;
        let repair = plan.windows1252 && was_utf8;
        let rows = below
            .iter()
            .map(|record| SheetRow {
                line: record.line,
                cells: record
                    .fields
                    .iter()
                    .map(|cell| fix(cell, repair, plan.trim))
                    .collect(),
            })
            .collect();
        Ok(Self { head, rows, join })
    }
}

fn fix(cell: &str, repair: bool, trim: bool) -> String {
    let repaired = if repair {
        windows1252::repair(cell)
    } else {
        None
    };
    let text = repaired.as_deref().unwrap_or(cell);
    if trim { text.trim() } else { text }.to_string()
}

/// The raw lines, the guess, and how many cells each repair would change.
#[must_use]
pub fn inspect(bytes: &[u8], plan: &CsvPlan) -> Inspection {
    let (text, _) = decode(bytes, plan.windows1252);
    let raw_lines = text
        .lines()
        .take(RAW_LINES)
        .map(|line| line.chars().take(RAW_LINE_CHARS).collect())
        .collect();
    let detected = detect(&text);
    let delimiter = plan.delimiter.as_char();
    let head = parse_with(&text, delimiter)
        .ok()
        .and_then(|records| records.into_iter().find(|r| r.line == plan.header_row))
        .map(|record| record.fields.iter().map(|h| h.trim().to_string()).collect())
        .unwrap_or_default();

    let as_written = String::from_utf8_lossy(bytes);
    let cells: Vec<String> = parse_with(&as_written, delimiter)
        .map(|records| records.into_iter().flat_map(|r| r.fields).collect())
        .unwrap_or_default();
    Inspection {
        head,
        raw_lines,
        detected,
        cells_look_like_1252: cells
            .iter()
            .filter(|c| windows1252::looks_misdecoded(c))
            .count(),
        cells_with_spaces: cells.iter().filter(|c| c.trim() != c.as_str()).count(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use std::collections::BTreeMap;

    /// The prototype's register, saved by a spreadsheet in Windows-1252 with
    /// semicolons and a title line above the header.
    fn register_1252() -> Vec<u8> {
        let mut bytes = b"BHC 006 accession register, exported 12/03/2024\r\n".to_vec();
        bytes.extend(b"Accession No;Title;Date;Shelf mark;Notes\r\n");
        bytes.extend(b"BHC_006_AF_001;Brass lamp;1890;A-3-12;\r\n");
        bytes.extend(b"BHC_006_AF_004;Caf\xE9 sign;1925;A-4-07;Enamel\r\n");
        bytes.extend(b"BHC_006_PH_101;  Harbour at dawn ;;B-1-01;\r\n");
        bytes
    }

    fn plan(delimiter: Delimiter, header_row: usize, windows1252: bool) -> CsvPlan {
        CsvPlan {
            file: "BHC006_items.csv".to_string(),
            delimiter,
            header_row,
            windows1252,
            trim: true,
            join_column: "Accession No".to_string(),
            columns: BTreeMap::new(),
        }
    }

    #[test]
    fn a_windows_1252_register_is_detected_and_repaired() {
        let bytes = register_1252();
        let seen = inspect(&bytes, &plan(Delimiter::Semicolon, 2, false));
        assert_eq!(
            seen.detected,
            Detected {
                delimiter: Delimiter::Semicolon,
                header_row: 2
            }
        );
        assert_eq!(seen.cells_look_like_1252, 1);
        assert_eq!(seen.cells_with_spaces, 1);

        let sheet = Sheet::read(&bytes, &plan(Delimiter::Semicolon, 2, true)).expect("sheet");
        assert_eq!(
            sheet.head,
            ["Accession No", "Title", "Date", "Shelf mark", "Notes"]
        );
        assert_eq!(sheet.join, 0);
        assert_eq!(sheet.rows[1].cells[1], "Café sign");
        assert_eq!(sheet.rows[2].cells[1], "Harbour at dawn");
        assert_eq!(sheet.rows[2].line, 5);
        let text: String = sheet.rows.iter().flat_map(|r| r.cells.clone()).collect();
        assert!(!text.contains('Ã') && !text.contains('\u{FFFD}'), "{text}");

        let unrepaired = Sheet::read(&bytes, &plan(Delimiter::Semicolon, 2, false)).expect("sheet");
        assert_eq!(unrepaired.rows[1].cells[1], "Caf\u{FFFD} sign");
    }

    #[test]
    fn utf8_mojibake_is_repaired_cell_by_cell() {
        let bytes = "id,title\nA1,CafÃ© sign\nA2,Café\n".as_bytes();
        let sheet = Sheet::read(
            bytes,
            &CsvPlan {
                join_column: "id".to_string(),
                ..plan(Delimiter::Comma, 1, true)
            },
        )
        .expect("sheet");
        assert_eq!(sheet.rows[0].cells[1], "Café sign");
        assert_eq!(sheet.rows[1].cells[1], "Café");
    }

    #[test]
    fn a_wrong_delimiter_or_header_line_is_refused_with_its_key() {
        let bytes = register_1252();
        let one_cell = Sheet::read(&bytes, &plan(Delimiter::Tab, 2, true)).expect_err("tab");
        assert_eq!(one_cell.key(), "delimiter");
        let title = Sheet::read(&bytes, &plan(Delimiter::Semicolon, 1, true)).expect_err("title");
        assert_eq!(title.key(), "headerRow");
        let join = Sheet::read(
            &bytes,
            &CsvPlan {
                join_column: "Ref".to_string(),
                ..plan(Delimiter::Semicolon, 2, true)
            },
        );
        assert_eq!(join.expect_err("join").key(), "joinColumn");
    }

    #[test]
    fn a_utf8_bom_is_not_part_of_the_first_header() {
        let bytes = "\u{FEFF}id,title\nA1,x\n".as_bytes();
        let sheet = Sheet::read(
            bytes,
            &CsvPlan {
                join_column: "id".to_string(),
                ..plan(Delimiter::Comma, 1, false)
            },
        )
        .expect("sheet");
        assert_eq!(sheet.head[0], "id");
    }
}
