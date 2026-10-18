//! The HTML purifier (`Omeka\Stdlib\HtmlPurifier`,
//! `application/src/Stdlib/HtmlPurifier.php`).
//!
//! Upstream builds one `HTMLPurifier` from `HTMLPurifier_Config::createDefault()`
//! and sets only two options, `Attr.AllowedFrameTargets = ['_blank']` and
//! `Cache.DefinitionImpl = null`. Every allowed element, attribute and CSS
//! property is therefore HTMLPurifier's own default, which `purify` reproduces
//! with the maintained `ammonia` sanitizer. The tables below are the default
//! set, read from the oracle's `HTMLDefinition` and `CSSDefinition`; the
//! `.phtml` templates that echo these values raw (`block-layout/html.phtml`,
//! `block-layout/file.phtml`) rely on this having run.
//!
//! Deliberate departures, none of them less safe. An `<img>` whose `src` fails
//! the URL check keeps an empty `<img>` here where HTMLPurifier drops the
//! element, because nothing loads without `src`. ammonia writes a `rel` on
//! every link where HTMLPurifier writes it on a `target="_blank"` link, and it
//! keeps `style` values as written where HTMLPurifier normalises them
//! (`color:red` against `color:#FF0000;`). Upstream's `alt` default, which
//! copies the `src` onto an `<img>` that names no `alt`, has no ammonia
//! equivalent. The allowed tags and attributes are the same set throughout.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use ammonia::{Builder, UrlRelative};

/// Purify an HTML fragment, removing anything a public page would execute.
///
/// `HtmlPurifier::purify`: with the setting on (upstream's installation
/// default) the fragment passes through the sanitizer; with it off the fragment
/// is returned unchanged. The port has no setting reach, so this always runs
/// the sanitizer.
#[must_use]
pub fn purify(html: &str) -> String {
    builder().clean(html).to_string()
}

/// The configured `Builder`, built once.
fn builder() -> &'static Builder<'static> {
    static BUILDER: OnceLock<Builder<'static>> = OnceLock::new();
    BUILDER.get_or_init(configured_builder)
}

