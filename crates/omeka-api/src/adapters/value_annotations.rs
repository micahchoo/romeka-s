//! `ValueAnnotationAdapter` registration.

use crate::adapters::AdapterSpec;
use crate::routes::Operation;

/// `Omeka\Api\Adapter\ValueAnnotationAdapter`.
pub const SPEC: AdapterSpec = AdapterSpec {
    name: "value_annotations",
    representation: "Omeka\\Api\\Representation\\ValueAnnotationRepresentation",
    operations: &[
        Operation::Search,
        Operation::Read,
        Operation::Create,
        Operation::Update,
        Operation::Delete,
    ],
};
