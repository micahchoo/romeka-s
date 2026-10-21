//! `GET /api/romeka/media/{id}/render`: the server's own render of one embed.
//!
//! The admin stage shows an oEmbed or HTML media the way the public site shows
//! it, so the editor sees what a visitor gets, and the admin never decides for
//! itself what provider markup is safe (media-ui-plan S3, gap G2 in
//! `briefs/media-ui/A-admin-media.md`). This route answers that render as one
//! complete document, which the stage puts in a sandboxed iframe.
//!
//! The route is a Romeka addition. It reads the caller the way every `/api`
//! route does, answers 404 for a media the caller may not see, and 415 for a
//! kind that is not an embed.
//!
//! Every answer carries the headers that make the document a frame and nothing
//! more: a CSP over the sources the render itself names, `X-Frame-Options` and
//! `frame-ancestors` for this origin alone, no cookie, and no shared cache.

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::Response;
use axum::routing::get;
use omeka_api::errors::ApiError;
use omeka_api::messages;
use omeka_api::read::catalog::Catalog;
use omeka_db::ids::MediaId;
use omeka_db::repos::{ItemRepo, MediaRepo};
use omeka_db::with_db;
use omeka_media::data::MediaData;
use omeka_media::file_render::FileRendererRegistry;
use omeka_media::render::{MediaView, RenderEnv, RenderOptions, RendererRegistry, View};
use omeka_media::text::escape_html;

use crate::site::load::media_core;
use crate::{AppState, actor_for, error_response};

/// The renderers this route answers: the two whose markup only the server may
/// build.
const EMBED_RENDERERS: [&str; 2] = ["oembed", "html"];

/// The stage's own path prefix for the bundled theme's assets. No renderer this
/// route reaches reads it, and a media view has to carry one.
const ASSET_BASE: &str = "/assets";

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/api/romeka/media/{id}/render", get(render))
}

async fn render(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    // An id that is not a number names no row, which is the 404 every other
    // route answers for one.
    let result = match id.parse::<u64>() {
        Ok(id) => document(&state, MediaId::from(id), &uri, &headers).await,
        Err(_) => Err(ApiError::not_found(messages::NOT_FOUND)),
    };
    match result {
        Ok(frame) => html_answer(frame),
        Err(error) => {
            let mut response = error_response(error);
            frame_headers(response.headers_mut());
            response
        }
    }
}

/// The frame document for one media, or the refusal it earns.
async fn document(
    state: &AppState,
    id: MediaId,
    uri: &Uri,
    headers: &HeaderMap,
) -> Result<Frame, ApiError> {
    let media = with_db!(&state.pool, |db| MediaRepo::new(db).get(id).await)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found(messages::NOT_FOUND))?;
    let actor = actor_for(state, uri, headers).await?;
    // A media is visible only with its item, by the rule the API applies to
    // each (`ResourceVisibilityFilter`). A missing item leaves the media
    // invisible, as an orphan row a caller cannot reach.
    let item = with_db!(&state.pool, |db| ItemRepo::new(db).get(media.item_id).await)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found(messages::NOT_FOUND))?;
    let sees = actor.sees_resource(media.resource.is_public, media.resource.owner_id)
        && actor.sees_resource(item.resource.is_public, item.resource.owner_id);
    if !sees {
        return Err(ApiError::not_found(messages::NOT_FOUND));
    }
    if !EMBED_RENDERERS.contains(&media.renderer.as_str()) {
        return Err(ApiError::UnsupportedMediaType {
            message: format!(
                "This route answers oEmbed and HTML media only; this media's renderer is \"{}\".",
                media.renderer
            ),
        });
    }
    let catalog = Catalog::load(&state.pool).await?;
    let alt_text = omeka_api::settings::media_alt_text(&state.pool, &catalog, &media)
        .await
        .map_err(ApiError::internal)?;
    let core = media_core(&media, alt_text);
    let base = state.base.as_str();
    let view = MediaView::new(
        core,
        MediaData::parse(&media.ingester, media.data.as_ref()).unwrap_or(MediaData::None),
        RenderEnv {
            server_url: base.to_string(),
            files: state.files.clone(),
            asset_base_path: ASSET_BASE.to_string(),
            viewer_url: format!("{base}/iiif-viewer"),
            ..RenderEnv::default()
        },
        format!("{base}/api/items/{}", media.item_id.0),
        format!("{base}/api/media/{}", id.0),
    );
    let renderers = RendererRegistry::with_defaults();
    let file_renderers = FileRendererRegistry::with_defaults();
    let fragment = renderers.render(
        &media.renderer,
        &View {
            file_renderers: &file_renderers,
        },
        &view,
        &RenderOptions::default(),
    );
    let policy = content_security_policy(&fragment);
    let html = document_of(
        &escape_html(&view.display_title),
        media.lang.as_deref(),
        base,
        &fragment,
    );
    Ok(Frame { html, policy })
}

