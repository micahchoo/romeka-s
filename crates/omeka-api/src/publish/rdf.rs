//! A published scheme as RDF: SKOS for a term list, RDFS/OWL for a vocabulary.
//!
//! [`graph`] turns a [`Snapshot`] into triples, and [`serialize`] writes them in one of the
//! syntaxes oxrdfio serializes. Every IRI in a snapshot was parsed when it was published
//! ([`super::validate_iris`]), so the triples are built without parsing again.

use chrono::NaiveDateTime;
use omeka_core::publication::{SchemeKind, Snapshot, Term, TermKind};
use oxrdf::vocab::{rdf, rdfs, xsd};
use oxrdf::{Literal, NamedNode, Triple};
use oxrdfio::{RdfFormat, RdfSerializer};

const SKOS: &str = "http://www.w3.org/2004/02/skos/core#";
const OWL: &str = "http://www.w3.org/2002/07/owl#";
const DCTERMS: &str = "http://purl.org/dc/terms/";

/// The prefixes a Turtle or RDF/XML document declares.
const PREFIXES: &[(&str, &str)] = &[
    ("skos", SKOS),
    ("owl", OWL),
    ("dcterms", DCTERMS),
    ("rdfs", "http://www.w3.org/2000/01/rdf-schema#"),
    ("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"),
    ("xsd", "http://www.w3.org/2001/XMLSchema#"),
];

/// Which numbered publish a document serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionMeta {
    pub number: u32,
    pub created: NaiveDateTime,
}

/// The RDF syntaxes a scheme is served in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syntax {
    Turtle,
    JsonLd,
    RdfXml,
    NTriples,
}

impl Syntax {
    #[must_use]
    pub fn media_type(self) -> &'static str {
        match self {
            Self::Turtle => "text/turtle",
            Self::JsonLd => "application/ld+json",
            Self::RdfXml => "application/rdf+xml",
            Self::NTriples => "application/n-triples",
        }
    }
}

fn iri(value: &str) -> NamedNode {
    NamedNode::new_unchecked(value)
}

fn term_of(namespace: &str, local: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("{namespace}{local}"))
}

/// A label in the scheme's language, or a plain string when it has none or the tag is not BCP 47.
fn label(value: &str, lang: Option<&str>) -> Literal {
    lang.and_then(|tag| Literal::new_language_tagged_literal(value, tag).ok())
        .unwrap_or_else(|| Literal::new_simple_literal(value))
}

/// Every triple of one version of a scheme.
#[must_use]
pub fn graph(snapshot: &Snapshot, version: VersionMeta) -> Vec<Triple> {
    let scheme = iri(snapshot.scheme_iri());
    let mut triples = scheme_triples(snapshot, &scheme, version);
    for term in &snapshot.terms {
        triples.extend(term_triples(snapshot, &scheme, term));
    }
    triples
}

/// The triples about one term, the document its own URI serves.
#[must_use]
pub fn term_graph(snapshot: &Snapshot, term: &Term) -> Vec<Triple> {
    term_triples(snapshot, &iri(snapshot.scheme_iri()), term)
}

fn scheme_triples(snapshot: &Snapshot, scheme: &NamedNode, version: VersionMeta) -> Vec<Triple> {
    let lang = snapshot.lang.as_deref();
    let mut triples = Vec::new();
    let mut add = |predicate: NamedNode, object: oxrdf::Term| {
        triples.push(Triple::new(scheme.clone(), predicate, object));
    };
    match snapshot.kind {
        SchemeKind::TermList => {
            add(
                rdf::TYPE.into_owned(),
                iri(&format!("{SKOS}ConceptScheme")).into(),
            );
            add(
                iri(&format!("{SKOS}prefLabel")),
                label(&snapshot.title, lang).into(),
            );
            for term in &snapshot.terms {
                add(
                    iri(&format!("{SKOS}hasTopConcept")),
                    term_of(&snapshot.namespace, &term.local).into(),
                );
            }
        }
        SchemeKind::Vocabulary => {
            add(
                rdf::TYPE.into_owned(),
                iri(&format!("{OWL}Ontology")).into(),
            );
            add(
                rdfs::LABEL.into_owned(),
                label(&snapshot.title, lang).into(),
            );
            if let Some(description) = &snapshot.description {
                add(rdfs::COMMENT.into_owned(), label(description, lang).into());
            }
        }
    }
    add(
        iri(&format!("{DCTERMS}title")),
        label(&snapshot.title, lang).into(),
    );
    add(
        iri(&format!("{OWL}versionInfo")),
        Literal::new_simple_literal(version.number.to_string()).into(),
    );
    add(
        iri(&format!("{DCTERMS}modified")),
        Literal::new_typed_literal(
            version.created.format("%Y-%m-%dT%H:%M:%S").to_string(),
            xsd::DATE_TIME,
        )
        .into(),
    );
    triples
}

