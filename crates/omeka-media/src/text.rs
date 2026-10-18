//! HTML and URL text helpers shared by the renderers.
//!
//! The renderers emit HTML fragments, so they need the same escaping the
//! upstream view helpers apply. Each function here mirrors one upstream helper
//! and cites it.

/// Escape text for HTML, matching `Laminas\Escaper\Escaper::escapeHtml`
/// (`htmlspecialchars` with `ENT_QUOTES | ENT_SUBSTITUTE | ENT_HTML5`).
///
/// Upstream calls this through `$view->escapeHtml()`, e.g.
/// `Media/Renderer/Youtube.php:38`.
#[must_use]
pub fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#039;"),
            other => out.push(other),
        }
    }
    out
}

/// Render HTML attributes, matching `Laminas\View\Helper\AbstractHtmlElement::htmlAttribs`.
///
/// A `true` value renders the attribute name as its own value
/// (`allowfullscreen="allowfullscreen"`); a `false` value is dropped. The port
/// carries attributes as ordered pairs so the emitted order matches the order
/// upstream builds them.
#[must_use]
pub fn html_attribs(attribs: &[(String, String)]) -> String {
    let mut out = String::new();
    for (key, value) in attribs {
        if value == "false" {
            continue;
        }
        if value == "true" {
            out.push_str(&format!(" {key}=\"{key}\""));
            continue;
        }
        out.push_str(&format!(" {key}=\"{}\"", escape_html(value)));
    }
    out
}

/// Render an HTML hyperlink, matching `Omeka\View\Helper\Hyperlink::__invoke`.
///
/// The text is escaped; the href is set last, after any caller attributes.
/// `aria-label` is filled from `title` when the link has no text, per the
/// helper's accessibility rule (`View/Helper/Hyperlink.php:38`).
#[must_use]
pub fn hyperlink(text: Option<&str>, href: Option<&str>, attribs: &[(String, String)]) -> String {
    hyperlink_raw(&escape_html(text.unwrap_or("")), href, attribs)
}

/// Render an HTML hyperlink with pre-escaped content, matching
/// `Omeka\View\Helper\Hyperlink::raw`.
#[must_use]
pub fn hyperlink_raw(html: &str, href: Option<&str>, attribs: &[(String, String)]) -> String {
    let mut all: Vec<(String, String)> = attribs.to_vec();
    all.push(("href".to_string(), href.unwrap_or("").to_string()));
    if html.is_empty()
        && let Some((_, title)) = all.iter().find(|(key, _)| key == "title")
    {
        let has_aria = all.iter().any(|(key, _)| key == "aria-label");
        if !has_aria {
            all.push(("aria-label".to_string(), title.clone()));
        }
    }
    format!("<a{}>{html}</a>", html_attribs(&all))
}

