#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::io::{Cursor, Write};
use std::num::NonZeroU32;

use rust_xlsxwriter::{ExcelDateTime, Format as CellFormat, Workbook as XlsxBook};

use super::*;
use crate::import::plan::{Auto, ImportPlan, PlanError};

fn options(header_line: u32, sheet_name: Option<&str>) -> ReadOptions {
    ReadOptions {
        encoding: EncodingChoice::Auto(Auto::Auto),
        delimiter: Delimiter::Auto,
        quote: Quote::Double,
        header_line: NonZeroU32::new(header_line).unwrap(),
        trim: true,
        sheet_name: sheet_name.map(str::to_string),
    }
}

fn key(index: u32) -> ColumnRef {
    ColumnRef {
        index,
        header: String::new(),
    }
}

fn cells(sheet: &Sheet) -> Vec<Vec<&str>> {
    sheet
        .rows
        .iter()
        .map(|row| row.cells.iter().map(String::as_str).collect())
        .collect()
}

/// A register as an archive keeps one: a title line, then the header on
/// line 2, a Devanagari title column, a date, numbers and a blank row.
fn register_xlsx() -> Vec<u8> {
    let mut book = XlsxBook::new();
    let date = CellFormat::new().set_num_format("dd/mm/yyyy");
    let sheet = book.add_worksheet().set_name("Register").unwrap();
    sheet.write(0, 0, "Bombay register, 1950").unwrap();
    for (col, header) in ["Identifier", "शीर्षक", "Date", "Pages", "Width", "Seen"]
        .into_iter()
        .enumerate()
    {
        sheet.write(1, col as u16, header).unwrap();
    }
    sheet.write(2, 0, "BHC_101").unwrap();
    sheet.write(2, 1, "मुंबई बंदर").unwrap();
    let day = ExcelDateTime::from_ymd(1950, 3, 4).unwrap();
    sheet.write_with_format(2, 2, &day, &date).unwrap();
    sheet.write(2, 3, 42).unwrap();
    sheet.write(2, 4, 3.5).unwrap();
    sheet.write(2, 5, true).unwrap();
    sheet.write(4, 0, "BHC_102").unwrap();
    sheet.write(4, 1, "दीपक").unwrap();
    sheet.write(5, 0, "BHC_101").unwrap();
    sheet.write(5, 6, "beyond the header").unwrap();
    let notes = book.add_worksheet().set_name("Notes").unwrap();
    notes.write(0, 0, "Note").unwrap();
    notes.write(1, 0, "kept apart").unwrap();
    book.save_to_buffer().unwrap()
}

/// The same register as the CSV Excel would save.
const REGISTER_CSV: &str = "Bombay register, 1950\r\n\
    Identifier,शीर्षक,Date,Pages,Width,Seen\r\n\
    BHC_101,मुंबई बंदर,1950-03-04,42,3.5,TRUE\r\n\
    ,,,,,\r\n\
    BHC_102,दीपक,,,,\r\n\
    BHC_101,,,,,,beyond the header\r\n";

fn zip(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, text) in entries {
        out.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        out.write_all(text.as_bytes()).unwrap();
    }
    out.finish().unwrap().into_inner()
}

fn ods(tables: &[(&str, &[&[&str]])]) -> Vec<u8> {
    let cell = |text: &str| match text.split_once(':') {
        Some(("date", value)) => format!(
            r#"<table:table-cell office:value-type="date" office:date-value="{value}"><text:p>04/03/50</text:p></table:table-cell>"#
        ),
        Some(("float", value)) => format!(
            r#"<table:table-cell office:value-type="float" office:value="{value}"><text:p>{value}</text:p></table:table-cell>"#
        ),
        _ if text.is_empty() => "<table:table-cell/>".to_string(),
        _ => format!(
            r#"<table:table-cell office:value-type="string"><text:p>{text}</text:p></table:table-cell>"#
        ),
    };
    let body: String = tables
        .iter()
        .map(|(name, rows)| {
            let rows: String = rows
                .iter()
                .map(|row| {
                    format!(
                        "<table:table-row>{}</table:table-row>",
                        row.iter().map(|text| cell(text)).collect::<String>()
                    )
                })
                .collect();
            format!(r#"<table:table table:name="{name}">{rows}</table:table>"#)
        })
        .collect();
    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" office:version="1.2"><office:body><office:spreadsheet>{body}</office:spreadsheet></office:body></office:document-content>"#
    );
    zip(&[
        ("mimetype", "application/vnd.oasis.opendocument.spreadsheet"),
        ("content.xml", &content),
        (
            "META-INF/manifest.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?><manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0"><manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/><manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/></manifest:manifest>"#,
        ),
    ])
}

