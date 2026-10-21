//! The archive's tree: the one way the engine reads which series takes a
//! file and which pattern, lists and slot rules that series has.
//!
//! A v2 plan is one series that takes every file: its pattern matches each
//! path relative to its source, as in v2, and its `slots.item.sets` rule is
//! its one membership.

use super::{
    Grouping, RecordKind, RelPath, SheetId, SheetPlan, SlotName, SlotRule, SlotRules, Slots,
    SourceId,
};

/// How a series names the item sets of its items. Today the v2
/// `slots.item.sets` rule; a caller names this type, not [`SlotRule`], so the
/// typed set source can replace it without the caller changing.
pub type SetSource = SlotRule;

/// The tree of a plan. Its members are private to `plan/`, so the engine
/// reads them only through these calls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Structure {
    pub(super) series: Series,
}

/// A series' place in its plan. Only [`Structure`] makes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SeriesIx(());

/// Where a file goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement<'p> {
    /// The series that takes the file, and the path its pattern matches.
    Series { at: SeriesIx, path: &'p str },
    /// The file is in no series' scope.
    Outside,
}

/// One series: its pattern, its lists and its slot rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Series {
    pub(super) grouping: Grouping,
    /// The admin's mark-an-example state, kept and never read.
    pub(super) example: Option<serde_json::Map<String, serde_json::Value>>,
    pub(super) lists: Vec<SheetPlan>,
    pub(super) slots: Slots,
}

impl Structure {
    /// The series that takes this file, and the path the pattern matches.
    #[must_use]
    pub fn place<'p>(&self, _source: &SourceId, path: &'p RelPath) -> Placement<'p> {
        Placement::Series {
            at: SeriesIx(()),
            path: path.as_str(),
        }
    }

    /// The series by its index.
    #[must_use]
    pub fn series(&self, _at: SeriesIx) -> &Series {
        &self.series
    }

    /// Every series, in plan order.
    pub fn all_series(&self) -> impl Iterator<Item = (SeriesIx, &Series)> {
        std::iter::once((SeriesIx(()), &self.series))
    }

    /// The series of a plan that holds one. Every plan holds one until a
    /// plan can scope its series; each caller reads a record's rules and
    /// must then read the record's own series instead.
    #[must_use]
    pub fn sole(&self) -> SeriesIx {
        SeriesIx(())
    }

    /// Every list with the series that holds it, in plan order.
    pub fn lists(&self) -> impl Iterator<Item = (SeriesIx, &SheetPlan)> {
        self.series.lists.iter().map(|list| (SeriesIx(()), list))
    }

    /// The list with this id, with the series that holds it.
    #[must_use]
    pub fn list(&self, id: &SheetId) -> Option<(SeriesIx, &SheetPlan)> {
        self.lists().find(|(_, list)| list.id == *id)
    }

    /// The set sources of an item of this series, in the order its sets
    /// are added.
    pub fn memberships(&self, _at: SeriesIx) -> impl Iterator<Item = &SetSource> {
        self.series.slots.item.get(&SlotName::Sets).into_iter()
    }

    /// Add a list to the series, after its other lists.
    pub fn push_list(&mut self, _at: SeriesIx, list: SheetPlan) {
        self.series.lists.push(list);
    }

    /// The series' slot rules for one record kind, to edit.
    pub fn rules_mut(&mut self, _at: SeriesIx, kind: RecordKind) -> &mut SlotRules {
        let slots = &mut self.series.slots;
        match kind {
            RecordKind::Item => &mut slots.item,
            RecordKind::Media => &mut slots.media,
            RecordKind::ItemSet => &mut slots.item_set,
        }
    }
}

impl Series {
    /// The pattern as the plan holds it: text a draft may still be typing.
    #[must_use]
    pub fn pattern(&self) -> Option<&str> {
        self.grouping.pattern.as_deref()
    }

    /// Whether the identifier group is compared case-folded.
    #[must_use]
    pub fn case_fold(&self) -> bool {
        self.grouping.case_fold
    }

    /// The lists that describe this series.
    #[must_use]
    pub fn lists(&self) -> &[SheetPlan] {
        &self.lists
    }

    /// The slot rules for one record kind.
    #[must_use]
    pub fn rules(&self, kind: RecordKind) -> &SlotRules {
        match kind {
            RecordKind::Item => &self.slots.item,
            RecordKind::Media => &self.slots.media,
            RecordKind::ItemSet => &self.slots.item_set,
        }
    }

    /// Every slot rule of every kind.
    pub fn every_rule(&self) -> impl Iterator<Item = (&SlotName, &SlotRule)> {
        self.slots.rules()
    }
}
