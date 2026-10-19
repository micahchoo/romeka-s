//! The parsed file, reduced to what `RdfImporter` reads: which local resources carry which
//! `rdf:type`, and their literal values.
//!
//! EasyRdf keeps the whole graph; the importer then asks it only for `allOfType` and for one
//! literal per property. So the parse keeps the subjects that pass the namespace test
//! (`$resource->getUri() === $namespaceUri . $resource->localName()`) and drops everything
//! else as it reads, in the order the triples arrive.

use std::borrow::Cow;
use std::collections::HashMap;

use oxrdfio::{JsonLdProfileSet, RdfFormat, RdfParser};

use super::{ExtractError, Format};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// How many leading bytes `Format::guessFormat` reads.
const GUESS_WINDOW: usize = 1024;

/// A parser the importer can run: [`Format`] with the guess resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Syntax {
    JsonLd,
    NTriples,
    RdfXml,
    Turtle,
}

impl Syntax {
    /// The parser for a format. A guess reads the response's content type first, as
    /// `Graph::load` does, then the content.
    pub(super) fn choose(
        format: Format,
        content_type: Option<&str>,
        bytes: &[u8],
    ) -> Result<Self, ExtractError> {
        match format {
            Format::JsonLd => Ok(Self::JsonLd),
            Format::NTriples => Ok(Self::NTriples),
            Format::RdfXml => Ok(Self::RdfXml),
            Format::Turtle => Ok(Self::Turtle),
            Format::Guess => content_type
                .and_then(Self::from_media_type)
                .or_else(|| Self::sniff(bytes))
                .ok_or(ExtractError::UnknownFormat),
        }
    }

    /// The RDF media types `EasyRdf\Format` registers for the formats parsed here.
    ///
    /// Upstream also maps `text/plain` to N-Triples and the generic XML and JSON types to RDF/XML
    /// and RDF/JSON. Servers send `.ttl` and `.jsonld` files under those generic types, so here
    /// they fall through to the content test instead.
    fn from_media_type(content_type: &str) -> Option<Self> {
        let essence = content_type
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        match essence.as_str() {
            "application/ld+json" => Some(Self::JsonLd),
            "application/n-triples"
            | "text/ntriples"
            | "application/ntriples"
            | "application/x-ntriples" => Some(Self::NTriples),
            "application/rdf+xml" => Some(Self::RdfXml),
            "text/turtle"
            | "application/turtle"
            | "application/x-turtle"
            | "text/n3"
            | "text/rdf+n3" => Some(Self::Turtle),
            _ => None,
        }
    }

    /// `EasyRdf\Format::guessFormat` over the first 1024 bytes, in its order, for the formats
    /// parsed here.
    ///
    /// A document that opens with `{` is RDF/JSON to EasyRdf, a format upstream's form does not
    /// offer; here it is JSON-LD, and so is one that opens with `[`, which EasyRdf cannot guess.
    /// SPARQL results and HTML (RDFa) are recognised and refused.
    fn sniff(bytes: &[u8]) -> Option<Self> {
        let window = String::from_utf8_lossy(&bytes[..bytes.len().min(GUESS_WINDOW)]);
        let start = window.trim_start();
        let lower = window.to_ascii_lowercase();
        if start.starts_with('{') || start.starts_with('[') {
            return Some(Self::JsonLd);
        }
        if lower.contains("<rdf:") || window.contains("<Ontology xmlns=") {
            return Some(Self::RdfXml);
        }
        if window.contains("http://www.w3.org/2005/sparql-results") || is_html(&window, &lower) {
            return None;
        }
        let directive =
            followed_by_space(&window, "@prefix") || followed_by_space(&window, "@base");
        let sparql_directive = (followed_by_space(&lower, "prefix")
            || followed_by_space(&lower, "base"))
            && !window.contains("<?xml");
        if directive || sparql_directive {
            return Some(Self::Turtle);
        }
        if window.lines().any(is_ntriples_line) {
            return Some(Self::NTriples);
        }
        None
    }

