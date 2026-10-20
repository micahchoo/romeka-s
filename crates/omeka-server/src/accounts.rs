//! The account routes: sign in, sign out, the current user, the password flows
//! and the admin's password link.
//!
//! Every decision lives in `omeka-api`; a handler here parses a JSON body,
//! calls one function, and copies the answer onto the wire with the cookie
//! header the flow needs. The route table is:
//!
//! | route | body | answer |
//! | --- | --- | --- |
//! | `POST /api/login` | `{email, password}` | 200, the session user document, `Set-Cookie` |
//! | `POST /api/logout` | | 204, the cookie cleared and the row deleted |
//! | `POST /logout?redirect=<path>` | | 303 back to `redirect`, the cookie cleared and the row deleted |
//! | `GET /api/me` | | the session user document, or 401 |
//! | `POST /api/password` | `{key, password}` | 200, the user document |
//! | `POST /api/password/forgot` | `{email}` | 204 |
//! | `POST /api/users/{id}/password-link` | | 200, `{key, link, sent}` |
//!
//! The session user document is the user document plus `o:admin_locale`, the
//! locale `MvcListeners::bootstrapLocale` would render the admin in
//! ([`omeka_api::settings::admin_locale`]).
//!
//! **Bodies.** A body that is not a JSON object is a 400 with the API error
//! document. A member that is absent or not a string reads as empty, which the
//! flow then treats as "no credentials", "no key" or "no password" and answers
//! 401, 404 or 422. Upstream's form validation would answer 400 for a missing
//! field; the port keeps one failure per flow instead, which is what the SPA
//! has to render.
//!
//! **CSRF.** [`same_origin_write`] is the middleware in front of every route:
//! a state-changing request that carries the session cookie must come from the
//! configured base URL's origin.

use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::time::Instant;

use axum::Router;
use axum::body::Body;
use axum::extract::connect_info::ConnectInfo;
use axum::extract::{FromRequestParts, Path, Request, State};
use axum::http::header::{COOKIE, LOCATION, ORIGIN, REFERER, RETRY_AFTER, SET_COOKIE};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::{get, post};
use omeka_api::jsonld::BaseUrl;
use omeka_api::login::{self, AuthError, SetPasswordError};
use omeka_api::messages;
use omeka_api::origin;
use omeka_api::password::PasswordRules;
use omeka_api::read::{session_user_response, user_response};
use omeka_api::session;
use omeka_db::repos::SettingRepo;
use omeka_site::blocks::browse_preview::parse_query;
use serde::{Deserialize, Serialize};

use crate::AppState;

/// The `/api` auth routes, mounted into [`crate::build_app`].
pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/logout", post(logout_form))
        .route("/api/me", get(current_user))
        .route("/api/password", post(set_password))
        .route("/api/password/forgot", post(forgot_password))
        .route("/api/users/{id}/password-link", post(password_link))
}

/// The socket peer of one request, when the server recorded one.
///
/// `Router::into_make_service_with_connect_info` installs
/// `ConnectInfo<SocketAddr>`; a test that drives the router through `oneshot`
/// installs nothing, and `None` is a usable answer, so this extractor never
/// rejects.
pub(crate) struct Peer(pub Option<SocketAddr>);

impl<S: Send + Sync> FromRequestParts<S> for Peer {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .extensions
                .get::<ConnectInfo<SocketAddr>>()
                .map(|info| info.0),
        ))
    }
}

/// `LoginController::loginAction`.
async fn login(
    State(state): State<AppState>,
    Peer(peer): Peer,
    headers: HeaderMap,
    body: crate::limits::Payload,
) -> Response {
    let Some(credentials) = from_json::<Credentials>(&body.0) else {
        return bad_body();
    };
    let session = login::login(
        &state.pool,
        &state.limiter,
        session_token(&headers),
        &credentials.email,
        &credentials.password,
        client_address(&headers, peer),
        Instant::now(),
    )
    .await;
    match session {
        Ok(session) => {
            let mut response =
                crate::finish(session_user_response(&state.base, &state.pool, &session.user).await);
            set_cookie(
                &mut response,
                &session::set_cookie(&session.token, state.secure_cookies),
            );
            response
        }
        Err(error) => auth_error(error),
    }
}

