//! Binding the socket, the shutdown signal, and the two entry points.
//!
//! [`serve`] keeps the signature `omeka-cli` calls; [`serve_with_config`] reads
//! the whole config and installs the log subscriber. Both stop on SIGTERM or
//! SIGINT: axum stops accepting connections, lets the in-flight requests finish,
//! and the future resolves.

use std::future::Future;
use std::net::SocketAddr;

use omeka_api::jsonld::BaseUrl;
use sqlx::SqlitePool;
use tokio::net::TcpListener;

use crate::config::Config;
use crate::logging::init_logging;

/// Bind `addr` and serve until the process stops.
///
/// The host, the port and the base URL come from the caller. Everything else
/// (the log level, the body cap, the request timeout) comes from the file and
/// the environment, through [`Config::load`].
///
/// # Errors
/// An [`std::io::Error`] when the config cannot be loaded, the address cannot be
/// bound, or the server fails.
pub async fn serve(addr: SocketAddr, pool: SqlitePool, base: BaseUrl) -> std::io::Result<()> {
    let mut config = Config::load().map_err(std::io::Error::other)?;
    config.set_address(addr);
    config.set_base_url(base);
    serve_with_config(config, pool).await
}

/// Bind the configured address and serve until a signal arrives.
///
/// # Errors
/// An [`std::io::Error`] when the address cannot be bound or the server fails.
pub async fn serve_with_config(config: Config, pool: SqlitePool) -> std::io::Result<()> {
    init_logging(config.log_level);
    let listener = TcpListener::bind((config.host.as_str(), config.port)).await?;
    tracing::info!(
        host = %config.host,
        port = config.port,
        base_url = config.base_url.as_str(),
        "listening"
    );
    serve_listener(listener, pool, &config, shutdown_signal()).await
}

/// Serve a bound listener until `shutdown` resolves.
async fn serve_listener<F>(
    listener: TcpListener,
    pool: SqlitePool,
    config: &Config,
    shutdown: F,
) -> std::io::Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let app = crate::build_router_with_config(pool, config);
    // The connect info is what gives a handler the socket peer, which the
    // login limit counts when no proxy header names an address.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
}

/// Resolve on SIGINT or SIGTERM, the two signals a container runtime sends.
pub async fn shutdown_signal() {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = interrupt => {}
        () = terminate => {}
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn the_server_answers_until_the_shutdown_future_resolves() {
        let pool = omeka_db::open_memory().await.unwrap();
        omeka_db::migrate(&pool).await.unwrap();
        let config = Config::default();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (stop, stopped) = oneshot::channel();
        let server = tokio::spawn(async move {
            serve_listener(listener, pool, &config, async move {
                let _ = stopped.await;
            })
            .await
        });

        let answer = get(addr, "/health").await;
        assert!(
            answer.contains("200 OK"),
            "health before shutdown: {answer}"
        );

        stop.send(()).unwrap();
        let outcome = tokio::time::timeout(Duration::from_secs(10), server)
            .await
            .expect("the server stops when the signal resolves")
            .expect("the server task does not panic");
        assert!(outcome.is_ok(), "the server returns cleanly: {outcome:?}");

        assert!(
            tokio::net::TcpStream::connect(addr).await.is_err(),
            "the listener is closed after shutdown"
        );
    }

    async fn get(addr: SocketAddr, path: &str) -> String {
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(
                format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();
        response
    }
}
