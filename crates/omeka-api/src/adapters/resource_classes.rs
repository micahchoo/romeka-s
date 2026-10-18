//! `ResourceClassAdapter` registration.

use crate::adapters::AdapterSpec;
use crate::routes::Operation;

/// `Omeka\Api\Adapter\ResourceClassAdapter`.
pub const SPEC: AdapterSpec = AdapterSpec {
    name: "resource_classes",
    representation: "Omeka\\Api\\Representation\\ResourceClassRepresentation",
    operations: &[
        Operation::Search,
        Operation::Read,
        Operation::Create,
        Operation::Update,
        Operation::Delete,
    ],
};
