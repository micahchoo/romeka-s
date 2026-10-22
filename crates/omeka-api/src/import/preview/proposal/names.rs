//! Detector 2, name parts (design section 2): where each series' item key
//! ends. It splits each media name into places, tries name cuts from the
//! fewest places to the most, and keeps the first cut its counts prove. It
//! compares places by class and parts by count, never by their text.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;

use super::folders::FolderSeries;
use super::shape::{Key, KeyShape, Place, PlaceClass, SLASH, places};
use super::thresholds::{KEY_SHAPE_FITS, MIN_ITEMS_PER_PART, MIN_RECURRING_PARTS};
use super::{CutRule, ItemCut, ScanText};
use crate::import::evidence::FileEntry;
use crate::import::join::normalize;
use crate::import::plan::{CutEdit, Edit, RelPath};
use crate::import::preview::columns::EVERY_FOLD;

/// One media file of a series, split for the cut.
pub(super) struct Name<'e> {
    /// The folders between the series' scope and the file.
    folders: Vec<String>,
    /// The name before its last dot.
    stem: &'e str,
    /// The text after the last dot; `None` without one.
    extension: Option<&'e str>,
    places: Vec<Place>,
}

impl<'e> Name<'e> {
    /// The file, as its series' pattern sees it at `within`.
    pub fn of(entry: &'e FileEntry, within: &str) -> Option<Self> {
        let name = Path::new(entry.name());
        let stem = name.file_stem()?.to_str()?;
        let extension = name
            .extension()
            .and_then(|extension| extension.to_str())
            .filter(|extension| !extension.is_empty());
        let within = RelPath::parse(within).ok()?;
        let mut folders: Vec<String> = within.segments().map(str::to_owned).collect();
        folders.pop();
        Some(Self {
            folders,
            stem,
            extension,
            places: places(stem),
        })
    }
}

/// A cut, with what the pattern needs to say it and what it counted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Cut {
    pub item: ItemCut,
    pub key: Key,
    pub counts: Counted,
}

/// What a cut does to a series' media, counted from the names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Counted {
    pub items: usize,
    /// The media the key fits.
    pub files: usize,
    /// Each item's number of files.
    pub per_item: Vec<usize>,
    /// Each part value with the number of items that hold it, most first.
    pub parts: Vec<(ScanText, usize)>,
    pub names_ending_here: usize,
    /// Media whose shape the key does not fit. They make no item.
    pub other_shapes: usize,
}

/// A series' cut and the next best one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SeriesCut {
    pub chosen: Cut,
    pub rival: Option<Cut>,
}

impl SeriesCut {
    /// Whether the cut or its rival keys items by a folder.
    pub fn has_folder_key(&self) -> bool {
        std::iter::once(&self.chosen)
            .chain(&self.rival)
            .any(|cut| matches!(cut.key, Key::Folder { .. }))
    }
}

/// One file of an item: what tells it from the item's other files.
struct Part<'n> {
    text: Option<&'n str>,
    places: usize,
    extension: Option<&'n str>,
    folders: usize,
}

/// The names grouped by a key: each item's files, by key text.
struct Grouped<'n> {
    items: BTreeMap<Vec<&'n str>, Vec<Part<'n>>>,
    other: usize,
}

