//! A small RFC 4180 CSV reader and writer.
//!
//! The workspace has no CSV crate, so both halves are here. Reading handles
//! quoted fields, doubled quotes inside a quoted field, embedded newlines and
//! delimiters, and both LF and CRLF record endings. The delimiter is a
//! parameter because an archive spreadsheet is often written with semicolons
//! or tabs. Writing quotes a field only when it needs it and ends every record
//! with LF; [`parse`] reads back what [`render`] wrote.

/// One CSV record with the line it started on (1-based).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub line: usize,
    pub fields: Vec<String>,
}

/// Input that is not CSV.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CsvError {
    /// A quoted field that the input never closes, by the line it opened on.
    #[error("line {line}: unterminated quoted field")]
    Unterminated { line: usize },
}

impl CsvError {
    /// The 1-based line the error is about.
    #[must_use]
    pub fn line(&self) -> usize {
        match self {
            Self::Unterminated { line } => *line,
        }
    }
}

/// Parse comma-separated input. Blank records are dropped.
///
/// # Errors
/// [`CsvError::Unterminated`] for a quoted field that is never closed.
pub fn parse(input: &str) -> Result<Vec<Record>, CsvError> {
    parse_with(input, ',')
}

/// Parse input separated by `delimiter`. Blank records are dropped.
///
/// # Errors
/// [`CsvError::Unterminated`] for a quoted field that is never closed.
pub fn parse_with(input: &str, delimiter: char) -> Result<Vec<Record>, CsvError> {
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
                Some(c) if *c == delimiter && !quoted => {
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
            return Err(CsvError::Unterminated { line: start_line });
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
#[must_use]
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
    #![allow(clippy::expect_used)]
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
        assert_eq!(error, CsvError::Unterminated { line: 2 });
    }

    #[test]
    fn a_semicolon_delimiter_keeps_commas_inside_a_cell() {
        let records = parse_with("id;title\nA1;Tin box, dented\n", ';').expect("parse");
        assert_eq!(records[1].fields, ["A1", "Tin box, dented"]);
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
