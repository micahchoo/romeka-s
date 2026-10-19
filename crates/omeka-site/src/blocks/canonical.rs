//! The canonical content form of a block's html, and the hash of it.
//!
//! An upstream admin save rewrites a stored `html` block through HTMLPurifier:
//! it pretty-prints, self-closes void elements, reorders attributes and drops
//! `rel`. A hash of the raw bytes therefore changes on every save even when no
//! content did. The canonical form keeps only what a reader sees, in order: the
//! visible text runs with their whitespace collapsed, each image's `src` and
//! `alt`, and each link's `href` with the text it wraps. Formatting, attribute
//! order, whitespace between elements and `rel`/`target` are dropped.
//!
//! The admin repeats this at `web/admin/src/pages/sites/editor/romeka.ts`,
//! through `DOMParser`. The generated registry carries a worked sample and both
//! sides test against it, so the two implementations cannot drift.
//!
//! The scanner is deliberately small. Its input is html this product generated
//! or an upstream purifier normalised, not arbitrary markup. Text inside
//! `script`, `style`, `template`, `head`, `title` and `noscript` is not visible
//! and is skipped. Named entities this product's escaping and the purifier emit
//! are decoded, plus numeric ones.

/// The separator between atoms in the canonical string.
const ATOM: char = '\u{1f}';

/// The separator inside one atom's fields.
const FIELD: char = '\u{1e}';

/// The canonical content string of `html`: the ordered atoms, joined.
#[must_use]
pub fn canonical_content(html: &str) -> String {
    let mut walker = Walker {
        html,
        pos: 0,
        skip: 0,
        open_a: false,
        atoms: Vec::new(),
    };
    walker.run();
    walker.atoms.join(&ATOM.to_string())
}

/// The hash `plain_hash` stores: FNV-1a 64 over the canonical content, as 16
/// lowercase hex digits.
#[must_use]
pub fn content_hash(html: &str) -> String {
    hex(fnv1a(canonical_content(html).as_bytes()))
}

/// The FNV-1a 64 hash of the raw bytes, as 16 lowercase hex digits.
///
/// This is the algorithm carrier version 1 stored. It is still computed to
/// accept a v1 hash that a write recorded before the canonical form existed.
/// A theme asset's URL version (`theme::ThemeAsset::version`) is the same
/// digest of the asset's bytes.
#[must_use]
pub fn byte_hash(html: &str) -> String {
    hex(fnv1a(html.as_bytes()))
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn hex(hash: u64) -> String {
    format!("{hash:016x}")
}

struct Walker<'a> {
    html: &'a str,
    pos: usize,
    /// How many hidden elements (script and friends) enclose the cursor.
    skip: usize,
    open_a: bool,
    atoms: Vec<String>,
}

impl Walker<'_> {
    fn run(&mut self) {
        while self.pos < self.html.len() {
            let rest = &self.html[self.pos..];
            if !rest.starts_with('<') {
                let next = rest.find('<').unwrap_or(rest.len());
                self.push_text(&rest[..next]);
                self.pos += next;
                continue;
            }
            if rest.starts_with("<!--") {
                self.pos += rest.find("-->").map_or(rest.len(), |end| end + 3);
                continue;
            }
            if rest.starts_with("<!") || rest.starts_with("<?") {
                self.pos += rest.find('>').map_or(rest.len(), |end| end + 1);
                continue;
            }
            match tag_end(rest) {
                Some(end) => {
                    self.tag(&rest[..end]);
                    self.pos += end;
                }
                None => {
                    self.push_text(rest);
                    self.pos = self.html.len();
                }
            }
        }
        if self.open_a {
            self.atoms.push("z".to_string());
        }
    }

    fn push_text(&mut self, text: &str) {
        if self.skip > 0 {
            return;
        }
        let collapsed = collapse(&decode_entities(text));
        if !collapsed.is_empty() {
            self.atoms.push(format!("t{}{}", FIELD, collapsed));
        }
    }

    fn tag(&mut self, tag: &str) {
        let inner = tag[1..tag.len() - 1].trim();
        let closing = inner.starts_with('/');
        let body = if closing {
            inner[1..].trim_start()
        } else {
            inner
        };
        let name_end = body
            .find(|ch: char| ch.is_ascii_whitespace() || ch == '/')
            .unwrap_or(body.len());
        let name = body[..name_end].to_ascii_lowercase();
        if name.is_empty() {
            return;
        }
        if is_hidden(&name) {
            if closing {
                self.skip = self.skip.saturating_sub(1);
            } else if !body.ends_with('/') {
                self.skip += 1;
            }
            return;
        }
        if closing {
            if name == "a" && self.skip == 0 && self.open_a {
                self.atoms.push("z".to_string());
                self.open_a = false;
            }
            return;
        }
        if self.skip > 0 {
            return;
        }
        let attrs = parse_attrs(body[name_end..].trim_end_matches('/'));
        match name.as_str() {
            "img" => {
                let src = attrs.get("src").map(String::as_str).unwrap_or("");
                let alt = attrs.get("alt").map(String::as_str).unwrap_or("");
                self.atoms
                    .push(format!("i{}{}{}{}", FIELD, src, FIELD, alt));
            }
            "a" => {
                if self.open_a {
                    self.atoms.push("z".to_string());
                }
                let href = attrs.get("href").map(String::as_str).unwrap_or("");
                self.atoms.push(format!("a{}{}", FIELD, href));
                self.open_a = true;
            }
            _ => {}
        }
    }
}

