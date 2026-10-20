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

use crate::i18n::{Messages, sprintf};

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

/// Page links for a result set: Previous, a window of page numbers, Next,
/// and a summary line.
///
/// Upstream's `pagination` helper prints a link per page, which is 38 links
/// for 950 items. The port prints the first and the last page, the current
/// page with one neighbour each side (three numbers at either end), and an
/// ellipsis for each gap of more than one page. Each link is
/// `{current_path}?{other query pairs}&page=N`, so a rendered page keeps its
/// other filters. An empty string means one page.
#[must_use]
pub fn pagination(
    current_path: &str,
    query: &[(String, String)],
    page: usize,
    per_page: usize,
    total: usize,
    messages: &Messages,
) -> String {
    if per_page == 0 {
        return String::new();
    }
    let pages = total.div_ceil(per_page);
    if pages <= 1 {
        return String::new();
    }
    let page = page.clamp(1, pages);
    let href = |number: usize| -> String {
        let pairs: Vec<(String, String)> = query
            .iter()
            .filter(|(key, _)| key != "page")
            .cloned()
            .chain(std::iter::once(("page".to_string(), number.to_string())))
            .collect();
        match query_string(&pairs) {
            Some(query) => format!("{current_path}?{query}"),
            None => current_path.to_string(),
        }
    };
    let mut out = String::from("<nav class=\"pagination\"><ul>");
    if page > 1 {
        out.push_str(&format!(
            "<li class=\"pager-prev\">{}</li>",
            hyperlink(
                messages.prev,
                Some(&href(page - 1)),
                &[Attr::text("rel", "prev")]
            )
        ));
    } else {
        out.push_str(&format!(
            "<li class=\"pager-prev\" aria-disabled=\"true\"><span>{}</span></li>",
            escape_html(messages.prev)
        ));
    }
    let mut last_shown = 0;
    for number in window(page, pages) {
        if number > last_shown + 1 {
            out.push_str("<li class=\"pager-gap\" aria-hidden=\"true\">…</li>");
        }
        if number == page {
            out.push_str(&format!(
                "<li class=\"active\" aria-current=\"page\">{number}</li>"
            ));
        } else {
            out.push_str(&format!(
                "<li>{}</li>",
                hyperlink(&number.to_string(), Some(&href(number)), &[])
            ));
        }
        last_shown = number;
    }
    if page < pages {
        out.push_str(&format!(
            "<li class=\"pager-next\">{}</li>",
            hyperlink(
                messages.next,
                Some(&href(page + 1)),
                &[Attr::text("rel", "next")]
            )
        ));
    } else {
        out.push_str(&format!(
            "<li class=\"pager-next\" aria-disabled=\"true\"><span>{}</span></li>",
            escape_html(messages.next)
        ));
    }
    out.push_str("</ul>");
    out.push_str(&format!(
        "<p class=\"pager-count\">{}</p></nav>",
        escape_html(&sprintf(
            messages.pager_count,
            &[&total.to_string(), &page.to_string(), &pages.to_string()]
        ))
    ));
    out
}

/// The page numbers the pager shows, ascending: the first, the last, and the
/// current page with a neighbour each side, widened to three numbers at
/// either end. A gap of exactly one page shows that page, since an ellipsis
/// would take the same room.
fn window(page: usize, pages: usize) -> Vec<usize> {
    let (low, high) = match page {
        1 => (1, 3),
        _ if page == pages => (pages.saturating_sub(2), pages),
        _ => (page - 1, page + 1),
    };
    let mut shown: Vec<usize> = (1..=pages)
        .filter(|&n| n == 1 || n == pages || (low..=high).contains(&n))
        .collect();
    let mut index = 1;
    while index < shown.len() {
        if shown[index] == shown[index - 1] + 2 {
            shown.insert(index, shown[index - 1] + 1);
        }
        index += 1;
    }
    shown
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

    /// The pager shows a window, not every page: 38 pages from page 1 read
    /// Previous, 1 2 3 … 38, Next, with the count below.
    #[test]
    fn pagination_shows_a_window_of_pages() {
        let messages = &crate::i18n::EN;
        let html = pagination("/s/main/item", &[], 1, 25, 950, messages);
        assert!(html.starts_with("<nav class=\"pagination\"><ul><li class=\"pager-prev\" aria-disabled=\"true\"><span>Prev</span></li><li class=\"active\" aria-current=\"page\">1</li><li><a href=\"/s/main/item?page=2\">2</a></li><li><a href=\"/s/main/item?page=3\">3</a></li><li class=\"pager-gap\" aria-hidden=\"true\">…</li><li><a href=\"/s/main/item?page=38\">38</a></li><li class=\"pager-next\"><a href=\"/s/main/item?page=2\" rel=\"next\">Next</a></li></ul>"), "{html}");
        assert!(html.ends_with("<p class=\"pager-count\">950 results · page 1 of 38</p></nav>"));
        assert_eq!(window(20, 38), vec![1, 19, 20, 21, 38]);
        assert_eq!(window(3, 38), vec![1, 2, 3, 4, 38]);
        assert_eq!(window(38, 38), vec![1, 36, 37, 38]);
        assert_eq!(window(2, 2), vec![1, 2]);
        assert_eq!(pagination("/x", &[], 1, 25, 25, messages), "");
    }

    /// A pager link keeps the other query pairs and replaces `page`.
    #[test]
    fn pagination_links_keep_the_query() {
        let query = vec![
            ("sort_by".to_string(), "title".to_string()),
            ("page".to_string(), "2".to_string()),
        ];
        let html = pagination("/s/main/item", &query, 2, 10, 30, &crate::i18n::EN);
        assert!(
            html.contains(
                "<a href=\"/s/main/item?sort_by=title&amp;page=1\" rel=\"prev\">Prev</a>"
            ),
            "{html}"
        );
        assert!(
            html.contains("<a href=\"/s/main/item?sort_by=title&amp;page=3\">3</a>"),
            "{html}"
        );
    }

    /// `urlencode` follows PHP: only `A-Za-z0-9-_.` pass, a space is `+`.
    #[test]
    fn urlencode_matches_php_urlencode() {
        assert_eq!(urlencode("site_id=1&a b~c"), "site_id%3D1%26a+b%7Ec");
    }
}
