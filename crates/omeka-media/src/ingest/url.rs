//! The URL ingester (`Media/Ingester/Url.php`).
//!
//! Downloads a file from an absolute HTTP URL and stores it as the original.

use crate::ingest::{
    ErrorStore, IngestEnv, IngestFile, IngestFlags, IngestRequest, Ingester, MediaDraft,
    ingest_file, truthy,
};
use crate::uri::HttpUri;

/// The URL ingester.
#[derive(Debug, Clone, Copy, Default)]
pub struct Url;

impl Ingester for Url {
    fn name(&self) -> &str {
        "url"
    }

    fn label(&self) -> String {
        "URL".to_string()
    }

    fn renderer(&self) -> &'static str {
        "file"
    }

    /// `Ingester/Url.php:52`.
    ///
    /// Reads `ingest_url` (required) and `store_original` (optional, default
    /// true). The source is set to the URL when the request has no `o:source`,
    /// so a caller can strip secrets from the stored value.
    fn ingest(
        &self,
        draft: &mut MediaDraft,
        request: &IngestRequest,
        env: &IngestEnv<'_>,
        errors: &mut ErrorStore,
    ) {
        let Some(raw) = request.string("ingest_url") else {
            errors.add("error", "No ingest URL specified");
            return;
        };
        let Some(uri) = HttpUri::parse(raw) else {
            errors.add("ingest_url", "Invalid ingest URL");
            return;
        };
        let response = match env.http.fetch(uri.raw()) {
            Ok(response) => response,
            Err(error) => {
                errors.add("download", format!("Error downloading {raw}: {error}"));
                return;
            }
        };
        if !response.is_ok() {
            errors.add(
                "download",
                format!(
                    "Error downloading {raw}: {} {}",
                    response.status, response.reason
                ),
            );
            return;
        }

        if !request.has("o:source") {
            draft.row.source = Some(uri.raw().to_string());
        }
        let store_original = request.value("store_original").map(truthy).unwrap_or(true);
        let file = IngestFile::new(response.body).with_source_name(uri.path().to_string());
        let flags = IngestFlags {
            store_original,
            store_thumbnails: true,
            hydrate_metadata: true,
        };
        if let Err(error) = ingest_file(env, draft, file, flags) {
            errors.add("error", format!("Error storing downloaded file: {error}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{Harness, media_row};

    /// `Ingester/Url.php:55`: `ingest_url` is required.
    #[test]
    fn url_requires_an_ingest_url() {
        let harness = Harness::new("url-requires-ingest-url");
        let env = harness.env();
        let request = IngestRequest::from_value(serde_json::json!({}));
        let mut draft = MediaDraft::new(media_row(1, "url", "file"));
        let mut errors = ErrorStore::default();
        Url.ingest(&mut draft, &request, &env, &mut errors);
        assert_eq!(
            errors.messages_for("error"),
            vec!["No ingest URL specified"]
        );
    }

    /// `Ingester/Url.php:62`: the URL must be absolute and http(s).
    #[test]
    fn url_rejects_a_relative_url() {
        let harness = Harness::new("url-relative");
        let env = harness.env();
        let request =
            IngestRequest::from_value(serde_json::json!({"ingest_url": "example.org/a.jpg"}));
        let mut draft = MediaDraft::new(media_row(1, "url", "file"));
        let mut errors = ErrorStore::default();
        Url.ingest(&mut draft, &request, &env, &mut errors);
        assert_eq!(
            errors.messages_for("ingest_url"),
            vec!["Invalid ingest URL"]
        );
    }

    /// `File/Downloader.php:80`: a non-2xx response is a `download` error.
    #[test]
    fn url_reports_a_failed_download() {
        let harness = Harness::new("url-404").with_response(
            "https://example.org/missing.jpg",
            404,
            "Not Found",
            b"",
        );
        let env = harness.env();
        let request = IngestRequest::from_value(
            serde_json::json!({"ingest_url": "https://example.org/missing.jpg"}),
        );
        let mut draft = MediaDraft::new(media_row(1, "url", "file"));
        let mut errors = ErrorStore::default();
        Url.ingest(&mut draft, &request, &env, &mut errors);
        assert_eq!(
            errors.messages_for("download"),
            vec!["Error downloading https://example.org/missing.jpg: 404 Not Found"]
        );
    }

    /// `Ingester/Url.php:72`: the source defaults to the URL, the extension
    /// comes from the URL path, and `store_original` defaults true.
    #[test]
    fn url_downloads_and_stores_the_original() {
        let harness = Harness::new("url-happy")
            .with_body("https://example.org/images/photo.jpg", b"jpegbytes");
        let env = harness.env();
        let request = IngestRequest::from_value(
            serde_json::json!({"ingest_url": "https://example.org/images/photo.jpg"}),
        );
        let mut draft = MediaDraft::new(media_row(1, "url", "file"));
        let mut errors = ErrorStore::default();
        Url.ingest(&mut draft, &request, &env, &mut errors);

        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            draft.row.source.as_deref(),
            Some("https://example.org/images/photo.jpg")
        );
        assert_eq!(draft.row.extension.as_deref(), Some("jpg"));
        assert!(draft.row.has_original);
        let storage_id = draft.row.storage_id.clone().expect("storage id");
        assert!(harness.store.exists(&format!("original/{storage_id}.jpg")));
        assert_eq!(
            harness.http.calls(),
            ["https://example.org/images/photo.jpg"]
        );
    }

    /// `Ingester/Url.php:78`: `store_original` false keeps the file facts but
    /// writes no original.
    #[test]
    fn url_honours_store_original_false() {
        let harness = Harness::new("url-no-original")
            .with_body("https://example.org/photo.jpg", b"jpegbytes");
        let env = harness.env();
        let request = IngestRequest::from_value(serde_json::json!({
            "ingest_url": "https://example.org/photo.jpg",
            "store_original": false
        }));
        let mut draft = MediaDraft::new(media_row(1, "url", "file"));
        let mut errors = ErrorStore::default();
        Url.ingest(&mut draft, &request, &env, &mut errors);

        assert!(errors.is_empty(), "{errors:?}");
        assert!(!draft.row.has_original);
        assert_eq!(draft.row.extension.as_deref(), Some("jpg"));
        assert!(draft.row.sha256.is_some());
        let storage_id = draft.row.storage_id.clone().expect("storage id");
        assert!(!harness.store.exists(&format!("original/{storage_id}.jpg")));
    }
}
