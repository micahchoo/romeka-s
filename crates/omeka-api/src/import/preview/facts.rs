//! The `shapes` and `histogram` sections: item records grouped by facts
//! the index states exactly (`docs/import-v2.md`, "What the admin screens
//! need at scale").
//!
//! A shape is never a guess about meaning. It is the file-count bucket, the
//! top folders, the kinds of media present, whether the pattern grouped the
//! files, whether a list row joined, and the texts of the pattern's `part`
//! and `page` groups when the person marked them. Two items share a shape
//! only when every one of these facts is equal.

use std::collections::{BTreeSet, HashMap};

use omeka_media::kind as media;
use serde::{Deserialize, Serialize};

use super::query::{FlagsBy, Page, PageQuery, RecordView, page};
use super::sample::sample;
use crate::import::evidence::FileEntry;
use crate::import::grouping::pattern::{FilePattern, PART};
use crate::import::model::{GroupKey, Model, Origin, Record};
use crate::import::plan::{ImportPlan, RecordKind};
use crate::import::scan::StoredIndex;

/// The pattern group that numbers the pages of one item.
pub const PAGE: &str = "page";
/// How many sample records a group shows.
pub const SAMPLES: usize = 3;

/// Files per item, in the buckets the admin shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Bucket {
    #[serde(rename = "0")]
    None,
    #[serde(rename = "1")]
    One,
    #[serde(rename = "2")]
    Two,
    #[serde(rename = "3")]
    Three,
    #[serde(rename = "4-6")]
    FourToSix,
    #[serde(rename = "7+")]
    SevenOrMore,
}

impl Bucket {
    pub const ALL: [Self; 6] = [
        Self::None,
        Self::One,
        Self::Two,
        Self::Three,
        Self::FourToSix,
        Self::SevenOrMore,
    ];

    #[must_use]
    pub fn of(files: usize) -> Self {
        match files {
            0 => Self::None,
            1 => Self::One,
            2 => Self::Two,
            3 => Self::Three,
            4..=6 => Self::FourToSix,
            _ => Self::SevenOrMore,
        }
    }
}

/// A file's kind, as the media pages will name it once the file is imported:
/// `omeka-media`'s kind of an uploaded file, reduced to the names the import
/// screens show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaKind {
    Image,
    Pdf,
    Audio,
    Video,
    Document,
    Other,
}

impl MediaKind {
    /// The kind of a file the scan sniffed. The extension names an office
    /// document whose bytes are only a zip. An image counts as one when its
    /// derivatives can be made, as it will on the media pages after import.
    #[must_use]
    pub fn of(sniffed: &str, path: &str) -> Self {
        let extension = std::path::Path::new(path)
            .extension()
            .and_then(std::ffi::OsStr::to_str);
        let kind = media::MediaKind::of(&media::KindInput {
            renderer: "file",
            ingester: "upload",
            media_type: Some(sniffed),
            extension,
            has_thumbnails: true,
        });
        match kind {
            media::MediaKind::LocalImage => Self::Image,
            media::MediaKind::Pdf { .. } => Self::Pdf,
            media::MediaKind::Audio => Self::Audio,
            media::MediaKind::Video => Self::Video,
            media::MediaKind::Document => Self::Document,
            media::MediaKind::IiifImage
            | media::MediaKind::Model3d
            | media::MediaKind::Captions
            | media::MediaKind::Embed
            | media::MediaKind::IiifPresentation
            | media::MediaKind::OtherFile => Self::Other,
        }
    }
}

/// What made an item record, as the admin words it when the item does not
/// fit the pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemOrigin {
    // Declared so that `Ord` puts the items the pattern did not group first,
    // as the `pattern: bool` it replaces did; ties in the groups sort by it.
    Row,
    Own,
    Pattern,
}

impl ItemOrigin {
    fn of(origin: &Origin) -> Self {
        match origin {
            Origin::Group(GroupKey::Pattern(_)) => Self::Pattern,
            Origin::Group(GroupKey::Own(_)) => Self::Own,
            Origin::Row(_) | Origin::File(_) => Self::Row,
        }
    }
}

/// The facts one item record has. The admin sends a group's facts back as
/// the `records` filter `shape`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShapeFacts {
    pub files: Bucket,
    /// The first folder of each file's path, distinct and sorted; `""` for
    /// a file at the top of its source.
    pub folders: Vec<String>,
    /// Distinct and sorted.
    pub media: Vec<MediaKind>,
    /// Why the item exists: the pattern grouped its files, a list row made
    /// it with no file, or a `move … own` decision gave a file its own item.
    pub origin: ItemOrigin,
    /// A list row joined the record, or made it.
    pub row: bool,
    /// The texts of the pattern's `part` group over the files, distinct and
    /// sorted; `None` when the pattern has no `part` group.
    #[serde(default)]
    pub parts: Option<Vec<String>>,
    /// The same for the `page` group.
    #[serde(default)]
    pub pages: Option<Vec<String>>,
}

