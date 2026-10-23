//! Detector 1, folder levels (design section 2): which top folders are
//! series, which are outside, and the set each series' folder could make.
//! It counts each top folder's files by the media table and `MediaKind`,
//! and compares the places of sibling folder names. It reads no folder name
//! for its meaning. Folder sets are chosen only when two series hold most
//! of one kind of file: folders that each hold their own kind are kinds of
//! file, not collections.

use std::collections::{BTreeMap, BTreeSet};

use super::shape::{KeyShape, places};
use super::{
    OutsideFolder, OutsideReason, ScanText, ScopeCounts, accepted, is_media, kinds_most_first,
};
use crate::import::evidence::FileEntry;
use crate::import::plan::{
    Edit, FolderScope, ListFile, RelPath, Series, SeriesScope, SetFrom, SourceId,
};
use crate::import::preview::facts::MediaKind;

use super::Evidence;

/// One series as detector 1 makes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FolderSeries {
    /// The top folders it takes: several when they are numbered.
    pub folders: Vec<FolderScope>,
    /// The scopes the plan states: the folders, or the whole source when
    /// this is the only series and nothing is outside.
    pub scopes: Vec<FolderScope>,
    /// The set its folder's name could make, when the archive has two
    /// series or more and the folder has a name. Chosen only when
    /// [`Folders::sets_chosen`].
    pub set: Option<SetFrom>,
    /// Numbered folders offer their folder level as a split.
    pub numbered: bool,
    /// The plan's series the person wrote for these folders, kept as
    /// written: an `own` edit names one of them.
    pub own: Option<Series>,
}

impl FolderSeries {
    /// The scope its evidence names.
    pub fn scope(&self) -> &FolderScope {
        &self.scopes[0]
    }

    /// Whether `scope` names this series.
    pub fn names(&self, scope: &FolderScope) -> bool {
        self.scopes.contains(scope) || self.folders.contains(scope)
    }

    /// Whether `edit` asks for a choice on this series while the person
    /// owns it. No choice is offered on a series written by hand, so such an
    /// edit is not applied (design open call 4).
    pub fn mutes(&self, edit: &Edit) -> bool {
        let Some(own) = &self.own else {
            return false;
        };
        let held = |list: &ListFile| {
            own.lists()
                .iter()
                .any(|sheet| sheet.source == list.source && sheet.file == list.file)
        };
        match edit {
            Edit::Items { scope, .. } | Edit::List { scope, .. } => self.names(scope),
            Edit::Grain { list, .. } | Edit::Pair { list, .. } => held(list),
            Edit::AddSet { from }
            | Edit::DropSet { from }
            | Edit::Title { from, .. }
            | Edit::Existing { from, .. } => match from {
                SetFrom::Folder { scope }
                | SetFrom::NamePlace { scope, .. }
                | SetFrom::FolderLevel { scope, .. } => self.names(scope),
                SetFrom::Column { list, .. } => held(list),
            },
            Edit::LeaveOut { .. } | Edit::Keep { .. } | Edit::Own { .. } => false,
        }
    }
}

/// What detector 1 chose.
#[derive(Debug, Clone)]
pub(super) struct Folders {
    pub series: Vec<FolderSeries>,
    pub outside: Vec<OutsideFolder>,
    /// The paths inside a series' folder that no series takes: files that
    /// are not media, and what a `leaveOut` edit names below a top folder.
    pub left: Vec<FolderScope>,
    /// Whether the series' folder sets are chosen: two series hold most of
    /// one kind of file, so their folders sort by something else than kind.
    /// Folders that each hold their own kind (Documents, Pictures, Music)
    /// are kinds of file, and their sets are only offered.
    pub sets_chosen: bool,
}

/// A first path segment of a source, `""` for the files at its top.
struct TopFolder<'e> {
    scope: FolderScope,
    entries: Vec<&'e FileEntry>,
}

impl TopFolder<'_> {
    fn counts(&self, evidence: &Evidence<'_>) -> ScopeCounts {
        let kinds = kinds_most_first(evidence, self.entries.iter().copied());
        ScopeCounts {
            scope: self.scope.clone(),
            files: self.entries.len(),
            media: kinds.iter().map(|(_, files)| files).sum(),
            kinds,
        }
    }
}

