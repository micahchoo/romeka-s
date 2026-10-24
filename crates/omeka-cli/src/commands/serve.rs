//! `romeka serve`: start the HTTP server.
//!
//! `omeka-server` owns the router and the socket. This command opens the
//! database (a SQLite file or a MySQL server), parses the base URL used in the
//! JSON-LD links, roots the file store, and hands the server a job runner that
//! starts `romeka job run` with the same database, store, base URL and archive
//! root. It also starts the
//! import staging sweep when it boots and then every hour, so uploaded files
//! go even when nobody uploads, and goes on with every import run a restart
//! killed ([`resume_imports`]). When a signal stops it, it sends each job it
//! started a TERM and waits for them ([`ProcessDispatch::shutdown`]): an
//! import run ends after the record in hand, `interrupted`.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use omeka_api::import::sweep::SWEEP_JOB;
use omeka_api::jsonld::BaseUrl;
use omeka_db::DbPool;
use omeka_db::Location;
use omeka_db::repos::HEARTBEAT_EVERY;
use omeka_db::repos::JobRepo;
use omeka_db::rows::NewJob;
use omeka_db::with_db;
use omeka_server::{Config, JobDispatch, ResumeEnv, Services, resume_imports};

use crate::error::CliError;
use crate::jobs::{JobCommand, ProcessDispatch};

/// How often the server starts the staging sweep.
const SWEEP_EVERY: Duration = Duration::from_secs(60 * 60);

/// How long a stopping server waits for its jobs to end after their TERM
/// before it kills them: inside `deploy/compose.yaml`'s `stop_grace_period`
/// (60 s), so the server, not the container runtime, ends them.
const JOB_GRACE: Duration = Duration::from_secs(50);

/// Bind `host:port` and serve until the process stops. `store` is the file
/// store root; `files` in the working directory when `None`, as before.
///
/// # Errors
/// A [`CliError::Message`] when the host, the port, the base URL or the config
/// is invalid, or when the database cannot be opened.
pub async fn run(
    host: &str,
    port: u16,
    database: &Location,
    base_url: &str,
    store: Option<&Path>,
) -> Result<(), CliError> {
    let address: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|error| CliError::message(format!("invalid address {host}:{port}: {error}")))?;
    let pool = database
        .open()
        .await
        .map_err(|error| CliError::message(format!("cannot open {database}: {error}")))?;
    let base = BaseUrl::parse(base_url)
        .map_err(|error| CliError::message(format!("invalid base url {base_url}: {error}")))?;
    let mut config = Config::load().map_err(|error| CliError::message(error.to_string()))?;
    config.set_address(address);
    config.set_base_url(base);
    let store_root = absolute(store.unwrap_or(Path::new(omeka_server::FILES_ROOT)))?;
    let dispatch = Arc::new(ProcessDispatch::new(JobCommand {
        program: None,
        database: match database {
            Location::File(path) => Location::File(absolute(path)?),
            Location::Url(_) => database.clone(),
        },
        store: Some(store_root.clone()),
        base_url: Some(base_url.to_string()),
        archive_root: config.archive_root.clone(),
    }));
    let jobs: Arc<dyn JobDispatch> = dispatch.clone();
    tokio::spawn(sweep_every(pool.clone(), Arc::clone(&jobs), SWEEP_EVERY));
    let store = omeka_server::file_store(store_root)
        .map_err(|error| CliError::message(error.to_string()))?;
    let resume = ResumeEnv {
        jobs: Arc::clone(&jobs),
        store: Arc::clone(&store),
        roots: config.archive_roots.clone(),
    };
    let resuming = pool.clone();
    tokio::spawn(async move { resume_imports(&resuming, &resume, HEARTBEAT_EVERY).await });
    let services = Services::new(store).with_jobs(jobs);
    let closing = pool.clone();
    let served = omeka_server::serve_with_services(config, pool, services).await;
    let waiting = tokio::task::spawn_blocking(move || dispatch.shutdown(JOB_GRACE))
        .await
        .unwrap_or_default();
    // A job that waited for a slot never ran: it ends `error` rather than stay `starting`.
    for job in waiting {
        let _ = with_db!(&closing, |db| JobRepo::new(db)
            .append_log(job, "the server stopped before the job started")
            .await);
        let _ = with_db!(&closing, |db| JobRepo::new(db).finish(job, "error").await);
    }
    closing.close().await;
    Ok(served?)
}

/// Start the staging sweep job now and then every `period`. The period is a
/// parameter so a test shortens the hour. A tick that fails is dropped: the
/// next one tries again, and the sweep is safe to repeat.
async fn sweep_every(pool: DbPool, jobs: Arc<dyn JobDispatch>, period: Duration) {
    let mut ticks = tokio::time::interval(period);
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticks.tick().await;
        let created = with_db!(&pool, |db| JobRepo::new(db)
            .create(&NewJob {
                owner_id: None,
                pid: None,
                status: Some("starting".to_string()),
                class: SWEEP_JOB.to_string(),
                args: Some(serde_json::json!({})),
                log: None,
                started: None,
            })
            .await);
        let Ok(job) = created else { continue };
        if let Err(error) = jobs.start(job.id) {
            let _ = with_db!(&pool, |db| JobRepo::new(db)
                .append_log(job.id, &error.to_string())
                .await);
            let _ = with_db!(&pool, |db| JobRepo::new(db).finish(job.id, "error").await);
        }
    }
}

/// A child started from another working directory reads the same files.
pub(crate) fn absolute(path: &Path) -> Result<PathBuf, CliError> {
    std::path::absolute(path)
        .map_err(|error| CliError::message(format!("cannot resolve {}: {error}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use omeka_db::ids::JobId;
    use omeka_server::DispatchError;
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

    struct Recorder(std::sync::Mutex<Vec<JobId>>);

    impl JobDispatch for Recorder {
        fn start(&self, job: JobId) -> Result<(), DispatchError> {
            self.0.lock().expect("lock").push(job);
            Ok(())
        }
    }

    #[tokio::test]
    async fn the_sweep_starts_at_boot_and_then_on_every_period() {
        let pool = omeka_db::open_memory().await.expect("open memory database");
        omeka_db::migrate(&pool).await.expect("run migrations");
        let recorder = Arc::new(Recorder(std::sync::Mutex::default()));
        let task = tokio::spawn(sweep_every(
            pool.clone().into(),
            Arc::clone(&recorder) as Arc<dyn JobDispatch>,
            Duration::from_millis(20),
        ));
        tokio::time::sleep(Duration::from_millis(150)).await;
        task.abort();

        let started = recorder.0.lock().expect("lock").clone();
        assert!(started.len() >= 3, "{} sweeps started", started.len());
        let job = JobRepo::new(&pool)
            .get(started[0])
            .await
            .expect("read job")
            .expect("job row");
        assert_eq!(job.class, SWEEP_JOB);
    }

    #[tokio::test]
    async fn run_rejects_an_invalid_host() {
        let error = run(
            "not a host",
            0,
            &Location::parse(":memory:"),
            "http://localhost",
            None,
        )
        .await
        .expect_err("invalid host");
        assert!(error.to_string().contains("invalid address"), "got {error}");
    }

    #[tokio::test]
    async fn run_rejects_an_invalid_base_url() {
        let error = run(
            "127.0.0.1",
            0,
            &Location::parse(":memory:"),
            "localhost",
            None,
        )
        .await
        .expect_err("invalid base url");
        assert!(
            error.to_string().contains("invalid base url"),
            "got {error}"
        );
    }
}
