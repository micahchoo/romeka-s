//! `ItemSetAdapter` registration.

use crate::adapters::AdapterSpec;
use crate::routes::Operation;

/// `Omeka\Api\Adapter\ItemSetAdapter`.
pub const SPEC: AdapterSpec = AdapterSpec {
    name: "item_sets",
    representation: "Omeka\\Api\\Representation\\ItemSetRepresentation",
    operations: &[
        Operation::Search,
        Operation::Read,
        Operation::Create,
        Operation::Update,
        Operation::Delete,
    ],
};
