//! What an undo would do, read without writing: how many resources it would
//! delete and restore, and which it would keep and why. The admin shows it
//! before the person presses Undo, and again with `force` to say what a
//! forced undo loses.

use omeka_db::Backend;
use omeka_db::ids::ImportId;
use omeka_db::rows::{ImportChange, ImportResourceRow};
use serde::Serialize;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Type};

use super::judge::{Judge, Verdict};
use super::report::{Counts, Force, Kept, Named};
use super::{UndoError, ledger};

/// The answer of `GET …/undo/preview`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoPreview {
    /// What the import made that the undo would delete.
    pub remove: Counts,
    /// What the import changed that the undo would put back.
    pub restore: Counts,
    pub kept: Vec<Kept>,
    /// Gone already; the undo would skip them.
    pub skipped: Vec<Named>,
    /// Touched, and restored or deleted anyway because `force` names them.
    pub forced: Vec<Named>,
}

/// What an undo of `import` with `force` would do now.
///
/// # Errors
/// [`UndoError`] when a read fails, a before-image is malformed, or the
/// import deleted resources (a delete is not undone).
pub async fn preview_undo<DB>(
    pool: &Pool<DB>,
    import: ImportId,
    force: Force,
) -> Result<UndoPreview, UndoError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> f64: Decode<'x, DB> + Type<DB>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
    for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let (rows, images) = ledger(pool, import).await?;
    let mut judge = Judge::new(pool, import, force, &rows, images);
    let mut preview = UndoPreview::default();
    for row in &rows {
        let verdict = judge.judge(row).await?;
        preview.add(row, verdict);
    }
    Ok(preview)
}

impl UndoPreview {
    fn add(&mut self, row: &ImportResourceRow, verdict: Verdict) {
        let kind = row.target.kind();
        match verdict {
            Verdict::Gone => self.skipped.push(Named::of(row.target)),
            Verdict::Keep(touches) => self.kept.push(Kept::of(row.target, touches)),
            Verdict::Act { forced } => {
                if forced {
                    self.forced.push(Named::of(row.target));
                }
                match row.change {
                    ImportChange::Changed => self.restore.add(kind),
                    ImportChange::Created => self.remove.add(kind),
                    // `ledger` refuses an import that deleted.
                    ImportChange::Deleted => {}
                }
            }
        }
    }
}
