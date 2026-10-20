#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use proptest::prelude::*;

use super::*;
use crate::import2::plan::{Auto, Encoding};

fn options(delimiter: Delimiter, quote: Quote, header_line: u32) -> ReadOptions {
    ReadOptions {
        encoding: EncodingChoice::Auto(Auto::Auto),
        delimiter,
        quote,
        header_line: NonZeroU32::new(header_line).expect("line"),
        trim: false,
        sheet_name: None,
    }
}

fn auto() -> ReadOptions {
    options(Delimiter::Auto, Quote::Double, 1)
}

fn column(index: u32) -> ColumnRef {
    ColumnRef {
        index,
        header: String::new(),
    }
}

fn sheet(bytes: &[u8], options: &ReadOptions) -> Sheet {
    read(bytes, options, None).expect("read")
}

fn cells(sheet: &Sheet) -> Vec<Vec<&str>> {
    sheet
        .rows
        .iter()
        .map(|row| row.cells.iter().map(String::as_str).collect())
        .collect()
}

fn headers(sheet: &Sheet) -> Vec<&str> {
    sheet.columns.iter().map(|c| c.header.as_str()).collect()
}

fn utf16le(text: &str, bom: bool) -> Vec<u8> {
    let mut bytes = if bom { vec![0xFF, 0xFE] } else { Vec::new() };
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    bytes
}

#[test]
fn a40_utf8_and_utf16_are_read_with_or_without_a_mark() {
    let text = "id,title\nA1,मुंबई\n";
    let marked = [b"\xEF\xBB\xBF".as_slice(), text.as_bytes()].concat();
    let utf8 = sheet(&marked, &auto());
    assert_eq!(
        headers(&utf8),
        ["id", "title"],
        "the mark is not in the header"
    );
    assert_eq!(utf8.encoding.encoding, Encoding::Utf8);
    assert_eq!(utf8.encoding.confidence, Some(1.0));

    for bom in [true, false] {
        let read = sheet(&utf16le(text, bom), &auto());
        assert_eq!(read.encoding.encoding, Encoding::Utf16Le, "bom {bom}");
        assert_eq!(headers(&read), ["id", "title"], "no hidden NULs");
        assert_eq!(cells(&read), [["A1", "मुंबई"]]);
    }
    let big: Vec<u8> = text.encode_utf16().flat_map(u16::to_be_bytes).collect();
    assert_eq!(sheet(&big, &auto()).encoding.encoding, Encoding::Utf16Be);
}

#[test]
fn a40_a_1252_sheet_is_guessed_and_a_stray_1252_byte_keeps_utf8() {
    let western = b"Accession No;Title\nBHC_006_AF_004;Caf\xE9 sign\nX;Na\xEFve \x93quoted\x94\n";
    let read = sheet(western, &auto());
    assert_eq!(read.encoding.encoding, Encoding::Windows1252);
    let confidence = read.encoding.confidence.expect("guessed");
    assert!(confidence > 0.0 && confidence < 1.0, "{confidence}");
    assert_eq!(cells(&read)[1], ["X", "Naïve “quoted”"]);

    let mixed = ["id,place\nA1,मुंबई\nA2,दिल्ली\nA3,Caf".as_bytes(), b"\xE9\n"].concat();
    let read = sheet(&mixed, &auto());
    assert_eq!(read.encoding.encoding, Encoding::Utf8);
    assert_eq!(cells(&read)[0], ["A1", "मुंबई"]);
    assert_eq!(
        read.counts.cells_suspect, 1,
        "the bad byte is U+FFFD and counted"
    );
}

#[test]
fn a40_an_encoding_with_no_decoder_is_refused_by_name() {
    let (japanese, _, _) =
        encoding_rs::SHIFT_JIS.encode("番号,題名\n一,東京の古い写真\n二,大阪の地図と港の絵葉書\n");
    assert_eq!(
        read(&japanese, &auto(), None),
        Err(SheetError::EncodingUnsupported("Shift_JIS"))
    );
    assert!(guess(&japanese).encoding.is_err());
    let utf32 = b"\xFF\xFE\0\0i\0\0\0";
    assert_eq!(
        read(utf32, &auto(), None),
        Err(SheetError::EncodingUnsupported("UTF-32LE"))
    );
}