/// Percent-encode a string the way PHP's `urlencode` does (RFC 1738: spaces
/// become `+`).
///
/// Upstream uses this through `http_build_query` in `View/Helper/Url.php` and
/// `urlencode` in `Stdlib/Oembed.php:64`.
#[must_use]
pub fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(byte));
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// Lowercase hex encoding, matching PHP's `bin2hex`.
#[must_use]
pub fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Remove HTML tags, matching PHP's `strip_tags`.
///
/// PHP treats `<` as a tag start when a letter, `/`, `!` or `?` follows; the
/// port follows that rule so a bare `<` in text survives.
#[must_use]
pub fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let is_tag = after
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '/' || c == '!' || c == '?');
        if !is_tag {
            out.push('<');
            rest = after;
            continue;
        }
        match after.find('>') {
            Some(end) => rest = &after[end + 1..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Decode HTML entities, matching `html_entity_decode` with `ENT_HTML5`.
///
/// Handles the named entities the crate emits plus numeric references. A
/// reference the port does not know is left as written, which is what PHP does
/// for an unknown named entity.
#[must_use]
pub fn html_entity_decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        let Some(end) = after.find(';') else {
            out.push('&');
            rest = &after[1..];
            continue;
        };
        let entity = &after[1..end];
        match decode_entity(entity) {
            Some(decoded) => {
                out.push_str(&decoded);
                rest = &after[end + 1..];
            }
            None => {
                out.push('&');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn decode_entity(entity: &str) -> Option<String> {
    let named = match entity {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => "\u{a0}",
        _ => "",
    };
    if !named.is_empty() {
        return Some(named.to_string());
    }
    let digits = entity.strip_prefix('#')?;
    let code = match digits.strip_prefix(['x', 'X']) {
        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
        None => digits.parse::<u32>().ok()?,
    };
    char::from_u32(code).map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `escapeHtml` with `ENT_QUOTES`: the five entities PHP emits.
    #[test]
    fn escape_html_matches_laminas_escape_html() {
        assert_eq!(
            escape_html("a&b<c>d\"e'f"),
            "a&amp;b&lt;c&gt;d&quot;e&#039;f"
        );
        assert_eq!(escape_html("plain"), "plain");
    }

    /// `htmlAttribs` skips a false value and expands a true one.
    #[test]
    fn html_attribs_matches_laminas_html_attribs() {
        let attribs = vec![
            ("class".to_string(), "media".to_string()),
            ("hidden".to_string(), "true".to_string()),
            ("checked".to_string(), "false".to_string()),
            ("title".to_string(), "a&b".to_string()),
        ];
        assert_eq!(
            html_attribs(&attribs),
            " class=\"media\" hidden=\"hidden\" title=\"a&amp;b\""
        );
    }

    /// `Hyperlink::__invoke` escapes the text and appends `href`.
    #[test]
    fn hyperlink_matches_upstream_view_helper() {
        assert_eq!(
            hyperlink(Some("a&b"), Some("/media/1"), &[]),
            "<a href=\"/media/1\">a&amp;b</a>"
        );
    }

    /// `Hyperlink::raw` adds `aria-label` from `title` when the content is empty.
    #[test]
    fn hyperlink_raw_fills_aria_label_from_title() {
        let attribs = vec![("title".to_string(), "Thumb".to_string())];
        assert_eq!(
            hyperlink_raw("", Some("/x"), &attribs),
            "<a title=\"Thumb\" href=\"/x\" aria-label=\"Thumb\"></a>"
        );
    }

    /// PHP `urlencode`: unreserved characters pass, space becomes `+`.
    #[test]
    fn urlencode_matches_php_urlencode() {
        assert_eq!(urlencode("a b/c"), "a+b%2Fc");
        assert_eq!(urlencode("{\"a\":1}"), "%7B%22a%22%3A1%7D");
    }

    /// `bin2hex`.
    #[test]
    fn hex_encode_matches_php_bin2hex() {
        assert_eq!(hex_encode(&[0x00, 0xab, 0xff]), "00abff");
    }

    /// `strip_tags` drops tags and keeps text, and keeps a bare `<`.
    #[test]
    fn strip_tags_matches_php_strip_tags() {
        assert_eq!(strip_tags("<p>a <b>b</b></p>"), "a b");
        assert_eq!(strip_tags("a < b"), "a < b");
        assert_eq!(strip_tags("<!-- c -->x"), "x");
        assert_eq!(strip_tags("trailing <b"), "trailing ");
    }

    /// `html_entity_decode` decodes the named entities and numeric references.
    #[test]
    fn html_entity_decode_matches_php() {
        assert_eq!(html_entity_decode("a &amp; b &lt;c&gt;"), "a & b <c>");
        assert_eq!(html_entity_decode("&#039;"), "'");
        assert_eq!(html_entity_decode("&#x41;"), "A");
        assert_eq!(html_entity_decode("&unknown;"), "&unknown;");
        assert_eq!(html_entity_decode("a & b"), "a & b");
    }
}
