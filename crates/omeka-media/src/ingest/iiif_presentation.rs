//! The IIIF presentation ingester (`Media/Ingester/IiifPresentation.php`).
//!
//! Fetches a IIIF manifest or collection, validates its version, stores it, and
//! downloads the top-level thumbnail when the document has one.

use serde_json::Value;

use crate::data::{IiifPresentationData, MediaData};
use crate::ingest::{
    ErrorStore, IngestEnv, IngestFile, IngestFlags, IngestRequest, Ingester, MediaDraft,
    ingest_file,
};
use crate::uri::HttpUri;

/// The IIIF presentation ingester.
#[derive(Debug, Clone, Copy, Default)]
pub struct IiifPresentation;

impl IiifPresentation {
    /// The presentation API version, matching `IiifPresentation::getVersion`
    /// (`Ingester/IiifPresentation.php:130`).
    ///
    /// Presentation API 1 is not compatible, which upstream documents. The
    /// `@type` or `type` member must also be a manifest or a collection, so the
    /// check validates the document a little as a side effect.
    #[must_use]
    pub fn version(&self, data: &IiifPresentationData) -> Option<u8> {
        let context = data.context()?;
        match context {
            "http://iiif.io/api/presentation/2/context.json"
            | "https://iiif.io/api/presentation/2/context.json" => match data.at_type() {
                Some("sc:Manifest" | "sc:Collection") => Some(2),
                _ => None,
            },
            "http://iiif.io/api/presentation/3/context.json"
            | "https://iiif.io/api/presentation/3/context.json" => match data.type_value() {
                Some("Manifest" | "Collection") => Some(3),
                _ => None,
            },
            _ => None,
        }
    }
}

impl Ingester for IiifPresentation {
    fn name(&self) -> &str {
        "iiif_presentation"
    }

    fn label(&self) -> String {
        "IIIF presentation".to_string()
    }