/// `LoginController::logoutAction`.
async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let token = session_token(&headers);
    if let Err(error) = login::logout(&state.pool, token).await {
        return crate::error_response(error);
    }
    let mut response = no_content();
    set_cookie(&mut response, &session::clear_cookie(state.secure_cookies));
    response
}

/// The public site's Logout form (`LoginController::logoutAction`): end the
/// session, then send the browser back to the page it came from.
///
/// Upstream's logout is a `GET` that lands on the login page. The port takes a
/// `POST`, so the same-origin check in front of every write guards it and a
/// link on another site cannot sign a reader out. `redirect` is honoured only
/// as a path on this origin ([`return_path`]); anything else returns to `/`.
async fn logout_form(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    if let Err(error) = login::logout(&state.pool, session_token(&headers)).await {
        return crate::error_response(error);
    }
    let query = parse_query(uri.query().unwrap_or_default());
    let target = query
        .iter()
        .find(|(key, _)| key == "redirect")
        .and_then(|(_, value)| return_path(value))
        .unwrap_or("/");
    let mut response = Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header(LOCATION, target)
        .body(Body::empty())
        .unwrap_or_else(|_| crate::empty_internal_error());
    set_cookie(&mut response, &session::clear_cookie(state.secure_cookies));
    response
}

/// A redirect target that stays on this origin: an absolute path, never a
/// scheme-relative `//host`, and no backslash, which some browsers read as a
/// slash. Only printable ASCII passes, so the value is a valid header; a path
/// the site printed is percent-encoded already.
fn return_path(raw: &str) -> Option<&str> {
    let rest = raw.strip_prefix('/')?;
    let printable = raw
        .bytes()
        .all(|byte| byte.is_ascii_graphic() && byte != b'\\');
    (printable && !rest.starts_with('/')).then_some(raw)
}

/// The identity of the caller, or 401.
async fn current_user(State(state): State<AppState>, headers: HeaderMap) -> Response {
    match login::current_user(&state.pool, session_token(&headers)).await {
        Ok(Some(user)) => {
            crate::finish(session_user_response(&state.base, &state.pool, &user).await)
        }
        Ok(None) => crate::document_response(
            StatusCode::UNAUTHORIZED,
            &omeka_api::errors::ErrorDocument::general(messages::AUTHENTICATION_REQUIRED),
        ),
        Err(error) => crate::error_response(error),
    }
}

/// `LoginController::createPasswordAction`.
async fn set_password(State(state): State<AppState>, body: crate::limits::Payload) -> Response {
    let Some(request) = from_json::<SetPasswordBody>(&body.0) else {
        return bad_body();
    };
    match login::set_password(
        &state.pool,
        &PasswordRules::default(),
        &request.key,
        &request.password,
    )
    .await
    {
        Ok(user) => crate::finish(user_response(&state.base, &user)),
        Err(error) => set_password_error(error),
    }
}

/// `LoginController::forgotPasswordAction`: 204 whether or not the address
/// matched, and whether or not a mail went out.
async fn forgot_password(
    State(state): State<AppState>,
    Peer(peer): Peer,
    headers: HeaderMap,
    body: crate::limits::Payload,
) -> Response {
    let Some(request) = from_json::<ForgotBody>(&body.0) else {
        return bad_body();
    };
    let issued = login::forgot_password(
        &state.pool,
        &state.limiter,
        &request.email,
        client_address(&headers, peer),
        Instant::now(),
    )
    .await;
    match issued {
        Ok(Some(issued)) => {
            let title = installation_title(&state.pool).await;
            if let Err(error) = state
                .mailer
                .send_reset_password(&issued.user, &issued.key, &title, &issued.expires_display())
                .await
            {
                // Upstream logs the mail failure and still tells the caller to
                // check their email (`forgotPasswordAction:174`).
                tracing::warn!(target: "romeka::mail", %error, "the reset mail did not go out");
            }
            no_content()
        }
        Ok(None) => no_content(),
        Err(error) => auth_error(error),
    }
}