#[test]
fn a40_a_named_encoding_is_used_as_named() {
    let bytes = b"id,sign\nA1,\x80\n";
    let mut latin1 = auto();
    latin1.encoding = EncodingChoice::Is(Encoding::Iso88591);
    let mut cp1252 = auto();
    cp1252.encoding = EncodingChoice::Is(Encoding::Windows1252);
    assert_eq!(cells(&sheet(bytes, &latin1))[0][1], "\u{80}");
    assert_eq!(cells(&sheet(bytes, &cp1252))[0][1], "€");
    assert_eq!(sheet(bytes, &cp1252).encoding.confidence, None);
}

#[test]
fn a41_every_delimiter_of_the_contract_is_found_or_used() {
    let cases = [
        ("id,title\nA1,Brass lamp\nA2,Tin box\n", Delimiter::Comma),
        ("id;title\nA1;Lamp, brass\nA2;Box\n", Delimiter::Semicolon),
        ("id\ttitle\nA1\tLamp\nA2\tBox\n", Delimiter::Tab),
        (
            "id|title|date\nA1|Lamp|1890\nA2|Box|1925\n",
            Delimiter::Pipe,
        ),
        ("id:title\nA1:Lamp\nA2:Box\n", Delimiter::Colon),
        ("id title\nA1 Lamp\nA2 Box\n", Delimiter::Space),
        ("id,time\nA1,10:30\nA2,11:45\n", Delimiter::Comma),
    ];
    for (text, expected) in cases {
        let read = sheet(text.as_bytes(), &auto());
        assert_eq!(read.delimiter, expected, "{text:?}");
        assert!(read.delimiter_guessed);
        assert_eq!(read.counts.cells_count, 0, "{text:?}");
        let chosen = sheet(text.as_bytes(), &options(expected, Quote::Double, 1));
        assert!(!chosen.delimiter_guessed);
        assert_eq!(chosen.rows, read.rows);
    }
}

#[test]
fn a42_cr_only_lines_are_lines() {
    let read = sheet(b"id,title\rA1,Lamp\rA2,Box\r", &auto());
    assert_eq!(cells(&read), [["A1", "Lamp"], ["A2", "Box"]]);
    assert_eq!(read.rows[1].line, 3);
    let mixed = sheet(b"id,title\r\nA1,Lamp\nA2,Box\rA3,Cup", &auto());
    assert_eq!(mixed.rows.len(), 3);
}

#[test]
fn a43_single_quotes_and_no_quotes_read_as_chosen() {
    let text = b"id,title\nA1,'Lamp, brass'\nA2,\"Box\"\n";
    let single = sheet(text, &options(Delimiter::Comma, Quote::Single, 1));
    assert_eq!(cells(&single)[0], ["A1", "Lamp, brass"]);
    assert_eq!(cells(&single)[1], ["A2", "\"Box\""]);
    let none = sheet(
        b"id,title\nA1,\"Box\"\n",
        &options(Delimiter::Comma, Quote::None, 1),
    );
    assert_eq!(cells(&none)[0], ["A1", "\"Box\""]);
}

#[test]
fn a43_a_broken_quote_flags_its_row_and_never_moves_later_rows() {
    let text = "id,title,date\nA1,\"Lamp,1890\nA2,Box,1925\nA3,\"Cup\"s,1930\nA4,Jar,1931\n";
    let read = sheet(text.as_bytes(), &auto());
    let ids: Vec<&str> = read.rows.iter().map(|row| row.cells[0].as_str()).collect();
    assert_eq!(ids, ["A1", "A2", "A3", "A4"]);
    assert_eq!(cells(&read)[1], ["A2", "Box", "1925"]);
    assert_eq!(read.rows[1].cells_count, None);
    let broken = read.rows[0].cells_count.as_ref().expect("flagged");
    assert_eq!(broken.problem, CellsProblem::Quote);
    assert_eq!(broken.raw, "A1,\"Lamp,1890");
    assert_eq!(
        read.rows[2].cells_count.as_ref().map(|c| c.problem),
        Some(CellsProblem::Quote)
    );
    assert_eq!(read.rows[3].line, 5);
    assert_eq!(read.counts.cells_count, 2);
}

