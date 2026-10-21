//! The oEmbed service.
//!
//! Ports `Omeka\Stdlib\Oembed`. The `OEmbed` ingester and the `OEmbed`
//! renderer both use it: the ingester to fetch and store a response, the
//! renderer to turn one back into markup.
//!
//! The allowlist is the `oembed.whitelist` table from
//! `application/config/module.config.php:906`. Upstream matches each entry's
//! regular expression against the source URL; the port stores the same rules as
//! host and path data, so it needs no regular expression engine. Two upstream
//! rules carry a per-segment pattern (`twitter.com/\w{1,15}$` and the
//! `issuu.com/.+/docs/.+` shape); the port matches their host and first path
//! segment instead, which is the documented approximation.

use serde_json::Value;

use crate::data::OembedData;
use crate::http::HttpFetch;
use crate::ingest::ErrorStore;
use crate::text::{escape_html, urlencode};
use crate::uri::HttpUri;

/// One allowlist entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowRule {
    /// Hosts that match. A leading dot matches the bare domain and any
    /// subdomain, e.g. `.vimeo.com` matches `vimeo.com` and `player.vimeo.com`.
    pub hosts: Vec<&'static str>,
    /// Path prefixes; empty means any path.
    pub path_prefixes: Vec<&'static str>,
    /// The configured oEmbed endpoint, or `None` to discover one from the page.
    pub endpoint: Option<&'static str>,
}

/// The oEmbed allowlist.
#[derive(Debug, Clone)]
pub struct OembedAllowList {
    rules: Vec<AllowRule>,
}

impl OembedAllowList {
    /// Build from an explicit rule list.
    #[must_use]
    pub fn new(rules: Vec<AllowRule>) -> Self {
        Self { rules }
    }

    /// The shipped rules.
    #[must_use]
    pub fn shipped() -> Self {
        Self::new(shipped_rules())
    }

    /// The rules, for tests and diagnostics.
    #[must_use]
    pub fn rules(&self) -> &[AllowRule] {
        &self.rules
    }

    /// Every host the rules allow, with a leading dot removed, for the frame
    /// check in [`render_oembed`].
    #[must_use]
    pub fn hosts(&self) -> Vec<&'static str> {
        self.rules
            .iter()
            .flat_map(|rule| rule.hosts.iter().map(|host| host.trim_start_matches('.')))
            .collect()
    }

    /// The endpoint for a source URL.
    ///
    /// `Some(None)` means the URL is allowed and its endpoint must be
    /// discovered from the page; `None` means the URL is not allowed.
    #[must_use]
    pub fn endpoint_for(&self, url: &str) -> Option<Option<&'static str>> {
        let uri = HttpUri::parse(url)?;
        self.rules
            .iter()
            .find(|rule| rule.matches(uri.host(), uri.path()))
            .map(|rule| rule.endpoint)
    }
}

impl AllowRule {
    fn matches(&self, host: &str, path: &str) -> bool {
        let host_ok = self
            .hosts
            .iter()
            .any(|pattern| match pattern.strip_prefix('.') {
                Some(suffix) => host == suffix || host.ends_with(&format!(".{suffix}")),
                None => host == *pattern,
            });
        if !host_ok {
            return false;
        }
        self.path_prefixes.is_empty()
            || self
                .path_prefixes
                .iter()
                .any(|prefix| path.starts_with(prefix))
    }
}

/// The oEmbed service (`Stdlib/Oembed.php`).
#[derive(Debug, Clone)]
pub struct Oembed {
    allow_list: OembedAllowList,
}

impl Oembed {
    /// Build with the shipped allowlist.
    #[must_use]
    pub fn new(allow_list: OembedAllowList) -> Self {
        Self { allow_list }
    }

    /// The allowlist.
    #[must_use]
    pub fn allow_list(&self) -> &OembedAllowList {
        &self.allow_list
    }

