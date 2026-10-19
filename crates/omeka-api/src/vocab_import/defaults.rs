//! Upstream's four default vocabularies: `InstallDefaultVocabulariesTask::$vocabularies` and
//! the RDF files it imports from `application/data/vocabularies/`, bundled under `data/vocabularies/`
//! byte for byte as the frozen SHA ships them.

use super::{ExtractError, Format, LabelPreference, Members};

/// One entry of `InstallDefaultVocabulariesTask::$vocabularies`.
#[derive(Debug, Clone, Copy)]
pub struct DefaultVocabulary {
    pub prefix: &'static str,
    pub namespace_uri: &'static str,
    pub label: &'static str,
    pub comment: &'static str,
    /// The RDF/XML file, as upstream ships it.
    file: &'static [u8],
}

/// The default vocabularies, in the order upstream installs them.
pub const DEFAULT_VOCABULARIES: [DefaultVocabulary; 4] = [
    DefaultVocabulary {
        prefix: "dcterms",
        namespace_uri: "http://purl.org/dc/terms/",
        label: "Dublin Core",
        comment: "Basic resource metadata (DCMI Metadata Terms)",
        file: include_bytes!("../../data/vocabularies/dcterms.rdf"),
    },
    DefaultVocabulary {
        prefix: "dctype",
        namespace_uri: "http://purl.org/dc/dcmitype/",
        label: "Dublin Core Type",
        comment: "Basic resource types (DCMI Type Vocabulary)",
        file: include_bytes!("../../data/vocabularies/dctype.rdf"),
    },
    DefaultVocabulary {
        prefix: "bibo",
        namespace_uri: "http://purl.org/ontology/bibo/",
        label: "Bibliographic Ontology",
        comment: "Bibliographic metadata (BIBO)",
        file: include_bytes!("../../data/vocabularies/bibo.owl"),
    },
    DefaultVocabulary {
        prefix: "foaf",
        namespace_uri: "http://xmlns.com/foaf/0.1/",
        label: "Friend of a Friend",
        comment: "Relationships between people and organizations (FOAF)",
        file: include_bytes!("../../data/vocabularies/foaf.rdf"),
    },
];

impl DefaultVocabulary {
    /// `RdfImporter::getMembers('file', ...)` on the bundled file, parsed against the namespace
    /// with the default label and comment paths, as the install task calls it.
    ///
    /// # Errors
    /// [`ExtractError`] when the bundled file does not parse, which is a bug in the build.
    pub fn members(&self) -> Result<Members, ExtractError> {
        super::extract(
            self.file,
            Format::RdfXml,
            None,
            self.namespace_uri,
            self.namespace_uri,
            &LabelPreference::default(),
        )
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    /// `InstallDefaultVocabulariesTask::$vocabularies` and the bundled files: FOAF has both given
    /// names, with upstream's labels.
    #[test]
    fn the_bundled_files_give_upstreams_members() {
        let prefixes: Vec<&str> = DEFAULT_VOCABULARIES.iter().map(|v| v.prefix).collect();
        assert_eq!(prefixes, ["dcterms", "dctype", "bibo", "foaf"]);
        let foaf = DEFAULT_VOCABULARIES[3].members().expect("foaf.rdf parses");
        let given: Vec<(&str, &str)> = foaf
            .properties
            .iter()
            .filter(|term| term.local_name.eq_ignore_ascii_case("givenname"))
            .map(|term| (term.local_name.as_str(), term.label.as_str()))
            .collect();
        assert_eq!(
            given,
            [("givenName", "Given name"), ("givenname", "Given name")]
        );
    }
}
