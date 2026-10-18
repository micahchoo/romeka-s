//! `ApiResourceAdapter` registration.

use crate::adapters::AdapterSpec;
use crate::routes::Operation;

/// `Omeka\Api\Adapter\ApiResourceAdapter`.
pub const SPEC: AdapterSpec = AdapterSpec {
    name: "api_resources",
    representation: "Omeka\\Api\\Representation\\ApiResourceRepresentation",
    operations: &[Operation::Search, Operation::Read],
};
