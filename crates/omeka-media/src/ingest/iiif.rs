//! The IIIF image ingester (`Media/Ingester/IIIF.php`).
//!
//! Fetches a IIIF image information document, validates it, stores it, and
//! downloads a pre-computed size large enough for thumbnailing.

use serde_json::Value;

use crate::data::{IiifImageData, MediaData};
use crate::ingest::{
    ErrorStore, IngestEnv, IngestFile, IngestFlags, IngestRequest, Ingester, MediaDraft,
    ingest_file,
};
use crate::uri::HttpUri;

/// The IIIF image ingester.
#[derive(Debug, Clone, Copy)]
pub struct Iiif {
    max_constraint: u32,
}

impl Default for Iiif {
    fn default() -> Self {
        Self::new(800)
    }
}

impl Iiif {
    /// Build with the minimum size a pre-computed candidate must meet.
    ///
    /// `IIIFFactory.php:20` takes this from the largest thumbnail constraint.
    #[must_use]
    pub const fn new(max_constraint: u32) -> Self {
        Self { max_constraint }
    }

    /// The minimum candidate size.
    #[must_use]
    pub const fn max_constraint(&self) -> u32 {
        self.max_constraint
    }

    /// Validate a IIIF image document, matching `IIIF::validate`
    /// (`Ingester/IIIF.php:157`).
    ///
    /// The check comes from OpenSeadragon's own validation, as upstream notes.
    /// One upstream branch reads `$IIIFData['profile'][0]("...")`, which calls a
    /// string as a function and would be a fatal error in PHP 8; the port
    /// implements the evident intent, a prefix test on the first profile entry.
    #[must_use]
    pub fn validate(&self, data: &IiifImageData) -> bool {
        if data.member("protocol") == Some("http://iiif.io/api/image") {
            return true;
        }
        if let Some(context) = data.context()
            && (context == "http://library.stanford.edu/iiif/image-api/1.1/context.json"
                || context == "http://iiif.io/api/image/1/context.json")
        {
            return true;
        }
        let profile_prefix = "http://library.stanford.edu/iiif/image-api/compliance.html";
        let profile = data.value.get("profile");
        let first_profile = match profile {
            Some(Value::Array(items)) => items.first().and_then(Value::as_str),
            Some(Value::String(text)) => Some(text.as_str()),
            _ => None,
        };
        if first_profile.is_some_and(|profile| profile.starts_with(profile_prefix)) {
            return true;
        }
        if data.member("identifier").is_some()
            && data.value.get("width").is_some_and(crate::ingest::truthy)
            && data.value.get("height").is_some_and(crate::ingest::truthy)
        {
            return true;
        }
        if let Some(element) = data.value.get("documentElement")
            && element.get("tagName").and_then(Value::as_str) == Some("info")
            && element.get("namespaceURI").and_then(Value::as_str)
                == Some("http://library.stanford.edu/iiif/image-api/ns/")
        {
            return true;
        }
        false
    }

    /// Pick the smallest pre-computed size that meets the constraint, matching
    /// `IIIF::selectSize` (`Ingester/IIIF.php:104`).
    #[must_use]
    pub fn select_size(&self, data: &IiifImageData) -> Option<String> {
        let mut best: Option<(u64, u64)> = None;
        for candidate in data.sizes() {
            let (Some(width), Some(height)) = (
                candidate.get("width").and_then(Value::as_u64),
                candidate.get("height").and_then(Value::as_u64),
            ) else {
                continue;
            };
            if width < u64::from(self.max_constraint) || height < u64::from(self.max_constraint) {
                continue;
            }
            if best
                .is_none_or(|(best_width, best_height)| width * height < best_width * best_height)
            {
                best = Some((width, height));
            }
        }
        best.map(|(width, height)| format!("{width},{height}"))
    }
}

impl Ingester for Iiif {
    fn name(&self) -> &str {
        "iiif"
    }

    fn label(&self) -> String {
        "IIIF image".to_string()
    }

    fn renderer(&self) -> &'static str {
        "iiif"
    }

    /// `Ingester/IIIF.php:47`.
    fn ingest(
        &self,
        draft: &mut MediaDraft,
        request: &IngestRequest,
        env: &IngestEnv<'_>,
        errors: &mut ErrorStore,
    ) {
        let Some(raw) = request.string("o:source") else {
            errors.add("o:source", "No IIIF image URL specified");
            return;
        };
        let Some(uri) = HttpUri::parse(raw) else {
            errors.add("o:source", "Invalid URL specified");
            return;
        };
        let Some(image) = self.fetch_document(uri.raw(), env, errors) else {
            return;
        };
        if !self.validate(&image) {
            errors.add("o:source", "URL does not link to IIIF JSON");
            return;
        }
        draft.data = MediaData::Iiif(image.clone());
        self.store_thumbnail(draft, env, &image, errors);
    }
}

impl Iiif {
    /// Fetch and decode the information document, recording an error and
    /// returning `None` when it cannot be read.
    fn fetch_document(
        &self,
        url: &str,
        env: &IngestEnv<'_>,
        errors: &mut ErrorStore,
    ) -> Option<IiifImageData> {
        let response = match env.http.fetch(url) {
            Ok(response) => response,
            Err(error) => {
                errors.add("o:source", format!("Error reading IIIF image: {error}"));
                return None;
            }
        };
        if !response.is_ok() {
            errors.add(
                "o:source",
                format!(
                    "Error reading {}: {} ({})",
                    self.label(),
                    response.reason,
                    response.status
                ),
            );
            return None;
        }
        let Ok(value) = serde_json::from_slice::<Value>(&response.body) else {
            errors.add("o:source", "Error decoding IIIF JSON");
            return None;
        };
        if !crate::ingest::truthy(&value) {
            errors.add("o:source", "Error decoding IIIF JSON");
            return None;
        }
        match MediaData::parse("iiif", Some(&value)) {
            Ok(MediaData::Iiif(image)) => Some(image),
            _ => {
                errors.add("o:source", "Error decoding IIIF JSON");
                None
            }
        }
    }

