//! `DataTypeAdapter` registration.

use crate::adapters::AdapterSpec;
use crate::routes::Operation;

/// `Omeka\Api\Adapter\DataTypeAdapter`.
pub const SPEC: AdapterSpec = AdapterSpec {
    name: "data_types",
    representation: "Omeka\\Api\\Representation\\DataTypeRepresentation",
    operations: &[Operation::Search, Operation::Read],
};
