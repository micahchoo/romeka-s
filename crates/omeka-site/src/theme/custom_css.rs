//! A site's own CSS, rendered in a `<style>` after the theme sheet so it wins.
//!
//! Stored text reaches the page as CSS here and nowhere else, so this module is
//! a security boundary. [`sanitize`] reads the text with a real CSS tokenizer
//! (`cssparser`, which `ammonia` already brings into the build), so an escape
//! such as `\75rl(` is read as the `url(` a browser would read. It refuses:
//!
//! - `@import` and `@document`, which load or scope by another document;
//! - a `url()`, or a URL string inside `src()`, `image()`, `image-set()` or
//!   `cross-fade()`, that is not a path under this site's `/files/` or
//!   `/assets/`. That covers an `@font-face` from elsewhere and the attribute
//!   selector that leaks a form value through a background request;
//! - `expression()`, `behavior` and `-moz-binding`, which run script in old
//!   engines;
//! - `</style`, which would end the element and let markup follow;
//! - more than [`MAX_BYTES`] bytes, and text the tokenizer cannot read.
//!
//! The site write calls it before storing, and the frame calls it again before
//! rendering, so text stored by any other path is checked on the way out too.

use std::fmt;

use cssparser::{ParseError, Parser, Token};

/// The most custom CSS a site may store, in bytes.
pub const MAX_BYTES: usize = 64 * 1024;

/// The path prefixes a `url()` may name: this site's file store and theme
/// assets.
const ALLOWED_URL_PREFIXES: [&str; 2] = ["/files/", "/assets/"];

/// Functions whose string arguments are URLs.
const URL_FUNCTIONS: [&str; 7] = [
    "url",
    "src",
    "image",
    "image-set",
    "-webkit-image-set",
    "cross-fade",
    "-webkit-cross-fade",
];

/// Custom CSS that passed [`sanitize`]. It has no other constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomCss(String);

impl CustomCss {
    /// The text, exactly as the curator wrote it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why a piece of custom CSS was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssProblem {
    /// More than [`MAX_BYTES`] bytes.
    TooLarge(usize),
    /// An `@import` rule.
    Import,
    /// An `@document` rule.
    Document,
    /// A URL outside this site's `/files/` and `/assets/`.
    ForeignUrl(String),
    /// `expression()`.
    Expression,
    /// The `behavior` property.
    Behavior,
    /// The `-moz-binding` property.
    MozBinding,
    /// `</style`, which ends the element.
    StyleBreakout,
    /// Text the tokenizer cannot read.
    Syntax(&'static str),
}

/// One refusal, at a 1-based line and column of the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssError {
    /// The 1-based line.
    pub line: u32,
    /// The 1-based column.
    pub column: u32,
    /// What was refused.
    pub problem: CssProblem,
}

impl fmt::Display for CssError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Line {}, column {}: ", self.line, self.column)?;
        match &self.problem {
            CssProblem::TooLarge(bytes) => write!(
                f,
                "custom CSS may be at most {MAX_BYTES} bytes; this is {bytes}."
            ),
            CssProblem::Import => f.write_str("@import is not allowed."),
            CssProblem::Document => f.write_str("@document is not allowed."),
            CssProblem::ForeignUrl(url) => write!(
                f,
                "url({url}) is not allowed; a URL must be a path under /files/ or /assets/."
            ),
            CssProblem::Expression => f.write_str("expression() is not allowed."),
            CssProblem::Behavior => f.write_str("behavior is not allowed."),
            CssProblem::MozBinding => f.write_str("-moz-binding is not allowed."),
            CssProblem::StyleBreakout => f.write_str("</style is not allowed."),
            CssProblem::Syntax(what) => write!(f, "{what}."),
        }
    }
}

/// Check custom CSS, keeping it unchanged when nothing in it is refused.
///
/// # Errors
/// Every refusal, in source order, each with its line and column.
pub fn sanitize(source: &str) -> Result<CustomCss, Vec<CssError>> {
    if source.len() > MAX_BYTES {
        return Err(vec![CssError {
            line: 1,
            column: 1,
            problem: CssProblem::TooLarge(source.len()),
        }]);
    }
    let mut errors = breakouts(source);
    let mut parser = Parser::new(source);
    walk(&mut parser, false, &mut errors);
    errors.sort_by_key(|error| (error.line, error.column));
    if errors.is_empty() {
        Ok(CustomCss(source.to_string()))
    } else {
        Err(errors)
    }
}

