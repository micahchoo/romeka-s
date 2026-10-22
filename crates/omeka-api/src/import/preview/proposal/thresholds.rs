//! Every number the proposer compares a count with. Each constant has a doc
//! comment that gives its reason; `lint_tests` fails on one that has none,
//! and on a number outside this file.

use super::Share;

/// The share of a scope's media a key shape must fit before the media of
/// another shape make no item and are listed instead (design open call 4,
/// decided 2026-10-21). The frozen BHC006 outcome does this with the 4 odd
/// names among the 696 images of `Artifacts`.
pub const KEY_SHAPE_FITS: Share = Share { n: 95, of: 100 };

/// The part values that must each recur before parts prove a name cut
/// (`RecurringParts`): two values are the least that tell two files of
/// one item apart. S1 `Artifacts` has two, `front` and `back`.
pub const MIN_RECURRING_PARTS: usize = 2;

/// The items a part value must be in to recur: two is the least count that
/// shows a value repeating. S1 `front` is in 4 items, BHC006 `a` in 279.
pub const MIN_ITEMS_PER_PART: usize = 2;

/// The items a list joined by file name must name two files or more of
/// before its rows describe files: one is enough, because rows that
/// describe items would give that item two rows. BHC006 `Artifacts` has
/// 272 such items, `Photo Narratives (PN)` none.
pub const MIN_ITEMS_WITH_FILE_ROWS: usize = 1;

/// The record kinds a column's cells must name before it is a list's kind
/// column: with one kind, every row is that kind and no column is needed.
/// S8's `Kind` names three.
pub const MIN_KINDS: usize = 2;

/// The sets a candidate must make before it is an offered split: with one,
/// every item is in it and it sorts nothing. BHC006 `Documents (D)` splits
/// into 2 by a name place (11 and 4 items), S3 into 2 (`AF`, `PH`).
pub const MIN_SETS: usize = 2;

/// The items one set of a split must hold: below two, every set holds one
/// item and the split sorts nothing. S3 `AF` holds 2.
pub const MIN_ITEMS_PER_SET: usize = 2;

/// The share of a list's rows a pairing suggestion must pair, each to one
/// file, before it is offered: below half, the rule explains a minority of
/// the list, which is then more likely about something else. BHC006
/// `BHC006_GMT_D.xlsx` pairs 15 of 15 rows by its last two places.
pub const MIN_SUGGESTION_SHARE: Share = Share { n: 1, of: 2 };
