//! The Web Annotation Protocol routes.
//!
//! One media's container (`/annotations/media/{id}/`), its keyset page
//! (`…/page`) and one annotation (`/annotations/{id}`) are W3C Web Annotation
//! Protocol answers. The page is also the IIIF `AnnotationPage` a canvas
//! references, so one document serves both standards. The domain work lives in
//! `omeka_api::annotation`; this module copies its answers onto the wire.
//!
//! This is a Romeka addition, not upstream's: upstream has no annotation
//! server, no `Idempotency-Key` and no strong per-row ETag. The container and
//! page also carry `Romeka-Surface` (what a selector may address),
//! `Romeka-Canvas` (the canvas IRI a create targets) and `Romeka-Track-Ids` (the
//! rows a caption import wrote), which a WAP client does not ask for and the
//! admin editor cannot work without. `docs/compat.md` records the difference.

mod counts;
mod kept;
mod tracks;
mod wap;

use axum::Router;
use axum::response::Response;
use axum::routing::{any, get};
use omeka_api::annotation::{
    Annotation, AnnotationError, Iris, MediaRef, Origin, OriginFilter, Page, TrackError,
};
use omeka_api::errors::ErrorDocument;
use omeka_api::messages;
use omeka_api::read::Params;
use omeka_media::kind::{Extent, Surface};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::AppState;

/// The W3C Web Annotation context.
const PROFILE: &str = "http://www.w3.org/ns/anno.jsonld";
/// The media type every WAP answer carries.
pub(crate) const ANNO_TYPE: &str =
    "application/ld+json; profile=\"http://www.w3.org/ns/anno.jsonld\"";
/// The Presentation 3 context, last in the page's context array as IIIF asks.
const PRESENTATION: &str = "http://iiif.io/api/presentation/3/context.json";

/// The headers a browser's script may read from a cross-origin WAP answer.
pub(crate) const EXPOSE: &str =
    "ETag, Location, Allow, Link, Romeka-Surface, Romeka-Canvas, Romeka-Track-Ids";
/// The request headers a preflight admits.
const ALLOW_HEADERS: &str = "Content-Type, If-Match, Idempotency-Key, Prefer";
/// The methods a preflight admits, on every WAP route.
const ALLOW_METHODS: &str = "GET, HEAD, POST, PUT, DELETE, OPTIONS";
/// The container's `Link` value (6.2): an LDP basic container constrained by WAP.
const CONTAINER_LINK: &str = "<http://www.w3.org/ns/ldp#BasicContainer>; rel=\"type\", \
     <http://www.w3.org/TR/annotation-protocol/>; rel=\"http://www.w3.org/ns/ldp#constrainedBy\"";
/// The annotation's `Link` value (6.4).
const ANNOTATION_LINK: &str = "<http://www.w3.org/ns/oa#Annotation>; rel=\"type\"";

/// The annotation routes, mounted into [`crate::build_router`].
pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/annotations/media/{id}/", any(wap::container))
        .route("/annotations/media/{id}/page", any(wap::page))
        .route("/annotations/media/{id}/tracks", any(tracks::tracks))
        .route("/annotations/media/{id}/tracks/undo", any(tracks::undo))
        .route(
            "/annotations/media/{id}/captions.vtt",
            any(tracks::captions),
        )
        .route("/annotations/item/{id}/counts", get(counts::counts))
        .route("/annotations/{id}", any(wap::annotation))
}

/// The origin filter a page or container asks for. `author` is the notes
/// view; `all` adds a caption import's rows.
pub(crate) fn origins(uri: &axum::http::Uri) -> Option<OriginFilter> {
    match Params::parse(uri.query().unwrap_or("")).get("origin") {
        None | Some("") | Some("author") => Some(OriginFilter::Author),
        Some("all") => Some(OriginFilter::All),
        Some(_) => None,
    }
}

/// A JSON document answer, with the headers every `/annotations` answer carries.
pub(crate) fn document(status: axum::http::StatusCode, body: String) -> Response {
    match Response::builder()
        .status(status)
        .header(axum::http::header::CONTENT_TYPE, ANNO_TYPE)
        .body(axum::body::Body::from(body))
    {
        Ok(response) => annotated(response),
        Err(_) => crate::empty_internal_error(),
    }
}

/// An upstream error document with the WAP headers, the shape 6.7 names.
pub(crate) fn error_document(status: axum::http::StatusCode, message: &str) -> Response {
    annotated(crate::document_response(
        status,
        &ErrorDocument::general(message.to_string()),
    ))
}

