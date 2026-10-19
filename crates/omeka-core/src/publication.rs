//! Publishing a term list or a vocabulary: the published scheme, how its term URIs are minted,
//! and the rule that keeps a published term resolvable.
//!
//! Romeka's own feature; upstream has no equivalent. A scheme is a **draft** until its first
//! publish and **published** after it. Each publish freezes a [`Snapshot`] as a numbered version.
//! After the first, the next snapshot may only add terms or deprecate them: [`check`] refuses one
//! that drops or renames a published term, moves the namespace, or takes a deprecation back.
//!
//! A term list's word is the value an item stores, so its URI cannot come from the word without
//! breaking on a typo fix. Each term instead gets a number, minted the first time a snapshot
//! carries it and kept in [`TermStates`] for good ([`draft`]). A vocabulary's terms already have
//! URIs, namespace plus local name, so they mint nothing.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// What is published.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemeKind {
    /// A term list, published as a SKOS concept scheme.
    TermList,
    /// A vocabulary, published as an RDFS/OWL ontology.
    Vocabulary,
}

/// What one term is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TermKind {
    /// A word of a term list (`skos:Concept`).
    Concept,
    /// A vocabulary's resource class (`rdfs:Class`).
    Class,
    /// A vocabulary's property (`rdf:Property`).
    Property,
}

/// The Romeka state of one term, stored beside the upstream row.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TermState {
    /// The minted local name; `None` until the term is first published.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub deprecated: bool,
    /// `skos:exactMatch`: the same concept in an authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact_match: Option<String>,
    /// `skos:closeMatch`: a near concept in an authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close_match: Option<String>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Every term's state, keyed by the term's key: a term list's word or URI, a vocabulary member's
/// local name.
pub type TermStates = BTreeMap<String, TermState>;

/// A scheme as its upstream rows describe it now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub kind: SchemeKind,
    pub title: String,
    pub description: Option<String>,
    /// The language of every label, as a term list's `lang` gives it.
    pub lang: Option<String>,
    pub terms: Vec<SourceTerm>,
}

/// One term as its upstream row describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceTerm {
    /// What identifies the term in its row: the word, the listed URI, or the local name.
    pub key: String,
    pub kind: TermKind,
    pub label: String,
    pub comment: Option<String>,
    /// The URI a URI list names for this entry. It is the term's exactMatch unless one is set.
    pub link: Option<String>,
}

/// One published scheme: what a version serves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub kind: SchemeKind,
    /// Ends in `/` or `#`. A term's URI is this plus its local name.
    pub namespace: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
    pub terms: Vec<Term>,
}

/// One published term.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Term {
    pub key: String,
    pub local: String,
    pub kind: TermKind,
    pub label: String,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub deprecated: bool,
    #[serde(default)]
    pub exact_match: Option<String>,
    #[serde(default)]
    pub close_match: Option<String>,
}

impl Snapshot {
    /// The scheme's own IRI: the namespace, without the `#` of a hash namespace.
    #[must_use]
    pub fn scheme_iri(&self) -> &str {
        self.namespace.strip_suffix('#').unwrap_or(&self.namespace)
    }

    #[must_use]
    pub fn term_iri(&self, term: &Term) -> String {
        format!("{}{}", self.namespace, term.local)
    }

    /// The term with this local name.
    #[must_use]
    pub fn term(&self, local: &str) -> Option<&Term> {
        self.terms.iter().find(|term| term.local == local)
    }
}

