//! `ResourceTemplateAdapter` registration.

use crate::adapters::AdapterSpec;
use crate::routes::Operation;

/// `Omeka\Api\Adapter\ResourceTemplateAdapter`.
pub const SPEC: AdapterSpec = AdapterSpec {
    name: "resource_templates",
    representation: "Omeka\\Api\\Representation\\ResourceTemplateRepresentation",
    operations: &[
        Operation::Search,
        Operation::Read,
        Operation::Create,
        Operation::Update,
        Operation::Delete,
    ],
};
