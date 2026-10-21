//! What a job reads besides the database: the file store, the installation's
//! base URL, the named archive roots, and how this process starts another job.

use std::sync::Arc;

use omeka_api::import::archive::ArchiveRoots;
use omeka_api::jsonld::BaseUrl;
use omeka_media::store::Store;

use super::dispatcher::JobCommand;

/// The surroundings of a job process. `romeka job run` builds it from its
/// flags; the server's dispatcher passes the values the server itself uses.
#[derive(Debug, Clone)]
pub struct JobEnv {
    pub store: Arc<dyn Store>,
    pub base_url: BaseUrl,
    /// The roots import sources name; empty when the archive import is off.
    pub archive_roots: ArchiveRoots,
    /// The `romeka job run` this process starts another job with, when its
    /// caller said where the database, the store and the roots are. The hourly
    /// sweep starts the derivative worker through it, so a long transcode never
    /// runs inside the sweep's own process. `None` when the process was built
    /// without one (a test, or a caller that runs its jobs in-process): a job
    /// that must fan out then leaves the work queued and says so.
    pub jobs: Option<JobCommand>,
}

/// The base URL when none is given, as `romeka serve` defaults its own.
pub const DEFAULT_BASE_URL: &str = "http://localhost";

impl Default for JobEnv {
    fn default() -> Self {
        Self {
            store: omeka_server::local_store(omeka_server::FILES_ROOT),
            base_url: BaseUrl::parse(DEFAULT_BASE_URL)
                .unwrap_or_else(|_| unreachable!("the default base URL literal parses")),
            archive_roots: ArchiveRoots::default(),
            jobs: None,
        }
    }
}
