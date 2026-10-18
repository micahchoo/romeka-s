//! `ResourceAdapter` registration.

use crate::adapters::AdapterSpec;
use crate::routes::Operation;

/// `Omeka\Api\Adapter\ResourceAdapter`.
///
/// The adapter declares `search` and `read` through
/// `AbstractResourceEntityAdapter` and inherits `AbstractAdapter::create`,
/// `::update` and `::delete`, which throw `OperationNotImplementedException`.
/// It therefore holds the two read operations and no write.
pub const SPEC: AdapterSpec = AdapterSpec {
    name: "resources",
    representation: "(none; delegates to the target resource adapter)",
    operations: &[Operation::Search, Operation::Read],
};
