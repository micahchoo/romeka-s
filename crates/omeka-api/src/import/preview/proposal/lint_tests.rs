//! The rule against overfitting (design section 3), enforced: a detector
//! may only count and compare. Every non-test file of the proposer is parsed,
//! and any literal that could name a token, an extension or a depth fails:
//! a string, character or byte literal other than the pattern writer's five
//! fragments, and a number other than 0 and 1 outside `thresholds.rs`. Each
//! constant in `thresholds.rs` must give its reason in a doc comment.
//! Attributes are not code, so `serde` names and doc comments pass; a macro's
//! tokens are code, so a literal in `format!` fails.

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};

use proc_macro2::{TokenStream, TokenTree};
use syn::visit::{self, Visit};

/// A literal as its source text.
fn text_of(literal: &syn::Lit) -> String {
    match literal {
        syn::Lit::Str(lit) => lit.token().to_string(),
        syn::Lit::ByteStr(lit) => lit.token().to_string(),
        syn::Lit::CStr(lit) => lit.token().to_string(),
        syn::Lit::Byte(lit) => lit.token().to_string(),
        syn::Lit::Char(lit) => lit.token().to_string(),
        syn::Lit::Int(lit) => lit.token().to_string(),
        syn::Lit::Float(lit) => lit.token().to_string(),
        syn::Lit::Bool(lit) => lit.value.to_string(),
        syn::Lit::Verbatim(lit) => lit.to_string(),
        _ => "a literal of a kind syn adds later".to_owned(),
    }
}

/// The only text the pattern writer holds, by the file allowed to hold it:
/// the class fragments, then the regex syntax that joins them. No syntax
/// piece holds a letter or a digit (`syntax_names_nothing`).
const FRAGMENTS: &[(&str, &[&str])] = &[(
    "shape.rs",
    &[
        r"[\p{L}\p{M}]+",
        r"\p{Nd}+",
        r"[^\p{L}\p{M}\p{Nd}/]*",
        r"[^\p{L}\p{M}\p{Nd}/]+",
        r"(?:[^/]+/)*",
        r"[^/]+",
        r"\.[^./]+",
        "^",
        "$",
        "/",
        "(",
        "(?<",
        ">",
        "(?:",
        ")",
        ")?",
        "|",
    ],
)];

/// The class fragments: every allowed text that is not syntax.
const CLASSES: usize = 7;

/// The one file that may hold numbers.
const THRESHOLDS: &str = "thresholds.rs";

/// A literal the rule refuses, or a threshold without a reason.
#[derive(Debug, PartialEq, Eq)]
struct Finding {
    file: String,
    what: String,
}

struct Lint<'f> {
    file: &'f str,
    texts: &'f [&'f str],
    numbers: bool,
    findings: Vec<Finding>,
}

impl Lint<'_> {
    fn refuse(&mut self, what: String) {
        self.findings.push(Finding {
            file: self.file.to_owned(),
            what,
        });
    }

    fn literal(&mut self, literal: &syn::Lit) {
        match literal {
            syn::Lit::Str(text) if self.texts.contains(&text.value().as_str()) => {}
            syn::Lit::Int(number) if matches!(number.base10_digits(), "0" | "1") => {}
            syn::Lit::Int(_) | syn::Lit::Float(_) if self.numbers => {}
            syn::Lit::Bool(_) => {}
            other => self.refuse(text_of(other)),
        }
    }

    fn tokens(&mut self, tokens: TokenStream) {
        for token in tokens {
            match token {
                TokenTree::Literal(literal) => self.literal(&syn::Lit::new(literal)),
                TokenTree::Group(group) => self.tokens(group.stream()),
                TokenTree::Ident(_) | TokenTree::Punct(_) => {}
            }
        }
    }
}

fn is_test_only(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("cfg")
            && attr
                .parse_args::<syn::Ident>()
                .is_ok_and(|word| word == "test")
    })
}

fn has_doc(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| match &attr.meta {
        syn::Meta::NameValue(doc) if doc.path.is_ident("doc") => matches!(
            &doc.value,
            syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(text), .. }) if !text.value().trim().is_empty()
        ),
        _ => false,
    })
}

fn item_attrs(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(item) => &item.attrs,
        syn::Item::Enum(item) => &item.attrs,
        syn::Item::Fn(item) => &item.attrs,
        syn::Item::Impl(item) => &item.attrs,
        syn::Item::Macro(item) => &item.attrs,
        syn::Item::Mod(item) => &item.attrs,
        syn::Item::Static(item) => &item.attrs,
        syn::Item::Struct(item) => &item.attrs,
        syn::Item::Trait(item) => &item.attrs,
        syn::Item::Type(item) => &item.attrs,
        syn::Item::Use(item) => &item.attrs,
        _ => &[],
    }
}

