//! Startup configuration: built-in defaults, then a file, then the environment.
//!
//! Every key has a default, so a deployment with no file and no variables
//! starts on [`DEFAULT_HOST`], [`DEFAULT_PORT`] and the rest below. The file is
//! JSON and is named by [`CONFIG_FILE_ENV`]; a key absent from the file keeps
//! its default, and a key absent from the environment keeps the file's value.
//! The environment names mirror the variables `packaging/entrypoint.sh` already
//! exports, so the container's settings reach the server unchanged.
//!
//! Precedence, lowest first: defaults, the file, the environment, then the
//! arguments `romeka serve` passes. The command line always passes a host, a
//! port and a base URL, so those three come from the command line in practice;
//! the log level and the limits come from the file and the environment.

use std::path::{Path, PathBuf};
use std::time::Duration;

use omeka_api::jsonld::BaseUrl;
use serde::Deserialize;
use thiserror::Error;
use tracing::level_filters::LevelFilter;

pub use crate::mail_config::{MailConfig, MailTls};

/// Interface the server binds when nothing overrides it.
pub const DEFAULT_HOST: &str = "127.0.0.1";
/// Port the server binds when nothing overrides it.
pub const DEFAULT_PORT: u16 = 8123;
/// Origin the JSON-LD `@id`s and generated links are built from.
pub const DEFAULT_BASE_URL: &str = "http://localhost:8123";
/// Log level name, one of `off`, `error`, `warn`, `info`, `debug`, `trace`.
pub const DEFAULT_LOG_LEVEL: &str = "info";
/// Largest accepted request body, 256 MiB. The container's nginx allows the same
/// (`packaging/nginx.conf.template` `client_max_body_size 256m`), so media
/// uploads hit the same wall inside and outside the container.
pub const DEFAULT_MAX_BODY_BYTES: usize = 256 * 1024 * 1024;
/// Seconds a request may run before the server answers 408.
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;

/// Variable naming the JSON config file. Unset means no file is read.
pub const CONFIG_FILE_ENV: &str = "ROMEKA_CONFIG";
/// Interface to bind.
pub const HOST_ENV: &str = "ROMEKA_API_HOST";
/// Port to bind.
pub const PORT_ENV: &str = "ROMEKA_API_PORT";
/// Origin for the JSON-LD `@id`s and generated links.
pub const BASE_URL_ENV: &str = "ROMEKA_BASE_URL";
/// Log level name.
pub const LOG_LEVEL_ENV: &str = "ROMEKA_LOG";
/// Largest accepted request body, in bytes.
pub const MAX_BODY_BYTES_ENV: &str = "ROMEKA_MAX_BODY_BYTES";
/// Request timeout, in seconds.
pub const REQUEST_TIMEOUT_ENV: &str = "ROMEKA_REQUEST_TIMEOUT_SECS";
/// SMTP relay host. Unset means no mail leaves the process.
pub const SMTP_HOST_ENV: &str = "ROMEKA_SMTP_HOST";
/// SMTP relay port. Unset means the port the TLS mode implies.
pub const SMTP_PORT_ENV: &str = "ROMEKA_SMTP_PORT";
/// SMTP user name.
pub const SMTP_USER_ENV: &str = "ROMEKA_SMTP_USER";
/// SMTP password.
pub const SMTP_PASSWORD_ENV: &str = "ROMEKA_SMTP_PASSWORD";
/// SMTP TLS mode: `starttls`, `tls` or `none`.
pub const SMTP_TLS_ENV: &str = "ROMEKA_SMTP_TLS";
/// Envelope sender of every message the server sends.
pub const MAIL_FROM_ENV: &str = "ROMEKA_MAIL_FROM";
/// The server folder the archive import reads. Unset or empty turns the import off.
pub const ARCHIVE_ROOT_ENV: &str = "ROMEKA_ARCHIVE_ROOT";

/// Everything the server needs to bind and serve.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub base_url: BaseUrl,
    pub log_level: LevelFilter,
    pub max_body_bytes: usize,
    pub request_timeout: Duration,
    pub mail: MailConfig,
    /// The archive import's root folder; `None` means the import is off.
    pub archive_root: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT,
            base_url: BaseUrl::parse(DEFAULT_BASE_URL)
                .unwrap_or_else(|_| unreachable!("the default base URL literal parses")),
            log_level: DEFAULT_LOG_LEVEL
                .parse()
                .unwrap_or_else(|_| unreachable!("the default log level parses")),
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            request_timeout: Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS),
            mail: MailConfig::default(),
            archive_root: None,
        }
    }
}

