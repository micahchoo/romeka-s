//! Structured request logs.
//!
//! One `tracing` event per request carries the method, the path, the status and
//! the duration in milliseconds. The path is logged without its query string, so
//! an API key passed as `key_credential` never reaches the log.
//!
//! [`init_logging`] installs the subscriber that prints those events to stdout.
//! The subscriber is plain text with the fields on one line, which is what a log
//! collector reads; `tracing-subscriber`'s `json` feature would produce JSON from
//! the same events if a deployment wants it.

use std::time::Instant;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use tracing::level_filters::LevelFilter;

/// Install the stdout subscriber at `level`.
///
/// Calling it more than once in a process is harmless: the second call is
/// ignored, because `tracing` keeps one global subscriber.
pub fn init_logging(level: LevelFilter) {
    let _ = tracing_subscriber::fmt()
        .with_max_level(level)
        .with_ansi(false)
        .with_target(false)
        .try_init();
}

/// Log one line per request once its response exists.
pub(crate) async fn log_request(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let started = Instant::now();
    let response = next.run(request).await;
    tracing::info!(
        method = %method,
        path = %path,
        status = response.status().as_u16(),
        duration_ms = started.elapsed().as_millis() as u64,
        "request"
    );
    response
}