impl<'ast> Visit<'ast> for Lint<'_> {
    fn visit_attribute(&mut self, _: &'ast syn::Attribute) {}

    fn visit_item(&mut self, item: &'ast syn::Item) {
        if is_test_only(item_attrs(item)) {
            return;
        }
        if self.numbers {
            let unexplained = match item {
                syn::Item::Const(item) => (!has_doc(&item.attrs)).then(|| item.ident.to_string()),
                syn::Item::Static(item) => (!has_doc(&item.attrs)).then(|| item.ident.to_string()),
                _ => None,
            };
            if let Some(name) = unexplained {
                self.refuse(format!("{name} gives no reason"));
            }
        }
        visit::visit_item(self, item);
    }

    fn visit_lit(&mut self, literal: &'ast syn::Lit) {
        self.literal(literal);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        self.tokens(mac.tokens.clone());
    }
}

/// What the rule refuses in `source`, read as the file `file`.
fn lint(file: &str, source: &str) -> Vec<Finding> {
    let parsed = match syn::parse_file(source) {
        Ok(parsed) => parsed,
        Err(error) => {
            return vec![Finding {
                file: file.to_owned(),
                what: format!("does not parse: {error}"),
            }];
        }
    };
    let texts = FRAGMENTS
        .iter()
        .find(|(name, _)| *name == file)
        .map_or(&[][..], |(_, texts)| texts);
    let mut lint = Lint {
        file,
        texts,
        numbers: file == THRESHOLDS,
        findings: Vec::new(),
    };
    lint.visit_file(&parsed);
    lint.findings
}

/// The proposer's non-test files: `proposal.rs` and every file under
/// `proposal/` but the tests.
fn sources() -> Vec<PathBuf> {
    fn under(dir: &Path, out: &mut Vec<PathBuf>) {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .expect("the proposal folder")
            .map(|entry| entry.expect("an entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                under(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs")
                && !path.to_string_lossy().ends_with("_tests.rs")
            {
                out.push(path);
            }
        }
    }
    let preview = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/import/preview");
    let mut out = vec![preview.join("proposal.rs")];
    under(&preview.join("proposal"), &mut out);
    out
}

#[test]
fn the_proposer_names_no_token() {
    let files = sources();
    let names: Vec<String> = files
        .iter()
        .map(|path| {
            path.file_name()
                .expect("a name")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(
        names.iter().any(|name| name == "proposal.rs")
            && names.iter().any(|name| name == THRESHOLDS),
        "the lint reads the proposer: {names:?}"
    );
    let findings: Vec<Finding> = files
        .iter()
        .zip(&names)
        .flat_map(|(path, name)| lint(name, &std::fs::read_to_string(path).expect("source")))
        .collect();
    assert!(findings.is_empty(), "{findings:#?}");
}

#[test]
fn the_lint_sees_a_token() {
    let source = r#"
        /// A doc comment's words are not code.
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Cut;
        fn cut(name: &str) -> bool {
            name.starts_with("BHC") && name.len() > 3 && name.len() > 1
        }
        fn told(name: &str) -> String {
            format!("{name}{}", 'x')
        }
        #[cfg(test)]
        mod tests {
            const KEPT: &str = "tests may name tokens";
        }
    "#;
    let found: Vec<String> = lint("names.rs", source)
        .into_iter()
        .map(|finding| finding.what)
        .collect();
    assert_eq!(found, ["\"BHC\"", "3", "\"{name}{}\"", "'x'"]);

    let fragment = r#"fn letters() -> &'static str { "[\\p{L}\\p{M}]+" }"#;
    assert_eq!(lint("shape.rs", fragment), []);
    assert_eq!(
        lint("names.rs", fragment).len(),
        1,
        "only shape.rs holds a fragment"
    );

    let thresholds = "pub const A: usize = 3;\n/// Why.\npub const B: usize = 4;\n";
    let found: Vec<String> = lint(THRESHOLDS, thresholds)
        .into_iter()
        .map(|finding| finding.what)
        .collect();
    assert_eq!(found, ["A gives no reason"]);
}

#[test]
fn syntax_names_nothing() {
    let (_, texts) = FRAGMENTS[0];
    for syntax in &texts[CLASSES..] {
        assert!(
            !syntax.chars().any(char::is_alphanumeric),
            "{syntax} could name a token"
        );
    }
}
