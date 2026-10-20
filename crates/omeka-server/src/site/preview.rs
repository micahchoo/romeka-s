//! The live preview route (`POST /s/:site-slug/preview`).
//!
//! A Romeka addition with no upstream counterpart. The admin site-page editor
//! renders a page before it saves, so this route checks what the `site_pages`
//! write checks, builds the page in memory and answers the document
//! `GET /s/:site-slug/page/:page-slug` answers with that content. It writes
//! nothing: [`omeka_api::write::site_pages_preview::preview`] resolves the body
//! and [`super::dispatch::render_preview`] resolves the frame, and both read
//! only.
//!
//! The public site router owns one catch-all path, which a second route cannot
//! share, so [`super::handle`] checks the method and path here before it
//! dispatches a public route ([`path_site`]).
//!
//! Credentials are the admin SPA's: the session cookie, the API key pair in the
//! query string, or neither, which the access check refuses with 403. A
//! cookie-bearing POST is a state-changing request, so the same-origin guard
//! ([`crate::accounts::same_origin_write`]) already covers it, exactly as it
//! covers a cookie-authenticated `/api` write.
//!
//! The answer carries `Cache-Control: no-store`, `X-Robots-Tag: noindex` and a
//! script-free CSP, because the editor renders it in a sandboxed iframe and
//! nothing here should be cached or indexed.

use axum::body::Body;
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::Response;

use omeka_api::errors::ErrorDocument;
use omeka_api::write::body::Payload;
use omeka_api::write::site_pages::ResolvedAttachment;
use omeka_api::write::site_pages_preview::{self, Preview};
use omeka_core::ids::{BlockId, PageId, SiteId};
use omeka_core::resource::Timestamp;
use omeka_core::site::{
    BlockLayout, BlockLayoutData, PageLayout, PageLayoutData, SiteBlock, SitePage, Slug,
};
use omeka_db::repos::SiteRepo;
use omeka_site::blocks::parse::parse_block_data;
use omeka_site::theme::tokens::ThemeSettings;

use super::{Outcome, SiteError, dispatch};
use crate::AppState;

/// The CSP the editor's sandboxed iframe reads: no script may run, and no other
/// origin may frame the document.
const PREVIEW_CSP: &str = "default-src 'self'; script-src 'none'; frame-ancestors 'self'";

/// The slug a draft with neither a slug nor a title renders under. It is
/// never stored: the save refuses an empty slug.
const UNTITLED_SLUG: &str = "untitled";

/// The site slug of a preview path, `None` for any other public path.
pub(crate) fn path_site(path: &str) -> Option<String> {
    let (site, tail) = path.strip_prefix("/s/")?.split_once('/')?;
    (tail == "preview").then(|| site.to_string())
}