/// A BIFF8 workbook of one sheet: text cells as UTF-16 labels, numbers as
/// NUMBER records, in the compound file's `Workbook` stream.
fn xls(name: &str, rows: &[&[&str]]) -> Vec<u8> {
    fn record(out: &mut Vec<u8>, kind: u16, data: &[u8]) {
        out.extend(kind.to_le_bytes());
        out.extend(u16::try_from(data.len()).unwrap().to_le_bytes());
        out.extend(data);
    }
    let bof = |kind: u16| {
        let mut data = vec![0x00, 0x06];
        data.extend(kind.to_le_bytes());
        data.extend([0xBB, 0x0D, 0xCC, 0x07, 0, 0, 0, 0, 0x06, 0, 0, 0]);
        data
    };
    let utf16 =
        |text: &str| -> Vec<u8> { text.encode_utf16().flat_map(u16::to_le_bytes).collect() };

    let mut sheet = Vec::new();
    record(&mut sheet, 0x0809, &bof(0x0010));
    for (row, cells) in rows.iter().enumerate() {
        for (col, text) in cells.iter().enumerate() {
            let mut data = Vec::new();
            data.extend(u16::try_from(row).unwrap().to_le_bytes());
            data.extend(u16::try_from(col).unwrap().to_le_bytes());
            data.extend(0_u16.to_le_bytes());
            if let Ok(number) = text.parse::<f64>() {
                data.extend(number.to_le_bytes());
                record(&mut sheet, 0x0203, &data);
            } else {
                data.extend(
                    u16::try_from(text.encode_utf16().count())
                        .unwrap()
                        .to_le_bytes(),
                );
                data.push(1);
                data.extend(utf16(text));
                record(&mut sheet, 0x0204, &data);
            }
        }
    }
    record(&mut sheet, 0x000A, &[]);

    let mut bound = vec![0, 0, 0, 0, 0, 0];
    bound.push(u8::try_from(name.encode_utf16().count()).unwrap());
    bound.push(1);
    bound.extend(utf16(name));
    let mut globals = Vec::new();
    record(&mut globals, 0x0809, &bof(0x0005));
    record(&mut globals, 0x0042, &1200_u16.to_le_bytes());
    let at = globals.len() + 4 + bound.len() + 4;
    bound[..4].copy_from_slice(&u32::try_from(at).unwrap().to_le_bytes());
    record(&mut globals, 0x0085, &bound);
    record(&mut globals, 0x000A, &[]);
    globals.extend(sheet);

    let mut file = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
    file.create_stream("Workbook")
        .unwrap()
        .write_all(&globals)
        .unwrap();
    file.flush().unwrap();
    file.into_inner().into_inner()
}

#[test]
fn a39_an_xlsx_register_reads_as_its_csv_reads() {
    let from_xlsx = read(&register_xlsx(), &options(2, None), Some(&key(0))).expect("xlsx");
    let from_csv = read(REGISTER_CSV.as_bytes(), &options(2, None), Some(&key(0))).expect("csv");
    assert_eq!(from_xlsx.columns, from_csv.columns);
    assert_eq!(from_xlsx.notes, from_csv.notes);
    assert_eq!(from_xlsx.counts, from_csv.counts);
    // The raw line differs (a workbook row has no delimiters); the rest of
    // each row, its flag included, is the same.
    let rows = |sheet: &Sheet| -> Vec<Row> {
        let mut rows = sheet.rows.clone();
        for row in &mut rows {
            if let Some(flag) = row.cells_count.as_mut() {
                flag.raw.clear();
            }
        }
        rows
    };
    assert_eq!(rows(&from_xlsx), rows(&from_csv));
    assert_eq!(
        from_xlsx.rows[2]
            .cells_count
            .as_ref()
            .map(|flag| flag.problem),
        Some(CellsProblem::Long { found: 7 })
    );
    assert_eq!(
        cells(&from_xlsx)[0],
        ["BHC_101", "मुंबई बंदर", "1950-03-04", "42", "3.5", "TRUE"]
    );
    assert_eq!(from_xlsx.rows[2].key.as_str(), "BHC_101#2");
    let book = from_xlsx.workbook.expect("a workbook");
    assert_eq!(book.kind, WorkbookKind::Xlsx);
    assert_eq!(book.sheet, "Register");
    assert_eq!(book.sheets, ["Register", "Notes"]);
    assert!(from_csv.workbook.is_none());
}

#[test]
fn a39_the_first_sheet_is_read_unless_the_plan_names_one() {
    let bytes = register_xlsx();
    let notes = read(&bytes, &options(1, Some("Notes")), None).expect("named");
    assert_eq!(notes.columns[0].header, "Note");
    assert_eq!(cells(&notes), [["kept apart"]]);
    assert_eq!(notes.workbook.unwrap().sheet, "Notes");
    assert_eq!(
        read(&bytes, &options(1, Some("Ledger")), None),
        Err(SheetError::NoSheet("Ledger".to_string()))
    );
    assert!(matches!(
        read(&bytes, &options(40, None), None),
        Err(SheetError::NoHeaderLine {
            header_line: 40,
            ..
        })
    ));
}