impl Config {
    /// The defaults, overlaid with the file named by [`CONFIG_FILE_ENV`] and
    /// then the environment.
    ///
    /// # Errors
    /// A [`ConfigError`] when the named file cannot be read or parsed, or when a
    /// file or environment value is not valid for its key.
    pub fn load() -> Result<Self, ConfigError> {
        let file = std::env::var_os(CONFIG_FILE_ENV).map(PathBuf::from);
        Self::from_sources(|key| std::env::var(key).ok(), file.as_deref())
    }

    /// Build a config from an explicit environment lookup and an optional file.
    ///
    /// The indirection keeps the process environment out of the tests, where
    /// setting it is unsafe.
    ///
    /// # Errors
    /// A [`ConfigError`] when the file cannot be read or parsed, or when a value
    /// is not valid for its key.
    pub fn from_sources(
        env: impl Fn(&str) -> Option<String>,
        file: Option<&Path>,
    ) -> Result<Self, ConfigError> {
        let mut config = Self::default();
        if let Some(path) = file {
            config.apply_file(&read_file(path)?)?;
        }
        config.apply_env(env)?;
        Ok(config)
    }

    /// The interface and port `romeka serve` was told to use override both the
    /// file and the environment.
    pub fn set_address(&mut self, addr: std::net::SocketAddr) {
        self.host = addr.ip().to_string();
        self.port = addr.port();
    }

    /// The base URL `romeka serve` was told to use overrides both the file and
    /// the environment.
    pub fn set_base_url(&mut self, base: BaseUrl) {
        self.base_url = base;
    }

    fn apply_file(&mut self, file: &FileConfig) -> Result<(), ConfigError> {
        if let Some(host) = &file.host {
            self.host = host.clone();
        }
        if let Some(port) = file.port {
            self.port = port;
        }
        if let Some(base_url) = &file.base_url {
            self.base_url = parse_base_url(BASE_URL_ENV, base_url)?;
        }
        if let Some(level) = &file.log_level {
            self.log_level = parse_level(LOG_LEVEL_ENV, level)?;
        }
        if let Some(bytes) = file.max_body_bytes {
            self.max_body_bytes = bytes;
        }
        if let Some(seconds) = file.request_timeout_secs {
            self.request_timeout = Duration::from_secs(seconds);
        }
        if let Some(host) = &file.smtp_host {
            self.mail.host = Some(host.clone());
        }
        if let Some(port) = file.smtp_port {
            self.mail.port = Some(port);
        }
        if let Some(user) = &file.smtp_user {
            self.mail.user = Some(user.clone());
        }
        if let Some(password) = &file.smtp_password {
            self.mail.password = Some(password.clone());
        }
        if let Some(tls) = &file.smtp_tls {
            self.mail.tls = parse_tls(SMTP_TLS_ENV, tls)?;
        }
        if let Some(from) = &file.mail_from {
            self.mail.from = from.clone();
        }
        if let Some(root) = &file.archive_root {
            self.archive_root = archive_root(root);
        }
        Ok(())
    }

    fn apply_env(&mut self, env: impl Fn(&str) -> Option<String>) -> Result<(), ConfigError> {
        if let Some(host) = env(HOST_ENV) {
            self.host = host;
        }
        if let Some(port) = env(PORT_ENV) {
            self.port = parse_port(PORT_ENV, &port)?;
        }
        if let Some(base_url) = env(BASE_URL_ENV) {
            self.base_url = parse_base_url(BASE_URL_ENV, &base_url)?;
        }
        if let Some(level) = env(LOG_LEVEL_ENV) {
            self.log_level = parse_level(LOG_LEVEL_ENV, &level)?;
        }
        if let Some(bytes) = env(MAX_BODY_BYTES_ENV) {
            self.max_body_bytes = parse_bytes(MAX_BODY_BYTES_ENV, &bytes)?;
        }
        if let Some(seconds) = env(REQUEST_TIMEOUT_ENV) {
            self.request_timeout =
                Duration::from_secs(parse_seconds(REQUEST_TIMEOUT_ENV, &seconds)?);
        }
        if let Some(host) = env(SMTP_HOST_ENV) {
            self.mail.host = Some(host);
        }
        if let Some(port) = env(SMTP_PORT_ENV) {
            self.mail.port = Some(parse_port(SMTP_PORT_ENV, &port)?);
        }
        if let Some(user) = env(SMTP_USER_ENV) {
            self.mail.user = Some(user);
        }
        if let Some(password) = env(SMTP_PASSWORD_ENV) {
            self.mail.password = Some(password);
        }
        if let Some(tls) = env(SMTP_TLS_ENV) {
            self.mail.tls = parse_tls(SMTP_TLS_ENV, &tls)?;
        }
        if let Some(from) = env(MAIL_FROM_ENV) {
            self.mail.from = from;
        }
        if let Some(root) = env(ARCHIVE_ROOT_ENV) {
            self.archive_root = archive_root(&root);
        }
        Ok(())
    }
}

