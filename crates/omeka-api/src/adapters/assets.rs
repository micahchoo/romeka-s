//! `AssetAdapter` registration.

use crate::adapters::AdapterSpec;
use crate::routes::Operation;

/// `Omeka\Api\Adapter\AssetAdapter`.
pub const SPEC: AdapterSpec = AdapterSpec {
    name: "assets",
    representation: "Omeka\\Api\\Representation\\AssetRepresentation",
    operations: &[
        Operation::Search,
        Operation::Read,
        Operation::Create,
        Operation::Update,
        Operation::Delete,
    ],
};
