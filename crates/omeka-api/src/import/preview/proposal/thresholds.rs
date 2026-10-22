//! Every number the proposer compares a count with. Each constant has a doc
//! comment that gives its reason; `lint_tests` fails on one that has none,
//! and on a number outside this file.

use super::Share;

/// The share of a scope's media a key shape must fit before the media of
/// another shape make no item and are listed instead (design open call 4,
/// decided 2026-10-21). The frozen BHC006 outcome does this with the 4 odd
/// names among the 696 images of `Artifacts`.
pub const KEY_SHAPE_FITS: Share = Share { n: 95, of: 100 };
