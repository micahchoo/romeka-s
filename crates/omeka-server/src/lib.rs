//! The HTTP surface: an axum app around the rules in `omeka-api`.
//!
//! This crate owns no behavior. Which method reaches which adapter operation,
//! which status a failure carries, and the error document shape all live in
//! `omeka-api`; the handlers here parse the request, call one `omeka-api`
//! entry point, and copy the answer onto the wire.
//!
//! It also owns what a production server owns: the startup [`Config`], the
//! request log, graceful shutdown, the readiness probe, response compression,
//! conditional GET, and the request limits. Those live in [`config`],
//! [`logging`], [`serve`], [`http`] and [`limits`].
//!
//! Nothing binds a socket except [`serve`]; tests drive the app through
//! `tower::ServiceExt::oneshot` or [`serve_with_config`] on an ephemeral port.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header};
use axum::response::Response;
use axum::routing::{any, get};
use omeka_db::DbPool;
use omeka_db::with_db;
use omeka_media::store::{FileUrls, Store};
use tower::ServiceBuilder;

use omeka_api::auth;
use omeka_api::auth_cache::AuthCache;
use omeka_api::errors::{ApiError, ErrorDocument};
use omeka_api::import::archive::ArchiveRoots;
use omeka_api::import::preview::Caches;
use omeka_api::jsonld::{self, BaseUrl};
use omeka_api::limits::Limiter;
use omeka_api::read::{Api, ApiResponse, Params};
use omeka_api::routes::{self, ApiPath, Method as ApiMethod, Resolution};
use omeka_api::session;

use mail::Mailer;

mod accounts;
mod annotations;
mod assets;
mod bulk;
mod config;
mod config_store;
mod files_allow;
mod fixity;
mod http;
mod iiif;
mod imports;
mod item_history;
mod items_csv;
pub mod jobs;
mod limits;
mod logging;
pub mod mail;
mod mail_config;
mod media_add;
mod media_derivatives;
mod media_render;
mod page_versions;
mod place_counts;
mod publish;
mod serve;
mod settings;
pub mod settings_upgrade;
mod site;
mod site_items;
mod tags;
mod tus;
mod uploads;
mod vocab_import;

pub use bulk::{purge_due_trash, settle_bulk_runs};
pub use config::{Config, ConfigError, MailConfig, MailTls};
pub use config_store::{StoreConfig, file_store, local_store, store_for};
pub use imports::{ResumeEnv, resume_imports};
pub use jobs::{DispatchError, JobDispatch, NoDispatch};
pub use logging::init_logging;
pub use serve::{serve, serve_with_config, serve_with_services, shutdown_signal};

/// The `/api` JSON-LD media type (`ApiJsonStrategy::$formats['jsonld']`).
pub const JSONLD_CONTENT_TYPE: &str = omeka_api::read::JSONLD_CONTENT_TYPE;

/// Storage root a router without an explicit store reads and writes:
/// upstream's `file_store.local.base_path`, `OMEKA_PATH . '/files'`, resolved
/// against the process's working directory.
pub const FILES_ROOT: &str = "files";
/// Storage base URI: upstream's `file_store.local.base_uri`.
pub const FILES_URI: &str = "/files";

/// State every handler needs: the storage pool, the canonical base URL, the
/// file store a media delete reaches, the API key credential cache that lives
/// for the process, the attempt counters, the mailer, and whether cookies are
/// marked `Secure`.
#[derive(Clone)]
pub struct AppState {
    pub(crate) pool: DbPool,
    pub(crate) base: BaseUrl,
    pub(crate) store: Arc<dyn Store>,
    /// The URLs the site renders for stored files, from `store`.
    pub(crate) files: FileUrls,
    /// One [`AuthCache`] per router build, so per process in `serve`.
    /// A key that verified once skips the bcrypt check on its later requests.
    pub(crate) auth_cache: AuthCache,
    /// The login and forgot-password counters, shared by every request.
    pub(crate) limiter: Limiter,
    /// The transport every message goes through.
    pub(crate) mailer: Mailer,
    /// True when the configured base URL is https, so a session cookie is
    /// marked `Secure`.
    pub(crate) secure_cookies: bool,
    /// The runner the import routes start their jobs through.
    pub(crate) jobs: Arc<dyn JobDispatch>,
    /// The named archive folders an import's folder sources read.
    pub(crate) archive_roots: Arc<ArchiveRoots>,
    /// The tag, sheet and evidence index caches every preview shares.
    pub(crate) import_caches: Arc<Caches>,
    /// The size or duration of each stored media a IIIF document has named.
    pub(crate) extents: Arc<iiif::Extents>,
}

