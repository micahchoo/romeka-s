//! `SitePageAdapter` registration.

use crate::adapters::AdapterSpec;
use crate::routes::Operation;

/// `Omeka\Api\Adapter\SitePageAdapter`.
pub const SPEC: AdapterSpec = AdapterSpec {
    name: "site_pages",
    representation: "Omeka\\Api\\Representation\\SitePageRepresentation",
    operations: &[
        Operation::Search,
        Operation::Read,
        Operation::Create,
        Operation::Update,
        Operation::Delete,
    ],
};
