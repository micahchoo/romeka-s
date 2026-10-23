//! The archive's tree: set levels, series and free sets, and the one way
//! the engine reads which series takes a file and which pattern, lists and
//! slot rules that series has. The engine never walks [`Level`].
//!
//! A lifted v2 plan is one series that takes every file, so its pattern
//! matches each path relative to its source, as in v2.

use serde::{Deserialize, Serialize};

use super::sets::{EachSet, EachValue, SetNames, SetSource};
use super::{
    Grouping, ImportPlan, PlanError, RecordKind, RelPath, SheetId, SheetPlan, SlotName, SlotRule,
    SlotRules, Slots, SourceId, SourcePlan,
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
    /// Paths under a source, folders or single files, that no series takes,
    /// whatever its scope: what the person left out inside a series' folder.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) outside: Vec<FolderScope>,
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
    /// A tree of these top levels with no free set, refused as
    /// [`ImportPlan::from_json`] refuses a plan on these sources. The
    /// proposer builds its trees here, so a proposal is always a valid plan.
    ///
    /// # Errors
    /// What [`ImportPlan::from_json`] refuses about the tree or the sources.
    pub fn build(
        levels: Vec<Level>,
        outside: Vec<FolderScope>,
        sources: &[SourcePlan],
    ) -> Result<Self, PlanError> {
        Self {
            levels,
            free_sets: Vec::new(),
            outside,
        }
        .checked(sources)
    }

    /// This tree with these free sets in place of its own, refused as
    /// [`Self::build`] refuses a tree.
    ///
    /// # Errors
    /// What [`ImportPlan::from_json`] refuses about the tree or the sources.
    pub fn with_free_sets(
        self,
        free_sets: Vec<SetSource>,
        sources: &[SourcePlan],
    ) -> Result<Self, PlanError> {
        Self { free_sets, ..self }.checked(sources)
    }

    fn checked(self, sources: &[SourcePlan]) -> Result<Self, PlanError> {
        let plan = ImportPlan {
            structure: self,
            ..super::default_plan(sources.to_vec())?
        };
        super::check::plan(&plan)?;
        Ok(plan.structure)
    }

    /// The tree a v2 plan is: one series `all` with no set, taking every
    /// file. Its `item.sets` rule is the one free set, read as v2 read it.
    pub(super) fn lifted(
        grouping: Grouping,
        example: Option<serde_json::Map<String, serde_json::Value>>,
        lists: Vec<SheetPlan>,
        mut slots: Slots,
    ) -> Self {
        let free_sets = slots
            .item
            .shift_remove(&SlotName::Sets)
            .map(|rule| {
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
            outside: Vec::new(),
        }
    }

    /// The series that takes this file, and the path its pattern matches:
    /// the series whose scope folder is the longest prefix of the file's
    /// path in its source, with `all` the shortest prefix. The path is
    /// relative to that folder. `Outside` for a file in no scope.
    #[must_use]
    pub fn place<'p>(&self, source: &SourceId, path: &'p RelPath) -> Placement<'p> {
        let path = path.as_str();
        let left_out = self.outside.iter().any(|left| {
            left.source == *source
                && path
                    .strip_prefix(left.folder.as_str())
                    .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
        });
        if left_out {
            return Placement::Outside;
        }
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

    /// The paths left out inside a series' folder.
    #[must_use]
    pub fn outside(&self) -> &[FolderScope] {
        &self.outside
    }

    /// Every series, in plan order (depth first).
    pub fn all_series(&self) -> impl Iterator<Item = (SeriesIx, &Series)> {
        let mut all = Vec::new();
        walk(&self.levels, &mut |_, ix, series| {
            all.push((SeriesIx(ix), series));
        });
        all.into_iter()
    }

    /// Each series' id as the wire names it, by [`SeriesIx::index`]: `None`
    /// for the one series of a plan of one, so the wire and the record keys
    /// of a lifted v2 plan stay v2's.
    #[must_use]
    pub fn series_names(&self) -> Vec<Option<&LevelId>> {
        let ids: Vec<&LevelId> = self.all_series().map(|(_, series)| &series.id).collect();
        let several = ids.len() > 1;
        ids.into_iter().map(|id| several.then_some(id)).collect()
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

    /// The set levels of this structure that hold the series at `at` and no
    /// other, outermost first: what a series written by hand keeps around
    /// it. A set level over several series is the proposer's to make again.
    #[must_use]
    pub fn sets_alone_around(&self, at: SeriesIx) -> Vec<&SetLevel> {
        fn path_to<'s>(
            levels: &'s [Level],
            at: usize,
            next: &mut usize,
            path: &mut Vec<&'s SetLevel>,
        ) -> bool {
            for level in levels {
                match level {
                    Level::Series(_) if *next == at => return true,
                    Level::Series(_) => *next += 1,
                    Level::Set(set) => {
                        path.push(set);
                        if path_to(&set.levels, at, next, path) {
                            return true;
                        }
                        path.pop();
                    }
                }
            }
            false
        }
        let mut path = Vec::new();
        if !path_to(&self.levels, at.0, &mut 0, &mut path) {
            return Vec::new();
        }
        let alone = |set: &SetLevel| {
            let mut series = 0;
            walk(&set.levels, &mut |_, _, _| series += 1);
            series == 1
        };
        let held = path.iter().rev().take_while(|set| alone(set)).count();
        path.split_off(path.len() - held)
    }

    /// `level` inside [`Self::sets_alone_around`] the series at `at`.
    #[must_use]
    pub fn inside_its_sets(&self, at: SeriesIx, level: Level) -> Level {
        self.sets_alone_around(at)
            .into_iter()
            .rev()
            .fold(level, |inner, set| {
                Level::Set(SetLevel::new(set.id.clone(), set.set.clone(), vec![inner]))
            })
    }

    /// Sets every item of the import is also put in, after its tree sets.
    #[must_use]
    pub fn free_sets(&self) -> &[SetSource] {
        &self.free_sets
    }

    /// Add a free set after the others.
    pub fn push_free_set(&mut self, set: SetSource) {
        self.free_sets.push(set);
    }

    /// Add a list to the series, after its other lists.
    pub fn push_list(&mut self, at: SeriesIx, list: SheetPlan) {
        self.series_mut(at).lists.push(list);
    }

    /// The list with this id, to edit.
    pub fn list_mut(&mut self, id: &SheetId) -> Option<&mut SheetPlan> {
        let mut all = Vec::new();
        leaves_mut(&mut self.levels, &mut all);
        all.into_iter()
            .flat_map(|series| series.lists.iter_mut())
            .find(|list| list.id == *id)
    }

    /// Every `each` set source's value map, to edit: the set levels', the
    /// series' own sets' and the free sets'.
    pub fn each_maps_mut(&mut self) -> Vec<&mut indexmap::IndexMap<String, String>> {
        fn each_map(source: &mut SetSource) -> Option<&mut indexmap::IndexMap<String, String>> {
            match source {
                SetSource::Each(EachValue { each }) => Some(&mut each.map),
                _ => None,
            }
        }
        fn go<'s>(
            levels: &'s mut [Level],
            out: &mut Vec<&'s mut indexmap::IndexMap<String, String>>,
        ) {
            for level in levels {
                match level {
                    Level::Series(series) => out.extend(series.set.as_mut().and_then(each_map)),
                    Level::Set(set) => {
                        out.extend(each_map(&mut set.set));
                        go(&mut set.levels, out);
                    }
                }
            }
        }
        let mut out = Vec::new();
        go(&mut self.levels, &mut out);
        out.extend(self.free_sets.iter_mut().filter_map(each_map));
        out
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

impl SetLevel {
    /// A set level over `levels`.
    #[must_use]
    pub fn new(id: LevelId, set: SetSource, levels: Vec<Level>) -> Self {
        Self { id, set, levels }
    }

    /// Unique among the plan's levels.
    #[must_use]
    pub fn id(&self) -> &LevelId {
        &self.id
    }

    /// The set its items join.
    #[must_use]
    pub fn set(&self) -> &SetSource {
        &self.set
    }
}

impl Series {
    /// A series with no set, no list and no slot rule.
    #[must_use]
    pub fn new(id: LevelId, scope: SeriesScope, grouping: Grouping) -> Self {
        Self {
            id,
            set: None,
            scope,
            grouping,
            example: None,
            lists: Vec::new(),
            slots: Slots::default(),
        }
    }

    /// The series with this set: its items join it.
    #[must_use]
    pub fn with_set(self, set: SetSource) -> Self {
        Self {
            set: Some(set),
            ..self
        }
    }

    /// The series with these lists.
    #[must_use]
    pub fn with_lists(self, lists: Vec<SheetPlan>) -> Self {
        Self { lists, ..self }
    }

    /// The series with this rule for a slot of one record kind.
    #[must_use]
    pub fn with_rule(mut self, kind: RecordKind, slot: SlotName, rule: SlotRule) -> Self {
        let rules = match kind {
            RecordKind::Item => &mut self.slots.item,
            RecordKind::Media => &mut self.slots.media,
            RecordKind::ItemSet => &mut self.slots.item_set,
        };
        rules.insert(slot, rule);
        self
    }

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

    /// Which files the series takes.
    #[must_use]
    pub fn scope(&self) -> &SeriesScope {
        &self.scope
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
