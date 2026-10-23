//! The person's acts on a proposal (`docs/import-v2.md`, "Simple and
//! Advanced"). Each edit names what it changes in the
//! archive (a folder, a list, a column), never a level id, so it still
//! names the same thing when a new proposal makes new level ids. The server
//! replays the edits on every proposal; derive and run never read them.

use serde::{Deserialize, Serialize};

use super::{ColumnRef, FolderScope, JoinNormalize, JoinOn, RecordKind, RelPath, SourceId};

/// A list file by where it is. Edits and the placement oracle name lists
/// this way, never by sheet id, because a sheet id exists only in a plan.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListFile {
    pub source: SourceId,
    pub file: RelPath,
}

/// One act of the person on a proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "edit", rename_all = "camelCase", deny_unknown_fields)]
pub enum Edit {
    /// Leave a folder the proposal makes a series outside.
    LeaveOut { scope: FolderScope },
    /// Make a folder the proposal leaves outside a series.
    Keep { scope: FolderScope },
    /// Cut the series' names into items another way.
    Items { scope: FolderScope, cut: CutEdit },
    /// Describe the series with another list, or with none.
    List {
        scope: FolderScope,
        #[serde(deserialize_with = "crate::import::nullable")]
        list: Option<ListFile>,
    },
    /// Say what the list's rows describe.
    Grain {
        list: ListFile,
        describes: RecordKind,
    },
    /// Take an offered set.
    AddSet { from: SetFrom },
    /// Drop a set the proposal chose.
    DropSet { from: SetFrom },
    /// Pair the list's rows by another rule.
    Pair { list: ListFile, rule: PairRule },
    /// Give a set the title the person typed, and a description if they
    /// typed one: the only way text that no scan read enters a proposal.
    Title {
        from: SetFrom,
        title: String,
        #[serde(
            default,
            deserialize_with = "crate::import::nullable",
            skip_serializing_if = "Option::is_none"
        )]
        description: Option<String>,
    },
    /// Put the items of a set in a set already in Omeka, by its id, in
    /// place of a new set. The same question as `title`: the later wins.
    Existing { from: SetFrom, id: i64 },
    /// Keep the plan's series for this folder as the person wrote it. The
    /// proposer counts it and offers nothing on it while this edit stands.
    Own { scope: FolderScope },
    /// These files are one item: each join puts the item that holds `with`
    /// into the item that holds `item`. Its files are named, never a rule.
    OneItem {
        scope: FolderScope,
        joins: Vec<ItemJoin>,
    },
    /// These rows are these files: each row joins the record of its file.
    RowIs { list: ListFile, pairs: Vec<RowPair> },
    /// These rows describe nothing: each makes no record.
    DescribesNothing { list: ListFile, rows: Vec<RowCell> },
}

/// One join of a `oneItem` edit. Both paths are relative to the scope's
/// source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemJoin {
    pub item: RelPath,
    pub with: RelPath,
}

/// A row of a list by its line and the text of its join cell there. The
/// line names the row; the cell checks it is still the row the person
/// saw.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RowCell {
    pub line: u32,
    pub cell: String,
}

/// One pair of a `rowIs` edit: the row, and the file of the list's source
/// it is. When the rows describe items, the file is one of the item's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RowPair {
    pub row: RowCell,
    pub file: RelPath,
}

/// The cut an `items` edit asks for. The proposer works the shape out again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cut", rename_all = "camelCase", deny_unknown_fields)]
pub enum CutEdit {
    /// Each name is one item.
    Whole,
    /// The key is the first `places` places of the name.
    Places { places: u8 },
    /// The key is the folder `depth` levels under the scope.
    Folder { depth: u8 },
}

/// Where a set takes its name or names from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "camelCase", deny_unknown_fields)]
pub enum SetFrom {
    /// The series folder's name.
    Folder { scope: FolderScope },
    /// One set per value of a column, or one set for its one value.
    Column { list: ListFile, column: ColumnRef },
    /// One set per value of a name place.
    NamePlace { scope: FolderScope, place: u8 },
    /// One set per sibling folder `depth` levels under the scope.
    FolderLevel { scope: FolderScope, depth: u8 },
}

/// How a list's rows find their files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "rule", rename_all = "camelCase", deny_unknown_fields)]
pub enum PairRule {
    /// The folds the plan states.
    Folds { normalize: JoinNormalize },
    /// Another column or join.
    Column { column: ColumnRef, on: JoinOn },
    /// Compare only the last `places` places of the cell and the file name.
    /// The plan cannot state this yet (design open call 3).
    LastPlaces { places: u8 },
}
