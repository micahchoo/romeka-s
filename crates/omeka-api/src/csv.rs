//! The one CSV writer: the import's lists and the items export both write their records here.

/// Starts every CSV file Romeka writes, so a spreadsheet reads Devanagari right.
pub const BYTE_ORDER_MARK: char = '\u{feff}';

/// Append one RFC 4180 record and its CRLF.
///
/// A field is quoted when it holds a comma, a double quote or a line ending; a double quote inside
/// a quoted field is doubled. Every other field is written as it stands, so a plain file stays
/// readable.
pub fn push_line(out: &mut String, cells: &[String]) {
    for (at, cell) in cells.iter().enumerate() {
        if at > 0 {
            out.push(',');
        }
        if cell.contains([',', '"', '\r', '\n']) {
            out.push('"');
            out.push_str(&cell.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(cell);
        }
    }
    out.push_str("\r\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(cells: &[&str]) -> String {
        let mut out = String::new();
        push_line(
            &mut out,
            &cells.iter().map(ToString::to_string).collect::<Vec<_>>(),
        );
        out
    }

    #[test]
    fn quotes_only_a_field_that_needs_it() {
        assert_eq!(
            line(&["dcterms:title", "x,y", "say \"hi\"", "two\nlines"]),
            "dcterms:title,\"x,y\",\"say \"\"hi\"\"\",\"two\nlines\"\r\n"
        );
    }

    #[test]
    fn an_empty_cell_stays_an_empty_cell() {
        assert_eq!(line(&["a", "", "c"]), "a,,c\r\n");
    }
}
