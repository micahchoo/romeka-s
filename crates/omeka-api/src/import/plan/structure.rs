//! The archive's tree: set levels, series and free sets, and the one way
//! the engine reads which series takes a file and which pattern, lists and
//! slot rules that series has. The engine never walks [`Level`].
//!
//! A lifted v2 plan is one series that takes every file, so its pattern
//! matches each path relative to its source, as in v2.

use serde::{Deserialize, Serialize};

use super::sets::{EachSet, EachValue, SetNames, SetSource};
use super::{
    Combine, Grouping, RecordKind, RelPath, SheetId, SheetPlan, SlotName, SlotRule, SlotRules,
    Slots, SourceId,
};

/// The archive's tree as the person confirmed it. Its members are private
/// to `plan/`, so the engine reads them only through these calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Structure {
    /// The top levels, in the order the screen shows them.
    pub(super) levels: Vec<Level>,
    /// Sets every item of the import is also put in, after its tree sets.
    /// A set source here names no level and has no place in the tree.
    pub(super) free_sets: Vec<SetSource>,
    /// Where a lifted v2 plan's `item.sets` rule stood among its item
    /// rules, so the lower writes the plan back byte for byte. Not on the
    /// v3 wire.
    #[serde(skip)]
    pub(super) v2_sets_at: Option<usize>,
}

/// One node of the tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[expect(
    clippy::large_enum_variant,
    reason = "a plan holds a handful of levels, read far more often than moved"
)]
pub enum Level {
    Set(SetLevel),
    Series(Series),
}

/// A node that puts every item of every series under it into its set.
/// Omeka item sets are flat: the tree orders the sets on screen, it does not
/// nest them in the install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetLevel {
    pub(super) id: LevelId,
    pub(super) set: SetSource,
    /// May be empty while a draft is built; a set level with no items under
    /// it makes no set.
    pub(super) levels: Vec<Level>,
}

/// A leaf: the files in its scope, grouped into items by its own pattern,
/// described by its own lists and slot rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Series {
    pub(super) id: LevelId,
    /// The set this series' items join, or none.
    #[serde(deserialize_with = "crate::import::nullable")]
    pub(super) set: Option<SetSource>,
    pub(super) scope: SeriesScope,
    /// The pattern matches each file's path relative to the scope folder
    /// that holds it.
    pub(super) grouping: Grouping,
    /// The admin's mark-an-example state, kept and never read.
    #[serde(deserialize_with = "crate::import::nullable")]
    pub(super) example: Option<serde_json::Map<String, serde_json::Value>>,
    /// A list joins only this series' groups. Sheet ids stay unique across
    /// the whole plan.
    pub(super) lists: Vec<SheetPlan>,
    /// Without `item.sets`: memberships are levels and free sets.
    pub(super) slots: Slots,
}

/// A level's id: unique in the plan. `simple` keys on it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LevelId(pub String);

/// Which files a series takes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SeriesScope {
    /// Every file of every source (`"all"`).
    All(AllFiles),
    /// The files under these folders.
    Folders(Vec<FolderScope>),
}

/// The text `all`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AllFiles {
    All,
}

/// A folder under a plan source. `folder: ""` is the whole source.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FolderScope {
    pub source: SourceId,
    pub folder: RelPath,
}

/// A series' place in its plan: its place among the series, depth first.
/// Only [`Structure`] makes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SeriesIx(usize);

impl SeriesIx {
    /// Its place among the plan's series, for an index built once.
    #[must_use]
    pub fn index(self) -> usize {
        self.0
    }
}

/// Where a file goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement<'p> {
    /// The series that takes the file, and the path its pattern matches.
    Series { at: SeriesIx, path: &'p str },
    /// The file is in no series' scope.
    Outside,
}

/// The id of the one series a v2 plan is.
pub(super) const LIFTED: &str = "all";

impl Structure {
    /// The tree a v2 plan is: one series `all` with no set, taking every
    /// file. Its `item.sets` rule is the one free set, read as v2 read it.
    pub(super) fn lifted(
        grouping: Grouping,
        example: Option<serde_json::Map<String, serde_json::Value>>,
        lists: Vec<SheetPlan>,
        mut slots: Slots,
    ) -> Self {
        let sets = slots.item.shift_remove_full(&SlotName::Sets);
        let v2_sets_at = sets.as_ref().map(|(at, _, _)| *at);
        let free_sets = sets
            .map(|(_, _, rule)| {
                SetSource::Each(EachValue {
                    each: EachSet {
                        sources: rule.sources,
                        combine: rule.combine,
                        map: rule.map,
                        names: SetNames::Text,
                    },
                })
            })
            .into_iter()
            .collect();
        Self {
            levels: vec![Level::Series(Series {
                id: LevelId(LIFTED.to_owned()),
                set: None,
                scope: SeriesScope::All(AllFiles::All),
                grouping,
                example,
                lists,
                slots,
            })],
            free_sets,
            v2_sets_at,
        }
    }

