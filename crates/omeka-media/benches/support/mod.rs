//! Shared support for the media benches.
//!
//! Every bench that writes files roots its scratch tree under the cargo target
//! directory, never under the system temporary directory. `/tmp` carries a quota
//! on the build machine, and a full bench run writes enough files to exhaust it;
//! the quota failure surfaces as a store error in the middle of a run, which
//! looks like a store bug (AGENTS.md).

use std::path::PathBuf;

/// The directory the bench scratch trees live in, created if missing.
///
/// `CARGO_TARGET_DIR` when the caller set one, otherwise the workspace `target`
/// directory beside the crate. Cargo puts its own bench output in the same
/// place, so both share the disk that is sized for builds.
#[must_use]
pub fn scratch_dir() -> PathBuf {
    let base = match std::env::var_os("CARGO_TARGET_DIR") {
        Some(directory) => PathBuf::from(directory),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"),
    };
    let scratch = base.join("bench-tmp");
    std::fs::create_dir_all(&scratch).unwrap_or_else(|error| {
        panic!(
            "could not create the bench scratch dir {}: {error}",
            scratch.display()
        )
    });
    scratch
}
