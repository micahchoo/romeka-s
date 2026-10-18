//! `JobAdapter` registration.

use crate::adapters::AdapterSpec;
use crate::routes::Operation;

/// `Omeka\Api\Adapter\JobAdapter`.
pub const SPEC: AdapterSpec = AdapterSpec {
    name: "jobs",
    representation: "Omeka\\Api\\Representation\\JobRepresentation",
    operations: &[
        Operation::Search,
        Operation::Read,
        Operation::Create,
        Operation::Update,
        Operation::Delete,
    ],
};