/// A frame answer: the document and the policy its response carries.
struct Frame {
    html: String,
    policy: String,
}

/// A whole HTML document around the rendered fragment.
///
/// The `<base>` is the server's root, where the public site's own fragments
/// resolve their relative URLs, and the CSP's `base-uri 'self'` keeps it the
/// only one. The one rule set is the frame's own base, because the document
/// carries no theme: without it a provider's heading would draw in the
/// browser's serif default inside the admin.
fn document_of(title: &str, lang: Option<&str>, base: &str, body: &str) -> String {
    let lang = lang
        .filter(|lang| !lang.is_empty())
        .map(|lang| format!(" lang=\"{}\"", escape_html(lang)))
        .unwrap_or_default();
    format!(
        "<!doctype html>\n<html{lang}>\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <base href=\"{}/\">\n<title>{title}</title>\n\
         <style>:root{{color-scheme:light dark}}body{{margin:1rem;font-family:system-ui,sans-serif;line-height:1.5}}</style>\n\
         </head>\n<body>\n{body}\n</body>\n</html>\n",
        escape_html(base.trim_end_matches('/')),
    )
}

/// The policy for one render: nothing loads unless the fragment itself names
/// it.
///
/// A script never runs, because the purifier removes every script and event
/// handler and the policy denies the scheme anyway. `img-src` allows this
/// origin for a fragment that points at a stored file, plus the `http` and
/// `https` origins its `img` and `iframe` sources name; `frame-src` allows the
/// frame origins alone. `style-src 'unsafe-inline'` keeps the `style`
/// attributes the purifier allows, which cannot execute and cannot read this
/// page.
fn content_security_policy(fragment: &str) -> String {
    let (images, frames) = loaded_origins(fragment);
    let mut policy =
        String::from("default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; ");
    policy.push_str("base-uri 'self'; form-action 'none'; frame-ancestors 'self'; img-src 'self'");
    for origin in &images {
        policy.push(' ');
        policy.push_str(origin);
    }
    if !frames.is_empty() {
        policy.push_str("; frame-src");
        for origin in &frames {
            policy.push(' ');
            policy.push_str(origin);
        }
    }
    policy
}

/// The origins the fragment's `img` and `iframe` sources name, in the order
/// they appear and without repeats. A source that is not an absolute `http` or
/// `https` URL contributes nothing: the sanitizer already dropped a frame
/// outside the host rule, and a relative source resolves against the document's
/// own base, which is this server.
fn loaded_origins(fragment: &str) -> (Vec<String>, Vec<String>) {
    let mut images = Vec::new();
    let mut frames = Vec::new();
    for element in elements(fragment) {
        let (name, body) = element;
        let Some(src) = attribute(body, "src") else {
            continue;
        };
        let Some(origin) = origin_of(src) else {
            continue;
        };
        match name.as_str() {
            "img" => push_unique(&mut images, &origin),
            "iframe" => push_unique(&mut frames, &origin),
            _ => {}
        }
    }
    (images, frames)
}

