//! The oEmbed ingester (`Media/Ingester/OEmbed.php`).
//!
//! Fetches an oEmbed response for the source URL, downloads its thumbnail, and
//! stores the response. It is a mutable ingester, so it can refresh the response
//! on an update.

use crate::data::MediaData;
use crate::ingest::{
    ErrorStore, IngestEnv, IngestFile, IngestFlags, IngestRequest, Ingester, MediaDraft,
    MutableIngester, ingest_file, truthy,
};

/// The oEmbed ingester.
#[derive(Debug, Clone, Copy, Default)]
pub struct OEmbed;

impl Ingester for OEmbed {
    fn name(&self) -> &str {
        "oembed"
    }

    fn label(&self) -> String {
        "oEmbed".to_string()
    }

    fn renderer(&self) -> &'static str {
        "oembed"
    }

    /// `Ingester/OEmbed.php:59`.
    fn ingest(
        &self,
        draft: &mut MediaDraft,
        request: &IngestRequest,
        env: &IngestEnv<'_>,
        errors: &mut ErrorStore,
    ) {
        let Some(source) = request.string("o:source") else {
            errors.add("o:source", "No OEmbed URL specified");
            return;
        };
        let Some(oembed) = env.oembed.get_oembed(env.http, source, errors, "o:source") else {
            return;
        };
        if let Some(thumbnail) = oembed.field("thumbnail_url")
            && let Ok(response) = env.http.fetch(&thumbnail)
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
                    format!("Error storing the oEmbed thumbnail: {error}"),
                );
            }
        }
        draft.data = MediaData::Oembed(oembed);
        draft.row.source = Some(source.to_string());
    }
}

impl MutableIngester for OEmbed {
    /// `Ingester/OEmbed.php:86`: `refresh_oembed` re-fetches the response for
    /// the media's stored source.
    fn update(
        &self,
        draft: &mut MediaDraft,
        request: &IngestRequest,
        env: &IngestEnv<'_>,
        errors: &mut ErrorStore,
    ) {
        let refresh = request.value("refresh_oembed").is_some_and(truthy);
        if !refresh {
            return;
        }
        let Some(source) = draft.row.source.clone() else {
            return;
        };
        if let Some(oembed) = env.oembed.get_oembed(env.http, &source, errors, "o:source") {
            draft.data = MediaData::Oembed(oembed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{Harness, media_row};

    /// `Ingester/OEmbed.php:61`: `o:source` is required.
    #[test]
    fn oembed_requires_a_source() {
        let harness = Harness::new("oembed-requires-source");
        let env = harness.env();
        let mut draft = MediaDraft::new(media_row(1, "oembed", "oembed"));
        let mut errors = ErrorStore::default();
        OEmbed.ingest(
            &mut draft,
            &IngestRequest::from_value(serde_json::json!({})),
            &env,
            &mut errors,
        );
        assert_eq!(
            errors.messages_for("o:source"),
            vec!["No OEmbed URL specified"]
        );
    }

    /// `Ingester/OEmbed.php:64`: the response is stored, the thumbnail is
    /// downloaded, and the source is recorded.
    #[test]
    fn oembed_ingest_stores_the_response_thumbnail_and_source() {
        let response = serde_json::json!({
            "type": "video",
            "title": "A video",
            "thumbnail_url": "https://example.org/thumb.jpg"
        });
        let harness = Harness::new("oembed-ingest")
            .with_json(
                "https://www.youtube.com/oembed?format=json&url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3Dx",
                response.clone(),
            )
            .with_body("https://example.org/thumb.jpg", b"jpeg");
        let env = harness.env();
        let mut draft = MediaDraft::new(media_row(1, "oembed", "oembed"));
        let mut errors = ErrorStore::default();
        OEmbed.ingest(
            &mut draft,
            &IngestRequest::from_value(
                serde_json::json!({"o:source": "https://www.youtube.com/watch?v=x"}),
            ),
            &env,
            &mut errors,
        );

        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(draft.data.to_value(), response);
        assert_eq!(
            draft.row.source.as_deref(),
            Some("https://www.youtube.com/watch?v=x")
        );
        assert!(draft.row.has_thumbnails);
        assert!(!draft.row.has_original);
    }

    /// `Ingester/OEmbed.php:88`: an update with `refresh_oembed` re-fetches, and
    /// without it leaves the payload alone.
    #[test]
    fn oembed_update_refreshes_only_when_asked() {
        let harness = Harness::new("oembed-update").with_json(
            "https://www.youtube.com/oembed?format=json&url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3Dx",
            serde_json::json!({"type": "video", "title": "Refreshed"}),
        );
        let env = harness.env();
        let mut draft = MediaDraft::new(media_row(1, "oembed", "oembed"));
        draft.row.source = Some("https://www.youtube.com/watch?v=x".to_string());
        let mut errors = ErrorStore::default();

        OEmbed.update(
            &mut draft,
            &IngestRequest::from_value(serde_json::json!({})),
            &env,
            &mut errors,
        );
        assert_eq!(draft.data, MediaData::None);
        assert!(harness.http.calls().is_empty());

        OEmbed.update(
            &mut draft,
            &IngestRequest::from_value(serde_json::json!({"refresh_oembed": true})),
            &env,
            &mut errors,
        );
        assert_eq!(draft.data.to_value()["title"], "Refreshed");
    }
}
