//! Import v2 (`docs/import-v2.md`). P3-wire renames this module to `import`
//! once its routes exist.
//!
//! - [`plan`]: the stored plan and its one parse boundary, [`plan::ImportPlanV2::from_json`].
//! - [`decisions`]: corrections keyed by an entry.
//! - [`values`]: value types and the cell rules every reader shares.
//! - [`starters`]: the starter item types `GET /api/romeka/starters` lists.
//!
//! Everything here is pure data. The evidence, the derive and the run read
//! the plan; none of them parses it again.

pub mod decisions;
pub mod plan;
pub mod starters;
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
