//! The fallback ingester (`Media/Ingester/Fallback.php`).
//!
//! `Manager::get` returns one of these for a name it does not know, so a row
//! with an ingester from a module the port does not ship still loads and
//! renders nothing.

use crate::ingest::{ErrorStore, IngestEnv, IngestRequest, Ingester, MediaDraft};

/// An ingester that does nothing, carrying the unknown name for its label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fallback {
    name: String,
}

impl Fallback {
    /// Build a fallback for an unknown ingester name.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

impl Ingester for Fallback {
    fn name(&self) -> &str {
        &self.name
    }

    /// `sprintf('%s [%s]', translate('Unknown'), $this->name)`
    /// (`Ingester/Fallback.php:35`). The port has no translator, so the literal
    /// English label is used.
    fn label(&self) -> String {
        format!("Unknown [{}]", self.name)
    }

    fn renderer(&self) -> &'static str {
        "fallback"
    }

    fn ingest(
        &self,
        _draft: &mut MediaDraft,
        _request: &IngestRequest,
        _env: &IngestEnv<'_>,
        _errors: &mut ErrorStore,
    ) {
        // Upstream's fallback ingest body is empty.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Fallback::getRenderer` is `fallback` and its ingest does nothing.
    #[test]
    fn fallback_matches_upstream_ingester() {
        let ingester = Fallback::new("from_a_module");
        assert_eq!(ingester.renderer(), "fallback");
        assert_eq!(ingester.label(), "Unknown [from_a_module]");
    }
}
