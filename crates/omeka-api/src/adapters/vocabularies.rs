//! `VocabularyAdapter` registration.

use crate::adapters::AdapterSpec;
use crate::routes::Operation;

/// `Omeka\Api\Adapter\VocabularyAdapter`.
pub const SPEC: AdapterSpec = AdapterSpec {
    name: "vocabularies",
    representation: "Omeka\\Api\\Representation\\VocabularyRepresentation",
    operations: &[
        Operation::Search,
        Operation::Read,
        Operation::Create,
        Operation::Update,
        Operation::Delete,
    ],
};
