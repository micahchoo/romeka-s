//! The archive import, read side: from a server folder and a plan to the
//! model every Import tab shows and the run writes.
//!
//! The server is the one validator (`docs/archive-import.md`). The modules follow
//! the domain:
//!
//! - [`plan`]: the stored plan and its parse boundary.
//! - [`archive`]: the archive root, safe relative paths, listings and walks.
//! - [`pattern`]: the file-name regex and its groups.
//! - [`csv`]: the RFC 4180 reader and the import's spreadsheet.
//! - [`tags`]: EXIF and file facts as value sources, read lazily.
//! - [`catalog`]: what the derive looks up in the database, and the starters.
//! - [`grouping`]: files into items, folders into sets.
//! - [`values`]: where a field's value comes from.
//! - [`problems`]: what can be wrong with a cell or a row.
//! - [`derive`]: the pure function from all of the above to the [`derive::Model`].
//! - [`check`]: the dry run.
//! - [`preview`]: the sections and pages the admin asks for.
//!
//! The write side:
//!
//! - [`work`]: the writes a run makes, decided before the first one.
//! - [`media_files`]: an item's files into the store.
//! - [`run`]: the run, one transaction per item.
//! - [`undo`]: removing what one import made.
//! - [`left`]: the rows a run leaves out, as a CSV file.
//!
//! The read side is blocking file I/O or pure; an async caller runs
//! [`preview()`] on a blocking thread. [`run::run`] and [`undo::undo`] are
//! async and put their file work on the blocking pool themselves.

pub mod archive;
pub mod catalog;
pub mod check;
pub mod csv;
pub mod derive;
pub mod grouping;
pub mod inputs;
pub mod left;
pub mod media_files;
pub mod pattern;
pub mod plan;
pub mod preview;
pub mod problems;
pub mod run;
pub mod tags;
pub mod undo;
pub mod values;
pub mod work;

#[cfg(test)]
mod date_parts_tests;
#[cfg(test)]
mod existing_tests;
#[cfg(test)]
mod left_tests;
#[cfg(test)]
mod run_tests;
#[cfg(test)]
mod run_values_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod undo_tests;

use archive::{ArchiveError, ArchiveFolder};
use omeka_db::ids::UserId;

use crate::auth::Actor;
use catalog::Catalog;
use inputs::Inputs;
use plan::ImportPlan;
use preview::{Preview, PreviewContext, PreviewRequest};
use tags::{FsTagReader, TagCache, TagReader};

/// Read the folder and answer one preview request, over the request's own
/// plan when it carries one and `stored` otherwise.
///
/// # Errors
/// [`ArchiveError`] when the folder cannot be walked.
pub fn preview(
    folder: &ArchiveFolder,
    stored: &ImportPlan,
    catalog: &Catalog,
    request: &PreviewRequest,
) -> Result<Preview, ArchiveError> {
    preview_with(folder, stored, catalog, request, &FsTagReader)
}

/// [`preview()`] with the caller's tag reader, such as a
/// [`tags::SharedTagReader`] that lives as long as the server.
///
/// # Errors
/// [`ArchiveError`] when the folder cannot be walked.
pub fn preview_with(
    folder: &ArchiveFolder,
    stored: &ImportPlan,
    catalog: &Catalog,
    request: &PreviewRequest,
    reader: &dyn TagReader,
) -> Result<Preview, ArchiveError> {
    let plan = request.plan.as_ref().unwrap_or(stored);
    let inputs = Inputs::load(folder, plan)?;
    let tags = TagCache::new(reader);
    let model = derive::derive(plan, &inputs, &tags, catalog);
    let context = PreviewContext {
        plan,
        inputs: &inputs,
        model: &model,
        tags: &tags,
        catalog,
    };
    Ok(preview::build(&context, request))
}

/// The actor run and undo write as: a job writes on no one's behalf, so it
/// reads and writes as a global administrator, as `romeka`'s jobs do.
fn system_actor() -> Actor {
    Actor::new(UserId(0), "global_admin")
}
