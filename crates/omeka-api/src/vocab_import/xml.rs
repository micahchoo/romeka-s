//! Two XML 1.0 rules oxrdfxml does not apply, and that upstream's own vocabulary files rely on.
//!
//! - An `<!ENTITY>` value may be quoted with `'` (XML 1.0 §4.2.2). `dcterms.rdf` declares its
//!   entities that way; oxrdfxml reads only `"`.
//! - A tab, line feed or carriage return inside an attribute value is read as a space, and a
//!   CR LF pair as one space (§3.3.3, attribute-value normalisation). `foaf.rdf` writes multi-line
//!   comments as attributes; PHP's XML parser, and so upstream, stores them on one line.
//!
//! [`conform`] rewrites a document so the parser sees what an XML 1.0 processor would. Text,
//! comments, CDATA sections and processing instructions pass through unchanged.

/// The document with both rules applied.
pub(super) fn conform(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let rest = &bytes[at..];
        let opaque = [
            (&b"<!--"[..], &b"-->"[..]),
            (b"<![CDATA[", b"]]>"),
            (b"<?", b"?>"),
        ]
        .into_iter()
        .find(|(open, _)| rest.starts_with(open));
        if let Some((open, close)) = opaque {
            let end = find(&rest[open.len()..], close)
                .map_or(rest.len(), |found| open.len() + found + close.len());
            out.extend_from_slice(&rest[..end]);
            at += end;
        } else if rest.starts_with(b"<!DOCTYPE") {
            at += doctype(rest, &mut out);
        } else if rest.starts_with(b"<") {
            at += tag(rest, &mut out);
        } else {
            out.push(rest[0]);
            at += 1;
        }
    }
    out
}

/// Copy a start or end tag, normalising the whitespace inside its quoted attribute values.
/// Returns the bytes consumed.
fn tag(rest: &[u8], out: &mut Vec<u8>) -> usize {
    let mut quote: Option<u8> = None;
    let mut at = 0;
    while at < rest.len() {
        let byte = rest[at];
        at += 1;
        match quote {
            Some(open) if byte == open => {
                quote = None;
                out.push(byte);
            }
            Some(_) if byte == b'\r' && rest.get(at) == Some(&b'\n') => {
                out.push(b' ');
                at += 1;
            }
            Some(_) if matches!(byte, b'\t' | b'\n' | b'\r') => out.push(b' '),
            Some(_) => out.push(byte),
            None if byte == b'"' || byte == b'\'' => {
                quote = Some(byte);
                out.push(byte);
            }
            None => {
                out.push(byte);
                if byte == b'>' {
                    break;
                }
            }
        }
    }
    at
}

/// Copy a `<!DOCTYPE>` declaration, double-quoting each single-quoted `<!ENTITY>` value of its
/// internal subset that holds no `"`. Returns the bytes consumed.
fn doctype(rest: &[u8], out: &mut Vec<u8>) -> usize {
    let first_close = rest
        .iter()
        .position(|byte| *byte == b'>')
        .map_or(rest.len(), |at| at + 1);
    let subset = rest[..first_close]
        .iter()
        .position(|byte| *byte == b'[')
        .and_then(|open| find(&rest[open..], b"]>").map(|close| (open + 1, open + close)));
    let Some((start, end)) = subset else {
        out.extend_from_slice(&rest[..first_close]);
        return first_close;
    };
    let copied = out.len();
    out.extend_from_slice(&rest[..end + 2]);
    quote_entities(&mut out[copied + start..copied + end]);
    end + 2
}

/// Double-quote each single-quoted `<!ENTITY>` value in an internal subset.
fn quote_entities(subset: &mut [u8]) {
    let skip_space = |cursor: &mut usize, subset: &[u8]| {
        while subset.get(*cursor).is_some_and(u8::is_ascii_whitespace) {
            *cursor += 1;
        }
    };
    let mut at = 0;
    while let Some(found) = find(&subset[at..], b"<!ENTITY") {
        let mut cursor = at + found + b"<!ENTITY".len();
        skip_space(&mut cursor, subset);
        if subset.get(cursor) == Some(&b'%') {
            cursor += 1;
            skip_space(&mut cursor, subset);
        }
        while subset
            .get(cursor)
            .is_some_and(|byte| !byte.is_ascii_whitespace())
        {
            cursor += 1;
        }
        skip_space(&mut cursor, subset);
        if subset.get(cursor) == Some(&b'\'')
            && let Some(length) = subset[cursor + 1..].iter().position(|byte| *byte == b'\'')
        {
            let close = cursor + 1 + length;
            if !subset[cursor + 1..close].contains(&b'"') {
                subset[cursor] = b'"';
                subset[close] = b'"';
            }
            cursor = close;
        }
        at = cursor + 1;
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conformed(text: &str) -> String {
        String::from_utf8(conform(text.as_bytes())).unwrap_or_default()
    }

    #[test]
    fn single_quoted_entity_values_are_double_quoted() {
        assert_eq!(
            conformed(
                "<!DOCTYPE rdf:RDF [\n <!ENTITY ns 'http://e/'>\n <!ENTITY q 'say \"x\"'>\n]>\n<a/>"
            ),
            "<!DOCTYPE rdf:RDF [\n <!ENTITY ns \"http://e/\">\n <!ENTITY q 'say \"x\"'>\n]>\n<a/>"
        );
    }

    #[test]
    fn attribute_whitespace_becomes_spaces_and_text_is_untouched() {
        assert_eq!(
            conformed("<a b=\"one\r\ntwo\tthree\nfour\">x\ny</a><!-- c\nd -->"),
            "<a b=\"one two three four\">x\ny</a><!-- c\nd -->"
        );
    }
}