/// Whether the media pages can show a file of this kind.
fn shown(kind: MediaKind) -> bool {
    kind != MediaKind::Other
}

/// Whether `path` is `scope`'s file, or under its folder.
fn within(scope: &FolderScope, source: &SourceId, path: &RelPath) -> bool {
    let mut segments = path.segments();
    scope.source == *source
        && scope
            .folder
            .segments()
            .all(|segment| segments.next() == Some(segment))
}

fn top_folders<'e>(evidence: &Evidence<'e>) -> Vec<TopFolder<'e>> {
    // `None` is the top of the source.
    let mut found: BTreeMap<(&SourceId, Option<&str>), Vec<&FileEntry>> = BTreeMap::new();
    for source in &evidence.index.sources {
        for entry in &source.walk.entries {
            let mut segments = entry.path.segments();
            let first = segments.next();
            let top = segments.next().and(first);
            found.entry((&source.id, top)).or_default().push(entry);
        }
    }
    found
        .into_iter()
        .filter_map(|((source, top), entries)| {
            let folder = match top {
                Some(top) => RelPath::parse(top).ok()?,
                None => RelPath::root(),
            };
            Some(TopFolder {
                scope: FolderScope {
                    source: source.clone(),
                    folder,
                },
                entries,
            })
        })
        .collect()
}

/// The groups of numbered sibling folders, by index into `folders`: names of
/// one shape with two places or more that differ in exactly one place.
fn numbered(folders: &[&FolderScope]) -> Vec<Vec<usize>> {
    // One folder name with one of its places left out.
    type Signature<'f> = (&'f SourceId, KeyShape, usize, Vec<&'f str>);
    let mut alike: BTreeMap<Signature<'_>, Vec<usize>> = BTreeMap::new();
    for (at, scope) in folders.iter().enumerate() {
        let name = scope.folder.as_str();
        let found = places(name);
        if found.len() <= 1 {
            continue;
        }
        for skip in 0..found.len() {
            let rest = found
                .iter()
                .enumerate()
                .filter(|(place, _)| *place != skip)
                .map(|(_, place)| &name[place.start..place.end])
                .collect();
            alike
                .entry((&scope.source, KeyShape::of(&found), skip, rest))
                .or_default()
                .push(at);
        }
    }
    let mut groups: Vec<Vec<usize>> = alike
        .into_values()
        .filter(|group| group.len() > 1)
        .collect();
    groups.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    let mut taken = vec![false; folders.len()];
    let mut out = Vec::new();
    for group in groups {
        let free: Vec<usize> = group.into_iter().filter(|at| !taken[*at]).collect();
        if free.len() > 1 {
            for at in &free {
                taken[*at] = true;
            }
            out.push(free);
        }
    }
    out
}

