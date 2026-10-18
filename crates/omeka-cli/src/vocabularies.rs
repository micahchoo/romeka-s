//! The four bundled vocabularies, shipped as data.
//!
//! Upstream `InstallDefaultVocabulariesTask` imports the four RDF files under
//! `application/data/vocabularies/` with `Omeka\Stdlib\RdfImporter`. The port
//! ships no PHP data directory, so the terms those files contribute at the
//! frozen SHA are extracted once and committed as
//! `data/default-vocabularies.json`. `scripts/generate-default-vocabularies.py`
//! reproduces the file from the upstream directory and documents the extraction
//! rule (RdfImporter's type set and the exact-URI local-name filter).
//!
//! The JSON is external data, so it is parsed here at the boundary. A parse
//! failure is a bug in the committed file, not an operator error.

use serde::Deserialize;

use crate::error::CliError;

/// One bundled vocabulary with its members.
#[derive(Debug, Clone, Deserialize)]
pub struct VocabularyData {
    pub prefix: String,
    pub namespace_uri: String,
    pub label: String,
    pub comment: Option<String>,
    pub classes: Vec<TermData>,
    pub properties: Vec<TermData>,
}

/// One class or property term.
#[derive(Debug, Clone, Deserialize)]
pub struct TermData {
    pub local_name: String,
    pub label: String,
    pub comment: Option<String>,
}

#[derive(Deserialize)]
struct Dataset {
    vocabularies: Vec<VocabularyData>,
}

const DEFAULT_VOCABULARIES: &str = include_str!("../data/default-vocabularies.json");

/// The default vocabularies, in the order upstream installs them.
///
/// # Errors
/// A [`CliError::Json`] when the committed data file is malformed.
pub fn default_vocabularies() -> Result<Vec<VocabularyData>, CliError> {
    let dataset: Dataset = serde_json::from_str(DEFAULT_VOCABULARIES)?;
    Ok(dataset.vocabularies)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_vocabularies_match_the_upstream_install_list() {
        let vocabularies = default_vocabularies().expect("bundled data parses");
        let prefixes: Vec<&str> = vocabularies.iter().map(|v| v.prefix.as_str()).collect();
        assert_eq!(prefixes, ["dcterms", "dctype", "bibo", "foaf"]);
        assert_eq!(vocabularies[0].namespace_uri, "http://purl.org/dc/terms/");
        let dcterms = &vocabularies[0];
        let title = dcterms
            .properties
            .iter()
            .find(|term| term.local_name == "title")
            .expect("dcterms:title is bundled");
        assert_eq!(title.label, "Title");
        assert_eq!(
            title.comment.as_deref(),
            Some("A name given to the resource.")
        );
    }
}
