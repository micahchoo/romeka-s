//! The outcome of BHC006, pinned as text before the plan's v3 structure. The
//! shape needs a scanned draft and the install's terms, so it is read
//! through the preview route here; `omeka-api`'s `derive/tests/outcome_pins.rs`
//! pins S1 to S8 in the same text, which the harness includes.

use std::path::Path;

use super::{bhc006, outcome_text, text};

#[tokio::test]
async fn lifted_plans_keep_their_outcome() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/import_shapes/outcome_pins");
    let outcome = outcome_text(&bhc006::shape(), &bhc006::SLOTS, bhc006::plan_for_bhc006).await;
    text::assert_pinned(&dir, "bhc006", &outcome);
}
