//! A published scheme or term as a page a person reads.
//!
//! The page carries what the RDF carries: the scheme, its version, each term with its URI,
//! deprecation and matches, and a link to every other syntax. It is self-contained (no theme, no
//! script), so it renders the same for every version of every scheme.

use omeka_core::publication::{SchemeKind, Snapshot, Term, TermKind};
use omeka_site::html::escape_html as esc;

use super::negotiate::{Representation, formats};
use super::rdf::VersionMeta;

/// Where a page sits: the scheme's path, and the version when the request named one.
#[derive(Debug, Clone, Copy)]
pub struct PageAddress<'a> {
    /// `/vocab/<slug>`.
    pub scheme_path: &'a str,
    /// The `version` the request asked for; `None` for the latest.
    pub version: Option<u32>,
}

const STYLE: &str = "\
:root{color-scheme:light dark;--fg:#1d2129;--muted:#5b6270;--line:#d7dbe0;--bg:#fff;--warn:#8a4b00}\
@media (prefers-color-scheme:dark){:root{--fg:#e8eaed;--muted:#a8adb6;--line:#3a3f47;--bg:#16181c;--warn:#f0b35a}}\
body{margin:0;background:var(--bg);color:var(--fg);font:16px/1.5 system-ui,sans-serif}\
main{max-width:56rem;margin:0 auto;padding:1.5rem 1rem}\
h1{font-size:1.75rem;margin:0 0 .25rem}p,dl{margin:.5rem 0}\
.muted{color:var(--muted)}code{font-size:.9em;overflow-wrap:anywhere}\
table{border-collapse:collapse;width:100%;margin:1rem 0}th,td{text-align:left;padding:.4rem .5rem;border-bottom:1px solid var(--line);vertical-align:top}\
.deprecated{color:var(--warn);font-weight:600}nav ul{list-style:none;padding:0;display:flex;flex-wrap:wrap;gap:.75rem}\
dt{font-weight:600}dd{margin:0 0 .5rem}a{color:inherit}";

fn kind_name(kind: SchemeKind) -> &'static str {
    match kind {
        SchemeKind::TermList => "Term list (SKOS concept scheme)",
        SchemeKind::Vocabulary => "Vocabulary (RDFS)",
    }
}

fn term_kind_name(kind: TermKind) -> &'static str {
    match kind {
        TermKind::Concept => "Concept",
        TermKind::Class => "Class",
        TermKind::Property => "Property",
    }
}

fn query(address: PageAddress<'_>, format: &str) -> String {
    match address.version {
        Some(version) => format!("?version={version}&amp;format={format}"),
        None => format!("?format={format}"),
    }
}

fn shell(title: &str, lang: Option<&str>, body: &str) -> String {
    let lang = lang.map_or_else(String::new, |tag| format!(" lang=\"{}\"", esc(tag)));
    format!(
        "<!doctype html>\n<html{lang}><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{}</title><style>{STYLE}</style></head><body><main>{body}</main></body></html>\n",
        esc(title)
    )
}

fn other_formats(address: PageAddress<'_>, path: &str) -> String {
    let links: String = formats()
        .iter()
        .filter(|(_, representation)| *representation != Representation::Html)
        .map(|(name, representation)| {
            format!(
                "<li><a href=\"{}{}\" type=\"{}\">{}</a></li>",
                esc(path),
                query(address, name),
                representation.media_type(),
                representation.media_type()
            )
        })
        .collect();
    format!("<nav aria-label=\"Other formats\"><p>Also as:</p><ul>{links}</ul></nav>")
}

fn version_line(version: VersionMeta) -> String {
    format!(
        "<p class=\"muted\">Version {}, published {}</p>",
        version.number,
        version.created.format("%Y-%m-%d")
    )
}