/// Every `</style`, matched without regard to case, wherever it sits. The HTML
/// parser ends the element at the raw text, before any CSS rule applies, so
/// the scan is over the text and not the tokens.
fn breakouts(source: &str) -> Vec<CssError> {
    let lower = source.to_ascii_lowercase();
    lower
        .match_indices("</style")
        .map(|(at, _)| {
            let (line, column) = position(source, at);
            CssError {
                line,
                column,
                problem: CssProblem::StyleBreakout,
            }
        })
        .collect()
}

/// The 1-based line and column of a byte offset.
fn position(source: &str, at: usize) -> (u32, u32) {
    let before = &source[..at];
    let line = before.matches('\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map_or(0, |tail| tail.chars().count())
        + 1;
    (
        u32::try_from(line).unwrap_or(u32::MAX),
        u32::try_from(column).unwrap_or(u32::MAX),
    )
}

/// Read every token at this nesting level, descending into each block.
/// `strings_are_urls` is set inside a function whose strings are URLs.
fn walk(parser: &mut Parser<'_>, strings_are_urls: bool, errors: &mut Vec<CssError>) {
    loop {
        let location = parser.current_source_location();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            return;
        };
        let at = |problem: CssProblem| CssError {
            line: location.line + 1,
            column: location.column,
            problem,
        };
        let mut nested_urls = None;
        match token {
            Token::AtKeyword(name) => {
                let name = name.to_ascii_lowercase();
                if name == "import" {
                    errors.push(at(CssProblem::Import));
                } else if name.ends_with("document") {
                    errors.push(at(CssProblem::Document));
                }
            }
            Token::UnquotedUrl(url) => {
                if !allowed_url(url) {
                    errors.push(at(CssProblem::ForeignUrl(url.to_string())));
                }
            }
            Token::QuotedString(text) if strings_are_urls => {
                if !allowed_url(text) {
                    errors.push(at(CssProblem::ForeignUrl(text.to_string())));
                }
            }
            Token::Ident(name) => {
                if let Some(problem) = forbidden_name(name) {
                    errors.push(at(problem));
                }
            }
            Token::Function(name) => {
                let lower = name.to_ascii_lowercase();
                if let Some(problem) = forbidden_name(&lower) {
                    errors.push(at(problem));
                }
                nested_urls = Some(URL_FUNCTIONS.contains(&lower.as_str()));
            }
            Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                nested_urls = Some(false);
            }
            Token::BadUrl(_) => errors.push(at(CssProblem::Syntax("this url( is not valid CSS"))),
            Token::BadString(_) => {
                errors.push(at(CssProblem::Syntax("this string is not closed")));
            }
            Token::CloseParenthesis => {
                errors.push(at(CssProblem::Syntax(") closes nothing")));
            }
            Token::CloseSquareBracket => {
                errors.push(at(CssProblem::Syntax("] closes nothing")));
            }
            Token::CloseCurlyBracket => {
                errors.push(at(CssProblem::Syntax("} closes nothing")));
            }
            _ => {}
        }
        if let Some(urls) = nested_urls {
            let _ = parser.parse_nested_block(|nested| {
                walk(nested, urls, errors);
                Ok::<(), ParseError<()>>(())
            });
        }
    }
}

/// The refusal an identifier or function name earns, if any.
fn forbidden_name(name: &str) -> Option<CssProblem> {
    let lower = name.to_ascii_lowercase();
    if lower.contains("expression") {
        Some(CssProblem::Expression)
    } else if lower.ends_with("behavior") {
        Some(CssProblem::Behavior)
    } else if lower.ends_with("binding") {
        Some(CssProblem::MozBinding)
    } else {
        None
    }
}

/// Whether a URL is a path under this site's `/files/` or `/assets/` that
/// cannot climb out of it.
fn allowed_url(url: &str) -> bool {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    ALLOWED_URL_PREFIXES
        .iter()
        .any(|prefix| lower.starts_with(prefix))
        && !lower.contains("..")
        && !lower.contains("%2e")
        && !lower.contains('\\')
        && !lower
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control())
}

#[cfg(test)]
#[path = "custom_css_tests.rs"]
mod tests;