    /// Fetch an oEmbed response, matching `Oembed::getOembed`.
    ///
    /// Errors land in `errors` under `error_key`, as upstream passes
    /// `o:source` from the ingester (`Ingester/OEmbed.php:64`).
    pub fn get_oembed(
        &self,
        http: &dyn HttpFetch,
        url: &str,
        errors: &mut ErrorStore,
        error_key: &str,
    ) -> Option<OembedData> {
        let Some(endpoint) = self.allow_list.endpoint_for(url) else {
            errors.add(error_key, format!("oEmbed: URL is not allowed {url}"));
            return None;
        };
        let oembed_url = match endpoint {
            Some(endpoint) => format!("{endpoint}?format=json&url={}", urlencode(url)),
            None => discover_endpoint(http, url, errors, error_key)?,
        };
        let response = match http.fetch(&oembed_url) {
            Ok(response) => response,
            Err(error) => {
                errors.add(
                    error_key,
                    format!("oEmbed: URL is unreadable at {url}: {error}"),
                );
                return None;
            }
        };
        if !response.is_success() {
            errors.add(
                error_key,
                format!(
                    "oEmbed: URL is unreadable at {url}: {} ({})",
                    response.reason, response.status
                ),
            );
            return None;
        }
        match serde_json::from_slice::<Value>(&response.body) {
            Ok(Value::Object(map)) => Some(OembedData { map }),
            _ => {
                errors.add(
                    error_key,
                    "oEmbed: response cannot be decoded to JSON ".to_string(),
                );
                None
            }
        }
    }
}

/// Render oEmbed markup, matching `Oembed::renderOembed`.
///
/// An `html` member passes through [`omeka_core::purify::purify_embed`] with
/// `source` and `provider_hosts`: upstream returns it verbatim, so a provider
/// could put a script or an event handler on the page. A `photo` becomes an
/// `img` whose alt is the response title, else the media's resolved alt
/// (`alt_text`), else empty. Anything else returns `None`, and
/// `Media/Renderer/OEmbed.php:32` then falls back to a hyperlink.
#[must_use]
pub fn render_oembed(
    oembed: &OembedData,
    source: &str,
    alt_text: Option<&str>,
    provider_hosts: &[&str],
) -> Option<String> {
    if let Some(html) = oembed.html() {
        let sanitized = omeka_core::purify::purify_embed(html, source, provider_hosts);
        return (!sanitized.is_empty()).then_some(sanitized);
    }
    if oembed.kind() != Some("photo") {
        return None;
    }
    let url = oembed.field("url").unwrap_or_default();
    let alt = oembed
        .title()
        .or_else(|| alt_text.filter(|text| !text.is_empty()).map(str::to_string))
        .unwrap_or_default();
    Some(format!(
        "<img src=\"{}\" width=\"{}\" height=\"{}\" alt=\"{}\">",
        escape_html(&url),
        escape_html(&oembed.field("width").unwrap_or_default()),
        escape_html(&oembed.field("height").unwrap_or_default()),
        escape_html(&alt),
    ))
}

fn discover_endpoint(
    http: &dyn HttpFetch,
    url: &str,
    errors: &mut ErrorStore,
    error_key: &str,
) -> Option<String> {
    if HttpUri::parse(url).is_none() {
        errors.add(error_key, format!("oEmbed: URL is invalid: {url}"));
        return None;
    }
    let response = match http.fetch(url) {
        Ok(response) => response,
        Err(error) => {
            errors.add(
                error_key,
                format!("oEmbed: URL is unreadable at {url}: {error}"),
            );
            return None;
        }
    };
    if !response.is_success() {
        errors.add(
            error_key,
            format!(
                "oEmbed: URL is unreadable at {url}: {} ({})",
                response.reason, response.status
            ),
        );
        return None;
    }
    match find_oembed_link(&response.text()) {
        Some(href) => Some(href),
        None => {
            errors.add(error_key, format!("oEmbed: links cannot be found at {url}"));
            None
        }
    }
}

