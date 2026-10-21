//! The YouTube ingester (`Media/Ingester/Youtube.php`).
//!
//! Reads the video id out of a `youtube.com/watch` or `youtu.be` URL, downloads
//! the poster frame, and stores the id with the optional start and end times.

use crate::data::{MediaData, YoutubeData};
use crate::ingest::{
    ErrorStore, IngestEnv, IngestFile, IngestFlags, IngestRequest, Ingester, MediaDraft,
    ingest_file, php_is_numeric, trim_scalar,
};
use crate::uri::HttpUri;

/// The YouTube ingester.
#[derive(Debug, Clone, Copy, Default)]
pub struct Youtube;

/// Why a URL is not one the YouTube ingester reads, with the message the
/// ingester reports (`Ingester/Youtube.php:49`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YoutubeUrl {
    /// Not an absolute http(s) URL.
    Invalid,
    /// `www.youtube.com` with a path other than `/watch`.
    NotWatch,
    /// A `watch` URL with no `v` parameter.
    NoVideoId,
    /// A host YouTube does not serve.
    NotYoutube,
}

impl YoutubeUrl {
    /// The message the ingester adds under `o:source`.
    #[must_use]
    pub fn message(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid YouTube URL specified",
            Self::NotWatch => "Invalid YouTube URL specified, missing \"/watch\" path",
            Self::NoVideoId => "Invalid YouTube URL specified, missing \"v\" parameter",
            Self::NotYoutube => "Invalid YouTube URL specified, not a YouTube URL",
        }
    }
}

/// The video id a URL names, reading `www.youtube.com/watch?v=` and
/// `youtu.be/<id>` as the ingester does. The link probe asks this without
/// ingesting anything, so the two agree on what a YouTube URL is.
///
/// # Errors
/// A [`YoutubeUrl`] naming why the URL is not one.
pub fn video_id(raw: &str) -> Result<String, YoutubeUrl> {
    let uri = HttpUri::parse(raw).ok_or(YoutubeUrl::Invalid)?;
    match uri.host() {
        "www.youtube.com" => {
            if uri.path() != "/watch" {
                return Err(YoutubeUrl::NotWatch);
            }
            uri.query_value("v")
                .map(str::to_string)
                .ok_or(YoutubeUrl::NoVideoId)
        }
        "youtu.be" => Ok(uri.path().strip_prefix('/').unwrap_or("").to_string()),
        _ => Err(YoutubeUrl::NotYoutube),
    }
}

impl Ingester for Youtube {
    fn name(&self) -> &str {
        "youtube"
    }

    fn label(&self) -> String {
        "YouTube".to_string()
    }

