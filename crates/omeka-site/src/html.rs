//! HTML text and attribute output, matching the Laminas view helpers upstream
//! uses in every site view.
//!
//! `escape_html` mirrors `Laminas\View\Helper\EscapeHtml`, which calls
//! `htmlspecialchars($value, ENT_QUOTES | ENT_SUBSTITUTE, 'UTF-8')`. No
//! `ENT_HTML5` flag, so a single quote becomes `&#039;` (PHP's HTML 4.01
//! table), and an invalid UTF-8 sequence becomes U+FFFD. The exact table is
//! inferred from PHP's documented behaviour, not read from the vendor tree
//! (upstream ships no `vendor/` in this checkout).
//!
//! `hyperlink` mirrors `Laminas\View\Helper\Hyperlink`: the text is escaped,
//! the href and every attribute value are escaped, and a `null` `href` renders
//! an empty `href` attribute.

/// Escape text for HTML body or attribute context.
#[must_use]
pub fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#039;"),
            _ => out.push(ch),
        }
    }
    out
}

/// One HTML attribute. `None` renders nothing, matching the Laminas helper that
/// drops null attributes.
#[derive(Debug, Clone, Copy)]
pub struct Attr<'a> {
    name: &'a str,
    value: AttrValue<'a>,
}

#[derive(Debug, Clone, Copy)]
enum AttrValue<'a> {
    Text(&'a str),
    /// A boolean attribute; `true` renders `name="name"`.
    Bool(bool),
}

impl<'a> Attr<'a> {
    /// A text-valued attribute.
    #[must_use]
    pub const fn text(name: &'a str, value: &'a str) -> Self {
        Self {
            name,
            value: AttrValue::Text(value),
        }
    }

    /// A boolean attribute, rendered as `name="name"` when true.
    #[must_use]
    pub const fn flag(name: &'a str, value: bool) -> Self {
        Self {
            name,
            value: AttrValue::Bool(value),
        }
    }

    fn write(&self, out: &mut String) {
        match self.value {
            AttrValue::Text(value) => {
                out.push(' ');
                out.push_str(self.name);
                out.push_str("=\"");
                out.push_str(&escape_html(value));
                out.push('"');
            }
            AttrValue::Bool(true) => {
                out.push(' ');
                out.push_str(self.name);
                out.push_str("=\"");
                out.push_str(self.name);
                out.push('"');
            }
            AttrValue::Bool(false) => {}
        }
    }
}

/// Render an opening tag with attributes, e.g. `<a href="/s/main">`.
#[must_use]
pub fn open_tag(name: &str, attrs: &[Attr<'_>]) -> String {
    let mut out = String::new();
    out.push('<');
    out.push_str(name);
    for attr in attrs {
        attr.write(&mut out);
    }
    out.push('>');
    out
}

/// Render `text` as an element, nothing when `text` is empty, matching the
/// upstream templates that guard each optional element.
#[must_use]
pub fn element(name: &str, attrs: &[Attr<'_>], body: &str) -> String {
    if body.is_empty() && attrs.is_empty() {
        return String::new();
    }
    format!("{}{}</{}>", open_tag(name, attrs), body, name)
}

/// `Laminas\View\Helper\Hyperlink`: an `<a>` with escaped text and href.
#[must_use]
pub fn hyperlink(text: &str, href: Option<&str>, extra: &[Attr<'_>]) -> String {
    let escaped = escape_html(text);
    let mut attrs = Vec::with_capacity(extra.len() + 1);
    attrs.push(Attr::text("href", href.unwrap_or("")));
    attrs.extend_from_slice(extra);
    format!("{}{}</a>", open_tag("a", &attrs), escaped)
}

/// Percent-encode one query element the way PHP's `http_build_query` does for
/// the keys and values the site layer emits (`urlencode`, spaces as `+`).
///
/// PHP's `urlencode` passes only `A-Z a-z 0-9 - _ .`; everything else,
/// including `~`, becomes `%XX` with uppercase hex digits.
#[must_use]
pub fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
                out.push(char::from(byte));
            }
            b' ' => out.push('+'),
            other => {
                out.push('%');
                out.push(
                    char::from_digit(u32::from(other >> 4), 16)
                        .unwrap_or('0')
                        .to_ascii_uppercase(),
                );
                out.push(
                    char::from_digit(u32::from(other & 0x0f), 16)
                        .unwrap_or('0')
                        .to_ascii_uppercase(),
                );
            }
        }
    }
    out
}