/// A configuration that cannot be used.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot read config file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot parse config file {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid {key} value {value:?}: {reason}")]
    Invalid {
        key: String,
        value: String,
        reason: String,
    },
}

/// The file's keys, all optional so a partial file keeps the defaults.
///
/// `deny_unknown_fields` turns a typo into a startup failure instead of a
/// silently ignored setting.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    log_level: Option<String>,
    #[serde(default)]
    max_body_bytes: Option<usize>,
    #[serde(default)]
    request_timeout_secs: Option<u64>,
    #[serde(default)]
    smtp_host: Option<String>,
    #[serde(default)]
    smtp_port: Option<u16>,
    #[serde(default)]
    smtp_user: Option<String>,
    #[serde(default)]
    smtp_password: Option<String>,
    #[serde(default)]
    smtp_tls: Option<String>,
    #[serde(default)]
    mail_from: Option<String>,
    #[serde(default)]
    archive_root: Option<String>,
}

/// An empty value turns the import off, so the environment can override a
/// file that turns it on.
fn archive_root(value: &str) -> Option<PathBuf> {
    let value = value.trim();
    (!value.is_empty()).then(|| PathBuf::from(value))
}

fn read_file(path: &Path) -> Result<FileConfig, ConfigError> {
    let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

fn parse_port(key: &str, value: &str) -> Result<u16, ConfigError> {
    value.parse().map_err(|error| invalid(key, value, error))
}

fn parse_bytes(key: &str, value: &str) -> Result<usize, ConfigError> {
    value.parse().map_err(|error| invalid(key, value, error))
}

fn parse_seconds(key: &str, value: &str) -> Result<u64, ConfigError> {
    value.parse().map_err(|error| invalid(key, value, error))
}

fn parse_level(key: &str, value: &str) -> Result<LevelFilter, ConfigError> {
    value.parse().map_err(|error| invalid(key, value, error))
}

fn parse_base_url(key: &str, value: &str) -> Result<BaseUrl, ConfigError> {
    BaseUrl::parse(value).map_err(|error| invalid(key, value, error))
}

fn parse_tls(key: &str, value: &str) -> Result<MailTls, ConfigError> {
    value.parse().map_err(|error| invalid(key, value, error))
}

fn invalid(key: &str, value: &str, reason: impl std::fmt::Display) -> ConfigError {
    ConfigError::Invalid {
        key: key.to_string(),
        value: value.to_string(),
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::mail_config::DEFAULT_MAIL_FROM;

    fn no_env(_key: &str) -> Option<String> {
        None
    }

    #[test]
    fn defaults_are_the_documented_values() {
        let config = Config::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8123);
        assert_eq!(config.base_url.as_str(), "http://localhost:8123");
        assert_eq!(config.log_level, LevelFilter::INFO);
        assert_eq!(config.max_body_bytes, 268_435_456);
        assert_eq!(config.request_timeout, Duration::from_secs(30));
    }

    #[test]
    fn the_environment_overrides_every_key() {
        let config = Config::from_sources(
            |key| match key {
                HOST_ENV => Some("0.0.0.0".to_string()),
                PORT_ENV => Some("9000".to_string()),
                BASE_URL_ENV => Some("https://museum.example".to_string()),
                LOG_LEVEL_ENV => Some("debug".to_string()),
                MAX_BODY_BYTES_ENV => Some("1024".to_string()),
                REQUEST_TIMEOUT_ENV => Some("5".to_string()),
                _ => None,
            },
            None,
        )
        .unwrap();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 9000);
        assert_eq!(config.base_url.as_str(), "https://museum.example");
        assert_eq!(config.log_level, LevelFilter::DEBUG);
        assert_eq!(config.max_body_bytes, 1024);
        assert_eq!(config.request_timeout, Duration::from_secs(5));
    }

    #[test]
    fn a_file_supplies_the_values_the_environment_does_not() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            file.path(),
            r#"{"host":"0.0.0.0","port":9100,"log_level":"warn","max_body_bytes":64}"#,
        )
        .unwrap();
        let config = Config::from_sources(
            |key| (key == PORT_ENV).then(|| "9200".to_string()),
            Some(file.path()),
        )
        .unwrap();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 9200, "the environment wins over the file");
        assert_eq!(config.log_level, LevelFilter::WARN);
        assert_eq!(config.max_body_bytes, 64);
        assert_eq!(
            config.base_url.as_str(),
            DEFAULT_BASE_URL,
            "unset keys keep defaults"
        );
    }

    #[test]
    fn an_unknown_file_key_is_rejected() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), r#"{"prt":8123}"#).unwrap();
        let error = Config::from_sources(no_env, Some(file.path())).unwrap_err();
        assert!(
            matches!(error, ConfigError::Parse { .. }),
            "unknown keys fail the parse: {error}"
        );
    }

    #[test]
    fn a_missing_file_is_a_read_error() {
        let error =
            Config::from_sources(no_env, Some(Path::new("/nonexistent/romeka.json"))).unwrap_err();
        assert!(matches!(error, ConfigError::Read { .. }), "{error}");
    }

    #[test]
    fn a_bad_value_names_its_key() {
        let error = Config::from_sources(|key| (key == PORT_ENV).then(|| "http".to_string()), None)
            .unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains(PORT_ENV) && message.contains("http"),
            "{message}"
        );
    }

    #[test]
    fn an_unknown_log_level_is_rejected() {
        let error = Config::from_sources(
            |key| (key == LOG_LEVEL_ENV).then(|| "loud".to_string()),
            None,
        )
        .unwrap_err();
        assert!(error.to_string().contains(LOG_LEVEL_ENV), "{error}");
    }

    #[test]
    fn the_command_line_address_overrides_the_file_and_the_environment() {
        let mut config =
            Config::from_sources(|key| (key == HOST_ENV).then(|| "0.0.0.0".to_string()), None)
                .unwrap();
        config.set_address("10.0.0.7:4242".parse().unwrap());
        assert_eq!(config.host, "10.0.0.7");
        assert_eq!(config.port, 4242);
    }

    #[test]
    fn the_mail_keys_default_to_no_relay() {
        let config = Config::default();
        assert_eq!(config.mail.host, None);
        assert_eq!(config.mail.user, None);
        assert_eq!(config.mail.password, None);
        assert_eq!(config.mail.tls, MailTls::Starttls);
        assert_eq!(config.mail.port(), 587, "the STARTTLS submission port");
        assert_eq!(config.mail.from, DEFAULT_MAIL_FROM);
        assert_eq!(MailTls::Tls.default_port(), 465);
        assert_eq!(MailTls::None.default_port(), 25);
    }

    #[test]
    fn the_environment_sets_the_mail_keys() {
        let config = Config::from_sources(
            |key| match key {
                SMTP_HOST_ENV => Some("smtp.example.org".to_string()),
                SMTP_PORT_ENV => Some("2525".to_string()),
                SMTP_USER_ENV => Some("mailer".to_string()),
                SMTP_PASSWORD_ENV => Some("secret".to_string()),
                SMTP_TLS_ENV => Some("tls".to_string()),
                MAIL_FROM_ENV => Some("romeka@example.org".to_string()),
                _ => None,
            },
            None,
        )
        .unwrap();
        assert_eq!(config.mail.host.as_deref(), Some("smtp.example.org"));
        assert_eq!(config.mail.port, Some(2525));
        assert_eq!(config.mail.port(), 2525, "an explicit port wins");
        assert_eq!(config.mail.user.as_deref(), Some("mailer"));
        assert_eq!(config.mail.password.as_deref(), Some("secret"));
        assert_eq!(config.mail.tls, MailTls::Tls);
        assert_eq!(config.mail.from, "romeka@example.org");
    }

    #[test]
    fn a_file_sets_the_mail_keys_too() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            file.path(),
            r#"{"smtp_host":"relay.example.org","smtp_tls":"none","mail_from":"a@b.example"}"#,
        )
        .unwrap();
        let config = Config::from_sources(no_env, Some(file.path())).unwrap();
        assert_eq!(config.mail.host.as_deref(), Some("relay.example.org"));
        assert_eq!(config.mail.tls, MailTls::None);
        assert_eq!(config.mail.port(), 25, "none implies port 25");
        assert_eq!(config.mail.from, "a@b.example");
    }

    #[test]
    fn an_unknown_tls_mode_names_its_key() {
        let error =
            Config::from_sources(|key| (key == SMTP_TLS_ENV).then(|| "ssl".to_string()), None)
                .unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains(SMTP_TLS_ENV) && message.contains("ssl"),
            "{message}"
        );
    }

    #[test]
    fn the_archive_root_comes_from_the_file_and_an_empty_variable_turns_it_off() {
        assert_eq!(Config::default().archive_root, None);
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), r#"{"archive_root":"/srv/archive"}"#).unwrap();
        let on = Config::from_sources(no_env, Some(file.path())).unwrap();
        assert_eq!(on.archive_root, Some(PathBuf::from("/srv/archive")));
        let off = Config::from_sources(
            |key| (key == ARCHIVE_ROOT_ENV).then(String::new),
            Some(file.path()),
        )
        .unwrap();
        assert_eq!(off.archive_root, None);
    }
}