/// The admin route that mints a password link for one user.
async fn password_link(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: axum::http::Uri,
    headers: HeaderMap,
) -> Response {
    let actor = match crate::actor_for(&state, &uri, &headers).await {
        Ok(actor) => actor,
        Err(error) => return crate::error_response(error),
    };
    match login::password_link(&state.pool, &actor, &id).await {
        Ok(issued) => {
            let link = state.mailer.link(&issued.key);
            let title = installation_title(&state.pool).await;
            let sent = state
                .mailer
                .send_user_activation(
                    &issued.user,
                    &issued.key,
                    &title,
                    &issued.expires_display(),
                )
                .await
                .map(|delivery| delivery == crate::mail::Delivery::Sent)
                .unwrap_or_else(|error| {
                    tracing::warn!(target: "romeka::mail", %error, "the activation mail did not go out");
                    false
                });
            json(
                StatusCode::OK,
                &PasswordLinkBody {
                    key: issued.key,
                    link,
                    sent,
                },
            )
        }
        Err(error) => crate::error_response(error),
    }
}

/// A cross-origin write that carries the session cookie is refused.
///
/// The check runs on every state-changing method, whatever the route, because a
/// cookie the browser attaches is what makes the request forgeable. A request
/// with no session cookie is left alone: it authenticates with a key pair in the
/// query string, which a hostile page cannot read.
pub(crate) async fn same_origin_write(
    State(base): State<BaseUrl>,
    request: Request,
    next: Next,
) -> Response {
    let state_changing = !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    );
    let carries_cookie = session_token(request.headers()).is_some();
    if state_changing && carries_cookie && !origin::is_same_origin(request.headers(), &base) {
        tracing::warn!(
            target: "romeka::auth",
            origin = request.headers().get(ORIGIN).and_then(|value| value.to_str().ok()),
            referer = request.headers().get(REFERER).and_then(|value| value.to_str().ok()),
            "a cookie-authenticated write came from another origin"
        );
        return crate::document_response(
            StatusCode::FORBIDDEN,
            &omeka_api::errors::ErrorDocument::general(messages::CROSS_ORIGIN_REFUSED),
        );
    }
    next.run(request).await
}

/// The body of `POST /api/login`.
#[derive(Debug, Default, Deserialize)]
struct Credentials {
    #[serde(default)]
    email: String,
    #[serde(default)]
    password: String,
}

/// The body of `POST /api/password`.
#[derive(Debug, Default, Deserialize)]
struct SetPasswordBody {
    #[serde(default)]
    key: String,
    #[serde(default)]
    password: String,
}

/// The body of `POST /api/password/forgot`.
#[derive(Debug, Default, Deserialize)]
struct ForgotBody {
    #[serde(default)]
    email: String,
}

/// The answer of `POST /api/users/{id}/password-link`.
#[derive(Debug, Serialize)]
struct PasswordLinkBody {
    key: String,
    link: String,
    /// Whether the message reached a relay. The log transport reports false,
    /// because the link is the part the caller can act on.
    sent: bool,
}

/// One request body as a route's payload type, or `None` when it is not a JSON
/// object.
///
/// The body must parse as a JSON object; an array, a string or anything that is
/// not an object is refused, so a caller cannot reach a flow with a body the
/// route does not describe. An absent member keeps its `#[serde(default)]`.
fn from_json<T: serde::de::DeserializeOwned + Default>(body: &[u8]) -> Option<T> {
    if body.is_empty() {
        return Some(T::default());
    }
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    if !value.is_object() {
        return None;
    }
    serde_json::from_value(value).ok()
}

/// The session cookie in the request, if it carries one.
fn session_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(session::cookie_token)
}

/// The caller's address, as the rate limit counts it.
///
/// `X-Real-IP` first, then the last entry of `X-Forwarded-For` (the address the
/// proxy itself saw; a client can only add entries to the front), then the
/// socket peer. Nothing at all leaves the address counter unused rather than
/// shared by every caller, which would turn the per-address limit into a global
/// one.
fn client_address(headers: &HeaderMap, peer: Option<SocketAddr>) -> Option<IpAddr> {
    let header = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());
    if let Some(address) = header("x-real-ip").and_then(|value| IpAddr::from_str(value.trim()).ok())
    {
        return Some(address);
    }
    if let Some(address) = header("x-forwarded-for")
        .and_then(|value| value.rsplit(',').next())
        .and_then(|value| IpAddr::from_str(value.trim()).ok())
    {
        return Some(address);
    }
    peer.map(|address| address.ip())
}

