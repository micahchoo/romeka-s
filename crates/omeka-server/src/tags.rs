//! The tag routes: the census of tags and of a term list's words, merging spellings, remembering
//! spellings that differ, and promoting tags into a list.
//!
//! | Route | Answer |
//! | --- | --- |
//! | `GET /api/tags` | `{values: [{value, resources, editable}], distinct}` |
//! | `POST /api/tags/promote` | `{list, created, changed, skipped, templates, templates_skipped}` |
//! | `POST /api/tags/merge` | `{changed, skipped}` |
//! | `POST /api/tags/distinct` | the census |
//! | `GET /api/custom_vocabs/{id}/values` | the census of the list's words |
//! | `POST /api/custom_vocabs/{id}/merge` | `{changed, skipped}` |
//! | `POST /api/custom_vocabs/{id}/distinct` | the census of the list's words |
//!
//! Every decision lives in `omeka_api::tagging`; a handler checks the content type, authenticates and
//! copies the answer onto the wire. Upstream has none of these routes.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Uri};
use axum::response::Response;
use axum::routing::{get, post};
use omeka_api::read::Api;
use omeka_api::tagging::{self as tags, Scope};

use crate::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/tags", get(tag_census))
        .route("/api/tags/promote", post(promote))
        .route("/api/tags/merge", post(tag_merge))
        .route("/api/tags/distinct", post(tag_distinct))
        .route("/api/custom_vocabs/{id}/values", get(list_census))
        .route("/api/custom_vocabs/{id}/merge", post(list_merge))
        .route("/api/custom_vocabs/{id}/distinct", post(list_distinct))
}

async fn tag_census(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    match crate::open(&state, &uri, &headers).await {
        Ok(api) => crate::finish(tags::census(&api, Scope::Tags).await),
        Err(error) => crate::error_response(error),
    }
}

async fn list_census(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let scope = match tags::list_scope(&id) {
        Ok(scope) => scope,
        Err(error) => return crate::error_response(error),
    };
    match crate::open(&state, &uri, &headers).await {
        Ok(api) => crate::finish(tags::census(&api, scope).await),
        Err(error) => crate::error_response(error),
    }
}

/// Which write a JSON body asks for.
#[derive(Clone, Copy)]
enum Write {
    Promote,
    Merge(Scope),
    Distinct(Scope),
}

async fn run(api: &Api<'_>, write: Write, body: &[u8]) -> Response {
    crate::finish(match write {
        Write::Promote => tags::promote::promote(api, body).await,
        Write::Merge(scope) => tags::merge(api, scope, body).await,
        Write::Distinct(scope) => tags::distinct(api, scope, body).await,
    })
}

async fn write(
    state: &AppState,
    uri: &Uri,
    headers: &HeaderMap,
    write: Write,
    body: &[u8],
) -> Response {
    if !crate::content_type_is_json(headers) {
        return crate::error_response(omeka_api::write::body::content_type_error(
            crate::media_type(headers),
        ));
    }
    match crate::open(state, uri, headers).await {
        Ok(api) => run(&api, write, body).await,
        Err(error) => crate::error_response(error),
    }
}

async fn promote(
    State(state): State<AppState>,
    uri: Uri,
    headers: HeaderMap,
    body: crate::limits::Payload,
) -> Response {
    write(&state, &uri, &headers, Write::Promote, &body.0).await
}

async fn tag_merge(
    State(state): State<AppState>,
    uri: Uri,
    headers: HeaderMap,
    body: crate::limits::Payload,
) -> Response {
    write(&state, &uri, &headers, Write::Merge(Scope::Tags), &body.0).await
}

async fn tag_distinct(
    State(state): State<AppState>,
    uri: Uri,
    headers: HeaderMap,
    body: crate::limits::Payload,
) -> Response {
    write(
        &state,
        &uri,
        &headers,
        Write::Distinct(Scope::Tags),
        &body.0,
    )
    .await
}

async fn list_merge(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
    body: crate::limits::Payload,
) -> Response {
    match tags::list_scope(&id) {
        Ok(scope) => write(&state, &uri, &headers, Write::Merge(scope), &body.0).await,
        Err(error) => crate::error_response(error),
    }
}

async fn list_distinct(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
    body: crate::limits::Payload,
) -> Response {
    match tags::list_scope(&id) {
        Ok(scope) => write(&state, &uri, &headers, Write::Distinct(scope), &body.0).await,
        Err(error) => crate::error_response(error),
    }
}