    fn renderer(&self) -> &'static str {
        "iiif_presentation"
    }

    /// `Ingester/IiifPresentation.php:60`.
    fn ingest(
        &self,
        draft: &mut MediaDraft,
        request: &IngestRequest,
        env: &IngestEnv<'_>,
        errors: &mut ErrorStore,
    ) {
        let source = request
            .string("o:source")
            .map(str::trim)
            .filter(|source| !source.is_empty());
        let Some(source) = source else {
            errors.add("o:source", "No IIIF presentation URL specified");
            return;
        };
        let Some(uri) = HttpUri::parse(source) else {
            errors.add("o:source", "Invalid IIIF presentation URL specified");
            return;
        };
        let response = match env.http.fetch(uri.raw()) {
            Ok(response) => response,
            Err(error) => {
                errors.add(
                    "o:source",
                    format!("Error connecting to IIIF presentation: {error}"),
                );
                return;
            }
        };
        if !response.is_ok() {
            errors.add(
                "o:source",
                format!(
                    "Error reading IIIF presentation: {} ({})",
                    response.reason, response.status
                ),
            );
            return;
        }
        let Ok(value) = serde_json::from_slice::<Value>(&response.body) else {
            errors.add("o:source", "Error decoding IIIF presentation JSON");
            return;
        };
        if !crate::ingest::truthy(&value) {
            errors.add("o:source", "Error decoding IIIF presentation JSON");
            return;
        }
        let data = IiifPresentationData { value };
        let Some(version) = self.version(&data) else {
            errors.add("o:source", "Invalid IIIF presentation JSON");
            return;
        };
        draft.data = MediaData::IiifPresentation(data.clone());

        let Some(thumbnail) = data.thumbnail_url(version) else {
            return;
        };
        if let Ok(response) = env.http.fetch(&thumbnail)
            && response.is_ok()
        {
            let file = IngestFile::new(response.body).with_media_type("image/jpeg");
            let flags = IngestFlags {
                store_original: false,
                store_thumbnails: true,
                hydrate_metadata: false,
            };
            if let Err(error) = ingest_file(env, draft, file, flags) {
                errors.add(
                    "error",
                    format!("Error storing the IIIF thumbnail: {error}"),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{Harness, media_row};

    fn presentation(value: serde_json::Value) -> IiifPresentationData {
        IiifPresentationData { value }
    }

    /// `Ingester/IiifPresentation.php:130`: v2 needs an `@type`, v3 needs a
    /// `type`, and presentation API 1 is not compatible.
    #[test]
    fn iiif_presentation_version_matches_upstream() {
        let ingester = IiifPresentation;
        assert_eq!(
            ingester.version(&presentation(serde_json::json!({
                "@context": "http://iiif.io/api/presentation/2/context.json",
                "@type": "sc:Manifest"
            }))),
            Some(2)
        );
        assert_eq!(
            ingester.version(&presentation(serde_json::json!({
                "@context": "https://iiif.io/api/presentation/3/context.json",
                "type": "Collection"
            }))),
            Some(3)
        );
        assert_eq!(
            ingester.version(&presentation(serde_json::json!({
                "@context": "http://iiif.io/api/presentation/2/context.json"
            }))),
            None
        );
        assert_eq!(
            ingester.version(&presentation(serde_json::json!({
                "@context": "http://iiif.io/api/presentation/1/context.json"
            }))),
            None
        );
    }

    /// `Ingester/IiifPresentation.php:107`: the manifest is stored and its v2
    /// thumbnail is downloaded.
    #[test]
    fn iiif_presentation_ingest_stores_the_manifest_and_thumbnail() {
        let manifest = serde_json::json!({
            "@context": "http://iiif.io/api/presentation/2/context.json",
            "@type": "sc:Manifest",
            "thumbnail": {"@id": "https://example.org/thumb.jpg"}
        });
        let harness = Harness::new("iiif-presentation")
            .with_json("https://example.org/manifest", manifest.clone())
            .with_body("https://example.org/thumb.jpg", b"jpeg");
        let env = harness.env();
        let request = IngestRequest::from_value(
            serde_json::json!({"o:source": "https://example.org/manifest"}),
        );
        let mut draft = MediaDraft::new(media_row(1, "iiif_presentation", "iiif_presentation"));
        let mut errors = ErrorStore::default();
        IiifPresentation.ingest(&mut draft, &request, &env, &mut errors);

        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(draft.data.to_value(), manifest);
        assert!(draft.row.has_thumbnails);
        assert_eq!(
            harness.http.calls(),
            [
                "https://example.org/manifest",
                "https://example.org/thumb.jpg"
            ]
        );
    }

    /// `Ingester/IiifPresentation.php:104`: a document with no version is
    /// rejected, and the required source is checked first.
    #[test]
    fn iiif_presentation_rejects_missing_source_and_bad_documents() {
        let harness = Harness::new("iiif-presentation-reject").with_json(
            "https://example.org/x",
            serde_json::json!({"@context": "https://example.org"}),
        );
        let env = harness.env();

        let mut draft = MediaDraft::new(media_row(1, "iiif_presentation", "iiif_presentation"));
        let mut errors = ErrorStore::default();
        IiifPresentation.ingest(
            &mut draft,
            &IngestRequest::from_value(serde_json::json!({"o:source": "  "})),
            &env,
            &mut errors,
        );
        assert_eq!(
            errors.messages_for("o:source"),
            vec!["No IIIF presentation URL specified"]
        );

        let mut draft = MediaDraft::new(media_row(1, "iiif_presentation", "iiif_presentation"));
        let mut errors = ErrorStore::default();
        IiifPresentation.ingest(
            &mut draft,
            &IngestRequest::from_value(serde_json::json!({"o:source": "https://example.org/x"})),
            &env,
            &mut errors,
        );
        assert_eq!(
            errors.messages_for("o:source"),
            vec!["Invalid IIIF presentation JSON"]
        );
    }
}