/// Build a query string from ordered pairs, `None` when there are no pairs.
#[must_use]
pub fn query_string(pairs: &[(String, String)]) -> Option<String> {
    if pairs.is_empty() {
        return None;
    }
    let encoded: Vec<String> = pairs
        .iter()
        .map(|(key, value)| format!("{}={}", urlencode(key), urlencode(value)))
        .collect();
    Some(encoded.join("&"))
}

/// Page links for a result set, the markup the pagination helper renders.
///
/// `Laminas`'s `pagination` helper wraps its links in a `nav` and carries
/// per-link classes the port does not reproduce. Each link is
/// `{current_path}?{other query pairs}&page=N`, so a rendered page keeps its
/// other filters. An empty string means one page.
#[must_use]
pub fn pagination(
    current_path: &str,
    query: &[(String, String)],
    page: usize,
    per_page: usize,
    total: usize,
) -> String {
    if per_page == 0 {
        return String::new();
    }
    let pages = total.div_ceil(per_page);
    if pages <= 1 {
        return String::new();
    }
    let mut out = String::from("<nav class=\"pagination\"><ul>");
    for number in 1..=pages {
        let pairs: Vec<(String, String)> = query
            .iter()
            .filter(|(key, _)| key != "page")
            .cloned()
            .chain(std::iter::once(("page".to_string(), number.to_string())))
            .collect();
        let href = match query_string(&pairs) {
            Some(query) => format!("{current_path}?{query}"),
            None => current_path.to_string(),
        };
        if number == page {
            out.push_str(&format!("<li class=\"active\">{number}</li>"));
        } else {
            out.push_str(&format!(
                "<li>{}</li>",
                hyperlink(&number.to_string(), Some(&href), &[])
            ));
        }
    }
    out.push_str("</ul></nav>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `EscapeHtml` escapes `& < > " '`, and only those.
    #[test]
    fn escape_html_matches_laminas_escape_html_helper() {
        assert_eq!(
            escape_html("<p>Tom & \"Jerry\" it's <b>fine</b></p>"),
            "&lt;p&gt;Tom &amp; &quot;Jerry&quot; it&#039;s &lt;b&gt;fine&lt;/b&gt;&lt;/p&gt;"
        );
        // Only the five characters change; UTF-8 passes through.
        assert_eq!(escape_html("naïve — café"), "naïve — café");
    }

    /// `Hyperlink` escapes the text and every attribute value.
    #[test]
    fn hyperlink_matches_laminas_hyperlink_helper() {
        let html = hyperlink(
            "A & B",
            Some("/s/main?a=1&b=2"),
            &[Attr::text("class", "x\"y")],
        );
        assert_eq!(
            html,
            "<a href=\"/s/main?a=1&amp;b=2\" class=\"x&quot;y\">A &amp; B</a>"
        );
        assert_eq!(hyperlink("Home", None, &[]), "<a href=\"\">Home</a>");
    }

    /// A boolean attribute renders only when true, as Laminas `htmlAttribs` does.
    #[test]
    fn boolean_attribute_renders_only_when_true() {
        assert_eq!(
            open_tag(
                "input",
                &[Attr::flag("checked", true), Attr::flag("disabled", false)]
            ),
            "<input checked=\"checked\">"
        );
    }

    /// `urlencode` follows PHP: only `A-Za-z0-9-_.` pass, a space is `+`.
    #[test]
    fn urlencode_matches_php_urlencode() {
        assert_eq!(urlencode("site_id=1&a b~c"), "site_id%3D1%26a+b%7Ec");
    }
}
