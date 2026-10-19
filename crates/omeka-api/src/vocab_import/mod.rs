//! Bring a published vocabulary in from an RDF file or URL, and bring a stored one up to date.
//!
//! This is upstream's `Omeka\Stdlib\RdfImporter` with the admin controller's three steps
//! (`Admin/VocabularyController::importAction`, `::editAction`, `::updateAction`):
//!
//! - [`extract`] is `RdfImporter::getMembers`: parse the RDF, keep every resource typed as one of
//!   the class or property types whose URI is the namespace plus its local name, and read its
//!   label and comment in the preferred language. It is pure, so the upstream rules are tested
//!   against upstream's own answers for the same files (`tests/fixtures/rdf/`).
//! - [`diff`] is `RdfImporter::calculateDiff`: new members, and changed labels and comments. It
//!   also lists the stored members the file no longer has, which upstream does not compute. They
//!   are kept, as upstream keeps them, and so are the values that use them.
//! - [`defaults`] is upstream's four default vocabularies, the files `romeka install` reads.
//! - [`routes`] answers the three Romeka routes that reach both, and [`fetch`] is the guarded URL
//!   read behind a `url` source.
//!
//! The format and source choices are upstream's form (`Form/VocabularyForm`), so a reader who
//! knows the upstream admin finds the same options.

pub mod defaults;
pub mod diff;
pub mod fetch;
mod graph;
mod members;
pub mod routes;
mod xml;

pub use diff::{Change, Diff, MemberDiff, diff};
pub use members::{LabelPreference, Members, Term};

/// The RDF serialisations the importer reads (`VocabularyForm`'s `format` select).
///
/// Upstream lists Notation3 as a label on the `turtle` value, so an `.n3` file goes to the Turtle
/// parser there and here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// `[Autodetect]`: by the URL's content type, then by the first 1024 bytes
    /// (`EasyRdf\Format::guessFormat`).
    Guess,
    JsonLd,
    NTriples,
    RdfXml,
    Turtle,
}

impl Format {
    /// The form's value for the format, as upstream posts it.
    #[must_use]
    pub fn from_form(value: &str) -> Option<Self> {
        match value {
            "guess" | "" => Some(Self::Guess),
            "jsonld" => Some(Self::JsonLd),
            "ntriples" => Some(Self::NTriples),
            "rdfxml" => Some(Self::RdfXml),
            "turtle" | "n3" => Some(Self::Turtle),
            _ => None,
        }
    }
}

/// Why a file yielded no members.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExtractError {
    /// The format could not be told from the content (`Format::guessFormat` returned null).
    #[error("the format could not be detected")]
    UnknownFormat,
    /// The parser refused the file. The text is the parser's own, with its position.
    #[error("{0}")]
    Syntax(String),
}

/// `RdfImporter::getMembers`: the classes and properties a file declares in `namespace_uri`.
///
/// `base_iri` resolves relative IRIs. Upstream parses a file against the namespace URI
/// (`$graph->parseFile($file, $format, $namespaceUri)`) and a URL against the URL itself.
/// `content_type` is the URL response's media type, which a guess reads first.
///
/// # Errors
/// [`ExtractError`] when the format cannot be detected or the file does not parse.
pub fn extract(
    bytes: &[u8],
    format: Format,
    content_type: Option<&str>,
    base_iri: &str,
    namespace_uri: &str,
    preference: &LabelPreference,
) -> Result<Members, ExtractError> {
    let syntax = graph::Syntax::choose(format, content_type, bytes)?;
    let graph = graph::Graph::parse(bytes, syntax, base_iri, namespace_uri)?;
    Ok(members::members(&graph, preference))
}
