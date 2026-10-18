//! The HTML ingester (`Media/Ingester/Html.php`).
//!
//! Stores a purified HTML fragment under `html`. It is a mutable ingester, so it
//! also handles an update.

use crate::data::HtmlData;
use crate::data::MediaData;
use crate::ingest::{ErrorStore, IngestEnv, IngestRequest, Ingester, MediaDraft, MutableIngester};

/// The HTML ingester.
#[derive(Debug, Clone, Copy, Default)]
pub struct Html;

impl Ingester for Html {
    fn name(&self) -> &str {
        "html"
    }

    fn label(&self) -> String {
        "HTML".to_string()
    }

    fn renderer(&self) -> &'static str {
        "html"
    }

    /// `Ingester/Html.php:62`: purify the `html` member and store it as the
    /// whole payload.
    fn ingest(
        &self,
        draft: &mut MediaDraft,
        request: &IngestRequest,
        env: &IngestEnv<'_>,
        _errors: &mut ErrorStore,
    ) {
        if let Some(html) = request.string("html") {
            draft.data = MediaData::Html(HtmlData {
                html: env.purifier.purify(html),
            });
        }
    }
}

impl MutableIngester for Html {
    /// `Ingester/Html.php:73`: an update reads the HTML from
    /// `o:media.__index__.html`.
    fn update(
        &self,
        draft: &mut MediaDraft,
        request: &IngestRequest,
        env: &IngestEnv<'_>,
        _errors: &mut ErrorStore,
    ) {
        let html = request
            .value("o:media")
            .and_then(|media| media.get("__index__"))
            .and_then(|media| media.get("html"))
            .and_then(serde_json::Value::as_str);
        if let Some(html) = html {
            draft.data = MediaData::Html(HtmlData {
                html: env.purifier.purify(html),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::HtmlPurifier;
    use crate::testing::{Harness, ReplacePurifier, media_row};

    /// `Ingester/Html.php:62`: the stored payload is the purified fragment.
    #[test]
    fn html_ingest_purifies_and_stores_the_fragment() {
        let harness = Harness::new("html-ingest").with_purifier(ReplacePurifier {
            from: "<script>",
            to: "",
        });
        let env = harness.env();
        let request = IngestRequest::from_value(serde_json::json!({"html": "<script><p>x</p>"}));
        let mut draft = MediaDraft::new(media_row(1, "html", "html"));
        let mut errors = ErrorStore::default();
        Html.ingest(&mut draft, &request, &env, &mut errors);
        assert_eq!(
            draft.data,
            MediaData::Html(HtmlData {
                html: "<p>x</p>".to_string()
            })
        );
    }

    /// `Ingester/Html.php:62`: the production purifier the port wires in drops a
    /// script and keeps the allowed markup.
    #[test]
    fn html_ingest_with_the_core_purifier_drops_script() {
        let harness = Harness::new("html-ingest-core").with_purifier(HtmlPurifier);
        let env = harness.env();
        let request = IngestRequest::from_value(
            serde_json::json!({"html": "<p>ok</p><script>alert(1)</script>"}),
        );
        let mut draft = MediaDraft::new(media_row(1, "html", "html"));
        let mut errors = ErrorStore::default();
        Html.ingest(&mut draft, &request, &env, &mut errors);
        assert_eq!(
            draft.data,
            MediaData::Html(HtmlData {
                html: "<p>ok</p>".to_string()
            })
        );
    }

    /// `Ingester/Html.php:64`: a request with no `html` member leaves the
    /// payload alone.
    #[test]
    fn html_ingest_ignores_a_request_without_html() {
        let harness = Harness::new("html-ingest-empty");
        let env = harness.env();
        let request = IngestRequest::from_value(serde_json::json!({"other": "x"}));
        let mut draft = MediaDraft::new(media_row(1, "html", "html"));
        let mut errors = ErrorStore::default();
        Html.ingest(&mut draft, &request, &env, &mut errors);
        assert_eq!(draft.data, MediaData::None);
    }

    /// `Ingester/Html.php:73`: an update replaces the payload from
    /// `o:media.__index__.html`.
    #[test]
    fn html_update_reads_the_nested_member() {
        let harness = Harness::new("html-update");
        let env = harness.env();
        let request = IngestRequest::from_value(
            serde_json::json!({"o:media": {"__index__": {"html": "<p>updated</p>"}}}),
        );
        let mut draft = MediaDraft::new(media_row(1, "html", "html"));
        let mut errors = ErrorStore::default();
        Html.update(&mut draft, &request, &env, &mut errors);
        assert_eq!(
            draft.data,
            MediaData::Html(HtmlData {
                html: "<p>updated</p>".to_string()
            })
        );
    }
}