/// Every element of a sanitizer's output as its lower-case name and the text
/// between its brackets. The output escapes `<`, `>` and `"` inside attribute
/// values, so a bracket ends an element.
fn elements(fragment: &str) -> Vec<(String, &str)> {
    let mut elements = Vec::new();
    let mut rest = fragment;
    while let Some(start) = rest.find('<') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('>') else {
            break;
        };
        let body = &after[..end];
        let name: String = body
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        if !name.is_empty() {
            elements.push((name, body));
        }
        rest = &after[end + 1..];
    }
    elements
}

/// The value of one quoted attribute in an element's text.
fn attribute<'a>(body: &'a str, name: &str) -> Option<&'a str> {
    let pattern = format!(" {name}=\"");
    let start = body.find(&pattern)? + pattern.len();
    let rest = &body[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

/// `scheme://authority` of an absolute `http` or `https` URL, lower case and
/// with no userinfo, or `None` for anything else. A port is kept, because a
/// source expression without one does not match a URL on another port.
fn origin_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .filter(|authority| !authority.is_empty())?;
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_user, host)| host)
        .to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }
    Some(format!("{scheme}://{host}"))
}

fn push_unique(list: &mut Vec<String>, value: &str) {
    if !list.iter().any(|seen| seen == value) {
        list.push(value.to_string());
    }
}

/// A frame document with its type, its policy and no cookie.
fn html_answer(frame: Frame) -> Response {
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CONTENT_SECURITY_POLICY, &frame.policy)
        .body(Body::from(frame.html))
        .unwrap_or_else(|_| crate::empty_internal_error());
    frame_headers(response.headers_mut());
    response
}

/// The headers every answer carries: no framing outside this origin, and no
/// shared copy, because the visibility of a private media varies by caller.
fn frame_headers(headers: &mut HeaderMap) {
    headers.insert(
        header::X_FRAME_OPTIONS,
        header::HeaderValue::from_static("SAMEORIGIN"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("private, no-cache"),
    );
    headers.insert(header::VARY, header::HeaderValue::from_static("Cookie"));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A source expression names the host and the port a frame or an image
    /// loads from, and refuses a relative source or another scheme.
    #[test]
    fn an_origin_is_the_scheme_and_the_authority() {
        assert_eq!(
            origin_of("https://www.youtube.com/embed/x"),
            Some("https://www.youtube.com".to_string())
        );
        assert_eq!(
            origin_of("HTTPS://CDN.Example.org:8443/a.png"),
            Some("https://cdn.example.org:8443".to_string())
        );
        assert_eq!(
            origin_of("https://user@example.org/a.png?x=1"),
            Some("https://example.org".to_string())
        );
        assert_eq!(origin_of("/files/large/7.jpg"), None);
        assert_eq!(origin_of("data:image/png;base64,AAAA"), None);
        assert_eq!(origin_of("javascript:alert(1)"), None);
    }

    /// The policy names the origins the fragment loads and nothing wider, and
    /// gives an image no source at all until the fragment names one.
    #[test]
    fn the_policy_names_the_sources_the_fragment_names() {
        let empty = content_security_policy("<p>watch</p>");
        assert!(
            empty.starts_with("default-src 'none'; script-src 'none';"),
            "{empty}"
        );
        assert!(empty.contains("img-src 'self'"), "{empty}");
        assert!(!empty.contains("frame-src"), "{empty}");
        assert!(!empty.contains('*'), "{empty}");

        let policy = content_security_policy(
            "<p>watch</p><img src=\"https://i.example.org/a.png\"><iframe src=\"https://www.youtube.com/embed/x\"></iframe>",
        );
        assert!(
            policy.contains("img-src 'self' https://i.example.org"),
            "{policy}"
        );
        assert!(
            policy.contains("frame-src https://www.youtube.com"),
            "{policy}"
        );
        assert!(policy.contains("frame-ancestors 'self'"), "{policy}");
    }
}
