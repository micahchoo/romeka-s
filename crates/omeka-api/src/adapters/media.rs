//! `MediaAdapter` registration.

use crate::adapters::AdapterSpec;
use crate::routes::Operation;

/// `Omeka\Api\Adapter\MediaAdapter`.
pub const SPEC: AdapterSpec = AdapterSpec {
    name: "media",
    representation: "Omeka\\Api\\Representation\\MediaRepresentation",
    operations: &[
        Operation::Search,
        Operation::Read,
        Operation::Create,
        Operation::Update,
        Operation::Delete,
    ],
};