fn set(items: &[&'static str]) -> HashSet<&'static str> {
    items.iter().copied().collect()
}

/// HTMLPurifier's default element set (`HTMLModuleManager`, HTML 4.01
/// Transitional), measured from the oracle's `HTMLDefinition::$info`.
const TAGS: &[&str] = &[
    "abbr",
    "acronym",
    "cite",
    "dfn",
    "kbd",
    "q",
    "samp",
    "var",
    "em",
    "strong",
    "code",
    "span",
    "br",
    "address",
    "blockquote",
    "pre",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "p",
    "div",
    "a",
    "ol",
    "ul",
    "dl",
    "li",
    "dd",
    "dt",
    "hr",
    "sub",
    "sup",
    "b",
    "big",
    "i",
    "small",
    "tt",
    "del",
    "ins",
    "bdo",
    "caption",
    "table",
    "td",
    "th",
    "tr",
    "col",
    "colgroup",
    "tbody",
    "thead",
    "tfoot",
    "img",
    "basefont",
    "center",
    "dir",
    "font",
    "menu",
    "s",
    "strike",
    "u",
];

/// The attributes every allowed element accepts. `id` is absent because upstream
/// leaves `Attr.EnableID` off, and `xml:lang` because the parser namespaces it.
const GENERIC_ATTRIBUTES: &[&str] = &["class", "title", "contenteditable", "style", "dir", "lang"];

/// The per-element attributes on top of [`GENERIC_ATTRIBUTES`], from
/// `HTMLDefinition::$info[$element]->attr`. `a`'s `target` is restricted by
/// value below, and its `rel` is owned by ammonia's link rule.
const TAG_ATTRIBUTES: &[(&str, &[&str])] = &[
    ("q", &["cite"]),
    ("br", &["clear"]),
    ("blockquote", &["cite"]),
    ("pre", &["width"]),
    ("h1", &["align"]),
    ("h2", &["align"]),
    ("h3", &["align"]),
    ("h4", &["align"]),
    ("h5", &["align"]),
    ("h6", &["align"]),
    ("p", &["align"]),
    ("div", &["align"]),
    ("a", &["href", "rev", "name"]),
    ("ol", &["compact", "start", "type"]),
    ("ul", &["compact", "type"]),
    ("dl", &["compact"]),
    ("li", &["value", "type"]),
    ("hr", &["align", "noshade", "size", "width"]),
    ("del", &["cite"]),
    ("ins", &["cite"]),
    ("caption", &["align"]),
    (
        "table",
        &[
            "border",
            "cellpadding",
            "cellspacing",
            "frame",
            "rules",
            "summary",
            "width",
            "align",
            "bgcolor",
        ],
    ),
    (
        "td",
        &[
            "abbr", "colspan", "rowspan", "scope", "align", "charoff", "valign", "bgcolor",
            "height", "nowrap", "width",
        ],
    ),
    (
        "th",
        &[
            "abbr", "colspan", "rowspan", "scope", "align", "charoff", "valign", "bgcolor",
            "height", "nowrap", "width",
        ],
    ),
    ("tr", &["align", "charoff", "valign", "bgcolor"]),
    ("col", &["span", "width", "align", "charoff", "valign"]),
    ("colgroup", &["span", "width", "align", "charoff", "valign"]),
    ("tbody", &["align", "charoff", "valign"]),
    ("thead", &["align", "charoff", "valign"]),
    ("tfoot", &["align", "charoff", "valign"]),
    (
        "img",
        &[
            "height", "width", "longdesc", "alt", "src", "name", "align", "border", "hspace",
            "vspace",
        ],
    ),
    ("basefont", &["color", "face", "size"]),
    ("dir", &["compact"]),
    ("font", &["color", "face", "size"]),
    ("menu", &["compact"]),
];

/// HTMLPurifier's default `CSS.AllowedProperties`, measured from the oracle's
/// `CSSDefinition::$info`. ammonia keeps only these declarations in a `style`
/// attribute and drops the rest, including any `url()` whose scheme is not
/// allowed.
const STYLE_PROPERTIES: &[&str] = &[
    "aspect-ratio",
    "background",
    "background-attachment",
    "background-color",
    "background-image",
    "background-position",
    "background-repeat",
    "background-size",
    "border",
    "border-bottom",
    "border-bottom-color",
    "border-bottom-style",
    "border-bottom-width",
    "border-collapse",
    "border-color",
    "border-left",
    "border-left-color",
    "border-left-style",
    "border-left-width",
    "border-right",
    "border-right-color",
    "border-right-style",
    "border-right-width",
    "border-spacing",
    "border-style",
    "border-top",
    "border-top-color",
    "border-top-style",
    "border-top-width",
    "border-width",
    "caption-side",
    "clear",
    "color",
    "direction",
    "float",
    "font",
    "font-family",
    "font-size",
    "font-style",
    "font-variant",
    "font-weight",
    "height",
    "letter-spacing",
    "line-height",
    "list-style",
    "list-style-image",
    "list-style-position",
    "list-style-type",
    "margin",
    "margin-bottom",
    "margin-left",
    "margin-right",
    "margin-top",
    "max-height",
    "max-width",
    "min-height",
    "min-width",
    "padding",
    "padding-bottom",
    "padding-left",
    "padding-right",
    "padding-top",
    "table-layout",
    "text-align",
    "text-decoration",
    "text-decoration-color",
    "text-decoration-line",
    "text-decoration-style",
    "text-decoration-thickness",
    "text-indent",
    "text-transform",
    "vertical-align",
    "white-space",
    "width",
    "word-spacing",
];

/// `URI.AllowedSchemes` (default) a URL attribute may carry. `javascript:` and
/// `data:` are absent, so those attributes are dropped.
const URL_SCHEMES: &[&str] = &["ftp", "http", "https", "mailto", "news", "nntp", "tel"];

fn configured_builder() -> Builder<'static> {
    let tag_attributes: HashMap<&'static str, HashSet<&'static str>> = TAG_ATTRIBUTES
        .iter()
        .map(|(tag, attrs)| (*tag, set(attrs)))
        .collect();
    // `Attr.AllowedFrameTargets = ['_blank']`: a `target` is kept only for that
    // value.
    let tag_attribute_values: HashMap<&'static str, HashMap<&'static str, HashSet<&'static str>>> =
        HashMap::from([("a", HashMap::from([("target", set(&["_blank"]))]))]);

    let mut builder = Builder::default();
    builder
        .tags(set(TAGS))
        .generic_attributes(set(GENERIC_ATTRIBUTES))
        .tag_attributes(tag_attributes)
        .tag_attribute_values(tag_attribute_values)
        .url_schemes(set(URL_SCHEMES))
        .url_relative(UrlRelative::PassThrough)
        .link_rel(Some("noreferrer noopener"))
        .filter_style_properties(set(STYLE_PROPERTIES))
        .strip_comments(true);
    builder
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The brief's measured payload: the script and the event handler are gone,
    /// the paragraph and the image stay. Captured from the oracle with the
    /// upstream config (`HTMLPurifier` at `5e413f0`).
    #[test]
    fn purify_drops_script_and_event_handler() {
        let out = purify(r#"<p>ok</p><script>alert(1)</script><img src=x onerror=alert(2)>"#);
        assert!(!out.contains("<script"), "{out}");
        assert!(!out.contains("onerror"), "{out}");
        assert!(out.contains("<p>ok</p>"), "{out}");
        assert!(out.contains("<img"), "{out}");
    }

    /// A `javascript:` URL and a `data:` URL fail the scheme check while their
    /// elements survive where upstream keeps them.
    #[test]
    fn purify_drops_unsafe_urls() {
        let out = purify(r#"<a href="javascript:alert(1)">x</a>"#);
        assert_eq!(out, "<a rel=\"noreferrer noopener\">x</a>");
        let out = purify(r#"<a href="mailto:x@example.org">mail</a>"#);
        assert!(out.contains("mailto:x@example.org"), "{out}");
        let out = purify(r#"<img src="data:image/png;base64,AAAA">"#);
        assert!(!out.contains("data:"), "{out}");
    }

    /// The dangerous elements and the `style` element (script and style tag
    /// contents included) are removed entirely.
    #[test]
    fn purify_drops_whole_elements() {
        for input in [
            "<iframe src=\"https://example.org/embed\"></iframe>",
            "<svg onload=alert(1)></svg>",
            "<object data=\"x.swf\"></object><embed src=\"x.swf\">",
            "<style>body{display:none}</style>",
        ] {
            assert_eq!(purify(input), "", "{input}");
        }
    }

    /// The kept formatting, links, tables and images match the oracle's kept
    /// tags and attributes.
    #[test]
    fn purify_keeps_allowed_markup() {
        assert_eq!(
            purify("<p><b>bold <i>italic</i></b> <em>em</em> <strong>st</strong></p>"),
            "<p><b>bold <i>italic</i></b> <em>em</em> <strong>st</strong></p>"
        );
        assert_eq!(
            purify("<p id=\"x\" class=\"y\" title=\"t\" align=\"center\">attrs</p>"),
            "<p class=\"y\" title=\"t\" align=\"center\">attrs</p>"
        );
        assert_eq!(
            purify(
                "<table><thead><tr><th scope=\"col\">h</th></tr></thead><tbody><tr><td>a</td></tr></tbody></table>"
            ),
            "<table><thead><tr><th scope=\"col\">h</th></tr></thead><tbody><tr><td>a</td></tr></tbody></table>"
        );
        assert_eq!(
            purify("<img src=\"https://example.org/a.png\" alt=\"a\" width=\"10\">"),
            "<img src=\"https://example.org/a.png\" alt=\"a\" width=\"10\">"
        );
        assert_eq!(
            purify("<ul><li>one</li><li>two</li></ul>"),
            "<ul><li>one</li><li>two</li></ul>"
        );
    }

    /// `Attr.AllowedFrameTargets`: `_blank` survives, any other target is
    /// dropped.
    #[test]
    fn purify_restricts_link_target() {
        let kept = purify(r#"<a href="https://example.org" target="_blank">link</a>"#);
        assert!(kept.contains("target=\"_blank\""), "{kept}");
        assert!(kept.contains("rel=\"noreferrer noopener\""), "{kept}");
        let dropped = purify(r#"<a href="https://example.org" target="evil">link</a>"#);
        assert!(!dropped.contains("target"), "{dropped}");
    }

    /// `CSS.AllowedProperties`: a safe declaration stays, `position` and a
    /// `url(javascript:)` value go.
    #[test]
    fn purify_filters_style_properties() {
        let out = purify(r#"<p style="color:red;position:absolute">styled</p>"#);
        assert!(out.contains("color:red"), "{out}");
        assert!(!out.contains("position"), "{out}");
        let out =
            purify(r#"<span style="color: red; background: url(javascript:alert(1))">s</span>"#);
        assert!(out.contains("color:red"), "{out}");
        assert!(!out.contains("javascript:"), "{out}");
    }

    /// The parsing closes malformed markup the way the oracle does, and
    /// purified output is a fixed point.
    #[test]
    fn purify_normalises_and_is_idempotent() {
        assert_eq!(purify("<div><p>unclosed"), "<div><p>unclosed</p></div>");
        assert_eq!(purify("<p>a<p>b</p>"), "<p>a</p><p>b</p>");
        for input in [
            r#"<p>ok</p><script>alert(1)</script>"#,
            r#"<a href="https://example.org" target="_blank">x</a>"#,
            r#"<p style="color:red">s</p>"#,
            r#"<table><tr><td>a</td></tr></table>"#,
        ] {
            let once = purify(input);
            assert_eq!(purify(&once), once, "{input}");
        }
    }
}
