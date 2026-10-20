//! Workbook sheets: an .xlsx, .xls or .ods file read through `calamine` as
//! the rows a CSV export of the same sheet would give.
//!
//! A row's line is its row number in the sheet and a column's index its
//! column, counted from A1, so line-keyed decisions and column refs mean
//! the same cell as in Excel. calamine stores no display format, so a cell
//! is its value as text: a date or time as ISO text, a whole number without
//! a decimal point, a boolean as `TRUE` or `FALSE`, an error as Excel writes
//! it (`#N/A`). A formula cell is its last computed value.

use std::io::{Cursor, Read};

use calamine::{Data, Reader, SheetType, SheetVisible, Sheets};

use super::{Parsed, SheetError};

/// The most bytes a workbook may unpack to. The rows calamine builds take
/// a small multiple of this, so the bound is on memory, not on the file.
pub const MAX_UNPACKED_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkbookKind {
    Xlsx,
    Xls,
    Ods,
}

const ZIP: &[u8] = b"PK\x03\x04";
const COMPOUND: &[u8] = &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// Whether the bytes start as a zip or a compound file does: never text,
/// so such a file is a workbook or no sheet at all.
#[must_use]
pub fn is_packed(bytes: &[u8]) -> bool {
    bytes.starts_with(ZIP) || bytes.starts_with(COMPOUND)
}

impl WorkbookKind {
    /// The kind the first bytes name: a zip holding a workbook part, or a
    /// compound file. `None` for anything else, a CSV included.
    #[must_use]
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(COMPOUND) {
            return Some(Self::Xls);
        }
        if !bytes.starts_with(ZIP) {
            return None;
        }
        let archive = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
        if archive.index_for_name("xl/workbook.xml").is_some() {
            Some(Self::Xlsx)
        } else if archive.index_for_name("content.xml").is_some() {
            Some(Self::Ods)
        } else {
            None
        }
    }
}

/// The sheet a workbook read, and every sheet it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workbook {
    pub kind: WorkbookKind,
    pub sheet: String,
    /// The worksheets, in workbook order.
    pub sheets: Vec<String>,
}

/// The rows of `name`, else of the first visible worksheet, from the
/// header line on: each row cut after its last non-empty cell, but never
/// shorter than the header, so a workbook row is never short.
pub(super) fn rows(
    bytes: &[u8],
    kind: WorkbookKind,
    name: Option<&str>,
    header_line: u32,
) -> Result<(Workbook, Vec<Parsed>), SheetError> {
    if kind != WorkbookKind::Xls {
        let unpacked = unpacked_size(bytes, MAX_UNPACKED_BYTES)?;
        if unpacked > MAX_UNPACKED_BYTES {
            return Err(SheetError::Unpacked(unpacked));
        }
    }
    let unread = |error: calamine::Error| SheetError::Workbook(error.to_string());
    let mut book: Sheets<Cursor<&[u8]>> = match kind {
        WorkbookKind::Xlsx => Sheets::Xlsx(
            calamine::open_workbook_from_rs(Cursor::new(bytes))
                .map_err(|error: calamine::XlsxError| SheetError::Workbook(error.to_string()))?,
        ),
        WorkbookKind::Xls => Sheets::Xls(
            calamine::open_workbook_from_rs(Cursor::new(bytes))
                .map_err(|error: calamine::XlsError| SheetError::Workbook(error.to_string()))?,
        ),
        WorkbookKind::Ods => Sheets::Ods(
            calamine::open_workbook_from_rs(Cursor::new(bytes))
                .map_err(|error: calamine::OdsError| SheetError::Workbook(error.to_string()))?,
        ),
    };
    let worksheets: Vec<_> = book
        .sheets_metadata()
        .iter()
        .filter(|sheet| sheet.typ == SheetType::WorkSheet)
        .cloned()
        .collect();
    let chosen = match name {
        Some(name) => worksheets
            .iter()
            .find(|sheet| sheet.name == name)
            .ok_or_else(|| SheetError::NoSheet(name.to_string()))?,
        None => worksheets
            .iter()
            .find(|sheet| sheet.visible == SheetVisible::Visible)
            .or(worksheets.first())
            .ok_or_else(|| SheetError::Workbook("it holds no worksheet".to_string()))?,
    }
    .name
    .clone();
    let range = book.worksheet_range(&chosen).map_err(unread)?;
    let workbook = Workbook {
        kind,
        sheet: chosen,
        sheets: worksheets.into_iter().map(|sheet| sheet.name).collect(),
    };

    let (top, left) = range.start().unwrap_or((0, 0));
    let last = range.end().map_or(0, |(bottom, _)| bottom + 1);
    if header_line > last.max(1) {
        return Err(SheetError::NoHeaderLine {
            header_line,
            lines: last,
        });
    }
    // Lines above the range are empty rows; the range's rows follow from
    // line `top + 1`.
    let mut lines = (1..=top)
        .map(|_| Vec::new())
        .chain(range.rows().map(|row| {
            let mut cells = vec![String::new(); left as usize];
            cells.extend(row.iter().map(text));
            let used = cells
                .iter()
                .rposition(|cell| !cell.is_empty())
                .map_or(0, |at| at + 1);
            cells.truncate(used);
            cells
        }))
        .skip(header_line as usize - 1);
    let header = lines.next().unwrap_or_default();
    let width = header.len();
    let mut parsed = vec![record(header_line, header)];
    for (line, mut cells) in (header_line + 1..).zip(lines) {
        if cells.len() < width {
            cells.resize(width, String::new());
        }
        parsed.push(record(line, cells));
    }
    Ok((workbook, parsed))
}