/// The headers 6.1 puts on every answer, including an error and a 304.
pub(crate) fn annotated(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(
        axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
        axum::http::HeaderValue::from_static("*"),
    );
    headers.insert(
        axum::http::header::VARY,
        axum::http::HeaderValue::from_static("Accept, Cookie, Prefer"),
    );
    headers.insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-cache"),
    );
    response
}

/// One status for an [`AnnotationError`]. The 400 sentence is the service's own
/// plain English (a `Problem`); the rest reuse upstream's registry text.
pub(crate) fn wap_error(error: AnnotationError, iris: &Iris) -> Response {
    use axum::http::StatusCode;
    match error {
        AnnotationError::NotFound => error_document(StatusCode::NOT_FOUND, messages::NOT_FOUND),
        AnnotationError::Forbidden => error_document(
            StatusCode::FORBIDDEN,
            &messages::permission_denied("update", "media"),
        ),
        AnnotationError::Invalid(problem) => {
            error_document(StatusCode::BAD_REQUEST, &problem.to_string())
        }
        AnnotationError::TooLarge => {
            error_document(StatusCode::PAYLOAD_TOO_LARGE, messages::BODY_TOO_LARGE)
        }
        AnnotationError::Stale(current) => stored(StatusCode::PRECONDITION_FAILED, iris, &current),
        // The three the API registry has no sentence for yet: the service's own
        // text, translated with the `Problem` messages in a later unit.
        AnnotationError::UnsupportedType => error_document(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            &AnnotationError::UnsupportedType.to_string(),
        ),
        AnnotationError::PreconditionRequired => error_document(
            StatusCode::PRECONDITION_REQUIRED,
            &AnnotationError::PreconditionRequired.to_string(),
        ),
        AnnotationError::KeyReused => error_document(
            StatusCode::UNPROCESSABLE_ENTITY,
            &AnnotationError::KeyReused.to_string(),
        ),
        AnnotationError::NothingToUndo => error_document(
            StatusCode::CONFLICT,
            &AnnotationError::NothingToUndo.to_string(),
        ),
        AnnotationError::Track(error) => {
            let status = match error {
                TrackError::TooLarge | TrackError::TooManyCues => StatusCode::PAYLOAD_TOO_LARGE,
                _ => StatusCode::BAD_REQUEST,
            };
            error_document(status, &error.to_string())
        }
        AnnotationError::Db(error) => {
            tracing::error!(%error, "an annotation route failed on storage");
            error_document(StatusCode::INTERNAL_SERVER_ERROR, messages::INTERNAL_ERROR)
        }
    }
}

/// One stored annotation, with its strong ETag and, for a create, its location.
pub(crate) fn stored(
    status: axum::http::StatusCode,
    iris: &Iris,
    annotation: &Annotation,
) -> Response {
    let body = annotation_document(annotation, iris).to_string();
    let mut response = document(status, body);
    let headers = response.headers_mut();
    if let Ok(tag) = axum::http::HeaderValue::from_str(&annotation.revision.etag()) {
        headers.insert(axum::http::header::ETAG, tag);
    }
    if status != axum::http::StatusCode::PRECONDITION_FAILED
        && let Ok(location) = axum::http::HeaderValue::from_str(&iris.annotation(annotation.id))
    {
        headers.insert(axum::http::header::LOCATION, location);
    }
    response
}

/// The W3C JSON of one annotation, with the context the envelope adds.
pub(crate) fn annotation_document(annotation: &Annotation, iris: &Iris) -> Value {
    let mut value = omeka_api::annotation::render(annotation, iris);
    if let Value::Object(map) = &mut value {
        map.insert("@context".into(), json!(PROFILE));
    }
    value
}

/// The page's IRI, with the origin filter when it is not the notes default, so
/// `next` cannot silently drop a caller back to authored rows.
fn page_iri(
    iris: &Iris,
    page: &Page,
    after: Option<omeka_api::annotation::AnnotationId>,
) -> String {
    let mut iri = iris.page(page.media.id, after);
    if page.origins == OriginFilter::All {
        iri.push(if after.is_some() { '&' } else { '?' });
        iri.push_str("origin=all");
    }
    iri
}