fn term_triples(snapshot: &Snapshot, scheme: &NamedNode, term: &Term) -> Vec<Triple> {
    let subject = term_of(&snapshot.namespace, &term.local);
    let lang = snapshot.lang.as_deref();
    let mut triples = Vec::new();
    let mut add = |predicate: NamedNode, object: oxrdf::Term| {
        triples.push(Triple::new(subject.clone(), predicate, object));
    };
    match term.kind {
        TermKind::Concept => {
            add(
                rdf::TYPE.into_owned(),
                iri(&format!("{SKOS}Concept")).into(),
            );
            add(
                iri(&format!("{SKOS}prefLabel")),
                label(&term.label, lang).into(),
            );
            add(iri(&format!("{SKOS}inScheme")), scheme.clone().into());
            add(iri(&format!("{SKOS}topConceptOf")), scheme.clone().into());
            if let Some(comment) = &term.comment {
                add(
                    iri(&format!("{SKOS}definition")),
                    label(comment, lang).into(),
                );
            }
            if let Some(uri) = &term.exact_match {
                add(iri(&format!("{SKOS}exactMatch")), iri(uri).into());
            }
            if let Some(uri) = &term.close_match {
                add(iri(&format!("{SKOS}closeMatch")), iri(uri).into());
            }
        }
        TermKind::Class | TermKind::Property => {
            let class = if term.kind == TermKind::Class {
                rdfs::CLASS
            } else {
                rdf::PROPERTY
            };
            add(rdf::TYPE.into_owned(), class.into_owned().into());
            add(rdfs::LABEL.into_owned(), label(&term.label, lang).into());
            if let Some(comment) = &term.comment {
                add(rdfs::COMMENT.into_owned(), label(comment, lang).into());
            }
            add(rdfs::IS_DEFINED_BY.into_owned(), scheme.clone().into());
        }
    }
    if term.deprecated {
        add(
            iri(&format!("{OWL}deprecated")),
            Literal::new_typed_literal("true", xsd::BOOLEAN).into(),
        );
    }
    if let Some(local) = &term.replaced_by {
        add(
            iri(&format!("{DCTERMS}isReplacedBy")),
            term_of(&snapshot.namespace, local).into(),
        );
    }
    triples
}