/// Find the discovery `link` href in an HTML page, matching the XPath in
/// `Stdlib/Oembed.php:82`.
#[must_use]
pub fn find_oembed_link(html: &str) -> Option<String> {
    let mut rest = html;
    while let Some(start) = rest.find("<link") {
        let after = &rest[start..];
        let end = after.find('>').unwrap_or(after.len());
        let tag = &after[..end];
        let rel = attribute(tag, "rel");
        let kind = attribute(tag, "type");
        let rel_ok = matches!(rel.as_deref(), Some("alternate") | Some("alternative"));
        let type_ok = matches!(
            kind.as_deref(),
            Some("application/json+oembed") | Some("text/json+oembed")
        );
        if rel_ok && type_ok {
            return attribute(tag, "href");
        }
        rest = &after[end.min(after.len())..];
    }
    None
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=");
    let mut search = tag;
    while let Some(pos) = search.find(&needle) {
        let before_ok = pos == 0 || search[..pos].ends_with([' ', '\t', '\n']);
        let value = &search[pos + needle.len()..];
        if before_ok {
            let quote = value.chars().next();
            if let Some(quote @ ('"' | '\'')) = quote {
                let rest = &value[1..];
                if let Some(end) = rest.find(quote) {
                    return Some(rest[..end].to_string());
                }
            }
        }
        search = &search[pos + needle.len()..];
    }
    None
}

fn shipped_rules() -> Vec<AllowRule> {
    UPSTREAM_RULES
        .iter()
        .map(|(hosts, path_prefixes, endpoint)| AllowRule {
            hosts: hosts.to_vec(),
            path_prefixes: path_prefixes.to_vec(),
            endpoint: Some(endpoint),
        })
        .collect()
}

