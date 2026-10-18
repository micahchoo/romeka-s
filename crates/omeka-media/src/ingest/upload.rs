//! The upload ingester (`Media/Ingester/Upload.php`).
//!
//! Reads the uploaded file named by `file_index`, stores it, and sets
//! `media.source` from the client file name when the request did not supply
//! `o:source`.

use crate::ingest::{
    ErrorStore, IngestEnv, IngestFile, IngestFlags, IngestRequest, Ingester, MediaDraft,
    index_value, ingest_file,
};

/// The upload ingester.
#[derive(Debug, Clone, Copy, Default)]
pub struct Upload;

impl Ingester for Upload {
    fn name(&self) -> &str {
        "upload"
    }

    fn label(&self) -> String {
        "Upload".to_string()
    }

    fn renderer(&self) -> &'static str {
        "file"
    }

    fn ingest(
        &self,
        draft: &mut MediaDraft,
        request: &IngestRequest,
        env: &IngestEnv<'_>,
        errors: &mut ErrorStore,
    ) {
        if request.files().is_empty() {
            errors.add("error", "No files were uploaded");
            return;
        }
        if !request.is_set("file_index") {
            errors.add("error", "No file index was specified");
            return;
        }
        let index = request.value("file_index").and_then(index_value);
        let Some(file) = index.and_then(|index| request.files().get(index)) else {
            errors.add("error", "No file uploaded for the specified index");
            return;
        };

        if !request.has("o:source") {
            draft.row.source = Some(file.name.clone());
        }
        let ingest = IngestFile::new(file.bytes.clone()).with_source_name(file.name.clone());
        if let Err(error) = ingest_file(env, draft, ingest, IngestFlags::default()) {
            errors.add("error", format!("Error storing uploaded file: {error}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::UploadedFile;
    use crate::testing::{Harness, media_row};

    /// `Ingester/Upload.php:45`: no file data is an error under `error`.
    #[test]
    fn upload_requires_files() {
        let harness = Harness::new("upload-requires-files");
        let env = harness.env();
        let request = IngestRequest::from_value(serde_json::json!({"file_index": 0}));
        let mut draft = MediaDraft::new(media_row(1, "upload", "file"));
        let mut errors = ErrorStore::default();
        Upload.ingest(&mut draft, &request, &env, &mut errors);
        assert_eq!(errors.messages_for("error"), vec!["No files were uploaded"]);
        assert!(draft.row.storage_id.is_none());
    }

    /// `Ingester/Upload.php:50`: a missing `file_index` is an error.
    #[test]
    fn upload_requires_a_file_index() {
        let harness = Harness::new("upload-requires-index");
        let env = harness.env();
        let request = IngestRequest::from_value(serde_json::json!({}))
            .with_files(vec![UploadedFile::new("photo.jpg", b"jpeg")]);
        let mut draft = MediaDraft::new(media_row(1, "upload", "file"));
        let mut errors = ErrorStore::default();
        Upload.ingest(&mut draft, &request, &env, &mut errors);
        assert_eq!(
            errors.messages_for("error"),
            vec!["No file index was specified"]
        );
    }

    /// `Ingester/Upload.php:55`: an index with no file is an error.
    #[test]
    fn upload_rejects_an_index_without_a_file() {
        let harness = Harness::new("upload-bad-index");
        let env = harness.env();
        let request = IngestRequest::from_value(serde_json::json!({"file_index": 3}))
            .with_files(vec![UploadedFile::new("photo.jpg", b"jpeg")]);
        let mut draft = MediaDraft::new(media_row(1, "upload", "file"));
        let mut errors = ErrorStore::default();
        Upload.ingest(&mut draft, &request, &env, &mut errors);
        assert_eq!(
            errors.messages_for("error"),
            vec!["No file uploaded for the specified index"]
        );
    }

    /// `Ingester/Upload.php:63`: the file is stored as the original, the source
    /// falls back to the client name, and the extension, checksum and size are
    /// recorded.
    #[test]
    fn upload_stores_the_file_and_sets_the_source_name() {
        let harness = Harness::new("upload-happy");
        let env = harness.env();
        let request = IngestRequest::from_value(serde_json::json!({"file_index": 0}))
            .with_files(vec![UploadedFile::new("Photo.JPG", b"jpegbytes")]);
        let mut draft = MediaDraft::new(media_row(1, "upload", "file"));
        let mut errors = ErrorStore::default();
        Upload.ingest(&mut draft, &request, &env, &mut errors);

        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(draft.row.source.as_deref(), Some("Photo.JPG"));
        assert_eq!(draft.row.extension.as_deref(), Some("jpg"));
        assert_eq!(draft.row.size, Some(9));
        assert!(draft.row.has_original);
        let storage_id = draft.row.storage_id.clone().expect("storage id");
        assert_eq!(storage_id.len(), 40);
        assert!(harness.store.exists(&format!("original/{storage_id}.jpg")));
    }

    /// `Ingester/Upload.php:61`: an `o:source` the caller already set is not
    /// overwritten by the client file name.
    #[test]
    fn upload_keeps_an_explicit_source() {
        let harness = Harness::new("upload-explicit-source");
        let env = harness.env();
        let request =
            IngestRequest::from_value(serde_json::json!({"file_index": 0, "o:source": "chosen"}))
                .with_files(vec![UploadedFile::new("photo.jpg", b"jpeg")]);
        let mut draft = MediaDraft::new(media_row(1, "upload", "file"));
        // The API hydrator sets `media.source` from `o:source` before the
        // ingester runs, so the ingester must leave it alone.
        draft.row.source = Some("chosen".to_string());
        let mut errors = ErrorStore::default();
        Upload.ingest(&mut draft, &request, &env, &mut errors);
        assert_eq!(draft.row.source.as_deref(), Some("chosen"));
    }
}