#[test]
fn a43_a_quote_that_closes_lines_later_in_a_row_of_the_wrong_width_moves_nothing() {
    let text = "id,title,date\nA1,\"Lamp\nA2,Box,1925\"\nA3,Cup,1930\n";
    let got = sheet(text.as_bytes(), &auto());
    let ids: Vec<&str> = got.rows.iter().map(|row| row.cells[0].as_str()).collect();
    assert_eq!(ids, ["A1", "A2", "A3"]);
    assert_eq!(
        got.rows[0].cells_count.as_ref().map(|c| c.problem),
        Some(CellsProblem::Quote)
    );
    assert_eq!(got.rows[2].cells, ["A3", "Cup", "1930"]);
}

#[test]
fn a_quoted_line_end_inside_a_row_of_the_right_width_is_one_cell() {
    let read = sheet(b"id,note\nA1,\"two\nlines\"\nA2,one\n", &auto());
    assert_eq!(cells(&read), [["A1", "two\nlines"], ["A2", "one"]]);
    assert_eq!(read.rows[1].line, 4);
}

#[test]
fn a44_short_rows_are_padded_and_long_rows_keep_their_extra_cells() {
    let text = "id,title,date\nA1,Lamp,1890,\nA2,Box\nA3,Cup,1930,glass,blue\n";
    let read = sheet(text.as_bytes(), &auto());
    assert_eq!(
        read.rows[0].cells_count, None,
        "a trailing delimiter is no cell"
    );
    assert!(read.rows[0].extra.is_empty());
    assert_eq!(cells(&read)[1], ["A2", "Box", ""]);
    let short = read.rows[1].cells_count.as_ref().expect("short");
    assert_eq!(short.problem, CellsProblem::Short { found: 2 });
    assert_eq!(short.raw, "A2,Box\n");
    assert_eq!(read.rows[2].extra, ["glass", "blue"]);
    assert_eq!(
        read.rows[2].cells_count.as_ref().map(|c| c.problem),
        Some(CellsProblem::Long { found: 5 })
    );
}

#[test]
fn a45_the_header_is_any_line_and_the_guess_skips_a_title() {
    let mut text = String::new();
    for line in 1..=6 {
        text.push_str(&format!("Preface line {line}\n"));
    }
    text.push_str("id,title\nA1,Lamp\nA2,Box\n");
    let got = sheet(text.as_bytes(), &options(Delimiter::Auto, Quote::Double, 7));
    assert_eq!(headers(&got), ["id", "title"]);
    assert_eq!(got.rows[0].line, 8);
    assert_eq!(got.counts.rows, 2);
    let offered = guess(text.as_bytes());
    assert_eq!(offered.header_line.get(), 7);
    assert_eq!(offered.delimiter, Delimiter::Comma);
    assert_eq!(
        read(
            b"id\nA1\n",
            &options(Delimiter::Auto, Quote::Double, 5),
            None
        ),
        Err(SheetError::NoHeaderLine {
            header_line: 5,
            lines: 2
        })
    );
}

#[test]
fn a46_duplicate_empty_and_hash_headers_are_reported_and_keyed_by_index() {
    let read = sheet(
        b"Subject, Subject ,,#note,Cafe\xCC\x81\nhistory,maps,x,y,z\n",
        &auto(),
    );
    assert_eq!(
        headers(&read),
        ["Subject", "Subject", "", "#note", "Caf\u{e9}"]
    );
    assert_eq!(
        read.notes,
        [
            HeaderNote::Empty { index: 2 },
            HeaderNote::Duplicate {
                header: "Subject".to_string(),
                indexes: vec![0, 1]
            }
        ]
    );
    let mappable: Vec<bool> = read.columns.iter().map(|c| c.mappable).collect();
    assert_eq!(mappable, [true, true, true, false, true]);
    assert_eq!(
        cells(&read)[0][1],
        "maps",
        "the second Subject has its own cells"
    );
}

#[test]
fn a56_counts_skip_the_lines_above_the_header_and_follow_the_trim() {
    let text = "  Register, exported 2024 \nid,title\nA1, Lamp \nA2,CafÃ© sign\n\n,,\nA3,Box\n";
    let mut plan = options(Delimiter::Comma, Quote::Double, 2);
    let raw = sheet(text.as_bytes(), &plan);
    assert_eq!(
        raw.counts,
        Counts {
            rows: 3,
            cells_count: 0,
            blank_lines: 2,
            cells_with_spaces: 1,
            cells_suspect: 1,
        }
    );
    plan.trim = true;
    let trimmed = sheet(text.as_bytes(), &plan);
    assert_eq!(trimmed.counts.cells_with_spaces, 0);
    assert_eq!(cells(&trimmed)[0], ["A1", "Lamp"]);
}