    /// The one series of a tree a v2 plan can say, with its free set back
    /// as its `item.sets` rule, or why the tree is not one: the exact
    /// reverse of [`Self::lifted`]. A rule a v3 plan gave ends its kind.
    pub(super) fn into_lifted(self) -> Result<Series, &'static str> {
        let mut free_sets = self.free_sets.into_iter();
        let rule = match (free_sets.next(), free_sets.next()) {
            (None, _) => None,
            (Some(SetSource::Each(EachValue { each })), None) if each.names == SetNames::Text => {
                Some(SlotRule {
                    sources: each.sources,
                    combine: each.combine,
                    map: each.map,
                })
            }
            _ => return Err("it has free sets other than the version 2 sets rule"),
        };
        let mut levels = self.levels.into_iter();
        let (Some(level), None) = (levels.next(), levels.next()) else {
            return Err("it has more than one top level, or none");
        };
        let Level::Series(mut series) = level else {
            return Err("it has a set level");
        };
        if series.id.0 != LIFTED {
            return Err("its series is not the series \"all\"");
        }
        if series.set.is_some() {
            return Err("its series has a set");
        }
        if series.scope != SeriesScope::All(AllFiles::All) {
            return Err("its series takes some folders, not every file");
        }
        if let Some(rule) = rule {
            let item = &mut series.slots.item;
            let at = self.v2_sets_at.unwrap_or(item.len()).min(item.len());
            item.shift_insert(at, SlotName::Sets, rule);
        }
        Ok(series)
    }

    /// The series that takes this file, and the path its pattern matches:
    /// the series whose scope folder is the longest prefix of the file's
    /// path in its source, with `all` the shortest prefix. The path is
    /// relative to that folder. `Outside` for a file in no scope.
    #[must_use]
    pub fn place<'p>(&self, source: &SourceId, path: &'p RelPath) -> Placement<'p> {
        let path = path.as_str();
        // `None` is `all`, shorter than every folder, the whole source too.
        let mut best: Option<(Option<usize>, usize, &'p str)> = None;
        let mut offer = |prefix: Option<usize>, ix: usize, within: &'p str| {
            if best.is_none_or(|(longest, _, _)| prefix > longest) {
                best = Some((prefix, ix, within));
            }
        };
        walk(&self.levels, &mut |_, ix, series| match &series.scope {
            SeriesScope::All(_) => offer(None, ix, path),
            SeriesScope::Folders(folders) => {
                for scope in folders.iter().filter(|scope| scope.source == *source) {
                    let folder = scope.folder.as_str();
                    let within = if folder.is_empty() {
                        Some(path)
                    } else {
                        path.strip_prefix(folder)
                            .and_then(|rest| rest.strip_prefix('/'))
                    };
                    if let Some(within) = within {
                        offer(Some(folder.len()), ix, within);
                    }
                }
            }
        });
        match best {
            Some((_, ix, path)) => Placement::Series {
                at: SeriesIx(ix),
                path,
            },
            None => Placement::Outside,
        }
    }

    /// The series by its index.
    #[must_use]
    pub fn series(&self, at: SeriesIx) -> &Series {
        let mut found = None;
        walk(&self.levels, &mut |_, ix, series| {
            if ix == at.0 {
                found = Some(series);
            }
        });
        found.unwrap_or_else(|| unreachable!("a SeriesIx names a series of its own structure"))
    }

    /// Every series, in plan order (depth first).
    pub fn all_series(&self) -> impl Iterator<Item = (SeriesIx, &Series)> {
        let mut all = Vec::new();
        walk(&self.levels, &mut |_, ix, series| {
            all.push((SeriesIx(ix), series));
        });
        all.into_iter()
    }

    /// Every list with the series that holds it, in plan order.
    pub fn lists(&self) -> impl Iterator<Item = (SeriesIx, &SheetPlan)> {
        self.all_series()
            .flat_map(|(at, series)| series.lists.iter().map(move |list| (at, list)))
    }

    /// The list with this id, with the series that holds it.
    #[must_use]
    pub fn list(&self, id: &SheetId) -> Option<(SeriesIx, &SheetPlan)> {
        self.lists().find(|(_, list)| list.id == *id)
    }

    /// The set sources of an item of this series, in the order its sets
    /// are added: the set levels above it, top first, then its own set,
    /// then the free sets. A source given twice counts once.
    #[must_use]
    pub fn memberships(&self, at: SeriesIx) -> Vec<&SetSource> {
        let mut sources: Vec<&SetSource> = Vec::new();
        walk(&self.levels, &mut |above, ix, series| {
            if ix == at.0 {
                let all = above
                    .iter()
                    .copied()
                    .chain(series.set.as_ref())
                    .chain(&self.free_sets);
                for source in all {
                    if !sources.contains(&source) {
                        sources.push(source);
                    }
                }
            }
        });
        sources
    }

    /// The free set a list's `sets` column joins: the first one read as v2
    /// read sets, made when there is none, so a lifted v2 plan keeps its
    /// one sets rule.
    pub fn text_sets_mut(&mut self) -> &mut EachSet {
        let at = self.free_sets.iter().position(|set| {
            matches!(set, SetSource::Each(EachValue { each }) if each.names == SetNames::Text)
        });
        let at = at.unwrap_or_else(|| {
            self.free_sets.push(SetSource::Each(EachValue {
                each: EachSet {
                    sources: Vec::new(),
                    combine: Combine::First,
                    map: indexmap::IndexMap::new(),
                    names: SetNames::Text,
                },
            }));
            self.free_sets.len() - 1
        });
        match &mut self.free_sets[at] {
            SetSource::Each(EachValue { each }) => each,
            _ => unreachable!("the position found or made is a text set"),
        }
    }

    /// Add a list to the series, after its other lists.
    pub fn push_list(&mut self, at: SeriesIx, list: SheetPlan) {
        self.series_mut(at).lists.push(list);
    }

    /// The series' slot rules for one record kind, to edit.
    pub fn rules_mut(&mut self, at: SeriesIx, kind: RecordKind) -> &mut SlotRules {
        let slots = &mut self.series_mut(at).slots;
        match kind {
            RecordKind::Item => &mut slots.item,
            RecordKind::Media => &mut slots.media,
            RecordKind::ItemSet => &mut slots.item_set,
        }
    }

    fn series_mut(&mut self, at: SeriesIx) -> &mut Series {
        let mut all = Vec::new();
        leaves_mut(&mut self.levels, &mut all);
        all.into_iter()
            .nth(at.0)
            .unwrap_or_else(|| unreachable!("a SeriesIx names a series of its own structure"))
    }

    /// Every set level, depth first.
    pub(super) fn set_levels(&self) -> Vec<&SetLevel> {
        fn visit<'s>(levels: &'s [Level], out: &mut Vec<&'s SetLevel>) {
            for level in levels {
                if let Level::Set(set) = level {
                    out.push(set);
                    visit(&set.levels, out);
                }
            }
        }
        let mut out = Vec::new();
        visit(&self.levels, &mut out);
        out
    }
}

/// Visit every series, depth first, with the sets of the set levels above
/// it, top first, and its index.
fn walk<'s>(levels: &'s [Level], visit: &mut impl FnMut(&[&'s SetSource], usize, &'s Series)) {
    fn go<'s>(
        levels: &'s [Level],
        above: &mut Vec<&'s SetSource>,
        next: &mut usize,
        visit: &mut impl FnMut(&[&'s SetSource], usize, &'s Series),
    ) {
        for level in levels {
            match level {
                Level::Series(series) => {
                    visit(above, *next, series);
                    *next += 1;
                }
                Level::Set(set) => {
                    above.push(&set.set);
                    go(&set.levels, above, next, visit);
                    above.pop();
                }
            }
        }
    }
    go(levels, &mut Vec::new(), &mut 0, visit);
}

fn leaves_mut<'s>(levels: &'s mut [Level], out: &mut Vec<&'s mut Series>) {
    for level in levels {
        match level {
            Level::Series(series) => out.push(series),
            Level::Set(set) => leaves_mut(&mut set.levels, out),
        }
    }
}

impl Series {
    /// Unique among the plan's levels.
    #[must_use]
    pub fn id(&self) -> &LevelId {
        &self.id
    }

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