/// Detector 1 on the scan, with the plan's `leaveOut` and `keep` edits
/// replayed. Each edit it applies is marked in `applied`.
pub(super) fn detect(evidence: &Evidence<'_>, edits: &[Edit], applied: &mut [bool]) -> Folders {
    let tops = top_folders(evidence);
    let any_shown = tops.iter().any(|top| shows_media(evidence, top));
    let mut outside: Vec<OutsideFolder> = Vec::new();
    let mut kept: Vec<&TopFolder<'_>> = Vec::new();
    for top in &tops {
        if any_shown && !shows_media(evidence, top) {
            outside.push(OutsideFolder {
                counts: top.counts(evidence),
                reason: OutsideReason::NoMedia,
            });
        } else {
            kept.push(top);
        }
    }
    // A file the media pages cannot take as media makes no item, even
    // inside a folder that is a series.
    let mut left: Vec<OutsideFolder> = kept
        .iter()
        .flat_map(|top| top.entries.iter().map(|entry| (&top.scope.source, *entry)))
        .filter(|(_, entry)| accepted(evidence, entry) && !is_media(evidence, entry))
        .map(|(source, entry)| OutsideFolder {
            counts: TopFolder {
                scope: FolderScope {
                    source: source.clone(),
                    folder: entry.path.clone(),
                },
                entries: vec![entry],
            }
            .counts(evidence),
            reason: OutsideReason::NoMedia,
        })
        .collect();
    for (at, edit) in edits.iter().enumerate() {
        match edit {
            Edit::LeaveOut { scope } => {
                if let Some(found) = kept.iter().position(|top| top.scope == *scope) {
                    let top = kept.remove(found);
                    left.retain(|path| {
                        !within(scope, &path.counts.scope.source, &path.counts.scope.folder)
                    });
                    outside.push(OutsideFolder {
                        counts: top.counts(evidence),
                        reason: OutsideReason::Edit,
                    });
                } else if !left.iter().any(|path| path.counts.scope == *scope)
                    && let Some(below) = below_kept(&kept, scope)
                {
                    left.push(OutsideFolder {
                        counts: below.counts(evidence),
                        reason: OutsideReason::Edit,
                    });
                }
                // Outside by its counts is what the person asked for too.
                applied[at] = outside
                    .iter()
                    .chain(&left)
                    .any(|folder| folder.counts.scope == *scope);
            }
            Edit::Keep { scope } => {
                if let Some(found) = outside
                    .iter()
                    .position(|folder| folder.counts.scope == *scope)
                {
                    outside.remove(found);
                    if let Some(top) = tops.iter().find(|top| top.scope == *scope) {
                        kept.push(top);
                    }
                }
                left.retain(|path| path.counts.scope != *scope);
                applied[at] = below_kept(&kept, scope).is_some();
            }
            _ => {}
        }
    }
    kept.sort_by(|a, b| {
        a.scope
            .source
            .cmp(&b.scope.source)
            .then_with(|| a.scope.folder.cmp(&b.scope.folder))
    });
    let scopes: Vec<&FolderScope> = kept.iter().map(|top| &top.scope).collect();
    let groups = numbered(&scopes);
    let mut grouped = vec![None; scopes.len()];
    for (group, members) in groups.iter().enumerate() {
        for at in members {
            grouped[*at] = Some(group);
        }
    }
    let mut series: Vec<FolderSeries> = Vec::new();
    for (at, scope) in scopes.iter().enumerate() {
        let folders = match grouped[at] {
            Some(group) if groups[group][0] == at => groups[group]
                .iter()
                .map(|member| scopes[*member].clone())
                .collect(),
            Some(_) => continue,
            None => vec![(*scope).clone()],
        };
        series.push(FolderSeries {
            numbered: folders.len() > 1,
            scopes: folders.clone(),
            folders,
            set: None,
            own: None,
        });
    }
    let mut main_kinds = BTreeSet::new();
    let sets_chosen = !series.iter().all(|one| {
        let entries = tops
            .iter()
            .filter(|top| one.folders.contains(&top.scope))
            .flat_map(|top| top.entries.iter().copied());
        let main = kinds_most_first(evidence, entries)
            .first()
            .map(|(kind, _)| *kind);
        main_kinds.insert(main)
    });
    let several = series.len() > 1;
    for one in &mut series {
        let scope = &one.folders[0];
        if several && !one.numbered && !scope.folder.as_str().is_empty() {
            one.set = Some(SetFrom::Folder {
                scope: scope.clone(),
            });
        }
    }
    if let [only] = series.as_mut_slice()
        && outside.is_empty()
    {
        only.scopes = vec![FolderScope {
            source: only.folders[0].source.clone(),
            folder: RelPath::root(),
        }];
    }
    let stated = numbered_at_parent(&tops, &outside, &mut series);
    replay_own(evidence, edits, applied, &mut series);
    let left_paths = left
        .iter()
        .map(|path| path.counts.scope.clone())
        .chain(stated)
        .collect();
    outside.extend(left);
    outside.sort_by(|a, b| {
        (&a.counts.scope.source, &a.counts.scope.folder)
            .cmp(&(&b.counts.scope.source, &b.counts.scope.folder))
    });
    Folders {
        series,
        outside,
        left: left_paths,
        sets_chosen,
    }
}