#[test]
fn a39_an_ods_register_gives_dates_as_iso_and_numbers_as_written() {
    let bytes = ods(&[
        ("Cover", &[&["not this one"]]),
        (
            "Register",
            &[
                &["Identifier", "Title", "Date", "Pages"],
                &["BHC_101", "मुंबई", "date:1950-03-04", "float:42"],
                &["BHC_102", "Lamp", "", "float:0.25"],
            ],
        ),
    ]);
    assert_eq!(WorkbookKind::sniff(&bytes), Some(WorkbookKind::Ods));
    let sheet = read(&bytes, &options(1, Some("Register")), None).expect("ods");
    assert_eq!(cells(&sheet)[0], ["BHC_101", "मुंबई", "1950-03-04", "42"]);
    assert_eq!(cells(&sheet)[1], ["BHC_102", "Lamp", "", "0.25"]);
    let first = read(&bytes, &options(1, None), None).expect("first");
    assert_eq!(first.columns[0].header, "not this one");
}

#[test]
fn a39_an_xls_register_reads_its_utf16_labels_and_numbers() {
    let bytes = xls(
        "Register",
        &[&["Identifier", "Title", "Pages"], &["BHC_101", "मुंबई", "42"]],
    );
    assert_eq!(WorkbookKind::sniff(&bytes), Some(WorkbookKind::Xls));
    let sheet = read(&bytes, &options(1, None), Some(&key(0))).expect("xls");
    assert_eq!(cells(&sheet), [["BHC_101", "मुंबई", "42"]]);
    assert_eq!(sheet.rows[0].key.as_str(), "BHC_101#1");
    assert_eq!(sheet.workbook.unwrap().kind, WorkbookKind::Xls);
}

#[test]
fn a39_a_zip_that_is_no_workbook_and_a_broken_workbook_do_not_read_as_text() {
    let other = zip(&[("readme.txt", "hello")]);
    assert_eq!(WorkbookKind::sniff(&other), None);
    let mut broken = register_xlsx();
    broken.truncate(broken.len() / 2);
    assert_eq!(WorkbookKind::sniff(&broken), None, "no zip directory");
    for bytes in [&other, &broken] {
        assert!(matches!(
            read(bytes, &options(1, None), None),
            Err(SheetError::Workbook(_))
        ));
    }
    let mut cut = xls("R", &[&["Id"]]);
    cut.truncate(600);
    assert!(matches!(
        read(&cut, &options(1, None), None),
        Err(SheetError::Workbook(_))
    ));
}

#[test]
fn a_workbook_that_unpacks_past_the_bound_is_refused_unread() {
    let big = "x".repeat(4096);
    let bytes = zip(&[
        ("xl/workbook.xml", &big),
        ("xl/worksheets/sheet1.xml", &big),
    ]);
    assert_eq!(workbook::unpacked_size(&bytes, 4096), Ok(4097));
    assert_eq!(workbook::unpacked_size(&bytes, 10_000), Ok(8192));
}

#[test]
fn the_guess_for_a_workbook_is_the_plain_default() {
    let guess = guess(&register_xlsx()[..1024]);
    assert_eq!(guess.delimiter, Delimiter::Comma);
    assert_eq!(guess.header_line.get(), 1);
}

#[test]
fn sheet_name_is_an_optional_string_at_the_boundary() {
    let full: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/full.json")).unwrap();
    let with = |name: serde_json::Value| {
        let mut plan = full.clone();
        plan["sheets"][0]["read"]["sheetName"] = name;
        ImportPlan::from_json(&plan.to_string())
    };
    let plain = ImportPlan::from_json(&full.to_string()).unwrap();
    let first = |plan: &ImportPlan| {
        plan.structure
            .lists()
            .next()
            .unwrap()
            .1
            .read
            .sheet_name
            .clone()
    };
    assert_eq!(first(&plain), None);
    assert!(
        !plain.to_json().contains("sheetName"),
        "unchanged on the wire"
    );
    let named = with(serde_json::json!("Register")).unwrap();
    assert_eq!(first(&named).as_deref(), Some("Register"));
    assert!(named.to_json().contains(r#""sheetName":"Register""#));
    assert_eq!(with(serde_json::Value::Null).unwrap(), plain);
    assert!(matches!(
        with(serde_json::json!(2)),
        Err(PlanError::Shape(_))
    ));
}

#[test]
fn a39_a_number_reads_with_the_15_digits_excel_shows() {
    let mut book = XlsxBook::new();
    let sheet = book.add_worksheet();
    sheet.write(0, 0, "N").unwrap();
    for (row, number) in (1_u32..).zip([0.1 + 0.2, 2.0 / 3.0, -1234.5, 1e20, 0.000_25, -0.0]) {
        sheet.write(row, 0, number).unwrap();
    }
    let read = read(&book.save_to_buffer().unwrap(), &options(1, None), None).expect("xlsx");
    let column: Vec<&str> = read.rows.iter().map(|row| row.cells[0].as_str()).collect();
    assert_eq!(
        column,
        [
            "0.3",
            "0.666666666666667",
            "-1234.5",
            "1E+20",
            "0.00025",
            "0"
        ]
    );
}
