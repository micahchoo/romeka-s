//! `ItemAdapter` registration.

use crate::adapters::AdapterSpec;
use crate::routes::Operation;

/// `Omeka\Api\Adapter\ItemAdapter`.
pub const SPEC: AdapterSpec = AdapterSpec {
    name: "items",
    representation: "Omeka\\Api\\Representation\\ItemRepresentation",
    operations: &[
        Operation::Search,
        Operation::Read,
        Operation::Create,
        Operation::Update,
        Operation::Delete,
    ],
};
