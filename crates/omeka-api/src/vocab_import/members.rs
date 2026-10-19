//! `RdfImporter::getMembersOfTypes`, `::getLabel` and `::getComment`.

use serde::{Deserialize, Serialize};

use super::graph::{Graph, Literal, Subject};

const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";
const OWL: &str = "http://www.w3.org/2002/07/owl#";
const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";

/// `RdfImporter::$classTypes`.
const CLASS_TYPES: [(&str, &str); 2] = [(RDFS, "Class"), (OWL, "Class")];

/// `RdfImporter::$propertyTypes`. `owl:AnnotationProperty` and `owl:OntologyProperty` are left
/// out upstream because they usually annotate the vocabulary itself.
const PROPERTY_TYPES: [(&str, &str); 7] = [
    (RDF, "Property"),
    (OWL, "ObjectProperty"),
    (OWL, "DatatypeProperty"),
    (OWL, "SymmetricProperty"),
    (OWL, "TransitiveProperty"),
    (OWL, "FunctionalProperty"),
    (OWL, "InverseFunctionalProperty"),
];

/// `getMembers`' default `label_property`.
const DEFAULT_LABEL: &str = "skos:prefLabel|rdfs:label|foaf:name|rss:title|dc:title|dc11:title";
/// `getMembers`' default `comment_property`.
const DEFAULT_COMMENT: &str = "rdfs:comment";

/// `EasyRdf\RdfNamespace::$initial_namespaces`: the prefixes a property path may name, so a
/// path typed for upstream reads the same here.
const PREFIXES: &[(&str, &str)] = &[
    ("as", "https://www.w3.org/ns/activitystreams#"),
    ("bibo", "http://purl.org/ontology/bibo/"),
    ("cc", "http://creativecommons.org/ns#"),
    ("cert", "http://www.w3.org/ns/auth/cert#"),
    ("csvw", "http://www.w3.org/ns/csvw#"),
    ("ctag", "http://commontag.org/ns#"),
    ("dc", "http://purl.org/dc/terms/"),
    ("dc11", "http://purl.org/dc/elements/1.1/"),
    ("dcat", "http://www.w3.org/ns/dcat#"),
    ("dcterms", "http://purl.org/dc/terms/"),
    ("doap", "http://usefulinc.com/ns/doap#"),
    ("dqv", "http://www.w3.org/ns/dqv#"),
    ("duv", "https://www.w3.org/ns/duv#"),
    ("exif", "http://www.w3.org/2003/12/exif/ns#"),
    ("foaf", "http://xmlns.com/foaf/0.1/"),
    ("geo", "http://www.w3.org/2003/01/geo/wgs84_pos#"),
    ("gr", "http://purl.org/goodrelations/v1#"),
    ("grddl", "http://www.w3.org/2003/g/data-view#"),
    ("ical", "http://www.w3.org/2002/12/cal/icaltzd#"),
    ("jsonld", "http://www.w3.org/ns/json-ld#"),
    ("ldp", "http://www.w3.org/ns/ldp#"),
    ("ma", "http://www.w3.org/ns/ma-ont#"),
    ("oa", "http://www.w3.org/ns/oa#"),
    ("odrl", "http://www.w3.org/ns/odrl/2/"),
    ("og", "http://ogp.me/ns#"),
    ("org", "http://www.w3.org/ns/org#"),
    ("owl", "http://www.w3.org/2002/07/owl#"),
    ("prov", "http://www.w3.org/ns/prov#"),
    ("qb", "http://purl.org/linked-data/cube#"),
    ("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"),
    ("rdfa", "http://www.w3.org/ns/rdfa#"),
    ("rdfs", "http://www.w3.org/2000/01/rdf-schema#"),
    ("rev", "http://purl.org/stuff/rev#"),
    ("rif", "http://www.w3.org/2007/rif#"),
    ("rr", "http://www.w3.org/ns/r2rml#"),
    ("rss", "http://purl.org/rss/1.0/"),
    ("schema", "http://schema.org/"),
    ("sd", "http://www.w3.org/ns/sparql-service-description#"),
    ("sioc", "http://rdfs.org/sioc/ns#"),
    ("skos", "http://www.w3.org/2004/02/skos/core#"),
    ("skosxl", "http://www.w3.org/2008/05/skos-xl#"),
    ("sosa", "http://www.w3.org/ns/sosa/"),
    ("ssn", "http://www.w3.org/ns/ssn/"),
    ("synd", "http://purl.org/rss/1.0/modules/syndication/"),
    ("time", "http://www.w3.org/2006/time#"),
    ("v", "http://rdf.data-vocabulary.org/#"),
    ("vcard", "http://www.w3.org/2006/vcard/ns#"),
    ("void", "http://rdfs.org/ns/void#"),
    ("wdr", "http://www.w3.org/2007/05/powder#"),
    ("wdrs", "http://www.w3.org/2007/05/powder-s#"),
    ("wot", "http://xmlns.com/wot/0.1/"),
    ("xhv", "http://www.w3.org/1999/xhtml/vocab#"),
    ("xml", "http://www.w3.org/XML/1998/namespace"),
    ("xsd", "http://www.w3.org/2001/XMLSchema#"),
];

