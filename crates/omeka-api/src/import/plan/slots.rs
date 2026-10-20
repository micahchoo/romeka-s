//! Slot rules: for each slot of each record kind, where its values come from.

use std::num::NonZeroU32;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::{ColumnRef, SheetId, SlotName};

/// The slot rules of one record kind. A slot with no rule has no values,
/// except the defaults the contract gives (`dcterms:identifier`, `itemKey`).
pub type SlotRules = IndexMap<SlotName, SlotRule>;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Slots {
    pub item: SlotRules,
    pub media: SlotRules,
    pub item_set: SlotRules,
}

impl Slots {
    /// Every rule of every kind.
    pub fn rules(&self) -> impl Iterator<Item = (&SlotName, &SlotRule)> {
        self.item.iter().chain(&self.media).chain(&self.item_set)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotRule {
    pub sources: Vec<SourceRef>,
    pub combine: Combine,
    /// The value map: a value whose text, compared as a title, is a key
    /// takes the mapped text; a key mapped to `""` drops the value. Plans
    /// saved before the map existed have none, so a missing member is empty.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub map: IndexMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Combine {
    /// The first source ref that yields a value.
    First,
    /// The union of every source ref's values.
    All,
}

/// Where a slot's values come from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "camelCase", deny_unknown_fields)]
pub enum SourceRef {
    /// A named group of the file-name pattern.
    Group {
        name: String,
    },
    Folder {
        level: FolderLevel,
    },
    Column {
        sheet: SheetId,
        column: ColumnRef,
    },
    Tag {
        tag: TagId,
        files: TagFiles,
    },
    File {
        fact: FileFact,
    },
    Constant {
        value: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FolderLevel {
    /// 1 is the top folder under the source.
    Level(NonZeroU32),
    Path(WholePath),
}

/// The text `path`: every folder of the file's path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WholePath {
    Path,
}

/// The file tags a slot can read: EXIF, file facts, then IPTC IIM and XMP,
/// each named as `omeka_media::image_metadata` keys it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TagId {
    DateTimeOriginal,
    Artist,
    Copyright,
    ImageDescription,
    #[serde(rename = "GPS")]
    Gps,
    Model,
    #[serde(rename = "File name")]
    FileName,
    Folder,
    Size,
    #[serde(rename = "Pixel dimensions")]
    PixelDimensions,
    Format,
    #[serde(rename = "IPTC By-line")]
    IptcByline,
    #[serde(rename = "IPTC Caption-Abstract")]
    IptcCaption,
    #[serde(rename = "IPTC Keywords")]
    IptcKeywords,
    #[serde(rename = "IPTC Object Name")]
    IptcObjectName,
    #[serde(rename = "IPTC Date Created")]
    IptcDateCreated,
    #[serde(rename = "IPTC City")]
    IptcCity,
    #[serde(rename = "IPTC Country")]
    IptcCountry,
    #[serde(rename = "XMP dc:title")]
    XmpTitle,
    #[serde(rename = "XMP dc:description")]
    XmpDescription,
    #[serde(rename = "XMP dc:creator")]
    XmpCreator,
    #[serde(rename = "XMP dc:subject")]
    XmpSubject,
    #[serde(rename = "XMP dc:rights")]
    XmpRights,
    #[serde(rename = "XMP photoshop:DateCreated")]
    XmpDateCreated,
    #[serde(rename = "XMP lr:hierarchicalSubject")]
    XmpHierarchicalSubject,
}

/// Which files of an item's group give the item its tag values.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TagFiles {
    Each(EachFile),
    Part(TagPart),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EachFile {
    First,
    All,
}

/// The file whose pattern `part` group is this text.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TagPart {
    pub part: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileFact {
    Name,
    Path,
    Size,
    /// The sniffed media type.
    Format,
}
