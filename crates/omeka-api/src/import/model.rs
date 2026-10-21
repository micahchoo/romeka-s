//! The model a derive returns: what the import would write, and every
//! finding about it (`docs/import-v2.md`, "The derive", step 7).
//!
//! The model holds only ordered collections (`Vec`, `BTreeMap`), so its
//! serialisation, and with it the [`Model::digest`], is a function of the
//! derive's input alone.

use std::collections::BTreeMap;

use serde::Serialize;

use super::decisions::{EntryRef, FileRef, RowRef};
use super::plan::{
    FlagKind, FlagPolicy, LevelId, Owner, RecordKind, SeriesIx, SheetId, SlotName, SourceId,
    Structure, Term,
};
use super::values::{Value, Visibility};

/// A record's place in [`Model::records`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct RecordId(pub u32);

impl RecordId {
    #[must_use]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// What groups files into one item record.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GroupKey {
    /// The pattern's `identifier` group text, NFC, case-folded when the
    /// series says so.
    Pattern(PatternKey),
    /// A file a `move` decision gave a group of its own. Its path is the
    /// key text, but it never merges with a pattern group of that text.
    Own(FileRef),
}

impl GroupKey {
    /// The text joins and the default `dcterms:identifier` read.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::Pattern(key) => &key.text,
            Self::Own(file) => file.path.as_str(),
        }
    }
}

/// A pattern group's key. Two series can make the same text, so in a plan
/// of several series the key also names its series. In a plan of one it
/// does not, and the record key stays what v2 wrote (`Origin::key`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PatternKey {
    series: Option<LevelId>,
    text: String,
}

/// The series part of each series' pattern keys, read once per derive.
pub(crate) struct PatternKeys(Vec<Option<LevelId>>);

impl PatternKeys {
    #[must_use]
    pub(crate) fn new(structure: &Structure) -> Self {
        let ids: Vec<&LevelId> = structure
            .all_series()
            .map(|(_, series)| series.id())
            .collect();
        let several = ids.len() > 1;
        Self(
            ids.into_iter()
                .map(|id| several.then(|| id.clone()))
                .collect(),
        )
    }

    /// The key of `text` grouped by the series `at`.
    #[must_use]
    pub(crate) fn key(&self, at: SeriesIx, text: String) -> PatternKey {
        PatternKey {
            series: self.0[at.index()].clone(),
            text,
        }
    }
}

/// The text alone in a plan of one series, as v2 wrote it; with the
/// series as `{ "series", "text" }` in a plan of several.
impl Serialize for PatternKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let Some(series) = &self.series else {
            return serializer.serialize_str(&self.text);
        };
        let mut key = serializer.serialize_struct("PatternKey", 2)?;
        key.serialize_field("series", series)?;
        key.serialize_field("text", &self.text)?;
        key.end()
    }
}

/// Where a record comes from. It gives the record the stable key the
/// ledger stores, so a retry knows which records it already wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Origin {
    /// The item of a file group.
    Group(GroupKey),
    /// The media of one file of a group.
    File(FileRef),
    /// An item with no files, a media row or an item-set row.
    Row(RowRef),
}

