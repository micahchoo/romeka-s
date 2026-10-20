//! Records of delimited text: one quote rule, three line ends.
//!
//! A record is RFC 4180 with the plan's delimiter and quote: a quoted field
//! may hold delimiters, line ends and a doubled quote. CR, LF and CRLF all end
//! a line. A quote that never closes, or text after a closing quote, makes
//! the record *broken*; the sheet then reads only that record's first line
//! and goes on at the next, so one stray quote never moves later rows.

/// The delimiter and the quote, `None` for no quoting.
#[derive(Debug, Clone, Copy)]
pub(super) struct Syntax {
    pub delimiter: char,
    pub quote: Option<char>,
}

pub(super) struct Record {
    pub fields: Vec<String>,
    /// The byte where the next record starts.
    pub end: usize,
    /// Line ends inside quoted fields.
    pub embedded: u32,
    pub broken: bool,
}

/// The record that starts at byte `start`.
pub(super) fn record_at(text: &str, start: usize, syntax: Syntax) -> Record {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut started = false;
    let mut quoted = false;
    let mut closed = false;
    let mut embedded = 0;
    let mut chars = text[start..].char_indices().peekable();
    let end = loop {
        let Some((at, c)) = chars.next() else {
            fields.push(field);
            return Record {
                fields,
                end: text.len(),
                embedded,
                broken: quoted,
            };
        };
        if quoted {
            if Some(c) == syntax.quote {
                if chars.peek().map(|(_, next)| *next) == syntax.quote {
                    chars.next();
                    field.push(c);
                } else {
                    quoted = false;
                    closed = true;
                }
            } else {
                if c == '\n' || (c == '\r' && chars.peek().map(|(_, next)| *next) != Some('\n')) {
                    embedded += 1;
                }
                field.push(c);
            }
            continue;
        }
        if c == syntax.delimiter {
            fields.push(std::mem::take(&mut field));
            started = false;
            closed = false;
            continue;
        }
        if c == '\n' || c == '\r' {
            let mut next = start + at + 1;
            if c == '\r' && chars.peek().map(|(_, next)| *next) == Some('\n') {
                next += 1;
            }
            break next;
        }
        if closed {
            fields.push(field);
            return Record {
                fields,
                end: start + at,
                embedded,
                broken: true,
            };
        }
        if !started && Some(c) == syntax.quote {
            quoted = true;
        } else {
            field.push(c);
        }
        started = true;
    };
    fields.push(field);
    Record {
        fields,
        end,
        embedded,
        broken: false,
    }
}

/// The physical line at byte `start`, without its line end, and the byte
/// where the next line starts.
pub(super) fn line_at(text: &str, start: usize) -> (&str, usize) {
    let rest = &text[start..];
    match rest.find(['\n', '\r']) {
        None => (rest, text.len()),
        Some(at) => {
            let skip = if rest[at..].starts_with("\r\n") { 2 } else { 1 };
            (&rest[..at], start + at + skip)
        }
    }
}

/// The byte where line `line` (1-based) starts, or `None` past the end.
pub(super) fn line_start(text: &str, line: u32) -> Option<usize> {
    let mut start = 0;
    for _ in 1..line {
        if start >= text.len() {
            return None;
        }
        start = line_at(text, start).1;
    }
    (start < text.len() || line == 1).then_some(start)
}

/// A broken record's first line, cut at every delimiter, quotes kept.
pub(super) fn split_plain(line: &str, delimiter: char) -> Vec<String> {
    line.split(delimiter).map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: Syntax = Syntax {
        delimiter: ',',
        quote: Some('"'),
    };

    fn fields(text: &str) -> (Vec<String>, bool, u32) {
        let record = record_at(text, 0, CSV);
        (record.fields, record.broken, record.embedded)
    }

    #[test]
    fn quoted_fields_hold_delimiters_line_ends_and_doubled_quotes() {
        let (cells, broken, lines) = fields("1,\"x,y\",\"say \"\"hi\"\"\"\n2");
        assert_eq!(cells, ["1", "x,y", "say \"hi\""]);
        assert!(!broken);
        assert_eq!(lines, 0);
        let (cells, _, lines) = fields("\"a\r\nb\",c\rnext");
        assert_eq!(cells, ["a\r\nb", "c"]);
        assert_eq!(lines, 1);
    }

    #[test]
    fn an_unclosed_quote_or_text_after_a_quote_is_broken() {
        assert!(fields("1,\"open\n2,b\n").1);
        assert!(fields("1,\"shut\"tail,3\n").1);
        assert!(
            !fields("1,in\"side,3\n").1,
            "a quote inside a field is text"
        );
    }

    #[test]
    fn lines_end_in_cr_lf_or_crlf() {
        let text = "a\rb\nc\r\nd";
        assert_eq!(line_at(text, 0), ("a", 2));
        assert_eq!(line_at(text, 2), ("b", 4));
        assert_eq!(line_at(text, 4), ("c", 7));
        assert_eq!(line_at(text, 7), ("d", 8));
        assert_eq!(line_start(text, 4), Some(7));
        assert_eq!(line_start(text, 5), None);
    }
}