/// What the process supplies beyond the pool and the [`Config`]: the file
/// store and the job runner.
#[derive(Clone)]
pub struct Services {
    pub store: Arc<dyn Store>,
    pub jobs: Arc<dyn JobDispatch>,
}

impl Services {
    /// `store`, and no job runner.
    #[must_use]
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self {
            store,
            jobs: Arc::new(NoDispatch),
        }
    }

    #[must_use]
    pub fn with_jobs(mut self, jobs: Arc<dyn JobDispatch>) -> Self {
        self.jobs = jobs;
        self
    }
}

/// Build the application: `/health`, `/ready`, `/api`, `/api-context`, the
/// collection and member paths, the public site routes, the theme asset routes,
/// and a fallback for everything else.
///
/// The default [`Config`] supplies the limits; [`build_router_with_config`]
/// takes the deployment's. `pool` is a [`DbPool`], or a SQLite pool a test
/// built.
pub fn build_router(pool: impl Into<DbPool>, base: BaseUrl) -> Router {
    build_router_with_store(pool, base, local_store(FILES_ROOT))
}

/// Build the application around one file store. A test roots the store in a
/// tempdir.
pub fn build_router_with_store(
    pool: impl Into<DbPool>,
    base: BaseUrl,
    store: Arc<dyn Store>,
) -> Router {
    let config = Config {
        base_url: base,
        ..Config::default()
    };
    build_router_with_services(pool, &config, Services::new(store))
}

/// Build the application with the configured base URL and limits.
pub fn build_router_with_config(pool: impl Into<DbPool>, config: &Config) -> Router {
    build_router_with_services(pool, config, Services::new(local_store(FILES_ROOT)))
}

/// Build the application with the configured base URL, limits and archive
/// root, around the process's file store and job runner.
pub fn build_router_with_services(
    pool: impl Into<DbPool>,
    config: &Config,
    services: Services,
) -> Router {
    build_app(pool.into(), services, config)
}

/// The routes, wrapped in the middleware stack. The outer layer is [`ServiceBuilder`]'s
/// first: the log sees the final status, the timeout bounds everything inside it,
/// compression runs on the tagged body, the same-origin guard refuses a
/// cookie-authenticated write from another origin, and the body cap is nearest
/// the routes.
fn build_app(pool: DbPool, services: Services, config: &Config) -> Router {
    let base = config.base_url.clone();
    let secure_cookies = base.as_str().starts_with("https://");
    let extents = Arc::new(iiif::Extents::new(pool.clone(), services.store.clone()));
    let state = AppState {
        pool,
        base: base.clone(),
        files: FileUrls::new(services.store.clone(), base.as_str()),
        store: services.store,
        auth_cache: AuthCache::new(),
        limiter: Limiter::new(omeka_api::limits::RateLimits::default()),
        mailer: Mailer::from_config(&config.mail, base.clone()),
        secure_cookies,
        jobs: services.jobs,
        archive_roots: Arc::new(config.archive_roots.clone()),
        import_caches: Arc::new(Caches::default()),
        extents,
    };
    let routes = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/admin", get(admin_redirect))
        .route("/admin/", get(admin_redirect))
        .route("/api", any(api_root))
        .route("/api-context", any(api_context))
        .route("/api/{resource}", any(collection))
        .route("/api/{resource}/{id}", any(member))
        .merge(accounts::router())
        .merge(annotations::router())
        .merge(media_render::router())
        .merge(media_derivatives::router())
        .merge(fixity::router())
        .merge(site_items::router())
        .merge(bulk::router())
        .merge(place_counts::router())
        .merge(item_history::router())
        .merge(items_csv::router())
        .merge(page_versions::router())
        .merge(imports::router())
        .merge(uploads::router())
        .merge(media_add::router())
        .merge(tags::router())
        .merge(vocab_import::router())
        .merge(settings::router())
        .merge(site::router())
        .merge(assets::router())
        .merge(iiif::router())
        .merge(publish::router())
        .merge(files_allow::router())
        .fallback(not_found)
        .with_state(state);
    routes.layer(
        ServiceBuilder::new()
            .layer(axum::middleware::from_fn(logging::log_request))
            .layer(axum::middleware::from_fn_with_state(
                config.request_timeout,
                limits::timeout_request,
            ))
            .layer(axum::middleware::from_fn(http::compress_response))
            .layer(axum::middleware::from_fn(http::conditional_get))
            .layer(axum::middleware::from_fn_with_state(
                base,
                accounts::same_origin_write,
            ))
            .layer(DefaultBodyLimit::max(config.max_body_bytes)),
    )
}