fn match_cell(term: &Term) -> String {
    let mut parts = Vec::new();
    if let Some(uri) = &term.exact_match {
        parts.push(format!(
            "Same as <a href=\"{0}\"><code>{0}</code></a>",
            esc(uri)
        ));
    }
    if let Some(uri) = &term.close_match {
        parts.push(format!(
            "Close to <a href=\"{0}\"><code>{0}</code></a>",
            esc(uri)
        ));
    }
    parts.join("<br>")
}

/// The page for one version of a scheme: its terms, its versions and its other syntaxes.
#[must_use]
pub fn scheme_page(
    snapshot: &Snapshot,
    version: VersionMeta,
    versions: &[u32],
    address: PageAddress<'_>,
) -> String {
    let rows: String = snapshot
        .terms
        .iter()
        .map(|term| {
            let state = if term.deprecated {
                "<span class=\"deprecated\">Deprecated</span>"
            } else {
                ""
            };
            format!(
                "<tr><td><a href=\"{}/{}\">{}</a> {state}</td><td>{}</td><td><code>{}</code></td><td>{}</td></tr>",
                esc(address.scheme_path),
                esc(&term.local),
                esc(&term.label),
                term_kind_name(term.kind),
                esc(&snapshot.term_iri(term)),
                match_cell(term)
            )
        })
        .collect();
    let history: String = versions
        .iter()
        .rev()
        .map(|number| {
            if *number == version.number {
                format!("<li aria-current=\"page\">Version {number}</li>")
            } else {
                format!(
                    "<li><a href=\"{}/?version={number}\">Version {number}</a></li>",
                    esc(address.scheme_path)
                )
            }
        })
        .collect();
    let description = snapshot
        .description
        .as_deref()
        .map_or_else(String::new, |text| format!("<p>{}</p>", esc(text)));
    let body = format!(
        "<h1>{}</h1><p class=\"muted\">{}</p>{}{description}\
         <dl><dt>Namespace</dt><dd><code>{}</code></dd></dl>\
         <table><caption class=\"muted\">{} terms</caption><thead><tr><th scope=\"col\">Label</th><th scope=\"col\">Kind</th><th scope=\"col\">URI</th><th scope=\"col\">Matches</th></tr></thead><tbody>{rows}</tbody></table>\
         <nav aria-label=\"Versions\"><p>Versions:</p><ul>{history}</ul></nav>{}",
        esc(&snapshot.title),
        kind_name(snapshot.kind),
        version_line(version),
        esc(&snapshot.namespace),
        snapshot.terms.len(),
        other_formats(address, &format!("{}/", address.scheme_path))
    );
    shell(&snapshot.title, snapshot.lang.as_deref(), &body)
}

/// The page for one term of one version.
#[must_use]
pub fn term_page(
    snapshot: &Snapshot,
    term: &Term,
    version: VersionMeta,
    address: PageAddress<'_>,
) -> String {
    let deprecated = if term.deprecated {
        "<p class=\"deprecated\">Deprecated. The term stays so links to it keep working, but new descriptions should not use it.</p>"
    } else {
        ""
    };
    let comment = term
        .comment
        .as_deref()
        .map_or_else(String::new, |text| format!("<p>{}</p>", esc(text)));
    let matches = match_cell(term);
    let matches = if matches.is_empty() {
        String::new()
    } else {
        format!("<dt>Matches</dt><dd>{matches}</dd>")
    };
    let body = format!(
        "<h1>{}</h1><p class=\"muted\">{} in <a href=\"{}/\">{}</a></p>{}{deprecated}{comment}\
         <dl><dt>URI</dt><dd><code>{}</code></dd>{matches}</dl>{}",
        esc(&term.label),
        term_kind_name(term.kind),
        esc(address.scheme_path),
        esc(&snapshot.title),
        version_line(version),
        esc(&snapshot.term_iri(term)),
        other_formats(address, &format!("{}/{}", address.scheme_path, term.local))
    );
    shell(&term.label, snapshot.lang.as_deref(), &body)
}
