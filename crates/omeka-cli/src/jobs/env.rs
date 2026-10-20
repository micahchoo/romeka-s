//! What a job reads besides the database: the file store, the installation's
//! base URL, and the archive root.

use std::path::PathBuf;
use std::sync::Arc;

use omeka_api::jsonld::BaseUrl;
use omeka_media::store::Store;

/// The surroundings of a job process. `romeka job run` builds it from its
/// flags; the server's dispatcher passes the values the server itself uses.
#[derive(Debug, Clone)]
pub struct JobEnv {
    pub store: Arc<dyn Store>,
    pub base_url: BaseUrl,
    /// `None` when the archive import is off.
    pub archive_root: Option<PathBuf>,
}

/// The base URL when none is given, as `romeka serve` defaults its own.
pub const DEFAULT_BASE_URL: &str = "http://localhost";

impl Default for JobEnv {
    fn default() -> Self {
        Self {
            store: omeka_server::local_store(omeka_server::FILES_ROOT),
            base_url: BaseUrl::parse(DEFAULT_BASE_URL)
                .unwrap_or_else(|_| unreachable!("the default base URL literal parses")),
            archive_root: None,
        }
    }
}