impl Origin {
    /// The ledger's record key: `group:<key>` in a plan of one series,
    /// `group:<series>/<key>` in a plan of several.
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Self::Group(GroupKey::Pattern(PatternKey { series, text })) => match series {
                Some(series) => format!("group:{}/{text}", series.0),
                None => format!("group:{text}"),
            },
            Self::Group(GroupKey::Own(file)) => format!("own:{}/{}", file.source.0, file.path),
            Self::File(file) => format!("file:{}/{}", file.source.0, file.path),
            Self::Row(row) => format!("row:{}/{}", row.sheet.0, row.key.as_str()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub id: RecordId,
    pub kind: RecordKind,
    /// The series whose slot rules the record follows. Its origin and rows
    /// fix it, so the digest leaves it out.
    #[serde(skip)]
    pub series: SeriesIx,
    pub origin: Origin,
    /// An item's kept files in media order, or a media record's file.
    pub files: Vec<FileRef>,
    /// The rows that made the record or joined it, at most one per sheet.
    pub rows: Vec<RowRef>,
    /// A media record's item.
    pub parent: Option<Parent>,
    /// An item's media records that are not left out, in media order. The
    /// first is the primary media.
    pub media: Vec<RecordId>,
    /// Every slot's values after the rule, the map and the `set` decisions.
    pub slots: BTreeMap<SlotName, Vec<SlotValue>>,
    pub resolved: Resolved,
    /// In-scope resources the record's match key finds.
    pub matches: Vec<i64>,
    pub fate: Fate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Parent {
    Record(RecordId),
    /// An item in the install, by id.
    Existing(i64),
    /// An item key that names no item.
    Missing(String),
}

/// A value and, for a `resource` value, what it links to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotValue {
    #[serde(flatten)]
    pub value: Value,
    pub link: Option<Link>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Link {
    Existing(i64),
    /// Written in the run's second pass, once every record exists.
    Record(RecordId),
}

/// The named slots, read as what they name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolved {
    pub sets: Vec<SetRef>,
    pub template: Option<TemplateRef>,
    pub class: Option<Term>,
    pub owner: OwnerRef,
    pub visibility: Visibility,
    pub sites: Vec<i64>,
    pub media_order: Option<String>,
    pub ingester: Option<String>,
    pub media_source: Option<String>,
    pub item_key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SetRef {
    Existing(i64),
    /// This import's item-set record.
    Record(RecordId),
    /// An index into [`Model::new_sets`].
    New(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TemplateRef {
    Existing(i64),
    /// An index into the plan's `newTypes`.
    New(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OwnerRef {
    Is(Owner),
    /// A value that is neither `runner`, a user id nor a user's email. The
    /// record gets the plan's default owner, which Check states.
    Unknown(String),
}

/// What the run does with a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Fate {
    Create,
    /// The plan's action on this existing resource.
    Act(i64),
    Left(LeftBy),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LeftBy {
    /// A `leave` or `move` decision, by its index in the plan.
    Decision(usize),
    Flag(FlagKind),
    /// The item it belongs to is left out.
    Parent,
    /// An item whose every file is left out and that no row joined.
    NoFiles,
    /// A file with the same bytes as a file the import keeps
    /// (`anomalies.identical: keepOne`).
    Identical,
    /// An item no list row joined (`anomalies.itemsWithoutRow: leave`).
    NoRow,
    /// A file in no series' scope: outside the structure.
    Outside,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Flag {
    pub kind: FlagKind,
    pub subject: Subject,
    pub slot: Option<SlotName>,
    /// `None` for a blocking kind.
    pub policy: Option<FlagPolicy>,
    /// The value, the refused type, or what else the flag names.
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Subject {
    Record(RecordId),
    Entry(EntryRef),
    Sheet(SheetId),
    Plan,
}

/// One entry of the evidence and where it landed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryState {
    pub entry: EntryRef,
    /// The media record a file lands in, or the record a row made or joined.
    pub record: Option<RecordId>,
    pub left: Option<LeftBy>,
    /// A file the pattern matched no `identifier` in.
    pub unmatched: bool,
    /// A file with a sniffed media type: the admin offers it as an example.
    pub example: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSet {
    /// As the first record wrote it.
    pub title: String,
    pub visibility: Visibility,
    pub is_open: bool,
}

/// A template the run creates from the mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTypeFields {
    /// An index into the plan's `newTypes`.
    pub new_type: usize,
    /// The required field first, then each term a record using the type
    /// writes, in term order.
    pub fields: Vec<Term>,
    pub required: Term,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Anomaly {
    /// An item whose files lie in more than one folder.
    SpanFolders {
        record: RecordId,
        folders: Vec<String>,
    },
    /// An item with more than 4 times the median file count and more than
    /// 5 files.
    LargeGroup {
        record: RecordId,
        files: usize,
        median: usize,
    },
    /// A key that exists in the install, but only outside the match scope.
    OutsideScope {
        record: RecordId,
        key: String,
        ids: Vec<i64>,
    },
    UnmappedColumns {
        sheet: SheetId,
        columns: Vec<(u32, String)>,
    },
    /// Two decisions on different entries of one record set one slot.
    DecisionOverlap {
        record: RecordId,
        slot: SlotName,
        decisions: Vec<usize>,
    },
    /// Entries no record uses, less those a decision or the structure left
    /// out.
    UnusedEntries { entries: Vec<EntryRef> },
    /// What the walk of a source skipped or decoded.
    Walk {
        source: SourceId,
        symlinks: u64,
        dot: u64,
        special: u64,
        unreadable: Vec<String>,
        unusable_names: Vec<String>,
        collisions: Vec<String>,
        decoded: Vec<String>,
    },
    /// The sheet's bytes differ from the hash the plan stores.
    SheetChanged {
        sheet: SheetId,
        stored: String,
        now: String,
    },
    /// Duplicate and empty headers.
    HeaderNotes { sheet: SheetId, notes: Vec<String> },
    /// A sheet the derive could not read: missing, too large, or no header
    /// line. `encoding-unsupported` is a flag instead.
    SheetUnread { sheet: SheetId, detail: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Orphan {
    /// The decision's index in the plan.
    pub decision: usize,
    /// The entry the reason is about: one of a `many` decision's entries,
    /// the missing side of a `pair`, or the file a `move` names.
    pub entry: EntryRef,
    pub reason: OrphanReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OrphanReason {
    /// The entry is not in the evidence index.
    MissingEntry,
    /// The entry lands in no record of the kind the decision targets.
    WrongTarget,
    /// A line-keyed row decision on a sheet whose bytes changed.
    LineKeyChanged,
    /// A `move` with a file that is in no group.
    NoGroup,
    /// A `move` that gives a group of its own to a file in no series'
    /// scope, which no series' slot rules describe.
    Outside,
}

/// Why the file pattern groups nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PatternState {
    /// The group names in the order the pattern writes them.
    Groups(Vec<String>),
    Missing,
    Syntax(String),
    NoIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub pattern: PatternState,
    pub records: Vec<Record>,
    pub entries: Vec<EntryState>,
    pub new_sets: Vec<NewSet>,
    pub new_types: Vec<NewTypeFields>,
    pub flags: Vec<Flag>,
    pub anomalies: Vec<Anomaly>,
    pub orphans: Vec<Orphan>,
    /// SHA-256 of the rest of the model, lower-case hex.
    #[serde(skip)]
    pub digest: String,
}

impl Model {
    #[must_use]
    pub fn record(&self, id: RecordId) -> &Record {
        &self.records[id.index()]
    }

    /// Records the run writes or acts on.
    pub fn kept(&self) -> impl Iterator<Item = &Record> {
        self.records
            .iter()
            .filter(|record| !matches!(record.fate, Fate::Left(_)))
    }

    /// Whether a blocking flag stops the run.
    #[must_use]
    pub fn blocked(&self) -> bool {
        self.flags.iter().any(|flag| flag.policy.is_none())
    }

    /// Hash the model's JSON as it is written. The bytes are those of
    /// `serde_json::to_vec`, so the digest of a model does not depend on how
    /// it is hashed.
    pub(crate) fn seal(mut self) -> Self {
        use sha2::{Digest, Sha256};
        // serde_json writes token by token; the buffer hands the hasher
        // whole blocks.
        let mut writer = std::io::BufWriter::with_capacity(64 * 1024, Sha256::new());
        let hasher = serde_json::to_writer(&mut writer, &self)
            .ok()
            .and_then(|()| writer.into_inner().ok());
        // Every map key is a string and the hasher never fails, so the
        // error arm is unreachable; it digests nothing rather than panic.
        let hash = hasher.map_or_else(|| Sha256::digest([]), Digest::finalize);
        self.digest = omeka_media::text::hex_encode(&hash);
        self
    }
}
