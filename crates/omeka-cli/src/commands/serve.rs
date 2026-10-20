//! `romeka serve`: start the HTTP server.
//!
//! `omeka-server` owns the router and the socket. This command opens the SQLite
//! database, parses the base URL used in the JSON-LD links, roots the file
//! store, and hands the server a job runner that starts `romeka job run` with
//! the same database, store, base URL and archive root.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use omeka_api::jsonld::BaseUrl;
use omeka_db::ids::JobId;
use omeka_server::{Config, DispatchError, JobDispatch, Services};

use crate::error::CliError;
use crate::jobs::JobCommand;

/// Bind `host:port` and serve until the process stops. `store` is the file
/// store root; `files` in the working directory when `None`, as before.
///
/// # Errors
/// A [`CliError::Message`] when the host, the port, the base URL or the config
/// is invalid, or when the database cannot be opened.
pub async fn run(
    host: &str,
    port: u16,
    database: &str,
    base_url: &str,
    store: Option<&Path>,
) -> Result<(), CliError> {
    let address: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|error| CliError::message(format!("invalid address {host}:{port}: {error}")))?;
    let pool = omeka_db::open_file(database)
        .await
        .map_err(|error| CliError::message(format!("cannot open {database}: {error}")))?;
    let base = BaseUrl::parse(base_url)
        .map_err(|error| CliError::message(format!("invalid base url {base_url}: {error}")))?;
    let mut config = Config::load().map_err(|error| CliError::message(error.to_string()))?;
    config.set_address(address);
    config.set_base_url(base);
    let store_root = absolute(store.unwrap_or(Path::new(omeka_server::FILES_ROOT)))?;
    let jobs = ProcessDispatch(JobCommand {
        program: None,
        database: absolute(Path::new(database))?,
        store: Some(store_root.clone()),
        base_url: Some(base_url.to_string()),
        archive_root: config.archive_root.clone(),
    });
    let store = omeka_server::file_store(store_root)
        .map_err(|error| CliError::message(error.to_string()))?;
    let services = Services::new(store).with_jobs(Arc::new(jobs));
    Ok(omeka_server::serve_with_services(config, pool, services).await?)
}

/// A child started from another working directory reads the same files.
fn absolute(path: &Path) -> Result<PathBuf, CliError> {
    std::path::absolute(path)
        .map_err(|error| CliError::message(format!("cannot resolve {}: {error}", path.display())))
}

/// Starts each job in a new `romeka job run` process and reaps it when it
/// exits, so a long-lived server leaves no zombie processes.
struct ProcessDispatch(JobCommand);

impl JobDispatch for ProcessDispatch {
    fn start(&self, job: JobId) -> Result<(), DispatchError> {
        let mut child = self
            .0
            .spawn(job)
            .map_err(|error| DispatchError(error.to_string()))?;
        std::thread::spawn(move || child.wait());
        Ok(())
    }
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
        let error = run("not a host", 0, ":memory:", "http://localhost", None)
            .await
            .expect_err("invalid host");
        assert!(error.to_string().contains("invalid address"), "got {error}");
    }

    #[tokio::test]
    async fn run_rejects_an_invalid_base_url() {
        let error = run("127.0.0.1", 0, ":memory:", "localhost", None)
            .await
            .expect_err("invalid base url");
        assert!(
            error.to_string().contains("invalid base url"),
            "got {error}"
        );
    }
}