/// The snapshot the next publish would write, and the term state with every new term list term
/// given its number.
///
/// A number is one more than the highest ever minted in `states`, counting terms the draft has
/// since dropped, so a number is never handed out twice.
#[must_use]
pub fn draft(source: &Source, namespace: &str, states: &TermStates) -> (Snapshot, TermStates) {
    let mut states = states.clone();
    let mut next = states
        .values()
        .filter_map(|state| state.local.as_deref()?.parse::<u64>().ok())
        .max()
        .unwrap_or(0);
    let mut terms = Vec::with_capacity(source.terms.len());
    for entry in &source.terms {
        let state = states.entry(entry.key.clone()).or_default();
        let local = match (&state.local, source.kind) {
            (Some(local), _) => local.clone(),
            (None, SchemeKind::Vocabulary) => entry.key.clone(),
            (None, SchemeKind::TermList) => {
                next += 1;
                next.to_string()
            }
        };
        state.local = Some(local.clone());
        terms.push(Term {
            key: entry.key.clone(),
            local,
            kind: entry.kind,
            label: entry.label.clone(),
            comment: entry.comment.clone(),
            deprecated: state.deprecated,
            exact_match: state.exact_match.clone().or_else(|| entry.link.clone()),
            close_match: state.close_match.clone(),
        });
    }
    let snapshot = Snapshot {
        kind: source.kind,
        namespace: namespace.to_string(),
        title: source.title.clone(),
        description: source.description.clone(),
        lang: source.lang.clone(),
        terms,
    };
    (snapshot, states)
}

/// Why a change to a published scheme is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// These published terms are gone or renamed, by key.
    Removed(Vec<String>),
    /// The namespace moved, which would change every term's URI.
    NamespaceChanged,
    /// A published deprecation was taken back.
    Undeprecated(String),
}

/// Whether `next` keeps every promise `published` made: the namespace, each term under its key
/// and URI, and each deprecation.
pub fn check(published: &Snapshot, next: &Snapshot) -> Result<(), Refusal> {
    if published.namespace != next.namespace {
        return Err(Refusal::NamespaceChanged);
    }
    let mut removed = Vec::new();
    for term in &published.terms {
        match next
            .terms
            .iter()
            .find(|candidate| candidate.key == term.key)
        {
            Some(kept) if kept.local == term.local => {
                if term.deprecated && !kept.deprecated {
                    return Err(Refusal::Undeprecated(term.key.clone()));
                }
            }
            _ => removed.push(term.key.clone()),
        }
    }
    if removed.is_empty() {
        Ok(())
    } else {
        Err(Refusal::Removed(removed))
    }
}

