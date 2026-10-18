//! `UserAdapter` registration.

use crate::adapters::AdapterSpec;
use crate::routes::Operation;

/// `Omeka\Api\Adapter\UserAdapter`.
pub const SPEC: AdapterSpec = AdapterSpec {
    name: "users",
    representation: "Omeka\\Api\\Representation\\UserRepresentation",
    operations: &[
        Operation::Search,
        Operation::Read,
        Operation::Create,
        Operation::Update,
        Operation::Delete,
    ],
};
