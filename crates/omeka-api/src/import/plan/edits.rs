//! What a person's acts on a proposal name: a list file by where it is.
//! Edits and the placement oracle name lists this way, never by sheet id,
//! because a sheet id exists only in a plan.

use serde::{Deserialize, Serialize};

use super::{RelPath, SourceId};

/// A list file by where it is.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListFile {
    pub source: SourceId,
    pub file: RelPath,
}