    fn renderer(&self) -> &'static str {
        "youtube"
    }

    /// `Ingester/Youtube.php:39`.
    ///
    /// Only `www.youtube.com/watch` and `youtu.be` are accepted. `start` and
    /// `end` are stored when `is_numeric` accepts them.
    fn ingest(
        &self,
        draft: &mut MediaDraft,
        request: &IngestRequest,
        env: &IngestEnv<'_>,
        errors: &mut ErrorStore,
    ) {
        let Some(raw) = request.string("o:source") else {
            errors.add("o:source", "No YouTube URL specified");
            return;
        };
        let youtube_id = match video_id(raw) {
            Ok(id) => id,
            Err(error) => {
                errors.add("o:source", error.message());
                return;
            }
        };

        let poster = format!("http://img.youtube.com/vi/{youtube_id}/0.jpg");
        if let Ok(response) = env.http.fetch(&poster)
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
                    format!("Error storing the YouTube thumbnail: {error}"),
                );
                return;
            }
        }

        let mut data = YoutubeData {
            id: youtube_id,
            start: None,
            end: None,
        };
        for (key, slot) in [("start", &mut data.start), ("end", &mut data.end)] {
            if let Some(value) = request.value(key) {
                let text = trim_scalar(value);
                if php_is_numeric(&text) {
                    *slot = Some(text);
                }
            }
        }
        draft.data = MediaData::Youtube(data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{Harness, media_row};

    fn run(harness: &Harness, content: serde_json::Value) -> (MediaDraft, ErrorStore) {
        let env = harness.env();
        let request = IngestRequest::from_value(content);
        let mut draft = MediaDraft::new(media_row(1, "youtube", "youtube"));
        let mut errors = ErrorStore::default();
        Youtube.ingest(&mut draft, &request, &env, &mut errors);
        (draft, errors)
    }

    /// `Ingester/Youtube.php:60`: the id comes from the `v` parameter, and the
    /// poster frame is downloaded.
    #[test]
    fn youtube_reads_the_watch_parameter() {
        let harness = Harness::new("youtube-watch")
            .with_body("http://img.youtube.com/vi/abc123/0.jpg", b"jpeg");
        let (draft, errors) = run(
            &harness,
            serde_json::json!({"o:source": "https://www.youtube.com/watch?v=abc123"}),
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            draft.data,
            MediaData::Youtube(YoutubeData {
                id: "abc123".to_string(),
                start: None,
                end: None,
            })
        );
        assert!(draft.row.has_thumbnails);
        assert!(!draft.row.has_original);
        assert_eq!(
            harness.http.calls(),
            ["http://img.youtube.com/vi/abc123/0.jpg"]
        );
    }

    /// `Ingester/Youtube.php:66`: the short host takes the id from the path.
    #[test]
    fn youtube_reads_the_short_url() {
        let harness =
            Harness::new("youtube-short").with_body("http://img.youtube.com/vi/xyz/0.jpg", b"jpeg");
        let (draft, errors) = run(
            &harness,
            serde_json::json!({"o:source": "https://youtu.be/xyz"}),
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            draft.data,
            MediaData::Youtube(YoutubeData {
                id: "xyz".to_string(),
                start: None,
                end: None,
            })
        );
    }

    /// `Ingester/Youtube.php:79`: numeric `start` and `end` are stored, and a
    /// non-numeric one is dropped.
    #[test]
    fn youtube_stores_numeric_start_and_end() {
        let harness = Harness::new("youtube-times");
        let (draft, errors) = run(
            &harness,
            serde_json::json!({
                "o:source": "https://youtu.be/xyz",
                "start": " 30 ",
                "end": "later"
            }),
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            draft.data,
            MediaData::Youtube(YoutubeData {
                id: "xyz".to_string(),
                start: Some("30".to_string()),
                end: None,
            })
        );
    }

    /// `Ingester/Youtube.php:49`: a watch URL without `v`, a wrong path, and a
    /// foreign host are all errors.
    #[test]
    fn youtube_rejects_malformed_urls() {
        let harness = Harness::new("youtube-rejects");
        let (_, errors) = run(
            &harness,
            serde_json::json!({"o:source": "https://www.youtube.com/watch"}),
        );
        assert_eq!(
            errors.messages_for("o:source"),
            vec!["Invalid YouTube URL specified, missing \"v\" parameter"]
        );

        let (_, errors) = run(
            &harness,
            serde_json::json!({"o:source": "https://www.youtube.com/channel/x"}),
        );
        assert_eq!(
            errors.messages_for("o:source"),
            vec!["Invalid YouTube URL specified, missing \"/watch\" path"]
        );

        let (_, errors) = run(
            &harness,
            serde_json::json!({"o:source": "https://example.org/watch?v=x"}),
        );
        assert_eq!(
            errors.messages_for("o:source"),
            vec!["Invalid YouTube URL specified, not a YouTube URL"]
        );

        let (_, errors) = run(&harness, serde_json::json!({"o:source": "not a url"}));
        assert_eq!(
            errors.messages_for("o:source"),
            vec!["Invalid YouTube URL specified"]
        );

        let (_, errors) = run(&harness, serde_json::json!({}));
        assert_eq!(
            errors.messages_for("o:source"),
            vec!["No YouTube URL specified"]
        );
    }

    /// The probe reads the same id the ingester does, from both URL forms.
    #[test]
    fn video_id_reads_the_two_url_forms() {
        assert_eq!(
            video_id("https://www.youtube.com/watch?v=abc123"),
            Ok("abc123".to_string())
        );
        assert_eq!(video_id("https://youtu.be/xyz"), Ok("xyz".to_string()));
        assert_eq!(
            video_id("https://vimeo.com/123"),
            Err(YoutubeUrl::NotYoutube)
        );
    }
}
