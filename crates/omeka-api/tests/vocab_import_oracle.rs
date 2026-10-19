//! The importer against upstream's own answers: for each fixture file and set of options, the
//! classes and properties [`extract`] finds equal what `Omeka\Stdlib\RdfImporter::getMembers`
//! returned for the same file in the upstream container, in the same order
//! (`tests/fixtures/rdf/README.md` says how each answer was measured).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use omeka_api::vocab_import::{Format, LabelPreference, Members, extract};
use serde_json::Value;

/// `(local_name, label, comment)` per member.
type Row = (String, String, Option<String>);
/// The classes and the properties.
type Kinds = (Vec<Row>, Vec<Row>);

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/rdf");

struct Case {
    oracle: &'static str,
    file: &'static str,
    namespace: &'static str,
    format: Format,
    lang: Option<&'static str>,
    label_property: Option<&'static str>,
    comment_property: Option<&'static str>,
}

const fn case(
    oracle: &'static str,
    file: &'static str,
    namespace: &'static str,
    format: Format,
) -> Case {
    Case {
        oracle,
        file,
        namespace,
        format,
        lang: None,
        label_property: None,
        comment_property: None,
    }
}

const FOAF: &str = "http://xmlns.com/foaf/0.1/";
const DCTYPE: &str = "http://purl.org/dc/dcmitype/";
const EX: &str = "http://example.org/ns#";

fn cases() -> Vec<Case> {
    vec![
        case(
            "dcterms",
            "dcterms.rdf",
            "http://purl.org/dc/terms/",
            Format::Guess,
        ),
        case("dctype", "dctype.rdf", DCTYPE, Format::Guess),
        case(
            "bibo",
            "bibo.owl",
            "http://purl.org/ontology/bibo/",
            Format::Guess,
        ),
        case("foaf", "foaf.rdf", FOAF, Format::Guess),
        case("dctype.ttl", "dctype.ttl", DCTYPE, Format::Turtle),
        case("dctype.nt", "dctype.nt", DCTYPE, Format::NTriples),
        case("dctype.nt.guess", "dctype.nt", DCTYPE, Format::Guess),
        case("dctype.jsonld", "dctype.jsonld", DCTYPE, Format::JsonLd),
        case(
            "skos",
            "skos.rdf",
            "http://www.w3.org/2004/02/skos/core#",
            Format::Guess,
        ),
        case(
            "dcat",
            "dcat.ttl",
            "http://www.w3.org/ns/dcat#",
            Format::Guess,
        ),
        Case {
            lang: Some("es"),
            ..case(
                "dcat.es",
                "dcat.ttl",
                "http://www.w3.org/ns/dcat#",
                Format::Turtle,
            )
        },
        case("lang", "lang.ttl", EX, Format::Turtle),
        case("lang.guess", "lang.ttl", EX, Format::Guess),
        Case {
            lang: Some("en"),
            ..case("lang.en", "lang.ttl", EX, Format::Turtle)
        },
        Case {
            lang: Some("fr"),
            ..case("lang.fr", "lang.ttl", EX, Format::Turtle)
        },
        Case {
            lang: Some("es"),
            ..case("lang.es", "lang.ttl", EX, Format::Turtle)
        },
        Case {
            label_property: Some("<http://example.org/ns#name>"),
            comment_property: Some("<http://example.org/ns#note>"),
            ..case("lang.custom", "lang.ttl", EX, Format::Turtle)
        },
        // The namespace without its trailing slash: no FOAF term is "0.1" plus a local name.
        case(
            "foaf.noslash",
            "foaf.rdf",
            "http://xmlns.com/foaf/0.1",
            Format::Guess,
        ),
    ]
}

/// The oracle file as `(local_name, label, comment)` per kind.
fn oracle(name: &str) -> Kinds {
    let text = std::fs::read_to_string(format!("{FIXTURES}/oracle/{name}.json")).unwrap();
    let document: Value = serde_json::from_str(&text).unwrap();
    let kind = |key: &str| {
        document[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|term| {
                (
                    term["local_name"].as_str().unwrap().to_string(),
                    term["label"].as_str().unwrap().to_string(),
                    term["comment"].as_str().map(str::to_string),
                )
            })
            .collect()
    };
    (kind("classes"), kind("properties"))
}

fn rows(members: &Members) -> Kinds {
    let kind = |terms: &[omeka_api::vocab_import::Term]| {
        terms
            .iter()
            .map(|term| {
                (
                    term.local_name.clone(),
                    term.label.clone(),
                    term.comment.clone(),
                )
            })
            .collect()
    };
    (kind(&members.classes), kind(&members.properties))
}

#[test]
fn every_fixture_yields_the_members_upstream_stores() {
    for case in cases() {
        let bytes = std::fs::read(format!("{FIXTURES}/{}", case.file)).unwrap();
        let preference =
            LabelPreference::new(case.lang, case.label_property, case.comment_property);
        // A file import parses against the namespace URI (`RdfImporter::getMembers`).
        let members = extract(
            &bytes,
            case.format,
            None,
            case.namespace,
            case.namespace,
            &preference,
        )
        .unwrap_or_else(|error| panic!("{}: {error}", case.oracle));
        let (classes, properties) = rows(&members);
        let (want_classes, want_properties) = oracle(case.oracle);
        assert_eq!(classes, want_classes, "{}: classes", case.oracle);
        assert_eq!(properties, want_properties, "{}: properties", case.oracle);
    }
}
