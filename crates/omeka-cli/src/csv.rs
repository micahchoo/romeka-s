//! The CSV writer for `romeka export csv`.

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
    use super::*;

    #[test]
    fn renders_plain_and_quoted_fields() {
        let rows = vec![
            vec!["dcterms:title".to_string(), "dcterms:creator".to_string()],
            vec!["x,y".to_string(), "say \"hi\"".to_string()],
        ];
        assert_eq!(
            render(&rows),
            "dcterms:title,dcterms:creator\n\"x,y\",\"say \"\"hi\"\"\"\n"
        );
    }

    #[test]
    fn an_empty_cell_stays_an_empty_cell() {
        let text = render(&[vec!["a".to_string(), String::new(), "c".to_string()]]);
        assert_eq!(text, "a,,c\n");
    }
}
