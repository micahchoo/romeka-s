//! Import v2 (`docs/import-v2.md`). P3-wire renames this module to `import`
//! once its routes exist.
//!
//! - [`plan`]: the stored plan and its one parse boundary, [`plan::ImportPlan::from_json`].
//! - [`decisions`]: corrections keyed by an entry.
//! - [`values`]: value types and the cell rules every reader shares.
//! - [`archive`]: named archive roots and the folders a source reads.
//! - [`evidence`]: the walk of a source into file entries and sheets.
//! - [`tags`]: file tags and file facts.
//! - [`sheet`]: the sheet reader.
//! - [`starters`]: the starter item types `GET /api/romeka/starters` lists.
//! - [`derive`]: the evidence, the plan and a [`catalog::Catalog`] in, the
//!   [`model::Model`] out, through [`grouping`], [`join`], [`slots`],
//!   `matching` and `flags`.
//! - [`run`]: the model written, with [`ledger`], [`media_in`] and [`left`].
//! - [`undo`]: an import taken back, with [`touched`].
//! - [`staging`]: the files a person uploaded, [`unzip`] for a ZIP among
//!   them, and [`sweep`], which removes them once they are no longer kept.
//!
//! The plan types are pure data. The evidence, the derive and the run read
//! the plan; none of them parses it again.

pub mod access_copies;
pub mod archive;
pub mod catalog;
pub mod catalog_load;
pub mod check;
#[cfg(any(test, feature = "meter"))]
pub mod clicks;
#[cfg(test)]
mod clicks_tests;
pub mod decisions;
pub mod derive;
pub mod draft;
pub mod evidence;
mod flags;
pub mod grouping;
pub mod join;
pub mod ledger;
pub mod left;
mod matching;
pub mod media_in;
pub mod model;
pub mod placement;
#[cfg(test)]
mod placement_tests;
pub mod places;
pub mod plan;
pub mod preview;
pub mod run;
pub mod scan;
pub mod sheet;
pub mod slots;
pub mod staging;
pub mod start;
pub mod starters;
pub mod sweep;
pub mod tags;
pub mod template;
pub mod touched;
pub mod undo;
pub mod unzip;
pub mod values;

use serde::{Deserialize, Deserializer};

/// A member the contract types as `T | null`: present, and possibly null.
///
/// A plain `Option` field lets serde treat a missing member as null, so the
/// boundary would accept a plan the contract does not define.
pub(crate) fn nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
