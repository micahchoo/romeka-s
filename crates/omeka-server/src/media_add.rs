//! `/api/romeka/upload-limits` and `/api/romeka/media/probe`: what an add
//! panel reads before it sends a file or a link (`briefs/media-ui`, G2 and G7).
//!
//! The answers are computed in `omeka_api::media_add`; these handlers
//! authenticate the caller, check the grant, and copy the value onto the wire.
//! The limits are readable by anyone who may create a media or an asset; the
//! probe needs the media-create grant, because it is the step before one.

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::Response;
use axum::routing::{get, post};
use omeka_api::auth::Actor;
use omeka_api::errors::ApiError;
use omeka_api::messages;
use omeka_api::routes::Operation;
use omeka_api::write::access;
use serde::Deserialize;

use crate::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/romeka/upload-limits", get(limits))
        .route("/api/romeka/media/probe", post(probe))
}

/// The caller, refused unless signed in.
async fn caller(state: &AppState, uri: &Uri, headers: &HeaderMap) -> Result<Actor, Box<Response>> {
    let actor = crate::actor_for(state, uri, headers)
        .await
        .map_err(|error| Box::new(crate::error_response(error)))?;
    if actor.user().is_some() {
        Ok(actor)
    } else {
        Err(Box::new(unauthorized()))
    }
}

/// The refusal a request with no session and no key gets on these routes.
fn unauthorized() -> Response {
    let body = serde_json::json!({ "errors": { "error": messages::AUTHENTICATION_REQUIRED } });
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(header::CONTENT_TYPE, crate::JSONLD_CONTENT_TYPE)
        .body(Body::from(body.to_string()))
        .unwrap_or_else(|_| crate::empty_internal_error())
}

/// `GET /api/romeka/upload-limits`: the size and the validation lists.
async fn limits(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    match caller(&state, &uri, &headers).await {
        Ok(actor) => {
            // A media create and an asset create carry the same roles; either
            // one lets a contributor read the limits.
            if let Err(error) = access::adapter(&actor, "media", Operation::Create)
                && access::adapter(&actor, "assets", Operation::Create).is_err()
            {
                return crate::error_response(error);
            }
            match omeka_api::media_add::limits(&state.pool).await {
                Ok(limits) => json(StatusCode::OK, &limits),
                Err(error) => crate::error_response(ApiError::internal(error)),
            }
        }
        Err(response) => *response,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeBody {
    url: String,
}

/// `POST /api/romeka/media/probe {"url"}`: what the link is, with no row.
async fn probe(
    State(state): State<AppState>,
    uri: Uri,
    headers: HeaderMap,
    payload: crate::limits::Payload,
) -> Response {
    let actor = match caller(&state, &uri, &headers).await {
        Ok(actor) => actor,
        Err(response) => return *response,
    };
    if let Err(error) = access::adapter(&actor, "media", Operation::Create) {
        return crate::error_response(error);
    }
    if !crate::content_type_is_json(&headers) {
        return crate::error_response(omeka_api::write::body::content_type_error(
            crate::media_type(&headers),
        ));
    }
    let body: ProbeBody = match serde_json::from_slice(&payload.0) {
        Ok(body) => body,
        Err(error) => {
            return crate::error_response(ApiError::bad_request(
                messages::import_request_unreadable(error),
            ));
        }
    };
    match omeka_api::media_add::probe(&state.pool, &body.url).await {
        Ok(probe) => json(StatusCode::OK, &probe),
        Err(error) => crate::error_response(error),
    }
}

fn json(status: StatusCode, value: &impl serde::Serialize) -> Response {
    match serde_json::to_string(value) {
        Ok(body) => Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap_or_else(|_| crate::empty_internal_error()),
        Err(error) => crate::error_response(ApiError::internal(error)),
    }
}
