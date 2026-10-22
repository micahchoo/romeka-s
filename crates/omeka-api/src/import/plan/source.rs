//! Sources and sheets: where evidence comes from and how a sheet is read.

use std::num::{NonZeroU8, NonZeroU32};

use serde::{Deserialize, Serialize};

use super::{RecordKind, RelPath, SheetId, SourceId};
use crate::import::values::ValueType;

/// A folder under a named archive root, or the import's staging area.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum SourcePlan {
    Folder {
        id: SourceId,
        /// The archive root's name (`ROMEKA_ARCHIVE_ROOTS`).
        root: String,
        /// Relative to the root; empty for the root itself.
        path: RelPath,
    },
    Staging {
        id: SourceId,
    },
}

impl SourcePlan {
    #[must_use]
    pub fn id(&self) -> &SourceId {
        match self {
            Self::Folder { id, .. } | Self::Staging { id } => id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SheetPlan {
    pub id: SheetId,
    pub source: SourceId,
    pub file: RelPath,
    /// The hash of the bytes the plan's line-keyed decisions were made on.
    #[serde(deserialize_with = "crate::import::nullable")]
    pub hash: Option<String>,
    pub read: ReadOptions,
    pub row_kind: RowKind,
    #[serde(deserialize_with = "crate::import::nullable")]
    pub join: Option<Join>,
    /// For media rows: the column naming their item.
    #[serde(deserialize_with = "crate::import::nullable")]
    pub item_key: Option<ColumnRef>,
    pub columns: Vec<ColumnPlan>,
}

impl SheetPlan {
    /// The mapped column at this index.
    #[must_use]
    pub fn column(&self, index: u32) -> Option<&ColumnPlan> {
        self.columns
            .iter()
            .find(|column| column.column.index == index)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadOptions {
    pub encoding: EncodingChoice,
    pub delimiter: Delimiter,
    pub quote: Quote,
    /// The 1-based line that holds the header.
    pub header_line: NonZeroU32,
    pub trim: bool,
    /// The workbook sheet to read; `None` reads the first. A plan written
    /// before workbooks were sheets has no member, and reads the first too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet_name: Option<String>,
}

/// The encodings the sheet reader decodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Encoding {
    #[serde(rename = "utf-8")]
    Utf8,
    #[serde(rename = "utf-16le")]
    Utf16Le,
    #[serde(rename = "utf-16be")]
    Utf16Be,
    #[serde(rename = "windows-1252")]
    Windows1252,
    #[serde(rename = "iso-8859-1")]
    Iso88591,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EncodingChoice {
    /// The reader guesses and reports its guess with a confidence.
    Auto(Auto),
    Is(Encoding),
}

/// The text `auto`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Auto {
    Auto,
}

/// The delimiters the "Sheets" section lists. The plan's TypeScript type
/// says `string`; any other string has no reader, so it is refused here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Delimiter {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = ",")]
    Comma,
    #[serde(rename = ";")]
    Semicolon,
    #[serde(rename = "\t")]
    Tab,
    #[serde(rename = "|")]
    Pipe,
    #[serde(rename = ":")]
    Colon,
    #[serde(rename = " ")]
    Space,
}

impl Delimiter {
    /// The character, or `None` for `auto`.
    #[must_use]
    pub fn as_char(self) -> Option<char> {
        match self {
            Self::Auto => None,
            Self::Comma => Some(','),
            Self::Semicolon => Some(';'),
            Self::Tab => Some('\t'),
            Self::Pipe => Some('|'),
            Self::Colon => Some(':'),
            Self::Space => Some(' '),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Quote {
    #[serde(rename = "\"")]
    Double,
    #[serde(rename = "'")]
    Single,
    #[serde(rename = "none")]
    None,
}

/// The kind of record every row makes, or the column that says it per row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RowKind {
    All(RecordKind),
    Column(KindColumn),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KindColumn {
    pub column: ColumnRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Join {
    pub on: JoinOn,
    pub column: ColumnRef,
    pub normalize: JoinNormalize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JoinOn {
    /// The cell equals the group key.
    Identifier,
    /// The cell names one of the group's files.
    FileName,
}

/// NFC always applies; these are the optional folds on top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JoinNormalize {
    pub case_fold: bool,
    pub leading_zeros: bool,
    /// `_`, `-`, `.` and space treated as one.
    pub separators: bool,
    /// Compare only the last this many places of the cell and of the file
    /// name or key (`places::last_places`), before the other folds. Left
    /// out of the wire when off, so a plan without it keeps its bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_places: Option<NonZeroU8>,
}

/// A sheet column. The index is the key; the header is checked against the
/// sheet and shown.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnRef {
    /// 0-based position in the row.
    pub index: u32,
    pub header: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnPlan {
    pub column: ColumnRef,
    pub options: ColumnOptions,
}

/// How a column's cells become values, before they are counted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnOptions {
    #[serde(deserialize_with = "crate::import::nullable")]
    pub split: Option<String>,
    #[serde(rename = "type")]
    pub value_type: ValueType,
    #[serde(deserialize_with = "crate::import::nullable")]
    pub lang: Option<String>,
    pub private: bool,
}
