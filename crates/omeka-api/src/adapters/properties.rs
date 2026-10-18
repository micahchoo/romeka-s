//! `PropertyAdapter` registration.

use crate::adapters::AdapterSpec;
use crate::routes::Operation;

/// `Omeka\Api\Adapter\PropertyAdapter`.
pub const SPEC: AdapterSpec = AdapterSpec {
    name: "properties",
    representation: "Omeka\\Api\\Representation\\PropertyRepresentation",
    operations: &[
        Operation::Search,
        Operation::Read,
        Operation::Create,
        Operation::Update,
        Operation::Delete,
    ],
};