    fn rdf_format(self) -> RdfFormat {
        match self {
            Self::JsonLd => RdfFormat::JsonLd {
                profile: JsonLdProfileSet::empty(),
            },
            Self::NTriples => RdfFormat::NTriples,
            Self::RdfXml => RdfFormat::RdfXml,
            Self::Turtle => RdfFormat::Turtle,
        }
    }
}

/// `/\WRDFa\W/i` or `/<!DOCTYPE html|<html/i`.
fn is_html(window: &str, lower: &str) -> bool {
    let rdfa = lower.match_indices("rdfa").any(|(at, _)| {
        let before = window[..at].chars().next_back();
        let after = window[at + 4..].chars().next();
        before.is_some_and(|c| !c.is_alphanumeric() && c != '_')
            && after.is_some_and(|c| !c.is_alphanumeric() && c != '_')
    });
    rdfa || lower.contains("<!doctype html") || lower.contains("<html")
}

/// Whether `word` occurs followed by whitespace (`/word\s/`).
fn followed_by_space(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(at, _)| {
        text[at + word.len()..]
            .chars()
            .next()
            .is_some_and(char::is_whitespace)
    })
}

/// `/^\s*<.+> <.+>/m` for one line.
fn is_ntriples_line(line: &str) -> bool {
    let Some(rest) = line.trim_start().strip_prefix('<') else {
        return false;
    };
    rest.match_indices("> <").any(|(at, _)| {
        at > 0
            && rest[at + 3..]
                .get(1..)
                .is_some_and(|tail| tail.contains('>'))
    })
}

/// One literal value, as `getSingleProperty` sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Literal {
    pub value: String,
    /// The language tag, lowercased by the parser.
    pub language: Option<String>,
}

/// A resource the namespace test kept.
#[derive(Debug, Default)]
pub(super) struct Subject {
    pub local_name: String,
    /// Each literal object, with its predicate, in arrival order.
    pub literals: Vec<(String, Literal)>,
}

/// The local resources of one file.
#[derive(Debug, Default)]
pub(super) struct Graph {
    subjects: Vec<Subject>,
    by_iri: HashMap<String, usize>,
    /// Type IRI to the subjects typed with it, in the order their type triple arrived.
    types: HashMap<String, Vec<usize>>,
}

impl Graph {
    /// Parse a file and keep its local resources.
    pub(super) fn parse(
        bytes: &[u8],
        syntax: Syntax,
        base_iri: &str,
        namespace_uri: &str,
    ) -> Result<Self, ExtractError> {
        let parser = RdfParser::from_format(syntax.rdf_format());
        // An unparseable base (a namespace like `urn:x` is fine, `x y` is not) leaves relative
        // IRIs unresolved, which fails only a file that uses them.
        let parser = parser.clone().with_base_iri(base_iri).unwrap_or(parser);
        let bytes = match syntax {
            Syntax::RdfXml => Cow::Owned(super::xml::conform(bytes)),
            _ => Cow::Borrowed(bytes),
        };
        let mut triples = Vec::new();
        for quad in parser.for_slice(bytes.as_ref()) {
            let quad = quad.map_err(|error| ExtractError::Syntax(error.to_string()))?;
            let oxrdf::NamedOrBlankNode::NamedNode(subject) = quad.subject else {
                continue;
            };
            let subject = subject.into_string();
            if local_name_in(&subject, namespace_uri).is_none() {
                continue;
            }
            triples.push((subject, quad.predicate.into_string(), quad.object));
        }
        // JSON-LD reaches EasyRdf through the toRdf algorithm, which emits subjects in code point
        // order; the other parsers emit document order. Members are stored in this order.
        if syntax == Syntax::JsonLd {
            triples.sort_by(|a, b| a.0.cmp(&b.0));
        }
        let mut graph = Self::default();
        for (subject, predicate, object) in triples {
            graph.add(subject, predicate, object, namespace_uri);
        }
        Ok(graph)
    }

