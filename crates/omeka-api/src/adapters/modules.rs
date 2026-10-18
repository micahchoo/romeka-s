//! `ModuleAdapter` registration.

use crate::adapters::AdapterSpec;
use crate::routes::Operation;

/// `Omeka\Api\Adapter\ModuleAdapter`.
pub const SPEC: AdapterSpec = AdapterSpec {
    name: "modules",
    representation: "Omeka\\Api\\Representation\\ModuleRepresentation",
    operations: &[Operation::Search, Operation::Read],
};
