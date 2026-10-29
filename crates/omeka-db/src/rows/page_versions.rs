//! `romeka_site_page_version`: Romeka's own table of saved site page states (migration 0007).
//! Upstream has none.
//!
//! `snapshot` is JSON. The row keeps it as JSON; `omeka-api` owns what it means.

use chrono::NaiveDateTime;
use serde_json::Value;

use crate::backend::Backend;
use crate::error::DbError;
use crate::ids::{ItemId, MediaId, PageId, UserId};
use crate::rows::{NewSitePage, SiteBlockAttachmentRow, SiteBlockRow, SitePageRow};

/// One saved state of a page, never changed afterwards.
#[derive(Debug, Clone, PartialEq)]
pub struct PageVersionRow {
    pub page_id: PageId,
    /// 1 for the first state kept, then one more per save.
    pub version: u32,
    /// When this state went live.
    pub created: NaiveDateTime,
    /// Who made it. `None` for [`PageVersionOrigin::BeforeHistory`], or once the user is deleted.
    pub user_id: Option<UserId>,
    pub origin: PageVersionOrigin,
    pub snapshot: Value,
}

/// What made a version (migration 0015). A restore is a plain save: the editor loads the old
/// version into the draft and the next save stores it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageVersionOrigin {
    /// The state a page had before its first recorded save, kept as version 1 with no user.
    BeforeHistory,
    /// The state the page create made, version 1.
    Create,
    Save,
}

impl PageVersionOrigin {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::BeforeHistory => "before_history",
            Self::Create => "create",
            Self::Save => "save",
        }
    }

    fn parse(raw: &str) -> Result<Self, DbError> {
        Ok(match raw {
            "before_history" => Self::BeforeHistory,
            "create" => Self::Create,
            "save" => Self::Save,
            _ => {
                return Err(DbError::Column {
                    column: "origin",
                    reason: format!("not a version origin: {raw}"),
                });
            }
        })
    }
}

/// Who made a version, as far as the row can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionAuthor {
    /// The user made it; the user row may since have gone.
    User(UserId),
    /// The state a page had before its first recorded save. No one was recorded as its author.
    NotRecorded,
    /// A create or save by a user whose row was deleted since (`ON DELETE SET NULL`).
    Deleted,
}

impl PageVersionRow {
    /// The state before history has no author. A create or save is written with its user, so one
    /// with no user lost it when the user was deleted.
    pub fn author(&self) -> VersionAuthor {
        match (self.origin, self.user_id) {
            (PageVersionOrigin::BeforeHistory, _) => VersionAuthor::NotRecorded,
            (_, Some(user)) => VersionAuthor::User(user),
            (_, None) => VersionAuthor::Deleted,
        }
    }

    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        let version = DB::int(row, "version")?;
        Ok(Self {
            page_id: DB::id(row, "page_id")?,
            version: u32::try_from(version).map_err(|_| DbError::Column {
                column: "version",
                reason: format!("not a version number: {version}"),
            })?,
            created: DB::datetime(row, "created")?,
            user_id: DB::opt_id(row, "user_id")?,
            origin: PageVersionOrigin::parse(&DB::text(row, "origin")?)?,
            snapshot: DB::json(row, "snapshot")?,
        })
    }
}

/// A page as stored at one instant: its row, then each block in display order with its
/// attachments in position order. A snapshot is made from this.
#[derive(Debug, Clone, PartialEq)]
pub struct PageImage {
    pub page: SitePageRow,
    pub blocks: Vec<PageImageBlock>,
}

/// One block of a [`PageImage`].
#[derive(Debug, Clone, PartialEq)]
pub struct PageImageBlock {
    pub block: SiteBlockRow,
    pub attachments: Vec<SiteBlockAttachmentRow>,
}

/// One block a save writes, with its attachments already resolved to stored ids.
#[derive(Debug, Clone, PartialEq)]
pub struct NewPageBlock {
    pub layout: String,
    pub data: Value,
    pub layout_data: Option<Value>,
    pub attachments: Vec<NewPageAttachment>,
}

/// One attachment of a [`NewPageBlock`]. Its position is its place in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPageAttachment {
    pub item_id: Option<ItemId>,
    pub media_id: Option<MediaId>,
    pub caption: String,
}

/// What a save does to the page's blocks: `PUT` replaces them, `PATCH` with `o:block` appends,
/// `PATCH` without it keeps them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlockChange<'a> {
    Keep,
    Replace(&'a [NewPageBlock]),
    Append(&'a [NewPageBlock]),
}

/// One save of a page.
#[derive(Debug, Clone, PartialEq)]
pub struct PageSave<'a> {
    /// The page's new column values. `created` and `modified` are ignored: the save keeps
    /// `created` and sets `modified` to now.
    pub page: &'a SitePageRow,
    pub blocks: BlockChange<'a>,
    /// Who saves.
    pub user: Option<UserId>,
    /// The `modified` the saver last read. When it is given and the stored page differs, the save
    /// writes nothing and answers [`PageSaveOutcome::Stale`].
    pub expected_modified: Option<NaiveDateTime>,
}

/// One page create: the page row, its blocks, and who creates it.
#[derive(Debug, Clone, PartialEq)]
pub struct PageCreate<'a> {
    pub page: &'a NewSitePage,
    pub blocks: &'a [NewPageBlock],
    pub user: Option<UserId>,
}

/// What [`crate::repos::PageVersionRepo::save`] did.
#[derive(Debug, Clone, PartialEq)]
pub enum PageSaveOutcome {
    /// The page, as stored now, and the version that holds it.
    Saved { page: SitePageRow, version: u32 },
    /// Someone saved since the saver read the page: when, and who (when known).
    Stale {
        modified: Option<NaiveDateTime>,
        saved_by: Option<UserId>,
    },
    /// The page no longer exists.
    Missing,
}