/// The body both probes answer with; neither is part of the upstream API.
#[derive(Debug, serde::Serialize)]
struct StatusBody {
    status: &'static str,
}

/// Liveness: the process is up. It touches no shared state.
async fn health() -> Response {
    status_response(StatusCode::OK, "ok")
}

/// Readiness: the database answers a query. A closed or unreachable pool is a
/// 503, so a load balancer stops sending traffic until storage is back.
async fn ready(State(state): State<AppState>) -> Response {
    match with_db!(&state.pool, |db| omeka_db::repos::DatabaseRepo::new(db)
        .ping()
        .await)
    {
        Ok(_) => status_response(StatusCode::OK, "ready"),
        Err(_) => status_response(StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
    }
}

/// `/admin` and `/admin/`: upstream's admin path, where an old bookmark lands.
/// The SPA is served at the root and loads its assets relative to its own
/// path, so the shell under `/admin/` would request `/admin/assets/*` and
/// render blank. Both spellings send the browser to the root instead.
async fn admin_redirect(State(state): State<AppState>) -> Response {
    Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, format!("{}/", state.base.as_str()))
        .body(Body::empty())
        .unwrap_or_else(|_| empty_internal_error())
}

fn status_response(status: StatusCode, value: &'static str) -> Response {
    let body = StatusBody { status: value };
    match serde_json::to_string(&body) {
        Ok(body) => Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap_or_else(|_| empty_internal_error()),
        Err(_) => empty_internal_error(),
    }
}

async fn api_root(
    State(state): State<AppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    match routes::resolve(ApiPath::Root, ApiMethod::from_http(&method)) {
        Resolution::NotImplemented { .. } => match open(&state, &uri, &headers).await {
            Ok(api) => finish(api.api_root().await),
            Err(error) => error_response(error),
        },
        other => respond(other),
    }
}

async fn api_context(
    State(state): State<AppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    match routes::resolve_context(ApiMethod::from_http(&method)) {
        Resolution::NotImplemented { .. } => match open(&state, &uri, &headers).await {
            Ok(api) => finish(api.context().await),
            Err(error) => error_response(error),
        },
        other => respond(other),
    }
}

async fn collection(
    State(state): State<AppState>,
    method: Method,
    Path(resource): Path<String>,
    uri: Uri,
    headers: HeaderMap,
    body: limits::Payload,
) -> Response {
    let api_method = ApiMethod::from_http(&method);
    match routes::resolve(ApiPath::Collection(&resource), api_method) {
        Resolution::NotFound { message } => error_response(ApiError::not_found(message)),
        Resolution::MethodNotAllowed { allow, .. } => error_response(ApiError::MethodNotAllowed {
            allow: allow.to_vec(),
        }),
        Resolution::NotImplemented { .. } => match api_method {
            ApiMethod::Get => match open(&state, &uri, &headers).await {
                Ok(api) => {
                    let params = Params::parse(uri.query().unwrap_or(""));
                    finish(api.search(&resource, &params).await)
                }
                Err(error) => error_response(error),
            },
            ApiMethod::Post => write(&state, &resource, None, &headers, &body.0, &uri, false).await,
            // `routes::resolve` answers 405 for every method a collection does
            // not route, so only `Method::Other` reaches here.
            _ => method_not_allowed(routes::COLLECTION_ALLOW),
        },
    }
}