    fn add(&mut self, iri: String, predicate: String, object: oxrdf::Term, namespace_uri: &str) {
        let index = match self.by_iri.get(&iri) {
            Some(index) => *index,
            None => {
                let local_name = local_name_in(&iri, namespace_uri).unwrap_or_default();
                self.subjects.push(Subject {
                    local_name: local_name.to_string(),
                    literals: Vec::new(),
                });
                self.by_iri.insert(iri, self.subjects.len() - 1);
                self.subjects.len() - 1
            }
        };
        match object {
            oxrdf::Term::NamedNode(node) if predicate == RDF_TYPE => {
                let typed = self.types.entry(node.into_string()).or_default();
                if !typed.contains(&index) {
                    typed.push(index);
                }
            }
            oxrdf::Term::Literal(literal) => {
                let (value, _datatype, language) = literal.destruct();
                self.subjects[index]
                    .literals
                    .push((predicate, Literal { value, language }));
            }
            _ => {}
        }
    }

    /// `Graph::allOfType`, for the local resources.
    pub(super) fn of_type(&self, type_iri: &str) -> impl Iterator<Item = &Subject> {
        self.types
            .get(type_iri)
            .into_iter()
            .flatten()
            .map(|index| &self.subjects[*index])
    }
}

/// The local name of `iri` when it is a member of the namespace: `Resource::localName`
/// (`|([^#:/]+)$|`) and the importer's exact-URI test. An IRI ending in `#`, `/` or `:` has no
/// local name, so it passes only when it is the namespace itself, as in upstream.
fn local_name_in<'a>(iri: &'a str, namespace_uri: &str) -> Option<&'a str> {
    let start = iri.rfind(['#', ':', '/']).map_or(0, |at| at + 1);
    let local_name = &iri[start..];
    (iri.len() == namespace_uri.len() + local_name.len() && iri.starts_with(namespace_uri))
        .then_some(local_name)
}

#[cfg(test)]
mod tests {
    use super::super::{ExtractError, Format, LabelPreference, extract};

    const NS: &str = "http://e.org/ns#";

    /// How many classes a guessed parse finds, or why it found none.
    fn guessed(content_type: Option<&str>, text: &str) -> Result<usize, ExtractError> {
        extract(
            text.as_bytes(),
            Format::Guess,
            content_type,
            NS,
            NS,
            &LabelPreference::default(),
        )
        .map(|members| members.classes.len())
    }

    #[test]
    fn the_guess_reads_the_content_as_easyrdf_does() {
        let rdfxml = r#"<?xml version="1.0"?><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:rdfs="http://www.w3.org/2000/01/rdf-schema#"><rdfs:Class rdf:about="http://e.org/ns#A"/></rdf:RDF>"#;
        let turtle = "@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n<http://e.org/ns#A> a rdfs:Class .";
        let sparql_turtle = "PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>\n<http://e.org/ns#A> a rdfs:Class .";
        let ntriples = "<http://e.org/ns#A> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2000/01/rdf-schema#Class> .";
        let jsonld = r#"[{"@id": "http://e.org/ns#A", "@type": "http://www.w3.org/2000/01/rdf-schema#Class"}]"#;
        for text in [rdfxml, turtle, sparql_turtle, ntriples, jsonld] {
            assert_eq!(guessed(None, text), Ok(1), "{text}");
        }
        assert_eq!(
            guessed(None, "<!DOCTYPE html><html></html>"),
            Err(ExtractError::UnknownFormat)
        );
        assert_eq!(
            guessed(None, "just words"),
            Err(ExtractError::UnknownFormat)
        );
    }

    #[test]
    fn an_rdf_content_type_decides_before_the_content_and_a_generic_one_does_not() {
        let ntriples = "<http://e.org/ns#A> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2000/01/rdf-schema#Class> .";
        assert_eq!(guessed(Some("text/turtle; charset=utf-8"), ntriples), Ok(1));
        assert!(matches!(
            guessed(Some("application/rdf+xml"), ntriples),
            Err(ExtractError::Syntax(_))
        ));
        assert_eq!(guessed(Some("text/plain"), ntriples), Ok(1));
    }
}