    /// Download the smallest pre-computed size that can be thumbnailed,
    /// choosing the size and quality by the document's API version
    /// (`Ingester/IIIF.php:86`).
    fn store_thumbnail(
        &self,
        draft: &mut MediaDraft,
        env: &IngestEnv<'_>,
        image: &IiifImageData,
        errors: &mut ErrorStore,
    ) {
        let (id, default_size, quality) = match image.context() {
            Some("http://iiif.io/api/image/3/context.json") => {
                (image.member("id"), "max", "default")
            }
            Some("http://iiif.io/api/image/2/context.json") => {
                (image.member("@id"), "full", "default")
            }
            _ => (image.member("@id"), "full", "native"),
        };
        let Some(id) = id else {
            return;
        };
        let size = self
            .select_size(image)
            .unwrap_or_else(|| default_size.to_string());
        let url = format!("{id}/full/{size}/0/{quality}.jpg");
        let Ok(response) = env.http.fetch(&url) else {
            return;
        };
        if !response.is_ok() {
            return;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{Harness, media_row};

    fn image(value: serde_json::Value) -> IiifImageData {
        IiifImageData { value }
    }

    /// `Ingester/IIIF.php:159`: the v2 protocol and context both validate.
    #[test]
    fn iiif_validate_matches_upstream_checks() {
        let ingester = Iiif::default();
        assert!(ingester.validate(&image(
            serde_json::json!({"protocol": "http://iiif.io/api/image"})
        )));
        assert!(ingester.validate(&image(serde_json::json!({
            "@context": "http://iiif.io/api/image/1/context.json"
        }))));
        assert!(ingester.validate(&image(serde_json::json!({
            "profile": ["http://library.stanford.edu/iiif/image-api/compliance.html#level1"]
        }))));
        assert!(ingester.validate(&image(serde_json::json!({
            "identifier": "x", "width": 100, "height": 100
        }))));
        assert!(ingester.validate(&image(serde_json::json!({
            "documentElement": {
                "tagName": "info",
                "namespaceURI": "http://library.stanford.edu/iiif/image-api/ns/"
            }
        }))));
        assert!(!ingester.validate(&image(
            serde_json::json!({"@context": "https://example.org/x"})
        )));
    }

    /// `Ingester/IIIF.php:104`: the smallest pre-computed size that meets the
    /// constraint wins.
    #[test]
    fn iiif_select_size_prefers_the_smallest_sufficient_candidate() {
        let ingester = Iiif::new(800);
        let data = image(serde_json::json!({"sizes": [
            {"width": 200, "height": 200},
            {"width": 1000, "height": 900},
            {"width": 900, "height": 800},
            {"width": 4000, "height": 3000}
        ]}));
        assert_eq!(ingester.select_size(&data).as_deref(), Some("900,800"));
        assert_eq!(
            Iiif::new(800).select_size(&image(serde_json::json!({"sizes": []}))),
            None
        );
    }

    /// `Ingester/IIIF.php:73`: the document is stored and a thumbnail is
    /// downloaded from the version's size and quality.
    #[test]
    fn iiif_ingest_stores_the_document_and_a_thumbnail() {
        let document = serde_json::json!({
            "@context": "http://iiif.io/api/image/2/context.json",
            "@id": "https://example.org/iiif/1",
            "sizes": [{"width": 900, "height": 800}],
            "profile": ["http://library.stanford.edu/iiif/image-api/compliance.html#level1"]
        });
        let harness = Harness::new("iiif-ingest")
            .with_json("https://example.org/iiif/1/info.json", document.clone())
            .with_body(
                "https://example.org/iiif/1/full/900,800/0/default.jpg",
                b"jpeg",
            );
        let env = harness.env();
        let request = IngestRequest::from_value(
            serde_json::json!({"o:source": "https://example.org/iiif/1/info.json"}),
        );
        let mut draft = MediaDraft::new(media_row(1, "iiif", "iiif"));
        let mut errors = ErrorStore::default();
        Iiif::new(800).ingest(&mut draft, &request, &env, &mut errors);

        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(draft.data.to_value(), document);
        assert!(draft.row.has_thumbnails);
        assert_eq!(
            harness.http.calls(),
            [
                "https://example.org/iiif/1/info.json",
                "https://example.org/iiif/1/full/900,800/0/default.jpg"
            ]
        );
    }

    /// `Ingester/IIIF.php:69`: a document that fails validation is rejected.
    #[test]
    fn iiif_ingest_rejects_a_non_iiif_document() {
        let harness = Harness::new("iiif-reject").with_json(
            "https://example.org/x",
            serde_json::json!({"hello": "world"}),
        );
        let env = harness.env();
        let request =
            IngestRequest::from_value(serde_json::json!({"o:source": "https://example.org/x"}));
        let mut draft = MediaDraft::new(media_row(1, "iiif", "iiif"));
        let mut errors = ErrorStore::default();
        Iiif::default().ingest(&mut draft, &request, &env, &mut errors);
        assert_eq!(
            errors.messages_for("o:source"),
            vec!["URL does not link to IIIF JSON"]
        );
    }
}