/// The `oembed.whitelist` table from `application/config/module.config.php:906`,
/// as host and path data.
///
/// Each entry is `(hosts, path prefixes, endpoint)`. A host with a leading dot
/// matches the bare domain and any subdomain. Two upstream rules carry a
/// per-segment pattern; they match on host and first path segment here.
const UPSTREAM_RULES: &[(&[&str], &[&str], &str)] = &[
    (
        &["youtube.com", "m.youtube.com", "www.youtube.com"],
        &["/watch", "/playlist", "/shorts/"],
        "https://www.youtube.com/oembed",
    ),
    (&["youtu.be"], &["/"], "https://www.youtube.com/oembed"),
    (&[".vimeo.com"], &["/"], "https://vimeo.com/api/oembed.json"),
    (
        &["dailymotion.com", "www.dailymotion.com"],
        &["/"],
        "https://www.dailymotion.com/services/oembed",
    ),
    (
        &["dai.ly"],
        &["/"],
        "https://www.dailymotion.com/services/oembed",
    ),
    (
        &["flickr.com", "www.flickr.com"],
        &["/"],
        "https://www.flickr.com/services/oembed/",
    ),
    (
        &["flic.kr"],
        &["/"],
        "https://www.flickr.com/services/oembed/",
    ),
    (
        &[".smugmug.com"],
        &["/"],
        "https://api.smugmug.com/services/oembed/",
    ),
    (
        &["scribd.com", "www.scribd.com"],
        &["/doc/", "/document/"],
        "https://www.scribd.com/services/oembed",
    ),
    (&["wordpress.tv"], &["/"], "https://wordpress.tv/oembed/"),
    (
        &[".crowdsignal.net"],
        &["/"],
        "https://api.crowdsignal.com/oembed",
    ),
    (
        &[".polldaddy.com"],
        &["/"],
        "https://api.crowdsignal.com/oembed",
    ),
    (&["poll.fm"], &["/"], "https://api.crowdsignal.com/oembed"),
    (
        &[".survey.fm"],
        &["/"],
        "https://api.crowdsignal.com/oembed",
    ),
    (
        &["twitter.com", "www.twitter.com"],
        &["/i/moments/"],
        "https://publish.twitter.com/oembed",
    ),
    (
        &["twitter.com", "www.twitter.com"],
        &["/"],
        "https://publish.twitter.com/oembed",
    ),
    (
        &["soundcloud.com", "www.soundcloud.com"],
        &["/"],
        "https://soundcloud.com/oembed",
    ),
    (
        &[".slideshare.net"],
        &["/"],
        "https://www.slideshare.net/api/oembed/2",
    ),
    (
        &["open.spotify.com", "play.spotify.com"],
        &["/"],
        "https://embed.spotify.com/oembed/",
    ),
    (&[".imgur.com"], &["/"], "https://api.imgur.com/oembed"),
    (
        &["meetup.com", "www.meetup.com", "meetu.ps", "www.meetu.ps"],
        &["/"],
        "https://api.meetup.com/oembed",
    ),
    (
        &["issuu.com", "www.issuu.com"],
        &["/"],
        "https://issuu.com/oembed_wp",
    ),
    (
        &["mixcloud.com", "www.mixcloud.com"],
        &["/"],
        "https://www.mixcloud.com/oembed",
    ),
    (
        &["ted.com", "www.ted.com", "embed.ted.com"],
        &["/talks/"],
        "https://www.ted.com/services/v1/oembed.json",
    ),
    (
        &[
            "animoto.com",
            "www.animoto.com",
            "video214.com",
            "www.video214.com",
        ],
        &["/play/"],
        "https://animoto.com/oembeds/create",
    ),
    (
        &[".tumblr.com"],
        &["/post/"],
        "https://www.tumblr.com/oembed/1.0",
    ),
    (&["cloudup.com"], &["/"], "https://cloudup.com/oembed"),
    (
        &["reverbnation.com", "www.reverbnation.com"],
        &["/"],
        "https://www.reverbnation.com/oembed",
    ),
    (
        &["reddit.com", "www.reddit.com"],
        &["/r/"],
        "https://www.reddit.com/oembed",
    ),
    (
        &["speakerdeck.com", "www.speakerdeck.com"],
        &["/"],
        "https://speakerdeck.com/oembed.json",
    ),
    (
        &["screencast.com", "www.screencast.com"],
        &["/"],
        "https://api.screencast.com/external/oembed",
    ),
    (
        &["tiktok.com", "www.tiktok.com"],
        &["/@"],
        "https://www.tiktok.com/oembed",
    ),
    (
        &[".pinterest.com"],
        &["/"],
        "https://www.pinterest.com/oembed.json",
    ),
    (
        &["wolframcloud.com", "www.wolframcloud.com"],
        &["/obj/"],
        "https://www.wolframcloud.com/oembed",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeHttp;

    /// The YouTube rule allows the watch, playlist and shorts paths only
    /// (`application/config/module.config.php:908`).
    #[test]
    fn allow_list_matches_upstream_whitelist() {
        let list = OembedAllowList::shipped();
        assert_eq!(
            list.endpoint_for("https://www.youtube.com/watch?v=x"),
            Some(Some("https://www.youtube.com/oembed"))
        );
        assert_eq!(
            list.endpoint_for("https://youtu.be/x"),
            Some(Some("https://www.youtube.com/oembed"))
        );
        assert_eq!(list.endpoint_for("https://www.youtube.com/channel/x"), None);
        assert_eq!(list.endpoint_for("https://example.org/x"), None);
        assert_eq!(
            list.endpoint_for("https://player.vimeo.com/video/1"),
            Some(Some("https://vimeo.com/api/oembed.json"))
        );
    }

    /// `getOembed` builds `<endpoint>?format=json&url=<encoded source>` and
    /// decodes the response (`Stdlib/Oembed.php:64,92`).
    #[test]
    fn get_oembed_fetches_the_configured_endpoint() {
        let http = FakeHttp::new().with_json(
            "https://www.youtube.com/oembed?format=json&url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3Dx",
            serde_json::json!({"type": "video", "title": "T"}),
        );
        let oembed = Oembed::new(OembedAllowList::shipped());
        let mut errors = ErrorStore::default();
        let data = oembed
            .get_oembed(
                &http,
                "https://www.youtube.com/watch?v=x",
                &mut errors,
                "o:source",
            )
            .expect("response");
        assert_eq!(data.title().as_deref(), Some("T"));
        assert!(errors.is_empty());
    }

    /// A URL outside the allowlist is an error under the caller's key
    /// (`Stdlib/Oembed.php:57`).
    #[test]
    fn get_oembed_rejects_a_url_outside_the_allow_list() {
        let http = FakeHttp::new();
        let oembed = Oembed::new(OembedAllowList::shipped());
        let mut errors = ErrorStore::default();
        assert!(
            oembed
                .get_oembed(&http, "https://example.org/x", &mut errors, "o:source")
                .is_none()
        );
        assert_eq!(
            errors.messages_for("o:source"),
            vec!["oEmbed: URL is not allowed https://example.org/x"]
        );
    }

    /// A provider's `html` passes the purifier with its frame kept and a
    /// sandbox set, and its `photo` markup takes the title or the given alt.
    #[test]
    fn render_oembed_sanitizes_html_and_builds_a_photo() {
        let raw =
            serde_json::json!({"html": "<blockquote>x</blockquote><script>alert(1)</script>"});
        let data = OembedData {
            map: raw.as_object().expect("object").clone(),
        };
        assert_eq!(
            render_oembed(&data, "", None, &[]).as_deref(),
            Some("<blockquote>x</blockquote>")
        );

        let raw = serde_json::json!({"type": "photo", "url": "https://x/y.jpg", "width": 640, "height": 480, "title": "A cat"});
        let data = OembedData {
            map: raw.as_object().expect("object").clone(),
        };
        assert_eq!(
            render_oembed(&data, "", Some("unused"), &[]).as_deref(),
            Some("<img src=\"https://x/y.jpg\" width=\"640\" height=\"480\" alt=\"A cat\">")
        );

        let raw = serde_json::json!({"type": "photo", "url": "https://x/y.jpg", "width": 640, "height": 480});
        let data = OembedData {
            map: raw.as_object().expect("object").clone(),
        };
        assert_eq!(
            render_oembed(&data, "", Some("A photo of a cat"), &[]).as_deref(),
            Some(
                "<img src=\"https://x/y.jpg\" width=\"640\" height=\"480\" alt=\"A photo of a cat\">"
            )
        );
        assert_eq!(
            render_oembed(&data, "", None, &[]).as_deref(),
            Some("<img src=\"https://x/y.jpg\" width=\"640\" height=\"480\" alt=\"\">")
        );

        let raw = serde_json::json!({"type": "link"});
        let data = OembedData {
            map: raw.as_object().expect("object").clone(),
        };
        assert_eq!(render_oembed(&data, "", None, &[]), None);
    }

    /// A provider frame from an allow-listed host survives the sanitizer, and
    /// one from another host does not.
    #[test]
    fn render_oembed_keeps_only_provider_frames() {
        let raw = serde_json::json!({
            "html": "<iframe src=\"https://player.vimeo.com/video/1\"></iframe><iframe src=\"https://evil.example/1\"></iframe>",
        });
        let data = OembedData {
            map: raw.as_object().expect("object").clone(),
        };
        let html = render_oembed(
            &data,
            "https://vimeo.com/1",
            None,
            &OembedAllowList::shipped().hosts(),
        )
        .expect("markup");
        assert!(html.contains("player.vimeo.com"), "{html}");
        assert!(html.contains("sandbox="), "{html}");
        assert!(!html.contains("evil.example"), "{html}");
    }

    /// The discovery link is the one with an oEmbed rel and type
    /// (`Stdlib/Oembed.php:82`).
    #[test]
    fn finds_the_oembed_discovery_link() {
        let html = r#"<html><head>
            <link rel="alternate" type="application/json+oembed" href="https://example.org/oembed?url=x">
            </head></html>"#;
        assert_eq!(
            find_oembed_link(html).as_deref(),
            Some("https://example.org/oembed?url=x")
        );
        assert_eq!(find_oembed_link("<html></html>"), None);
    }
}