/// `POST /s/:site-slug/preview`.
pub(crate) async fn run(
    state: &AppState,
    site: &str,
    uri: &Uri,
    headers: &HeaderMap,
    body: &[u8],
) -> Response {
    if !crate::content_type_is_json(headers) {
        return crate::error_response(omeka_api::write::body::content_type_error(
            crate::media_type(headers),
        ));
    }
    let Ok(slug) = Slug::parse(site) else {
        return not_found();
    };
    let site_row = match SiteRepo::new(&state.pool).get_by_slug(slug.as_str()).await {
        Ok(Some(row)) => row,
        Ok(None) => return not_found(),
        Err(_) => return crate::empty_internal_error(),
    };
    let api = match crate::open(state, uri, headers).await {
        Ok(api) => api,
        Err(error) => return crate::error_response(error),
    };
    let payload = match Payload::parse(body) {
        Ok(payload) => payload,
        Err(error) => return crate::error_response(error),
    };
    let preview = match site_pages_preview::preview(&api, &payload, site_row.id).await {
        Ok(preview) => preview,
        Err(error) => return crate::error_response(error),
    };
    // Draft custom CSS the sanitiser refuses is answered the way the site
    // write answers it, so the editor can mark the lines while the curator
    // types; the frame would drop it anyway.
    if let Some(messages) = preview
        .theme_settings
        .as_ref()
        .and_then(omeka_api::theme_settings::custom_css_errors)
    {
        let mut errors = omeka_api::errors::ValidationErrors::new();
        errors.insert("o:theme_settings".to_string(), messages);
        return crate::error_response(omeka_api::errors::ApiError::Validation(errors));
    }
    let actor = api.actor().clone();
    let Some(page) = build_page(&preview) else {
        return crate::empty_internal_error();
    };
    // Each block's render id is its one-based position, the key `build_page`
    // gives the blocks it parses and the key the provider looks attachments up
    // by.
    let attachments: Vec<(BlockId, Vec<ResolvedAttachment>)> = preview
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (BlockId::new(index as u64 + 1), block.attachments.clone()))
        .collect();
    let created = preview.created.map(Timestamp::from_naive);
    let modified = preview.modified.map(Timestamp::from_naive);
    // `o:theme_settings` in the body overrides the stored tokens for this
    // render only. It is parsed the same way the stored map is and is never
    // written.
    let theme_override = preview
        .theme_settings
        .as_ref()
        .map(ThemeSettings::from_value);
    match dispatch::render_preview(
        state,
        uri,
        &actor,
        &dispatch::PreviewPage {
            page: &page,
            attachments: &attachments,
            created,
            modified,
            theme_settings: theme_override.as_ref(),
        },
    )
    .await
    {
        Ok(Outcome::Html(html)) => html_response(html),
        // The frame renders a page; a redirect here would be a bug, not a page.
        Ok(Outcome::Redirect(_)) => crate::empty_internal_error(),
        Ok(Outcome::NotFound(_)) | Err(SiteError::NotFound) => not_found(),
        Err(SiteError::Internal(_)) => crate::empty_internal_error(),
    }
}

/// The page a [`Preview`] describes, in the shape `omeka-site` renders.
///
/// A block whose `o:data` does not parse for its layout is skipped, the way
/// `block_from_row` skips one at render time, so the rest of the page still
/// shows. Each surviving block keeps its one-based position as its id.
fn build_page(preview: &Preview) -> Option<SitePage> {
    let slug = if preview.slug.is_empty() {
        Slug::parse(UNTITLED_SLUG).ok()?
    } else {
        Slug::parse(&preview.slug).ok()?
    };
    let mut page = SitePage::new(
        PageId::new(preview.id.map_or(0, |id| id.0)),
        SiteId::new(preview.site.0),
        slug,
        preview.title.clone(),
    );
    page.is_public = preview.is_public;
    page.layout = PageLayout::from_stored(preview.layout.as_deref());
    page.layout_data = PageLayoutData::parse(&preview.layout_data.clone().unwrap_or_default());
    page.blocks = preview
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(index, block)| {
            let layout = BlockLayout::from_wire_name(&block.layout);
            let data = parse_block_data(&layout, &block.data).ok()?;
            Some(SiteBlock {
                id: BlockId::new(index as u64 + 1),
                layout,
                position: index as i64 + 1,
                data,
                layout_data: BlockLayoutData::parse(&block.layout_data.clone().unwrap_or_default()),
            })
        })
        .collect();
    Some(page)
}

/// The preview document, stamped so nothing caches, indexes or scripts it.
fn html_response(html: String) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-robots-tag", "noindex")
        .header("content-security-policy", PREVIEW_CSP)
        .body(Body::from(html))
        .unwrap_or_else(|_| crate::empty_internal_error())
}

/// An unknown site, answered with the API's 404 document.
fn not_found() -> Response {
    crate::document_response(
        StatusCode::NOT_FOUND,
        &ErrorDocument::general(omeka_api::messages::NOT_FOUND),
    )
}