async fn member(
    State(state): State<AppState>,
    method: Method,
    Path((resource, id)): Path<(String, String)>,
    uri: Uri,
    headers: HeaderMap,
    body: limits::Payload,
) -> Response {
    let api_method = ApiMethod::from_http(&method);
    match routes::resolve(ApiPath::Member(&resource, &id), api_method) {
        Resolution::NotFound { message } => error_response(ApiError::not_found(message)),
        Resolution::MethodNotAllowed { allow, .. } => error_response(ApiError::MethodNotAllowed {
            allow: allow.to_vec(),
        }),
        Resolution::NotImplemented { .. } => match api_method {
            ApiMethod::Get => match open(&state, &uri, &headers).await {
                Ok(api) => finish(api.read_member(&resource, &id).await),
                Err(error) => error_response(error),
            },
            ApiMethod::Post => write(&state, &resource, None, &headers, &body.0, &uri, false).await,
            ApiMethod::Put => {
                write(&state, &resource, Some(&id), &headers, &body.0, &uri, false).await
            }
            ApiMethod::Patch => {
                write(&state, &resource, Some(&id), &headers, &body.0, &uri, true).await
            }
            ApiMethod::Delete => delete(&state, &resource, &id, &uri, &headers).await,
            _ => method_not_allowed(routes::MEMBER_ALLOW),
        },
    }
}

/// A routed method with no handler of its own. `routes::resolve` already
/// answers 405 for every method it does not route, so this arm only sees
/// `Method::Other` and keeps the match total.
fn method_not_allowed(allow: &'static [ApiMethod]) -> Response {
    error_response(ApiError::MethodNotAllowed {
        allow: allow.to_vec(),
    })
}

/// A delete: authenticate, check the adapter grant, and let the API layer
/// remove the row. Upstream's `ApiController::delete` ignores the request body,
/// so this path requires no content type.
async fn delete(
    state: &AppState,
    resource: &str,
    id: &str,
    uri: &Uri,
    headers: &HeaderMap,
) -> Response {
    match open(state, uri, headers).await {
        Ok(api) => finish(api.delete(resource, id).await),
        Err(error) => error_response(error),
    }
}

/// A create or update: check the content type, authenticate, and hand the raw
/// body to the API layer.
async fn write(
    state: &AppState,
    resource: &str,
    id: Option<&str>,
    headers: &HeaderMap,
    body: &[u8],
    uri: &Uri,
    partial: bool,
) -> Response {
    if !content_type_is_json(headers) {
        return error_response(omeka_api::write::body::content_type_error(media_type(
            headers,
        )));
    }
    let api = match open(state, uri, headers).await {
        Ok(api) => api,
        Err(error) => return error_response(error),
    };
    let result = match id {
        Some(id) => api.update(resource, id, body, partial).await,
        None => api.create(resource, body, content_type(headers)).await,
    };
    // A media or item create queues derivatives the request did not make
    // (`omeka_api::derivatives`); start the one worker that drains them.
    if id.is_none() && matches!(resource, "media" | "items") && result.is_ok() {
        let owner = api.actor().user();
        jobs::start_derivatives(&state.pool, state.jobs.as_ref(), owner).await;
    }
    finish(result)
}

/// `ApiController::checkContentType`: a write body must be JSON or multipart.
pub(crate) fn content_type_is_json(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value.starts_with("application/json") || value.starts_with("multipart/form-data")
        })
        .unwrap_or(false)
}

/// The raw `Content-Type` header, so a multipart create keeps its boundary.
fn content_type(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
}