/// A numbered series' items are told apart by their numbered folder (`Box 1`
/// and `Box 2` each hold a `Folder 1`), so it is scoped, as a series alone
/// is, at the folders' parent: the source. Its folders' keys then keep the
/// numbered level. Another series of the source keeps its folder, which the
/// engine prefers as the longer scope, and the source's outside folders and
/// files are returned, to be stated outside so the parent does not take
/// them. Only a source with one numbered series and no series of its own
/// files can give its top to one.
fn numbered_at_parent(
    tops: &[TopFolder<'_>],
    outside: &[OutsideFolder],
    series: &mut [FolderSeries],
) -> Vec<FolderScope> {
    let mut stated = Vec::new();
    let sources: BTreeSet<SourceId> = series
        .iter()
        .filter(|one| one.numbered)
        .map(|one| one.folders[0].source.clone())
        .collect();
    for source in sources {
        let root = FolderScope {
            source: source.clone(),
            folder: RelPath::root(),
        };
        let of_source = |one: &&mut FolderSeries| one.folders[0].source == source;
        let mut here: Vec<&mut FolderSeries> = series.iter_mut().filter(of_source).collect();
        let numbered = here.iter().filter(|one| one.numbered).count();
        let top_taken = here
            .iter()
            .any(|one| !one.numbered && one.scopes.contains(&root));
        if numbered > 1 || top_taken {
            continue;
        }
        if let Some(one) = here.iter_mut().find(|one| one.numbered) {
            one.scopes = vec![root.clone()];
        }
        for top in tops.iter().filter(|top| {
            top.scope.source == source && outside.iter().any(|out| out.counts.scope == top.scope)
        }) {
            if top.scope.folder.as_str().is_empty() {
                stated.extend(top.entries.iter().map(|entry| FolderScope {
                    source: source.clone(),
                    folder: entry.path.clone(),
                }));
            } else {
                stated.push(top.scope.clone());
            }
        }
    }
    stated
}

/// Replay the `own` edits: the series an edit's folder names keeps the
/// plan's series for that folder, and takes its folders as its scope.
fn replay_own(
    evidence: &Evidence<'_>,
    edits: &[Edit],
    applied: &mut [bool],
    series: &mut [FolderSeries],
) {
    for (at, edit) in edits.iter().enumerate() {
        let Edit::Own { scope } = edit else {
            continue;
        };
        let written = evidence
            .structure
            .all_series()
            .map(|(_, written)| written)
            .find(|written| match written.scope() {
                SeriesScope::Folders(folders) => folders.contains(scope),
                SeriesScope::All(_) => scope.folder.as_str().is_empty(),
            });
        let found = series.iter_mut().find(|one| one.names(scope));
        if let (Some(written), Some(one)) = (written, found) {
            if let SeriesScope::Folders(folders) = written.scope() {
                one.scopes.clone_from(folders);
            }
            one.own = Some(written.clone());
            applied[at] = true;
        }
    }
}

/// The entries `scope` names inside a kept top folder, when it names any.
fn below_kept<'e>(kept: &[&TopFolder<'e>], scope: &FolderScope) -> Option<TopFolder<'e>> {
    let entries: Vec<&FileEntry> = kept
        .iter()
        .filter(|top| top.scope.source == scope.source)
        .flat_map(|top| &top.entries)
        .copied()
        .filter(|entry| within(scope, &scope.source, &entry.path))
        .collect();
    (!entries.is_empty()).then(|| TopFolder {
        scope: scope.clone(),
        entries,
    })
}

fn shows_media(evidence: &Evidence<'_>, top: &TopFolder<'_>) -> bool {
    top.entries.iter().any(|entry| {
        is_media(evidence, entry) && shown(MediaKind::of(&entry.sniffed, entry.path.as_str()))
    })
}

/// A set's name as the scan read it: the folder's name.
pub(super) fn folder_title(scope: &FolderScope) -> ScanText {
    ScanText(scope.folder.as_str().to_owned())
}
