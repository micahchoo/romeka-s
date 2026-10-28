//! `GET /api/romeka/items.csv?<item search query>`: every item `GET /api/items` lists for the same
//! query and caller, as a CSV download. Upstream has no export route.
//!
//! The file is `omeka_api::read::export::items_csv`, the same one `romeka export csv` writes, so
//! the admin download and the CLI share one column set.

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::Response;
use axum::routing::get;
use omeka_api::read::{Params, export};

use crate::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/api/romeka/items.csv", get(items_csv))
}

async fn items_csv(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    let query = Params::parse(uri.query().unwrap_or(""));
    let api = match crate::open(&state, &uri, &headers).await {
        Ok(api) => api,
        Err(error) => return crate::error_response(error),
    };
    match export::items_csv(&api, &query, &[]).await {
        Ok(file) => attachment(file.text),
        Err(error) => crate::error_response(error),
    }
}

/// The file as a download named for today, which no cache keeps: it answers one caller's
/// visibility and follows every write.
fn attachment(text: String) -> Response {
    let today = chrono::Utc::now().date_naive();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"items-{today}.csv\""),
        )
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(text))
        .unwrap_or_else(|_| crate::empty_internal_error())
}
