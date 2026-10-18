//! `SiteAdapter` registration.

use crate::adapters::AdapterSpec;
use crate::routes::Operation;

/// `Omeka\Api\Adapter\SiteAdapter`.
pub const SPEC: AdapterSpec = AdapterSpec {
    name: "sites",
    representation: "Omeka\\Api\\Representation\\SiteRepresentation",
    operations: &[
        Operation::Search,
        Operation::Read,
        Operation::Create,
        Operation::Update,
        Operation::Delete,
    ],
};