pub(crate) fn media_type(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(';').next().unwrap_or(value).trim())
}

async fn not_found() -> Response {
    document_response(
        StatusCode::NOT_FOUND,
        &ErrorDocument::general(omeka_api::messages::NOT_FOUND),
    )
}

/// Authenticate the caller and load the read surface.
pub(crate) async fn open<'a>(
    state: &'a AppState,
    uri: &Uri,
    headers: &HeaderMap,
) -> Result<Api<'a>, ApiError> {
    let actor = actor_for(state, uri, headers).await?;
    Api::load_with_store(
        &state.pool,
        state.base.clone(),
        actor,
        Some(state.store.clone()),
    )
    .await
}

/// The caller of one request: the session cookie first, then the key pair.
///
/// `MvcListeners::authenticateApiKey` reads the key pair on every `/api`
/// request. The session is the port's addition: the admin SPA holds a cookie
/// and no key, so a request that carries a live session is that user, and a
/// request that carries none falls through to the key pair exactly as before.
pub(crate) async fn actor_for(
    state: &AppState,
    uri: &Uri,
    headers: &HeaderMap,
) -> Result<auth::Actor, ApiError> {
    if let Some(actor) = session_actor(state, headers).await? {
        return Ok(actor);
    }
    let params = Params::parse(uri.query().unwrap_or(""));
    auth::authenticate_with(&state.pool, &state.auth_cache, params.pairs()).await
}

/// The actor the request's session cookie names, `None` when it carries no live
/// session.
///
/// The public site routes read only this: `MvcListeners::authenticateApiKey`
/// runs the key pair on `/api` routes alone (`__KEYAUTH__`), so a key in a `/s`
/// query string is not an identity upstream.
pub(crate) async fn session_actor(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Option<auth::Actor>, ApiError> {
    let token = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(session::cookie_token);
    Ok(omeka_api::login::current_user(&state.pool, token)
        .await?
        .map(|user| auth::Actor::new(user.id, user.role)))
}

/// Turn one resolution into a response.
fn respond(resolution: Resolution) -> Response {
    error_response(ApiError::from(resolution))
}

/// An [`ApiResponse`] copied onto the wire.
pub(crate) fn finish(result: Result<ApiResponse, ApiError>) -> Response {
    match result {
        Ok(answer) => {
            let mut builder = Response::builder().status(answer.status);
            for (name, value) in answer.headers.iter() {
                builder = builder.header(name, value);
            }
            builder
                .body(Body::from(answer.body))
                .unwrap_or_else(|_| empty_internal_error())
        }
        Err(error) => error_response(error),
    }
}

/// An [`ApiError`] with its status, document, and, for 405, its Allow header.
pub(crate) fn error_response(error: ApiError) -> Response {
    let allow = match &error {
        ApiError::MethodNotAllowed { allow } => Some(allow_header_value(allow)),
        _ => None,
    };
    let mut response = document_response(error.status(), &error.document());
    if let Some(value) = allow {
        response.headers_mut().insert(header::ALLOW, value);
    }
    response
}

fn allow_header_value(allow: &[ApiMethod]) -> HeaderValue {
    let list = allow
        .iter()
        .map(|method| method.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    HeaderValue::from_str(&list).unwrap_or_else(|_| HeaderValue::from_static("GET"))
}

pub(crate) fn document_response(status: StatusCode, document: &ErrorDocument) -> Response {
    match jsonld::to_string(document) {
        Ok(body) => Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, JSONLD_CONTENT_TYPE)
            .header(
                omeka_api::read::response::VERSION_HEADER,
                omeka_api::read::OMEKA_VERSION,
            )
            .body(Body::from(body))
            .unwrap_or_else(|_| empty_internal_error()),
        Err(_) => empty_internal_error(),
    }
}

pub(crate) fn empty_internal_error() -> Response {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(header::CONTENT_TYPE, JSONLD_CONTENT_TYPE)
        .body(Body::from(format!(
            r#"{{"errors":{{"error":"{}"}}}}"#,
            omeka_api::messages::INTERNAL_ERROR
        )))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}