/// The installation title, `Mailer::getInstallationTitle`.
async fn installation_title(pool: &sqlx::SqlitePool) -> String {
    SettingRepo::new(pool)
        .get("installation_title")
        .await
        .ok()
        .flatten()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "Romeka S".to_string())
}

fn bad_body() -> Response {
    crate::document_response(
        StatusCode::BAD_REQUEST,
        &omeka_api::errors::ErrorDocument::general(messages::BODY_NOT_OBJECT),
    )
}

fn no_content() -> Response {
    Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(Body::empty())
        .unwrap_or_else(|_| crate::empty_internal_error())
}

fn json<T: Serialize>(status: StatusCode, body: &T) -> Response {
    match serde_json::to_string(body) {
        Ok(text) => Response::builder()
            .status(status)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from(text))
            .unwrap_or_else(|_| crate::empty_internal_error()),
        Err(_) => crate::empty_internal_error(),
    }
}

/// Add a `Set-Cookie` value to a response.
fn set_cookie(response: &mut Response, value: &str) {
    match HeaderValue::from_str(value) {
        Ok(value) => {
            response.headers_mut().insert(SET_COOKIE, value);
        }
        Err(_) => {
            // Every attribute in the value is a literal, and the token is
            // alphanumeric, so a rejected value means the token is not one this
            // module minted.
            tracing::error!(target: "romeka::auth", "the session cookie value is not a header value");
        }
    }
}

fn auth_error(error: AuthError) -> Response {
    if matches!(error, AuthError::Internal { .. }) {
        tracing::error!(target: "romeka::auth", %error, "the login route failed");
    }
    let status = error.status();
    let mut response = crate::document_response(status, &error.document());
    if let Some(seconds) = error.retry_after_secs()
        && let Ok(value) = HeaderValue::from_str(&seconds.to_string())
    {
        response.headers_mut().insert(RETRY_AFTER, value);
    }
    response
}

fn set_password_error(error: SetPasswordError) -> Response {
    if matches!(error, SetPasswordError::Internal { .. }) {
        tracing::error!(target: "romeka::auth", %error, "the set-password route failed");
    }
    crate::document_response(error.status(), &error.document())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use axum::http::HeaderValue;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.insert(
                axum::http::HeaderName::from_str(name).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        headers
    }

    fn peer() -> Option<SocketAddr> {
        Some(SocketAddr::from(([127, 0, 0, 1], 4242)))
    }

    #[test]
    fn the_socket_address_is_the_fallback() {
        assert_eq!(
            client_address(&headers(&[]), peer()),
            Some(IpAddr::from([127, 0, 0, 1]))
        );
    }

    #[test]
    fn a_forwarded_header_wins_over_the_socket() {
        assert_eq!(
            client_address(&headers(&[("x-real-ip", "203.0.113.7")]), peer()),
            Some(IpAddr::from([203, 0, 113, 7]))
        );
        assert_eq!(
            client_address(
                &headers(&[("x-forwarded-for", "198.51.100.9, 203.0.113.7")]),
                None
            ),
            Some(IpAddr::from([203, 0, 113, 7])),
            "the last entry is the address the proxy saw"
        );
    }

    #[test]
    fn a_spoofed_or_absent_address_leaves_the_counter_unused() {
        assert_eq!(
            client_address(&headers(&[("x-real-ip", "not an address")]), peer()),
            Some(IpAddr::from([127, 0, 0, 1])),
            "a value that is not an address falls through to the socket"
        );
        assert_eq!(client_address(&headers(&[]), None), None);
    }

    #[test]
    fn a_body_that_is_not_a_json_object_is_none_and_an_empty_body_is_default() {
        assert!(from_json::<Credentials>(b"[]").is_none());
        assert!(from_json::<Credentials>(b"not json").is_none());
        assert_eq!(from_json::<Credentials>(b"").unwrap().email, "");
        assert_eq!(
            from_json::<Credentials>(br#"{"email":"a@b.example"}"#)
                .unwrap()
                .email,
            "a@b.example"
        );
    }
}