fn record(line: u32, fields: Vec<String>) -> Parsed {
    Parsed {
        line,
        raw: fields.join("\t"),
        fields,
        broken: false,
    }
}

/// The bytes the zip's entries unpack to, counted as they unpack, since a
/// zip's directory need not tell the truth; counting stops past `bound`.
pub(super) fn unpacked_size(bytes: &[u8], bound: u64) -> Result<u64, SheetError> {
    let unread = |error: &dyn std::fmt::Display| SheetError::Workbook(error.to_string());
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| unread(&e))?;
    let mut total: u64 = 0;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|e| unread(&e))?;
        let room = bound - total + 1;
        total +=
            std::io::copy(&mut entry.take(room), &mut std::io::sink()).map_err(|e| unread(&e))?;
        if total > bound {
            break;
        }
    }
    Ok(total)
}

/// A cell as text, as the module comment says.
fn text(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(text) | Data::DateTimeIso(text) | Data::DurationIso(text) => text.clone(),
        Data::Int(number) => number.to_string(),
        Data::Float(number) => float(*number),
        Data::Bool(true) => "TRUE".to_string(),
        Data::Bool(false) => "FALSE".to_string(),
        Data::Error(error) => error.to_string(),
        Data::DateTime(moment) => {
            let (year, month, day, hour, minute, second, _) = moment.to_ymd_hms_milli();
            let serial = moment.as_f64();
            if moment.is_duration() {
                let seconds = (serial * 86_400.0).round();
                #[allow(clippy::cast_possible_truncation)]
                let seconds = seconds as i64;
                format!(
                    "{}:{:02}:{:02}",
                    seconds / 3600,
                    seconds % 3600 / 60,
                    seconds % 60
                )
            } else if serial < 1.0 {
                format!("{hour:02}:{minute:02}:{second:02}")
            } else if (hour, minute, second) == (0, 0, 0) {
                format!("{year:04}-{month:02}-{day:02}")
            } else {
                format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}")
            }
        }
    }
}

/// A number as Excel's General format shows it in the formula bar: 15
/// significant digits, no trailing zeros, and `1E+20` past them.
fn float(number: f64) -> String {
    if number == 0.0 || !number.is_finite() {
        return if number.is_finite() {
            "0".to_string()
        } else {
            number.to_string()
        };
    }
    let magnitude = number.abs().log10().floor();
    if !(-9.0..15.0).contains(&magnitude) {
        let text = format!("{number:.14e}");
        let (mantissa, exponent) = text.split_once('e').unwrap_or((&text, "0"));
        let mantissa = mantissa.trim_end_matches('0').trim_end_matches('.');
        let exponent: i32 = exponent.parse().unwrap_or(0);
        return format!("{mantissa}E{exponent:+03}");
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let decimals = (14.0 - magnitude).max(0.0) as usize;
    let text = format!("{number:.decimals$}");
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text
    }
}