/// What the facts read besides the model: each file's index entry, and the
/// pattern when it has a `part` or `page` group.
pub(super) struct Facts<'d> {
    entries: HashMap<(&'d str, &'d str), &'d FileEntry>,
    pattern: Option<FilePattern>,
    parts: bool,
    pages: bool,
}

impl<'d> Facts<'d> {
    pub(super) fn new(plan: &ImportPlan, stored: &'d StoredIndex) -> Self {
        let entries = stored
            .index
            .sources
            .iter()
            .flat_map(|source| {
                source
                    .walk
                    .entries
                    .iter()
                    .map(move |entry| ((source.id.0.as_str(), entry.path.as_str()), entry))
            })
            .collect();
        let pattern = FilePattern::compile(plan.grouping.pattern.as_deref()).ok();
        let has = |name: &str| {
            pattern
                .as_ref()
                .is_some_and(|pattern| pattern.group_names().iter().any(|group| group == name))
        };
        let (parts, pages) = (has(PART), has(PAGE));
        Self {
            entries,
            pattern: pattern.filter(|_| parts || pages),
            parts,
            pages,
        }
    }

    /// The facts of an item record; `None` for any other kind.
    pub(super) fn shape(&self, record: &Record) -> Option<ShapeFacts> {
        if record.kind != RecordKind::Item {
            return None;
        }
        let mut folders = BTreeSet::new();
        let mut media = BTreeSet::new();
        let mut parts = BTreeSet::new();
        let mut pages = BTreeSet::new();
        for file in &record.files {
            let path = file.path.as_str();
            folders.insert(path.split_once('/').map_or("", |(top, _)| top));
            if let Some(entry) = self.entries.get(&(file.source.0.as_str(), path)) {
                media.insert(MediaKind::of(&entry.sniffed, path));
            }
            if let Some(groups) = self.pattern.as_ref().and_then(|p| p.groups(path)) {
                if let Some(part) = groups.get(PART) {
                    parts.insert(part.text.clone());
                }
                if let Some(page) = groups.get(PAGE) {
                    pages.insert(page.text.clone());
                }
            }
        }
        Some(ShapeFacts {
            files: Bucket::of(record.files.len()),
            folders: folders.into_iter().map(str::to_string).collect(),
            media: media.into_iter().collect(),
            origin: ItemOrigin::of(&record.origin),
            row: !record.rows.is_empty(),
            parts: self.parts.then(|| parts.into_iter().collect()),
            pages: self.pages.then(|| pages.into_iter().collect()),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shapes {
    /// The items the groups cover: every item the run writes.
    pub records: usize,
    /// Largest first.
    pub groups: Page<ShapeGroup>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapeGroup {
    pub facts: ShapeFacts,
    pub count: usize,
    /// Under 2% of the items, or 7 or more files each.
    pub odd: bool,
    /// At most [`SAMPLES`], drawn by the request's seed.
    pub samples: Vec<RecordView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BucketCount {
    pub files: Bucket,
    pub count: usize,
}

pub(super) fn shapes(
    model: &Model,
    facts: &Facts<'_>,
    flags: &FlagsBy<'_>,
    query: &PageQuery,
    seed: u64,
) -> Shapes {
    let mut by: HashMap<ShapeFacts, Vec<&Record>> = HashMap::new();
    let mut records = 0;
    for record in model.kept() {
        if let Some(shape) = facts.shape(record) {
            records += 1;
            by.entry(shape).or_default().push(record);
        }
    }
    let mut groups: Vec<(ShapeFacts, Vec<&Record>)> = by.into_iter().collect();
    groups.sort_by(|(a, a_records), (b, b_records)| {
        b_records.len().cmp(&a_records.len()).then_with(|| a.cmp(b))
    });
    let groups = page(
        groups.into_iter(),
        query.page,
        query.per_page,
        |(facts, members)| {
            let count = members.len();
            ShapeGroup {
                odd: count * 50 < records || facts.files == Bucket::SevenOrMore,
                samples: sample(
                    members
                        .iter()
                        .map(|record| (u64::from(record.id.0), *record)),
                    seed,
                    SAMPLES,
                )
                .into_iter()
                .map(|record| flags.record_view(record))
                .collect(),
                facts,
                count,
            }
        },
    );
    Shapes { records, groups }
}

/// Items the run writes, by files per item; every bucket, in order.
pub(super) fn histogram(model: &Model) -> Vec<BucketCount> {
    let mut counts = [0_usize; 6];
    for record in model.kept() {
        if record.kind == RecordKind::Item {
            counts[Bucket::ALL
                .iter()
                .position(|bucket| *bucket == Bucket::of(record.files.len()))
                .unwrap_or(0)] += 1;
        }
    }
    Bucket::ALL
        .into_iter()
        .zip(counts)
        .map(|(files, count)| BucketCount { files, count })
        .collect()
}