fn group<'n>(names: &'n [Name<'_>], key: &Key) -> Grouped<'n> {
    let mut items: BTreeMap<Vec<&'n str>, Vec<Part<'n>>> = BTreeMap::new();
    let mut other = 0;
    for name in names {
        let folders = name.folders.len();
        let found = match key {
            Key::Path => Some((
                name.folders
                    .iter()
                    .map(String::as_str)
                    .chain([name.stem])
                    .collect(),
                Part {
                    text: None,
                    places: 0,
                    extension: name.extension,
                    folders,
                },
            )),
            Key::Whole => Some((
                vec![name.stem],
                Part {
                    text: None,
                    places: 0,
                    extension: name.extension,
                    folders,
                },
            )),
            Key::Places(shape) => shape
                .starts(&name.places)
                .then_some(())
                .zip(name.extension)
                .and_then(|((), extension)| {
                    let key = &name.places[..shape.len()];
                    let rest = &name.places[shape.len()..];
                    let key = &name.stem[key.first()?.start..key.last()?.end];
                    // The rest of the name: `a (1)`, not the places alone.
                    let rest_text = rest.first().map(|place| &name.stem[place.start..]);
                    Some((
                        vec![key],
                        Part {
                            text: rest_text,
                            places: rest.len(),
                            extension: Some(extension),
                            folders,
                        },
                    ))
                }),
            Key::Folder { depth, .. } => (folders >= usize::from(*depth))
                .then_some(())
                .zip(name.extension)
                .map(|((), extension)| {
                    (
                        name.folders[..usize::from(*depth)]
                            .iter()
                            .map(String::as_str)
                            .collect(),
                        Part {
                            text: Some(name.stem),
                            places: name.places.len(),
                            extension: Some(extension),
                            folders,
                        },
                    )
                }),
        };
        match found {
            Some((key, part)) => items.entry(key).or_default().push(part),
            None => other += 1,
        }
    }
    Grouped { items, other }
}

impl Grouped<'_> {
    /// Two files of one item with the same parts and the same extension.
    fn collisions(&self) -> impl Iterator<Item = &Part<'_>> {
        self.items.values().flat_map(|parts| {
            parts.iter().filter(move |part| {
                parts
                    .iter()
                    .filter(|other| other.text == part.text && other.extension == part.extension)
                    .count()
                    > 1
            })
        })
    }

    fn several(&self) -> impl Iterator<Item = &Vec<Part<'_>>> {
        self.items.values().filter(|parts| parts.len() > 1)
    }

    fn counted(&self) -> Counted {
        let mut parts: BTreeMap<&str, usize> = BTreeMap::new();
        for files in self.items.values() {
            let held: BTreeSet<&str> = files.iter().filter_map(|part| part.text).collect();
            for text in held {
                *parts.entry(text).or_default() += 1;
            }
        }
        let mut parts: Vec<(ScanText, usize)> = parts
            .into_iter()
            .map(|(text, items)| (ScanText(text.to_owned()), items))
            .collect();
        parts.sort_by(|(a, a_items), (b, b_items)| b_items.cmp(a_items).then_with(|| a.cmp(b)));
        let per_item: Vec<usize> = self.items.values().map(Vec::len).collect();
        Counted {
            items: per_item.len(),
            files: per_item.iter().sum(),
            per_item,
            names_ending_here: self
                .items
                .values()
                .flatten()
                .filter(|part| part.text.is_none())
                .count(),
            parts,
            other_shapes: self.other,
        }
    }
}

/// Which rule makes this name cut valid, if one does.
fn valid(grouped: &Grouped<'_>, key_places: usize, counted: &Counted) -> Option<CutRule> {
    let collides = grouped.collisions().next().is_some();
    let mut several = grouped.several().peekable();
    if collides || several.peek().is_none() {
        return None;
    }
    let short_parts = several.flatten().all(|part| part.places < key_places);
    if !short_parts {
        return None;
    }
    let recurring = counted
        .parts
        .iter()
        .filter(|(_, items)| *items >= MIN_ITEMS_PER_PART)
        .count();
    if recurring >= MIN_RECURRING_PARTS {
        Some(CutRule::RecurringParts)
    } else if counted.names_ending_here > 0 {
        Some(CutRule::NamesEndHere)
    } else {
        None
    }
}

fn places_cut(shape: KeyShape, rule: CutRule, counts: Counted) -> Cut {
    Cut {
        item: ItemCut::Places {
            places: u8::try_from(shape.len()).unwrap_or(u8::MAX),
            rule,
        },
        key: Key::Places(shape),
        counts,
    }
}

