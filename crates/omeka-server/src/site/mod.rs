//! The public site routes (`/s/:site-slug/...`).
//!
//! Upstream serves these through the `Omeka\Controller\Site\*` controllers,
//! which read rows through the API manager and render HTML through the current
//! theme (`application/config/routes.config.php:16-116`,
//! `src/Mvc/MvcListeners.php::prepareSite` ~line 398). The port keeps that split:
//! handlers read rows through `omeka-db`, hand values to `omeka-site`, and put
//! the rendered document on the wire.
//!
//! `prepareSite` reads the site by slug and turns a failed read into a dispatch
//! error, so an unknown slug is a 404 before any controller runs. The port does
//! the same: [`dispatch`] resolves the slug first and every failure becomes a
//! [`SiteError`].
//!
//! [`handle`] also serves the admin editor's live preview
//! (`POST /s/:site-slug/preview`, [`preview`]), which is a Romeka addition: it
//! checks a page write's body, renders it in memory and writes nothing.

mod dispatch;
mod load;
mod pages;
mod preview;
mod provider;
mod settings;

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header};
use axum::response::Response;
use axum::routing::any;

use omeka_api::auth::Actor;
use omeka_core::ids::SiteId;
use omeka_core::site::{Site, Slug};
use omeka_site::slug::{PublicRoute, parse_public_path};
use omeka_site::theme::{DefaultTheme, Theme};
use omeka_site::view::{ErrorView, NoResources, RenderContext, View};

use crate::AppState;

/// The `/s` routes, mounted into [`crate::build_router`].
///
/// The `POST` preview is dispatched from [`handle`] rather than registered as a
/// second route, because a router cannot hold a named segment beside the
/// catch-all this path needs for every other public route.
pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/s", any(handle))
        .route("/s/{*rest}", any(handle))
}

/// What a served public route answers with.
pub(crate) enum Outcome {
    /// A full HTML document.
    Html(String),
    /// A 302 to the given location.
    Redirect(String),
}

/// A public route that could not be served.
#[derive(Debug, thiserror::Error)]
pub(crate) enum SiteError {
    /// No site, page or resource at this path, or one the caller cannot see
    /// (`view/error/404.phtml`). Upstream answers a hidden site, page or
    /// resource with the same 404, because the visibility filters remove it
    /// from the read.
    #[error("not found")]
    NotFound,
    /// A storage failure, answered with the generic 500 document.
    #[error(transparent)]
    Internal(#[from] omeka_db::DbError),
}

async fn handle(
    State(state): State<AppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: crate::limits::Payload,
) -> Response {
    if method == Method::POST
        && let Some(site) = preview::path_site(uri.path())
    {
        return preview::run(&state, &site, &uri, &headers, &body.0).await;
    }
    if method != Method::GET {
        return method_not_allowed();
    }
    let actor = match crate::session_actor(&state, &headers).await {
        Ok(Some(actor)) => actor,
        Ok(None) => Actor::anonymous(),
        // A storage failure while reading the session is the site layer's
        // generic 500, not the API's JSON error document.
        Err(_) => return crate::empty_internal_error(),
    };
    match dispatch::dispatch(&state, &uri, &actor).await {
        Ok(outcome) => outcome_response(outcome),
        Err(error) => error_response(&uri, error),
    }
}

fn outcome_response(outcome: Outcome) -> Response {
    match outcome {
        Outcome::Html(html) => html_response(StatusCode::OK, html),
        Outcome::Redirect(location) => match HeaderValue::from_str(&location) {
            Ok(value) => Response::builder()
                .status(StatusCode::FOUND)
                .header(header::LOCATION, value)
                .body(Body::empty())
                .unwrap_or_else(|_| crate::empty_internal_error()),
            Err(_) => crate::empty_internal_error(),
        },
    }
}

/// An HTML response, always `text/html; charset=utf-8`.
fn html_response(status: StatusCode, html: String) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(html))
        .unwrap_or_else(|_| crate::empty_internal_error())
}

/// A 405 with `Allow: GET`; only `GET` reaches the site routes.
fn method_not_allowed() -> Response {
    Response::builder()
        .status(StatusCode::METHOD_NOT_ALLOWED)
        .header(header::ALLOW, "GET")
        .body(Body::empty())
        .unwrap_or_else(|_| crate::empty_internal_error())
}

fn error_response(uri: &Uri, error: SiteError) -> Response {
    match error {
        SiteError::NotFound => html_response(
            StatusCode::NOT_FOUND,
            render_error(uri.path(), &ErrorView::not_found()),
        ),
        SiteError::Internal(_) => crate::empty_internal_error(),
    }
}

/// The error page inside the site layout. `prepareSite` returns no site when the
/// slug does not resolve, so the layout renders over a placeholder whose slug
/// comes from the request path when it parses and `none` otherwise. The error
/// layout reads only the title and message, so the placeholder's other fields
/// never reach the document.
fn render_error(path: &str, error: &ErrorView) -> String {
    let site = placeholder_site(path);
    let ctx = RenderContext::new(&site, &NoResources);
    DefaultTheme.render(&View::Error(error), &ctx)
}

fn placeholder_site(path: &str) -> Site {
    let slug = parse_public_path(path)
        .ok()
        .map(|route| route_slug(&route))
        .unwrap_or_else(none_slug);
    Site::new(SiteId::new(0), slug, "default", "")
}

/// The site slug any parsed public route carries.
pub(crate) fn route_slug(route: &PublicRoute) -> Slug {
    match route {
        PublicRoute::Home { site }
        | PublicRoute::PageBrowse { site }
        | PublicRoute::Page { site, .. }
        | PublicRoute::CrossSiteSearch { site, .. }
        | PublicRoute::ResourceBrowse { site, .. }
        | PublicRoute::ResourceAction { site, .. }
        | PublicRoute::ResourceShow { site, .. }
        | PublicRoute::ResourceShowAction { site, .. } => site.clone(),
    }
}

/// `none` satisfies the slug rule (`Api/Adapter/SiteAdapter.php:260-266`), so
/// this parse cannot fail; the arm is unreachable for that reason.
fn none_slug() -> Slug {
    Slug::parse("none").unwrap_or_else(|_| unreachable!("the literal \"none\" is a valid slug"))
}
