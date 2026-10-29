//! One struct per upstream table, with a `from_db_row` that parses one database row.
//!
//! `from_db_row` is the storage boundary: the database is external data, so a column that does not
//! hold a representable value returns [`DbError::Column`](crate::DbError::Column) instead of
//! panicking. Inside the crate the parsed rows are trusted. Every read goes through
//! [`Backend`](crate::backend::Backend), which owns the per-engine decoding, so the same struct
//! parses a SQLite row and a MySQL row. Each struct also keeps a `from_row` that takes a
//! `SqliteRow`, which is the shape the read layer of `omeka-api` already calls.
//!
//! Column names match upstream `schema.sql` exactly, including `type` (`ValueRow::r#type`). The
//! resource tables (`item`, `item_set`, `media`, `value_annotation`) share their primary key with
//! `resource`, so their row structs nest a [`ResourceRow`] rather than repeat its columns.

mod bulk;
mod custom_vocabs;
mod derivatives;
mod import_evidence;
mod import_writes;
mod imports;
pub(crate) mod item_history;
mod page_versions;
mod publications;
mod resources;
mod rewrites;
mod sites;
mod system;
mod templates;
pub(crate) mod trash;
mod user_uploads;
mod users;
mod values;
mod vocabularies;

pub use bulk::{
    Applied, BulkEnding, BulkKind, BulkPhase, BulkRow, BulkStatus, BulkTargetRow, Described,
    FailReason, ItemMembers, NewBulk, Restore, Reverted, TargetCounts, TargetState, ValueWrites,
    Verdict, Written,
};
pub use custom_vocabs::{CustomVocabRow, NewCustomVocab};
pub use derivatives::{DerivativeKind, DerivativeRow, DerivativeState, NewDerivative};
pub use import_evidence::{EntryRow, NewEntry, NewScan, NewUpload, ScanRow, UploadRow};
pub use import_writes::{
    BeforeImage, ImageMemberships, ImageValue, ImportAction, ImportFields, ImportResource,
    ItemSlots, NewImportFields, NewImportItem, NewImportItemSet, NewImportMedia, Slot, StoredFile,
};
pub use imports::{
    BeforeImageRow, ImportChange, ImportEnding, ImportKind, ImportResourceRow, ImportRow,
    ImportStatus, ImportTarget, NewImport, NewImportResource, may_run,
};
pub use item_history::{FieldChange, HistoryEntryRow, HistorySource, HistoryStamp};
pub use page_versions::{
    BlockChange, NewPageAttachment, NewPageBlock, PageCreate, PageImage, PageImageBlock, PageSave,
    PageSaveOutcome, PageVersionOrigin, PageVersionRow, VersionAuthor,
};
pub use publications::{PublicationRow, PublicationTarget, PublicationVersionRow};
pub use resources::{
    ItemRow, ItemSetRow, MediaRow, NewItem, NewItemSet, NewMedia, NewResource, NewWholeItem,
    ResourceRow, ValueAnnotationRow, resource_type,
};
pub use rewrites::{MergeWrite, PromoteList, PromoteWrite, TextValueRow};
pub use sites::{
    NewFirstPage, NewFirstPageBlock, NewSite, NewSiteBlock, NewSiteBlockAttachment, NewSitePage,
    NewWholeSite, SiteBlockAttachmentRow, SiteBlockRow, SiteItemSetRow, SitePageRow,
    SitePermissionRow, SiteRow, SiteSettingRow,
};
pub use system::{
    AssetRow, FulltextSearchRow, ItemItemSetRow, ItemSiteRow, JobRow, MigrationRow, ModuleRow,
    NewAsset, NewJob, SessionRow, SettingRow,
};
pub use templates::{NewTemplate, NewTemplateProperty, TemplatePropertyRow, TemplateRow};
pub use trash::{Emptied, IncomingLink, Pointer, TrashImage, TrashKind, TrashRow, TrashedMedia};
pub use user_uploads::{NewUserUpload, UserUploadRow};
pub use users::{
    ApiKeyRow, NewApiKey, NewPasswordCreation, NewUser, PasswordCreationRow, UserRow,
    UserSettingRow,
};
pub use values::{AnnotationTree, NewValue, ValueContent, ValueRow, ValueTree};
pub use vocabularies::{
    ClassRow, NewClass, NewProperty, NewVocabulary, PropertyRow, VocabularyRow,
};

use sqlx::sqlite::SqliteRow;

use crate::DbError;

/// Name the SQLite spelling of each row parser.
///
/// The parser itself is generic ([`from_db_row`] on each struct) because the repositories build
/// statements for either engine. Callers outside this crate hold a `SqlitePool` and pass a
/// `SqliteRow`, so each struct keeps a `from_row` that takes exactly that, which is the signature
/// they already call.
macro_rules! sqlite_row_parser {
    ($($row:ty),* $(,)?) => {
        $(
            impl $row {
                /// Parse one row of this table from a SQLite result row.
                pub fn from_row(row: &SqliteRow) -> Result<Self, DbError> {
                    <$row>::from_db_row::<sqlx::Sqlite>(row)
                }
            }
        )*
    };
}

sqlite_row_parser!(
    AssetRow,
    ClassRow,
    CustomVocabRow,
    PublicationRow,
    PublicationVersionRow,
    ImportRow,
    ImportResourceRow,
    FulltextSearchRow,
    ItemItemSetRow,
    ItemRow,
    ItemSetRow,
    ItemSiteRow,
    JobRow,
    MediaRow,
    MigrationRow,
    ModuleRow,
    PasswordCreationRow,
    PropertyRow,
    ResourceRow,
    SessionRow,
    SettingRow,
    SiteBlockAttachmentRow,
    SiteBlockRow,
    SiteItemSetRow,
    SitePageRow,
    SitePermissionRow,
    SiteRow,
    SiteSettingRow,
    TemplatePropertyRow,
    TemplateRow,
    TextValueRow,
    UserRow,
    UserSettingRow,
    ValueAnnotationRow,
    ValueRow,
    VocabularyRow,
);