/// The first valid name cut, from the fewest places to the most; at each
/// count of places, the key shape most names start with first.
fn first_valid(names: &[Name<'_>]) -> Option<Cut> {
    let most = names.iter().map(|name| name.places.len()).max()?;
    for key_places in 1..=most {
        let mut shapes: BTreeMap<KeyShape, usize> = BTreeMap::new();
        for name in names.iter().filter(|name| name.places.len() >= key_places) {
            *shapes
                .entry(KeyShape::of(&name.places[..key_places]))
                .or_default() += 1;
        }
        let mut shapes: Vec<(KeyShape, usize)> = shapes.into_iter().collect();
        shapes.sort_by(|(a, a_names), (b, b_names)| b_names.cmp(a_names).then_with(|| a.cmp(b)));
        for (shape, _) in shapes {
            let grouped = group(names, &Key::Places(shape.clone()));
            let counted = grouped.counted();
            if let Some(rule) = valid(&grouped, key_places, &counted) {
                return Some(places_cut(shape, rule, counted));
            }
        }
    }
    None
}

/// The cut of the archive as found: each name an item. When whole names
/// collide, the same name is in two folders: then the folder the colliding
/// files are in is the item when every folder there restarts its names
/// (each folder of several files holds a name another folder holds, as
/// page numbers do), and is only part of each file's key otherwise (a
/// camera counter that restarted once).
fn as_found(names: &[Name<'_>]) -> (Cut, Option<Cut>) {
    let whole = group(names, &Key::Whole);
    let depth = whole
        .collisions()
        .map(|part| part.folders)
        .min()
        .and_then(|depth| u8::try_from(depth).ok())
        .filter(|depth| *depth > 0);
    let Some(depth) = depth else {
        let whole = Cut {
            item: ItemCut::Whole,
            key: Key::Whole,
            counts: whole.counted(),
        };
        return (whole, None);
    };
    let each_file = Cut {
        item: ItemCut::Whole,
        counts: group(names, &Key::Path).counted(),
        key: Key::Path,
    };
    let key = Key::Folder { depth, lead: None };
    let folder = Cut {
        item: ItemCut::Folder { depth },
        counts: group(names, &key).counted(),
        key,
    };
    if every_folder_restarts(names, usize::from(depth)) {
        (folder, Some(each_file))
    } else {
        (each_file, Some(folder))
    }
}

/// Whether each folder `depth` levels down that holds several files holds
/// a name (with its extension) that another folder there holds too.
fn every_folder_restarts(names: &[Name<'_>], depth: usize) -> bool {
    let mut by_folder: BTreeMap<&[String], BTreeSet<(&str, Option<&str>)>> = BTreeMap::new();
    for name in names.iter().filter(|name| name.folders.len() >= depth) {
        by_folder
            .entry(&name.folders[..depth])
            .or_default()
            .insert((name.stem, name.extension));
    }
    let mut holders: BTreeMap<(&str, Option<&str>), usize> = BTreeMap::new();
    for held in by_folder.values() {
        for name in held {
            *holders.entry(*name).or_default() += 1;
        }
    }
    by_folder
        .values()
        .filter(|held| held.len() > 1)
        .all(|held| held.iter().any(|name| holders[name] > 1))
}

/// A folder cut whose folder names a list names by their first places
/// keys its items by those places. `cells` is every cell of every list,
/// folded as a join folds with every fold on. The lead is the fewest first
/// places of the key's last folder name that have one shape in every item,
/// tell every item apart, and are in more cells than the whole key is.
pub(super) fn lead_by_lists(cut: &mut SeriesCut, names: &[Name<'_>], cells: &HashSet<String>) {
    let folder = std::iter::once(&mut cut.chosen)
        .chain(cut.rival.as_mut())
        .find(|cut| matches!(cut.key, Key::Folder { lead: None, .. }));
    let Some(folder) = folder else {
        return;
    };
    let Key::Folder { depth, .. } = folder.key else {
        return;
    };
    let at = usize::from(depth);
    let items: BTreeSet<&[String]> = names
        .iter()
        .filter(|name| name.folders.len() >= at)
        .map(|name| &name.folders[..at])
        .collect();
    let named = |text: &str| cells.contains(&normalize(text, EVERY_FOLD));
    let whole = items.iter().filter(|key| named(&key.join(SLASH))).count();
    let lasts: Vec<(&str, Vec<Place>)> = items
        .iter()
        .filter_map(|key| key.last())
        .map(|last| (last.as_str(), places(last)))
        .collect();
    let fewest = lasts
        .iter()
        .map(|(_, found)| found.len())
        .min()
        .unwrap_or_default();
    let mut best: Option<(usize, KeyShape)> = None;
    for count in 1..=fewest {
        let shapes: BTreeSet<KeyShape> = lasts
            .iter()
            .map(|(_, found)| KeyShape::of(&found[..count]))
            .collect();
        let leads: BTreeSet<&str> = lasts
            .iter()
            .filter_map(|(last, found)| {
                Some(&last[found.first()?.start..found.get(count - 1)?.end])
            })
            .collect();
        let mut shapes = shapes.into_iter();
        let (Some(shape), None) = (shapes.next(), shapes.next()) else {
            continue;
        };
        if leads.len() < items.len() {
            continue;
        }
        let hits = leads.iter().filter(|lead| named(lead)).count();
        if hits > whole && best.as_ref().is_none_or(|(most, _)| hits > *most) {
            best = Some((hits, shape));
        }
    }
    if let Some((_, lead)) = best {
        folder.key = Key::Folder {
            depth,
            lead: Some(lead),
        };
    }
}

/// Whether a key that fits `files` of `of` media fits enough of them for
/// the rest to be listed rather than make a second scheme.
fn fits(files: usize, of: usize) -> bool {
    files * KEY_SHAPE_FITS.of >= KEY_SHAPE_FITS.n * of
}

/// Detector 2 on one series' media.
pub(super) fn cut(names: &[Name<'_>]) -> SeriesCut {
    let (found, found_rival) = as_found(names);
    match first_valid(names) {
        Some(valid) if fits(valid.counts.files, names.len()) => SeriesCut {
            chosen: valid,
            rival: Some(found),
        },
        Some(valid) => SeriesCut {
            chosen: found,
            rival: Some(valid),
        },
        None => SeriesCut {
            chosen: found,
            rival: found_rival,
        },
    }
}

/// A series that takes each name as an item gets, as its rival, the key
/// shape a sibling series proved, when that shape starts every one of its
/// names. It never wins alone: no count in the folder decides.
pub(super) fn same_as_sibling(cuts: &mut [SeriesCut], names: &[Vec<Name<'_>>]) {
    let proved: Vec<Option<KeyShape>> = cuts
        .iter()
        .map(|cut| match &cut.chosen.key {
            Key::Places(shape) => Some(shape.clone()),
            Key::Whole | Key::Path | Key::Folder { .. } => None,
        })
        .collect();
    for (at, cut) in cuts.iter_mut().enumerate() {
        if cut.chosen.key != Key::Whole || cut.rival.is_some() || names[at].is_empty() {
            continue;
        }
        let sibling = proved
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != at)
            .find_map(|(_, shape)| {
                let shape = shape.as_ref()?;
                names[at]
                    .iter()
                    .all(|name| shape.starts(&name.places) && name.extension.is_some())
                    .then(|| shape.clone())
            });
        let Some(shape) = sibling else {
            continue;
        };
        let grouped = group(&names[at], &Key::Places(shape.clone()));
        // A sibling's key that keeps each name an item is no other choice.
        if grouped.several().next().is_some() {
            cut.rival = Some(places_cut(shape, CutRule::SameAsSibling, grouped.counted()));
        }
    }
}

fn asks_for(cut: &Cut, edit: &CutEdit) -> bool {
    match (edit, &cut.key) {
        (CutEdit::Whole, Key::Whole | Key::Path) => true,
        (CutEdit::Places { places }, Key::Places(shape)) => shape.len() == usize::from(*places),
        (CutEdit::Folder { depth }, Key::Folder { depth: found, .. }) => depth == found,
        _ => false,
    }
}

/// Replay the `items` edits: each takes the series' rival, when the rival
/// is the cut it asks for. Each edit it applies is marked in `applied`.
pub(super) fn replay(
    cuts: &mut [SeriesCut],
    series: &[FolderSeries],
    edits: &[Edit],
    applied: &mut [bool],
) {
    for (at, edit) in edits.iter().enumerate() {
        let Edit::Items { scope, cut: asked } = edit else {
            continue;
        };
        let Some(found) = series
            .iter()
            .position(|one| one.scopes.contains(scope) || one.folders.contains(scope))
        else {
            continue;
        };
        let one = &mut cuts[found];
        if asks_for(&one.chosen, asked) {
            applied[at] = true;
        } else if let Some(rival) = one.rival.take_if(|rival| asks_for(rival, asked)) {
            one.rival = Some(std::mem::replace(&mut one.chosen, rival));
            applied[at] = true;
        }
    }
}

/// One place every item's key holds, after the same classes before it: the
/// classes up to it, and each value with the items that hold it.
pub(super) struct PlaceValues<'n> {
    pub classes: KeyShape,
    pub values: BTreeMap<&'n str, usize>,
}

/// Each name place a series' items could be sorted by, for its key: the
/// key's places for a name cut, the name's places for `Whole`. A folder key
/// has no name place. A place is counted only when every item's key holds
/// it, with the same classes before it, so one pattern can read it.
pub(super) fn place_values<'n>(names: &'n [Name<'_>], key: &Key) -> Vec<PlaceValues<'n>> {
    let mut items: BTreeMap<&'n str, Vec<(PlaceClass, &'n str)>> = BTreeMap::new();
    for name in names {
        let held = match key {
            Key::Whole => &name.places[..],
            Key::Places(shape) if shape.starts(&name.places) && name.extension.is_some() => {
                &name.places[..shape.len()]
            }
            Key::Places(_) | Key::Path | Key::Folder { .. } => continue,
        };
        let (Some(first), Some(last)) = (held.first(), held.last()) else {
            continue;
        };
        let texts = held
            .iter()
            .map(|place| (place.class, &name.stem[place.start..place.end]))
            .collect();
        items
            .entry(&name.stem[first.start..last.end])
            .or_insert(texts);
    }
    let mut keys = items.values();
    let Some(first) = keys.next() else {
        return Vec::new();
    };
    let common = keys.fold(first.len(), |common, other| {
        first
            .iter()
            .zip(other)
            .take(common)
            .take_while(|((a, _), (b, _))| a == b)
            .count()
    });
    (0..common)
        .map(|at| {
            let mut values: BTreeMap<&str, usize> = BTreeMap::new();
            for places in items.values() {
                *values.entry(places[at].1).or_default() += 1;
            }
            PlaceValues {
                classes: KeyShape::of_classes(first[..=at].iter().map(|(class, _)| *class)),
                values,
            }
        })
        .collect()
}