/// The index just past the `>` that closes the tag `rest`, `None` when no
/// closing angle bracket exists outside a quoted attribute value.
fn tag_end(rest: &str) -> Option<usize> {
    let bytes = rest.as_bytes();
    let mut quote: Option<u8> = None;
    for (index, byte) in bytes.iter().enumerate().skip(1) {
        match quote {
            Some(open) => {
                if *byte == open {
                    quote = None;
                }
            }
            None => match byte {
                b'"' | b'\'' => quote = Some(*byte),
                b'>' => return Some(index + 1),
                _ => {}
            },
        }
    }
    None
}

/// Whether an element's text is not visible and must be skipped.
fn is_hidden(name: &str) -> bool {
    matches!(
        name,
        "script" | "style" | "template" | "head" | "title" | "noscript"
    )
}

/// The attributes of a start tag's body, names lowercased and values decoded.
/// A name that repeats keeps its last value, which purified html cannot
/// produce.
fn parse_attrs(body: &str) -> std::collections::BTreeMap<String, String> {
    let mut attrs = std::collections::BTreeMap::new();
    let bytes = body.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let name_start = index;
        while index < bytes.len() && !is_attr_boundary(bytes[index]) {
            index += 1;
        }
        if index == name_start {
            index += 1;
            continue;
        }
        let name = body[name_start..index].to_ascii_lowercase();
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let mut value = String::new();
        if index < bytes.len() && bytes[index] == b'=' {
            index += 1;
            while index < bytes.len() && bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            if index < bytes.len() && (bytes[index] == b'"' || bytes[index] == b'\'') {
                let quote = bytes[index];
                index += 1;
                let start = index;
                while index < bytes.len() && bytes[index] != quote {
                    index += 1;
                }
                value = decode_entities(&body[start..index]);
                if index < bytes.len() {
                    index += 1;
                }
            } else {
                let start = index;
                while index < bytes.len()
                    && !bytes[index].is_ascii_whitespace()
                    && bytes[index] != b'/'
                {
                    index += 1;
                }
                value = decode_entities(&body[start..index]);
            }
        }
        attrs.insert(name, value);
    }
    attrs
}

fn is_attr_boundary(byte: u8) -> bool {
    byte.is_ascii_whitespace() || byte == b'=' || byte == b'/'
}

/// The characters collapsed to one space: ASCII whitespace and the
/// no-break space. Defined here and in `romeka.ts` so a run of them collapses
/// the same way on both sides.
fn collapsible(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{a0}')
}

/// Collapse each run of collapsible characters to one space and trim the ends.
fn collapse(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending = false;
    for ch in text.chars() {
        if collapsible(ch) {
            pending = true;
            continue;
        }
        if pending && !out.is_empty() {
            out.push(' ');
        }
        pending = false;
        out.push(ch);
    }
    out
}