#[test]
fn rows_are_keyed_by_their_key_value_with_a_count_or_by_line() {
    let text = "id,title\n A1 ,Lamp\nA2,Box\nA1,Lamp again\nCafe\u{301},Sign\n";
    let by_value = read(text.as_bytes(), &auto(), Some(&column(0))).expect("read");
    let keys: Vec<&str> = by_value.rows.iter().map(|row| row.key.as_str()).collect();
    assert_eq!(keys, ["A1#1", "A2#1", "A1#2", "Caf\u{e9}#1"]);
    let by_line = sheet(text.as_bytes(), &auto());
    let keys: Vec<&str> = by_line.rows.iter().map(|row| row.key.as_str()).collect();
    assert_eq!(keys, ["2", "3", "4", "5"]);
}

#[test]
fn the_key_column_is_the_join_column_else_the_item_key_column() {
    let plan: SheetPlan = serde_json::from_value(serde_json::json!({
        "id": "s", "source": "a", "file": "r.csv", "hash": null,
        "read": { "encoding": "auto", "delimiter": "auto", "quote": "\"", "headerLine": 1, "trim": false },
        "rowKind": "media",
        "join": null,
        "itemKey": { "index": 1, "header": "item" },
        "columns": []
    }))
    .expect("plan");
    assert_eq!(key_column(&plan).map(|c| c.index), Some(1));
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("r.csv");
    std::fs::write(&path, "file,item\na.jpg,X1\nb.jpg,X1\n").expect("write");
    let read = read_file(&path, &plan).expect("read");
    let keys: Vec<&str> = read.rows.iter().map(|row| row.key.as_str()).collect();
    assert_eq!(keys, ["X1#1", "X1#2"]);
}

#[test]
fn the_hash_is_the_sha256_of_the_bytes() {
    let read = sheet(b"id\nA1\n", &auto());
    assert_eq!(read.hash, omeka_media::store::sha256_hex(b"id\nA1\n"));
    assert_eq!(read.hash.len(), 64);
}

#[test]
fn a_sheet_over_64_mb_is_refused_before_it_is_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("huge.csv");
    let file = std::fs::File::create(&path).expect("create");
    file.set_len(MAX_SHEET_BYTES + 1).expect("sparse");
    let plan: SheetPlan = serde_json::from_value(serde_json::json!({
        "id": "s", "source": "a", "file": "huge.csv", "hash": null,
        "read": { "encoding": "auto", "delimiter": "auto", "quote": "\"", "headerLine": 1, "trim": false },
        "rowKind": "item", "join": null, "itemKey": null, "columns": []
    }))
    .expect("plan");
    assert_eq!(
        read_file(&path, &plan),
        Err(SheetError::TooLarge(MAX_SHEET_BYTES + 1))
    );
}

#[test]
fn a_text_file_is_a_sheet_only_when_its_lines_split_evenly() {
    assert!(looks_delimited(b"id|title\nA1|Lamp\nA2|Box\n"));
    assert!(looks_delimited(b"id\ttitle\nA1\tLamp\n"));
    assert!(!looks_delimited(
        b"These scans came from the harbour office.\nSee box 4.\n"
    ));
    assert!(!looks_delimited(b"one line, with a comma\n"));
}

fn csv(rows: &[(String, String)]) -> String {
    let mut text = "id,title\n".to_string();
    for (id, title) in rows {
        text.push_str(&format!("{id},{title}\n"));
    }
    text
}

fn keyed(text: &str) -> std::collections::BTreeMap<String, Vec<String>> {
    let read = read(text.as_bytes(), &auto(), Some(&column(0))).expect("read");
    read.rows
        .into_iter()
        .map(|row| (row.key.as_str().to_string(), row.cells))
        .collect()
}

proptest! {
    #[test]
    fn a_sheet_resorted_by_its_key_column_keeps_every_row_key(
        rows in prop::collection::vec(
            (prop::sample::select(vec!["A1", "A2", "b 3", "", "Ä4"]), "[a-z ]{0,8}"),
            1..30,
        )
    ) {
        let rows: Vec<(String, String)> =
            rows.into_iter().map(|(id, title)| (id.to_string(), title)).collect();
        let mut sorted = rows.clone();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        prop_assert_eq!(keyed(&csv(&rows)), keyed(&csv(&sorted)));
    }
}
