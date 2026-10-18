//! A small RFC 4180 CSV reader and writer for `omeka import csv` and
//! `omeka export csv`.
//!
//! The workspace has no CSV crate and the CLI may not fetch one, so both halves
//! are here. Reading handles quoted fields, doubled quotes inside a quoted
//! field, embedded newlines and commas, and both LF and CRLF record endings.
//! Input is external, so a malformed file returns a [`CliError::Parse`] naming
//! the record it broke on. Writing quotes a field only when it needs it and
//! ends every record with LF; [`parse`] reads back what [`render`] wrote.

use crate::error::CliError;

/// One CSV record with the line it started on (1-based).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub line: usize,
    pub fields: Vec<String>,
}

/// Parse the whole input. Blank records are dropped.
///
/// # Errors
/// A [`CliError::Parse`] for an unterminated quoted field.
pub fn parse(input: &str) -> Result<Vec<Record>, CliError> {
    let characters: Vec<char> = input.chars().collect();
    let mut records = Vec::new();
    let mut index = 0;
    let mut line = 1;

    while index < characters.len() {
        let start_line = line;
        let mut fields = Vec::new();
        let mut field = String::new();
        let mut quoted = false;
        let mut at_record_end = false;

        while !at_record_end {
            match characters.get(index) {
                None => break,
                Some('"') if quoted && characters.get(index + 1) == Some(&'"') => {
                    field.push('"');
                    index += 2;
                }
                Some('"') if quoted => {
                    quoted = false;
                    index += 1;
                }
                Some('"') if field.is_empty() => {
                    quoted = true;
                    index += 1;
                }
                Some(',') if !quoted => {
                    fields.push(std::mem::take(&mut field));
                    index += 1;
                }
                Some('\r') if !quoted => index += 1,
                Some('\n') if !quoted => {
                    index += 1;
                    line += 1;
                    at_record_end = true;
                }
                Some('\n') => {
                    field.push('\n');
                    index += 1;
                    line += 1;
                }
                Some(character) => {
                    field.push(*character);
                    index += 1;
                }
            }
        }

        if quoted {
            return Err(CliError::Parse {
                line: start_line,
                message: "unterminated quoted field".to_string(),
            });
        }
        fields.push(field);
        if fields.len() == 1 && fields[0].is_empty() {
            continue;
        }
        records.push(Record {
            line: start_line,
            fields,
        });
    }
    Ok(records)
}

/// Render rows as an RFC 4180 CSV document, one LF per record.
///
/// A field is quoted when it holds a comma, a double quote or a line ending; a
/// double quote inside a quoted field is doubled. Every other field is written
/// as it stands, so a plain export stays readable.
pub fn render(rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    for row in rows {
        for (index, field) in row.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            write_field(&mut out, field);
        }
        out.push('\n');
    }
    out
}

fn write_field(out: &mut String, field: &str) {
    if field.contains([',', '"', '\n', '\r']) {
        out.push('"');
        for character in field.chars() {
            if character == '"' {
                out.push('"');
            }
            out.push(character);
        }
        out.push('"');
    } else {
        out.push_str(field);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_quoted_fields() {
        let records = parse("a,b,c\n1,\"x,y\",\"say \"\"hi\"\"\"\n").expect("parse");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].fields, ["a", "b", "c"]);
        assert_eq!(records[1].fields, ["1", "x,y", "say \"hi\""]);
        assert_eq!(records[1].line, 2);
    }

    #[test]
    fn handles_crlf_and_embedded_newlines() {
        let records = parse("h1,h2\r\n\"a\nb\",c\r\n").expect("parse");
        assert_eq!(records.len(), 2);
        assert_eq!(records[1].fields, ["a\nb", "c"]);
    }

    #[test]
    fn rejects_an_unterminated_quote() {
        let error = parse("a,b\n1,\"oops\n").expect_err("unterminated");
        assert!(
            matches!(error, CliError::Parse { line: 2, .. }),
            "got {error:?}"
        );
    }

    #[test]
    fn renders_plain_and_quoted_fields_that_parse_back() {
        let rows = vec![
            vec!["dcterms:title".to_string(), "dcterms:creator".to_string()],
            vec!["x,y".to_string(), "say \"hi\"".to_string()],
        ];
        let text = render(&rows);
        assert_eq!(
            text,
            "dcterms:title,dcterms:creator\n\"x,y\",\"say \"\"hi\"\"\"\n"
        );
        let back = parse(&text).expect("parse");
        assert_eq!(back.len(), 2);
        assert_eq!(back[1].fields, ["x,y", "say \"hi\""]);
    }

    #[test]
    fn an_empty_cell_stays_an_empty_cell() {
        let text = render(&[vec!["a".to_string(), String::new(), "c".to_string()]]);
        assert_eq!(text, "a,,c\n");
        let back = parse(&text).expect("parse");
        assert_eq!(back[0].fields, ["a", "", "c"]);
    }
}