/// The path segment a scheme is served under, `/vocab/<slug>/`: lowercase ASCII letters and
/// digits in hyphen-separated runs, at most 64 bytes.
#[must_use]
pub fn is_slug(candidate: &str) -> bool {
    !candidate.is_empty()
        && candidate.len() <= 64
        && candidate.split('-').all(|run| {
            !run.is_empty()
                && run
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

/// A slug suggested from a label: its ASCII letters and digits, lowercased, every other run a
/// hyphen. Empty when the label has none, as a label in Devanagari has.
#[must_use]
pub fn suggest_slug(label: &str) -> String {
    let mut slug = String::new();
    for ch in label.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let mut slug = slug.trim_end_matches('-').to_string();
    slug.truncate(64);
    slug.trim_end_matches('-').to_string()
}

/// The namespace this archive serves a slug under: `<base>/vocab/<slug>/`.
#[must_use]
pub fn archive_namespace(base: &str, slug: &str) -> String {
    format!("{}/vocab/{slug}/", base.trim_end_matches('/'))
}

/// The slug a namespace under this archive's `/vocab/` names, for `<base>/vocab/<slug>/` and
/// `<base>/vocab/<slug>#`.
#[must_use]
pub fn archive_slug<'a>(base: &str, namespace: &'a str) -> Option<&'a str> {
    let prefix = format!("{}/vocab/", base.trim_end_matches('/'));
    let rest = namespace.strip_prefix(&prefix)?;
    let slug = rest.strip_suffix('/').or_else(|| rest.strip_suffix('#'))?;
    is_slug(slug).then_some(slug)
}

/// The persistent address services a namespace may live at: they redirect to this archive and
/// outlive a change of host.
pub const PERSISTENT_HOSTS: &[&str] = &["w3id.org", "purl.org"];

/// The namespaces of the vocabularies an install ships. They belong to their maintainers, so this
/// archive never publishes them.
pub const BUNDLED_NAMESPACES: &[&str] = &[
    "http://purl.org/dc/terms/",
    "http://purl.org/dc/dcmitype/",
    "http://purl.org/ontology/bibo/",
    "http://xmlns.com/foaf/0.1/",
];

/// Whether a namespace is an address at a persistent service: `http` or `https`, a host in
/// [`PERSISTENT_HOSTS`], a path below the host, and a final `/` or `#`.
#[must_use]
pub fn is_persistent_namespace(namespace: &str) -> bool {
    if BUNDLED_NAMESPACES.contains(&namespace) {
        return false;
    }
    let Some(rest) = namespace
        .strip_prefix("https://")
        .or_else(|| namespace.strip_prefix("http://"))
    else {
        return false;
    };
    let Some((host, path)) = rest.split_once('/') else {
        return false;
    };
    PERSISTENT_HOSTS.contains(&host)
        && path.len() > 1
        && (path.ends_with('/') || path.ends_with('#'))
        && !path.contains(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(key: &str) -> SourceTerm {
        SourceTerm {
            key: key.to_string(),
            kind: TermKind::Concept,
            label: key.to_string(),
            comment: None,
            link: None,
        }
    }

    fn places(words: &[&str]) -> Source {
        Source {
            kind: SchemeKind::TermList,
            title: "Places".to_string(),
            description: None,
            lang: Some("en".to_string()),
            terms: words.iter().map(|key| word(key)).collect(),
        }
    }

    const NS: &str = "http://localhost/vocab/places/";

    #[test]
    fn a_term_list_numbers_its_terms_in_list_order() {
        let (snapshot, states) = draft(&places(&["Cork", "Dublin"]), NS, &TermStates::new());

        let uris: Vec<String> = snapshot
            .terms
            .iter()
            .map(|term| snapshot.term_iri(term))
            .collect();
        assert_eq!(
            uris,
            [
                "http://localhost/vocab/places/1",
                "http://localhost/vocab/places/2"
            ]
        );
        assert_eq!(states["Dublin"].local.as_deref(), Some("2"));
    }

    #[test]
    fn a_number_is_kept_by_its_term_and_never_handed_out_again() {
        let (_, states) = draft(
            &places(&["Cork", "Dublin", "Galway"]),
            NS,
            &TermStates::new(),
        );

        // Galway was dropped in the draft, Dublin moved to the front, and Ennis is new.
        let (snapshot, _) = draft(&places(&["Dublin", "Cork", "Ennis"]), NS, &states);

        let numbered: Vec<(&str, &str)> = snapshot
            .terms
            .iter()
            .map(|term| (term.key.as_str(), term.local.as_str()))
            .collect();
        assert_eq!(numbered, [("Dublin", "2"), ("Cork", "1"), ("Ennis", "4")]);
    }

    #[test]
    fn a_vocabulary_term_keeps_its_local_name() {
        let source = Source {
            kind: SchemeKind::Vocabulary,
            title: "Local fields".to_string(),
            description: None,
            lang: None,
            terms: vec![SourceTerm {
                key: "numberOfPages".to_string(),
                kind: TermKind::Property,
                label: "Number of pages".to_string(),
                comment: None,
                link: None,
            }],
        };

        let (snapshot, _) = draft(&source, "http://localhost/vocab/local/", &TermStates::new());

        assert_eq!(
            snapshot.term_iri(&snapshot.terms[0]),
            "http://localhost/vocab/local/numberOfPages"
        );
    }

    #[test]
    fn a_uri_list_entry_matches_its_own_link_unless_another_is_set() {
        let mut source = places(&[]);
        source.terms = vec![
            SourceTerm {
                link: Some("https://sws.geonames.org/2965140/".to_string()),
                ..word("https://sws.geonames.org/2965140/")
            },
            SourceTerm {
                link: Some("https://sws.geonames.org/2964574/".to_string()),
                ..word("https://sws.geonames.org/2964574/")
            },
        ];
        let mut states = TermStates::new();
        states.insert(
            "https://sws.geonames.org/2964574/".to_string(),
            TermState {
                exact_match: Some("https://www.wikidata.org/entity/Q1761".to_string()),
                ..TermState::default()
            },
        );

        let (snapshot, _) = draft(&source, NS, &states);

        let matches: Vec<Option<&str>> = snapshot
            .terms
            .iter()
            .map(|term| term.exact_match.as_deref())
            .collect();
        assert_eq!(
            matches,
            [
                Some("https://sws.geonames.org/2965140/"),
                Some("https://www.wikidata.org/entity/Q1761")
            ]
        );
    }

    #[test]
    fn adding_and_deprecating_keep_the_promise() {
        let (published, mut states) = draft(&places(&["Cork", "Dublin"]), NS, &TermStates::new());
        if let Some(state) = states.get_mut("Cork") {
            state.deprecated = true;
        }

        let (next, _) = draft(&places(&["Cork", "Dublin", "Ennis"]), NS, &states);

        assert_eq!(check(&published, &next), Ok(()));
    }

    #[test]
    fn removing_or_renaming_a_published_term_is_refused() {
        let (published, states) = draft(
            &places(&["Cork", "Dublin", "Galway"]),
            NS,
            &TermStates::new(),
        );

        let (next, _) = draft(&places(&["Cork", "Dubln"]), NS, &states);

        assert_eq!(
            check(&published, &next),
            Err(Refusal::Removed(vec![
                "Dublin".to_string(),
                "Galway".to_string()
            ]))
        );
    }

    #[test]
    fn a_moved_namespace_or_a_withdrawn_deprecation_is_refused() {
        let (_, mut states) = draft(&places(&["Cork"]), NS, &TermStates::new());
        if let Some(state) = states.get_mut("Cork") {
            state.deprecated = true;
        }
        let (published, mut states) = draft(&places(&["Cork"]), NS, &states);

        let (moved, _) = draft(&places(&["Cork"]), "https://w3id.org/places/", &states);
        assert_eq!(check(&published, &moved), Err(Refusal::NamespaceChanged));

        if let Some(state) = states.get_mut("Cork") {
            state.deprecated = false;
        }
        let (withdrawn, _) = draft(&places(&["Cork"]), NS, &states);
        assert_eq!(
            check(&published, &withdrawn),
            Err(Refusal::Undeprecated("Cork".to_string()))
        );
    }

    #[test]
    fn a_snapshot_round_trips_through_json() {
        let (snapshot, _) = draft(&places(&["Cork"]), NS, &TermStates::new());

        let stored = serde_json::to_value(&snapshot).unwrap();

        assert_eq!(
            serde_json::from_value::<Snapshot>(stored).unwrap(),
            snapshot
        );
    }

    #[test]
    fn a_slug_is_lowercase_ascii_in_hyphenated_runs() {
        assert!(is_slug("places"));
        assert!(is_slug("irish-places-2"));
        assert!(!is_slug("Places"));
        assert!(!is_slug("places-"));
        assert!(!is_slug("pl--aces"));
        assert!(!is_slug(""));
        assert!(!is_slug(&"a".repeat(65)));
        assert_eq!(suggest_slug("Irish places (2026)"), "irish-places-2026");
        assert_eq!(suggest_slug("स्थान"), "");
    }

    #[test]
    fn an_archive_namespace_names_its_slug() {
        assert_eq!(
            archive_namespace("http://localhost/", "places"),
            "http://localhost/vocab/places/"
        );
        assert_eq!(
            archive_slug("http://localhost", "http://localhost/vocab/local/"),
            Some("local")
        );
        assert_eq!(
            archive_slug("http://localhost", "http://localhost/vocab/local#"),
            Some("local")
        );
        assert_eq!(
            archive_slug("http://localhost", "http://localhost/vocab/a/b/"),
            None
        );
        assert_eq!(
            archive_slug("http://localhost", "http://example.org/vocab/local/"),
            None
        );
    }

    #[test]
    fn a_persistent_namespace_is_at_w3id_or_purl_and_not_a_bundled_vocabulary() {
        assert!(is_persistent_namespace(
            "https://w3id.org/my-archive/places/"
        ));
        assert!(is_persistent_namespace("http://purl.org/my-archive/terms#"));
        assert!(!is_persistent_namespace("https://w3id.org/"));
        assert!(!is_persistent_namespace(
            "https://w3id.org/my-archive/places"
        ));
        assert!(!is_persistent_namespace("https://example.org/places/"));
        assert!(!is_persistent_namespace("http://purl.org/dc/terms/"));
    }
}
