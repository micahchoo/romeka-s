//! `omeka serve`: start the HTTP server.
//!
//! `omeka-server` owns the router and the socket. This command opens the SQLite
//! database, parses the base URL used in the JSON-LD links, and hands both to
//! the server.

use std::net::SocketAddr;

use omeka_api::jsonld::BaseUrl;

use crate::error::CliError;

/// Bind `host:port` and serve until the process stops.
///
/// # Errors
/// A [`CliError::Message`] when the host, the port or the base URL is invalid,
/// or when the database cannot be opened.
pub async fn run(host: &str, port: u16, database: &str, base_url: &str) -> Result<(), CliError> {
    let address: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|error| CliError::message(format!("invalid address {host}:{port}: {error}")))?;
    let pool = omeka_db::open_file(database)
        .await
        .map_err(|error| CliError::message(format!("cannot open {database}: {error}")))?;
    let base = BaseUrl::parse(base_url)
        .map_err(|error| CliError::message(format!("invalid base url {base_url}: {error}")))?;
    Ok(omeka_server::serve(address, pool, base).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn the_router_answers_a_request_on_port_zero() {
        let pool = omeka_db::open_memory().await.expect("open memory database");
        omeka_db::migrate(&pool).await.expect("run migrations");
        let base = BaseUrl::parse("http://localhost").expect("base url");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral port");
        let address = listener.local_addr().expect("local address");
        tokio::spawn(async move {
            let _ = axum::serve(listener, omeka_server::build_router(pool, base)).await;
        });

        let health = request(address, "/health").await;
        assert!(
            health.starts_with("HTTP/1.1 200"),
            "health status line: {health}"
        );
        assert!(
            health.contains(r#"{"status":"ok"}"#),
            "health body: {health}"
        );

        // The read endpoints are live after the apiwire unit. The API root lists
        // the registered resource types and needs no database rows.
        let api = request(address, "/api").await;
        assert!(api.starts_with("HTTP/1.1 200"), "api status line: {api}");
        assert!(api.contains("api_resources"), "api body: {api}");
    }

    async fn request(address: SocketAddr, path: &str) -> String {
        let mut stream = tokio::net::TcpStream::connect(address)
            .await
            .expect("connect");
        stream
            .write_all(
                format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await
            .expect("write request");
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .await
            .expect("read response");
        response
    }

    #[tokio::test]
    async fn run_rejects_an_invalid_host() {
        let error = run("not a host", 0, ":memory:", "http://localhost")
            .await
            .expect_err("invalid host");
        assert!(error.to_string().contains("invalid address"), "got {error}");
    }

    #[tokio::test]
    async fn run_rejects_an_invalid_base_url() {
        let error = run("127.0.0.1", 0, ":memory:", "localhost")
            .await
            .expect_err("invalid base url");
        assert!(
            error.to_string().contains("invalid base url"),
            "got {error}"
        );
    }
}