/// Triples written in one syntax.
pub fn serialize(triples: &[Triple], syntax: Syntax) -> Result<Vec<u8>, std::io::Error> {
    let format = RdfFormat::from_media_type(syntax.media_type()).ok_or_else(|| {
        std::io::Error::other(format!("no RDF format for {}", syntax.media_type()))
    })?;
    let mut serializer = RdfSerializer::from_format(format);
    for (name, namespace) in PREFIXES {
        serializer = serializer
            .with_prefix(*name, *namespace)
            .map_err(std::io::Error::other)?;
    }
    let mut writer = serializer.for_writer(Vec::new());
    for triple in triples {
        writer.serialize_triple(triple)?;
    }
    writer.finish()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use chrono::NaiveDate;
    use omeka_core::publication::{Source, SourceTerm, TermState, TermStates, draft};
    use oxrdf::Graph;
    use oxrdfio::RdfParser;

    use super::*;

    fn version() -> VersionMeta {
        VersionMeta {
            number: 2,
            created: NaiveDate::from_ymd_opt(2026, 10, 18)
                .unwrap()
                .and_hms_opt(9, 30, 0)
                .unwrap(),
        }
    }

    fn places() -> Snapshot {
        let source = Source {
            kind: SchemeKind::TermList,
            title: "Places".to_string(),
            description: None,
            lang: Some("en".to_string()),
            terms: ["Cork", "Dún Laoghaire"]
                .iter()
                .map(|word| SourceTerm {
                    key: (*word).to_string(),
                    kind: TermKind::Concept,
                    label: (*word).to_string(),
                    comment: None,
                    link: None,
                })
                .collect(),
        };
        let mut states = TermStates::new();
        states.insert(
            "Cork".to_string(),
            TermState {
                exact_match: Some("https://sws.geonames.org/2965140/".to_string()),
                close_match: Some("http://www.wikidata.org/entity/Q36405".to_string()),
                ..TermState::default()
            },
        );
        states.insert(
            "Dún Laoghaire".to_string(),
            TermState {
                deprecated: true,
                ..TermState::default()
            },
        );
        draft(&source, "http://localhost/vocab/places/", &states).0
    }

    fn local_fields() -> Snapshot {
        let member = |key: &str, kind: TermKind, label: &str, comment: Option<&str>| SourceTerm {
            key: key.to_string(),
            kind,
            label: label.to_string(),
            comment: comment.map(str::to_string),
            link: None,
        };
        let source = Source {
            kind: SchemeKind::Vocabulary,
            title: "Local fields".to_string(),
            description: Some("Fields this archive made for itself.".to_string()),
            lang: None,
            terms: vec![
                member("Letter", TermKind::Class, "Letter", None),
                member(
                    "numberOfPages",
                    TermKind::Property,
                    "Number of pages",
                    Some("How many pages the item has."),
                ),
            ],
        };
        let mut states = TermStates::new();
        states.insert(
            "Letter".to_string(),
            TermState {
                deprecated: true,
                ..TermState::default()
            },
        );
        draft(&source, "http://localhost/vocab/local/", &states).0
    }

    fn parsed(bytes: &[u8], syntax: Syntax) -> Graph {
        let format = RdfFormat::from_media_type(syntax.media_type()).unwrap();
        let mut graph = Graph::new();
        for quad in RdfParser::from_format(format).for_slice(bytes) {
            let quad = quad.unwrap();
            graph.insert(&Triple::new(quad.subject, quad.predicate, quad.object));
        }
        graph
    }

    const PLACES_TURTLE: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix owl: <http://www.w3.org/2002/07/owl#> .
        @prefix dcterms: <http://purl.org/dc/terms/> .
        @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
        <http://localhost/vocab/places/> a skos:ConceptScheme ;
            skos:prefLabel "Places"@en ;
            dcterms:title "Places"@en ;
            owl:versionInfo "2" ;
            dcterms:modified "2026-10-18T09:30:00"^^xsd:dateTime ;
            skos:hasTopConcept <http://localhost/vocab/places/1>, <http://localhost/vocab/places/2> .
        <http://localhost/vocab/places/1> a skos:Concept ;
            skos:prefLabel "Cork"@en ;
            skos:inScheme <http://localhost/vocab/places/> ;
            skos:topConceptOf <http://localhost/vocab/places/> ;
            skos:exactMatch <https://sws.geonames.org/2965140/> ;
            skos:closeMatch <http://www.wikidata.org/entity/Q36405> .
        <http://localhost/vocab/places/2> a skos:Concept ;
            skos:prefLabel "Dún Laoghaire"@en ;
            skos:inScheme <http://localhost/vocab/places/> ;
            skos:topConceptOf <http://localhost/vocab/places/> ;
            owl:deprecated true .
    "#;

    const LOCAL_TURTLE: &str = r#"
        @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
        @prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
        @prefix owl: <http://www.w3.org/2002/07/owl#> .
        @prefix dcterms: <http://purl.org/dc/terms/> .
        @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
        <http://localhost/vocab/local/> a owl:Ontology ;
            rdfs:label "Local fields" ;
            rdfs:comment "Fields this archive made for itself." ;
            dcterms:title "Local fields" ;
            owl:versionInfo "2" ;
            dcterms:modified "2026-10-18T09:30:00"^^xsd:dateTime .
        <http://localhost/vocab/local/Letter> a rdfs:Class ;
            rdfs:label "Letter" ;
            rdfs:isDefinedBy <http://localhost/vocab/local/> ;
            owl:deprecated true .
        <http://localhost/vocab/local/numberOfPages> a rdf:Property ;
            rdfs:label "Number of pages" ;
            rdfs:comment "How many pages the item has." ;
            rdfs:isDefinedBy <http://localhost/vocab/local/> .
    "#;

    const SYNTAXES: [Syntax; 4] = [
        Syntax::Turtle,
        Syntax::JsonLd,
        Syntax::RdfXml,
        Syntax::NTriples,
    ];

    #[test]
    fn a_term_list_reads_back_as_its_skos_scheme_in_every_syntax() {
        let expected = parsed(PLACES_TURTLE.as_bytes(), Syntax::Turtle);
        let triples = graph(&places(), version());
        for syntax in SYNTAXES {
            let bytes = serialize(&triples, syntax).unwrap();
            assert_eq!(parsed(&bytes, syntax), expected, "{syntax:?}");
        }
    }

    #[test]
    fn a_vocabulary_reads_back_as_its_rdfs_ontology_in_every_syntax() {
        let expected = parsed(LOCAL_TURTLE.as_bytes(), Syntax::Turtle);
        let triples = graph(&local_fields(), version());
        for syntax in SYNTAXES {
            let bytes = serialize(&triples, syntax).unwrap();
            assert_eq!(parsed(&bytes, syntax), expected, "{syntax:?}");
        }
    }

    #[test]
    fn a_term_document_holds_only_that_term() {
        let snapshot = places();
        let cork = snapshot.term("1").unwrap();
        let bytes = serialize(&term_graph(&snapshot, cork), Syntax::Turtle).unwrap();
        let graph = parsed(&bytes, Syntax::Turtle);

        assert_eq!(graph.len(), 6);
        assert!(
            graph
                .iter()
                .all(|triple| triple.subject.to_string() == "<http://localhost/vocab/places/1>")
        );
    }

    #[test]
    fn a_label_with_a_tag_that_is_not_bcp_47_stays_a_plain_string() {
        let mut snapshot = places();
        snapshot.lang = Some("not a tag".to_string());
        let bytes = serialize(&graph(&snapshot, version()), Syntax::NTriples).unwrap();
        let text = String::from_utf8(bytes).unwrap();

        assert!(
            text.contains("<http://www.w3.org/2004/02/skos/core#prefLabel> \"Cork\" ."),
            "{text}"
        );
    }
}
