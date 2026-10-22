//! The held-out archive shapes (`heldout/`). A builder of the import proposer
//! never reads them; only the land gate runs this target. Every path of it
//! is listed in `heldout/HELDOUT`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
#[path = "heldout/mod.rs"]
mod heldout;
mod imports_fixture;
#[path = "import_shapes/harness.rs"]
mod shapes;
