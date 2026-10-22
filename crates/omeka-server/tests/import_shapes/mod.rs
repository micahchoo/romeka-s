//! Archive shapes and the outcome an archivist wants from each, compared
//! through the preview route. The held-out shapes are in their own target,
//! `import_shapes_heldout`, which includes the same harness.

mod harness;
pub use harness::*;

pub mod bhc006;
pub mod outcome_pins;
pub mod v3_outcomes;