/// Decode the entities this product's escaping and the purifier emit.
fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find('&') {
        out.push_str(&rest[..index]);
        let tail = &rest[index..];
        match entity_at(tail) {
            Some((decoded, len)) => {
                out.push_str(&decoded);
                rest = &tail[len..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The entity at the start of `s`, which begins with `&`, with its length.
fn entity_at(s: &str) -> Option<(String, usize)> {
    let semicolon = s.find(';')?;
    if semicolon > 32 {
        return None;
    }
    let name = &s[1..semicolon];
    let decoded = match name {
        "amp" => "&".to_string(),
        "lt" => "<".to_string(),
        "gt" => ">".to_string(),
        "quot" => "\"".to_string(),
        "apos" => "'".to_string(),
        "nbsp" => "\u{a0}".to_string(),
        _ => {
            let number = name.strip_prefix('#')?;
            let code = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse::<u32>().ok()?,
            };
            char::from_u32(code)?.to_string()
        }
    };
    Some((decoded, semicolon + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The port's plain rendering and the html an upstream admin save stores
    /// for it have the same canonical form: formatting, void self-closing,
    /// attribute order and `rel` are dropped.
    #[test]
    fn formatting_and_attribute_order_do_not_change_the_hash() {
        let plain = "<h2>Romeka &amp; Co</h2><p>Hello <em>world</em>.</p>\
                     <img src=\"/files/asset/a.png\" alt=\"A shelf\">\
                     <a href=\"/s/main/page/about\" rel=\"noreferrer noopener\">About</a>";
        let normalised = "<h2>Romeka &amp; Co</h2>\n\n<p>Hello <em>world</em>.</p>\n\n\
                          <p><img alt=\"A shelf\" src=\"/files/asset/a.png\" />\
                          <a href=\"/s/main/page/about\" target=\"_blank\">About</a></p>\n";
        assert_eq!(content_hash(plain), content_hash(normalised));
    }

    /// Entities a purifier decodes or re-encodes do not change the hash.
    #[test]
    fn entity_spelling_does_not_change_the_hash() {
        assert_eq!(
            content_hash("<p>a &amp; b</p>"),
            content_hash("<p>a &#38; b</p>")
        );
        assert_eq!(
            content_hash("<p>it&#039;s</p>"),
            content_hash("<p>it's</p>")
        );
        assert_eq!(
            content_hash("<p>a &amp;lt; b</p>"),
            content_hash("<p>a &#38;lt; b</p>")
        );
        assert_eq!(
            content_hash("<a href=\"/x?a=1&amp;b=2\">Go</a>"),
            content_hash("<a href=\"/x?a=1&b=2\">Go</a>")
        );
    }

    /// Whitespace runs collapse; a tag still separates two text runs, so a
    /// paragraph break is not the same as a space.
    #[test]
    fn whitespace_collapses_between_and_inside_runs() {
        assert_eq!(
            content_hash("<p>one\n\n\ttwo</p>"),
            content_hash("<p>one two</p>")
        );
        assert_ne!(
            content_hash("<p>one</p><p>two</p>"),
            content_hash("<p>onetwo</p>")
        );
        assert_eq!(
            content_hash("<p>a</p>\n\n<p>b</p>"),
            content_hash("<p>a</p><p>b</p>")
        );
    }

    /// Invisible elements contribute nothing.
    #[test]
    fn hidden_elements_are_ignored() {
        assert_eq!(
            content_hash("<p>ok</p><script>alert(1)</script>"),
            content_hash("<p>ok</p>")
        );
        assert_eq!(
            content_hash("<head><title>t</title></head><p>ok</p>"),
            content_hash("<p>ok</p>")
        );
    }

    /// A changed word, image source or link href changes the hash.
    #[test]
    fn content_changes_change_the_hash() {
        let base = "<h2>Welcome</h2><img src=\"/a.png\" alt=\"A\"><a href=\"/one\">Go</a>";
        for changed in [
            "<h2>Welcom</h2><img src=\"/a.png\" alt=\"A\"><a href=\"/one\">Go</a>",
            "<h2>Welcome</h2><img src=\"/b.png\" alt=\"A\"><a href=\"/one\">Go</a>",
            "<h2>Welcome</h2><img src=\"/a.png\" alt=\"A\"><a href=\"/two\">Go</a>",
            "<h2>Welcome</h2><img src=\"/a.png\" alt=\"B\"><a href=\"/one\">Go</a>",
            "<h2>Welcome</h2><img src=\"/a.png\" alt=\"A\"><a href=\"/one\">Stop</a>",
        ] {
            assert_ne!(content_hash(base), content_hash(changed), "{changed}");
        }
    }

    /// The port's plain rendering and the html the upstream admin left behind,
    /// captured from the oracle round trip on 2026-10-18. They differ byte for
    /// byte (blank lines, a self-closed void, a moved attribute, a dropped
    /// `rel`) and canonicalise to the same content, which is why the port
    /// renders the rich hero again after an upstream save.
    #[test]
    fn the_oracle_normalised_html_hashes_equal_to_the_port_plain_html() {
        let plain = "<h2>Welcome from Romeka</h2><p>A subtitle</p>\
                     <p>Body copy with <em>emphasis</em>.</p>\
                     <img src=\"/files/asset/proof.png\" alt=\"A proof image\">\
                     <a href=\"https://example.org/browse\" rel=\"noreferrer noopener\">Browse</a>";
        let normalised = "<h2>Welcome from Romeka</h2>\n\n<p>A subtitle</p>\n\n\
                          <p>Body copy with <em>emphasis</em>.</p>\n\n\
                          <p><img alt=\"A proof image\" src=\"/files/asset/proof.png\" />\
                          <a href=\"https://example.org/browse\">Browse</a></p>\n\n";
        assert_ne!(plain, normalised);
        assert_eq!(content_hash(plain), content_hash(normalised));
    }

    /// The legacy byte hash is the raw-byte FNV-1a, so formatting changes it.
    /// This is what a carrier version 1 stored.
    #[test]
    fn the_byte_hash_sees_formatting() {
        assert_eq!(byte_hash(""), "cbf29ce484222325");
        assert_eq!(byte_hash("a"), "af63dc4c8601ec8c");
        assert_ne!(byte_hash("<p>a</p>"), byte_hash("<p>a</p>\n"));
    }

    /// The canonical form is the atom sequence, so a reader can see what was
    /// hashed.
    #[test]
    fn canonical_content_lists_the_visible_atoms() {
        assert_eq!(
            canonical_content("<h2>A</h2> <p>B</p>"),
            "t\u{1e}A\u{1f}t\u{1e}B"
        );
        assert_eq!(
            canonical_content("<a href=\"/x\">Go</a>"),
            "a\u{1e}/x\u{1f}t\u{1e}Go\u{1f}z"
        );
        assert_eq!(
            canonical_content("<img src=\"s\" alt=\"a\">"),
            "i\u{1e}s\u{1e}a"
        );
    }
}