/// One class or property the file declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Term {
    #[serde(rename = "o:local_name")]
    pub local_name: String,
    #[serde(rename = "o:label")]
    pub label: String,
    #[serde(rename = "o:comment")]
    pub comment: Option<String>,
}

/// What `getMembers` returns: classes and properties, each in the order upstream stores them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Members {
    pub classes: Vec<Term>,
    pub properties: Vec<Term>,
}

impl Members {
    /// `if (!$members['classes'] && !$members['properties'])`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.classes.is_empty() && self.properties.is_empty()
    }
}

/// Which literal becomes a member's label and comment: `getMembers`' `label_property`,
/// `comment_property` and `lang` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelPreference {
    label: Vec<String>,
    comment: Vec<String>,
    language: Option<String>,
}

impl Default for LabelPreference {
    fn default() -> Self {
        Self::new(None, None, None)
    }
}

impl LabelPreference {
    /// The options as the form posts them. An empty field is the default, as the form's
    /// `ToNull` filter makes it; an empty language is any language, as `null == ''` in PHP.
    ///
    /// A path is `<full IRI>`, or prefixed names joined by `|` and tried in order.
    #[must_use]
    pub fn new(
        language: Option<&str>,
        label_property: Option<&str>,
        comment_property: Option<&str>,
    ) -> Self {
        fn present(text: Option<&str>) -> Option<&str> {
            text.map(str::trim).filter(|text| !text.is_empty())
        }
        Self {
            label: path(present(label_property).unwrap_or(DEFAULT_LABEL)),
            comment: path(present(comment_property).unwrap_or(DEFAULT_COMMENT)),
            language: present(language).map(str::to_ascii_lowercase),
        }
    }
}

/// A property path as IRIs, in the order `Graph::get` tries them.
fn path(text: &str) -> Vec<String> {
    text.split('|')
        .map(|part| {
            let part = part.trim();
            if let Some(iri) = part
                .strip_prefix('<')
                .and_then(|rest| rest.strip_suffix('>'))
            {
                return iri.to_string();
            }
            match part.split_once(':') {
                Some((prefix, local)) => PREFIXES
                    .iter()
                    .find(|(known, _)| *known == prefix)
                    .map_or_else(|| part.to_string(), |(_, base)| format!("{base}{local}")),
                None => part.to_string(),
            }
        })
        .collect()
}

/// `getMembers`' two calls to `getMembersOfTypes`.
pub(super) fn members(graph: &Graph, preference: &LabelPreference) -> Members {
    Members {
        classes: of_types(graph, &CLASS_TYPES, preference),
        properties: of_types(graph, &PROPERTY_TYPES, preference),
    }
}

/// Every local resource of the types, keyed by local name: a resource with two of the types
/// keeps the place its first type gave it.
fn of_types(graph: &Graph, types: &[(&str, &str)], preference: &LabelPreference) -> Vec<Term> {
    let mut terms: Vec<Term> = Vec::new();
    for (namespace, name) in types {
        for subject in graph.of_type(&format!("{namespace}{name}")) {
            if terms
                .iter()
                .any(|term| term.local_name == subject.local_name)
            {
                continue;
            }
            let label = literal(subject, &preference.label, preference.language.as_deref())
                .map(|literal| literal.value.as_str())
                .filter(|value| !value.is_empty())
                .unwrap_or(&subject.local_name)
                .to_string();
            let comment = literal(subject, &preference.comment, preference.language.as_deref())
                .map(|literal| literal.value.clone());
            terms.push(Term {
                local_name: subject.local_name.clone(),
                label,
                comment,
            });
        }
    }
    terms
}

/// `$resource->get($path, 'literal', $lang) ?: $resource->get($path, 'literal')`: the first
/// alternative with a literal in the language, else the first alternative with any literal.
/// Tags compare without case, which BCP 47 asks for and the parser's lowercasing requires.
fn literal<'a>(
    subject: &'a Subject,
    path: &[String],
    language: Option<&str>,
) -> Option<&'a Literal> {
    let first = |wanted: Option<&str>| {
        path.iter().find_map(|predicate| {
            subject
                .literals
                .iter()
                .find(|(name, literal)| {
                    name == predicate
                        && wanted.is_none_or(|wanted| literal.language.as_deref() == Some(wanted))
                })
                .map(|(_, literal)| literal)
        })
    };
    language
        .and_then(|wanted| first(Some(wanted)))
        .or_else(|| first(None))
}
