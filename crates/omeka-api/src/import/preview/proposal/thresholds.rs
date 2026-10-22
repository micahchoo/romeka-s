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

/// The share of a split's memberships (the sum of its set sizes) that must
/// be in sets of [`MIN_ITEMS_PER_SET`] items or more before it is offered:
/// a set of one item sorts nothing, so a split that leaves most of its
/// items alone sorts most of them nothing. Half is the bar of
/// [`MIN_SUGGESTION_SHARE`]. S3 groups 2 of 3; the BHC006 replica's
/// `Extras` name place 1 groups 16 of 163, and goes to "More…".
pub const MIN_GROUPED_SHARE: Share = Share { n: 1, of: 2 };

/// The share of the items a column's split places that must each hold one
/// of its values, not several, before it is offered: an item in two sets of
/// a split is tagged, not sorted, so a column whose items mostly hold
/// several values (a keyword per row) is tags. [`MIN_GROUPED_SHARE`] does
/// not see it when the tags recur. Half, as there: a few items whose rows
/// spell one value two ways (P6's value map joins them) keep a split.
pub const MIN_ITEMS_OF_ONE_VALUE: Share = Share { n: 1, of: 2 };

/// The share of a list's rows a pairing suggestion must pair, each to one
/// file, before it is offered: below half, the rule explains a minority of
/// the list, which is then more likely about something else. BHC006
/// `BHC006_GMT_D.xlsx` pairs 15 of 15 rows by its last two places.
pub const MIN_SUGGESTION_SHARE: Share = Share { n: 1, of: 2 };

/// The share of a column's filled cells that must hold one separator before
/// the proposer cuts the column's values at it: Simple suggests a split at
/// the same bar (`suggestedSplit`). BHC006 AF `Keywords` holds a comma in
/// most cells.
pub const SPLIT_WHEN_HELD: Share = Share { n: 1, of: 2 };

/// The share of a list's shared items (items with rows for two files or
/// more) in which a column's rows must be disjoint, sharing no value, before
/// the column goes on each file (P6 roll-up rule C). Below it the column
/// goes on the item, where a wrong default loses no value. BHC006 AF
/// `Keywords` is disjoint in 0 of 272 items, `Digitized by` in 1.
pub const FILE_WHEN_DISJOINT: Share = Share { n: 1, of: 2 };