/// A page document: the WAP page and the IIIF `AnnotationPage` in one, 6.3.
pub(crate) fn page_document(page: &Page, iris: &Iris, iri_items: bool) -> Value {
    let mut map = Map::new();
    map.insert("@context".into(), json!([PROFILE, PRESENTATION]));
    map.insert("id".into(), json!(page_iri(iris, page, page.after)));
    map.insert("type".into(), json!("AnnotationPage"));
    map.insert(
        "partOf".into(),
        json!({
            "id": iris.container(page.media.id),
            "type": "AnnotationCollection",
            "total": page.total,
        }),
    );
    if page.after.is_none() {
        map.insert("startIndex".into(), json!(0));
    }
    let items: Vec<Value> = page
        .items
        .iter()
        .map(|annotation| {
            if iri_items {
                json!(iris.annotation(annotation.id))
            } else {
                omeka_api::annotation::render(annotation, iris)
            }
        })
        .collect();
    map.insert("items".into(), Value::Array(items));
    if let Some(next) = page.next {
        map.insert("next".into(), json!(page_iri(iris, page, Some(next))));
    }
    Value::Object(map)
}

/// The container document, 6.2. `first` is the page document, or its IRI when
/// the client asked for a minimal container.
pub(crate) fn container_document(
    page: &Page,
    iris: &Iris,
    title: Option<&str>,
    minimal: bool,
    iri_items: bool,
) -> Value {
    let mut map = Map::new();
    map.insert(
        "@context".into(),
        json!([PROFILE, "http://www.w3.org/ns/ldp.jsonld"]),
    );
    map.insert("id".into(), json!(iris.container(page.media.id)));
    map.insert(
        "type".into(),
        json!(["BasicContainer", "AnnotationCollection"]),
    );
    map.insert(
        "label".into(),
        json!(match title {
            Some(title) => format!("Notes on {title}"),
            None => "Notes".to_string(),
        }),
    );
    map.insert("total".into(), json!(page.total));
    map.insert(
        "first".into(),
        if minimal {
            json!(page_iri(iris, page, None))
        } else {
            page_document(page, iris, iri_items)
        },
    );
    Value::Object(map)
}

/// `Romeka-Surface`: the RFC 8941 item the editor draws from (6.2).
pub(crate) fn surface_header(media: &MediaRef, iris: &Iris) -> String {
    match media.kind.surface() {
        Surface::Image => {
            let mut header = String::from("image");
            if let Extent {
                size: Some((width, height)),
                ..
            } = media.extent
            {
                header.push_str(&format!("; width={width}; height={height}"));
            }
            header.push_str(&format!("; service=\"{}\"", iris.image(media.id)));
            header
        }
        Surface::Time => match media.extent.duration_ms {
            Some(milliseconds) => format!("time; duration={}", seconds(milliseconds)),
            None => "time".to_string(),
        },
        Surface::Whole => "whole".to_string(),
    }
}

/// `Romeka-Track-Ids`: the ids of the caption rows on the page it answers.
pub(crate) fn track_ids(page: &Page) -> Option<String> {
    let ids: Vec<String> = page
        .items
        .iter()
        .filter(|annotation| annotation.origin == Origin::Track)
        .map(|annotation| annotation.id.0.to_string())
        .collect();
    (!ids.is_empty()).then(|| ids.join(" "))
}

/// A strong page tag: `"p"` and 16 hex characters of a SHA-256 over the content
/// type and the body, so the WAP and IIIF forms of one page differ.
pub(crate) fn page_tag(content_type: &str, body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content_type.as_bytes());
    hasher.update(body.as_bytes());
    let digest = hasher.finalize();
    let mut tag = String::with_capacity(18);
    tag.push_str("\"p");
    for byte in &digest[..8] {
        tag.push_str(&format!("{byte:02x}"));
    }
    tag.push('"');
    tag
}

/// `If-None-Match` uses weak comparison, so `W/` is ignored on either side.
pub(crate) fn if_none_match(header: Option<&str>, tag: &str) -> bool {
    let Some(list) = header else {
        return false;
    };
    list.split(',').any(|candidate| {
        let candidate = candidate.trim();
        candidate == "*" || candidate.trim_start_matches("W/") == tag.trim_start_matches("W/")
    })
}

/// Seconds with at most three decimals, as the media-frags `t=` writes them.
fn seconds(milliseconds: u64) -> String {
    let whole = milliseconds / 1000;
    let fraction = milliseconds % 1000;
    if fraction == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{fraction:03}")
            .trim_end_matches('0')
            .to_string()
    }
}
