//! Which store holds the files: the local folder, or a bucket on an
//! S3-compatible service, from `ROMEKA_STORE_BACKEND` and `ROMEKA_S3_*`.
//!
//! The switch is not `ROMEKA_STORE`: the deployment already uses that name for
//! the local store's root path (`deploy/compose.yaml`, the entrypoint's
//! `serve --store`, the backup scripts and Caddy's `/files` root).
//!
//! Only the environment sets these, not the config file: the secret key
//! belongs with the other secrets in the deployment's `.env`. Every process
//! reads the same variables, so `romeka serve`, the `romeka job run` children
//! it starts and the file commands all reach the same store.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use omeka_media::store::{DEFAULT_URL_TTL, FileStore, S3Config, S3Store, Store};

use crate::FILES_URI;
use crate::config::ConfigError;

/// `local` (the default) or `s3`.
pub const STORE_ENV: &str = "ROMEKA_STORE_BACKEND";
/// The S3 service endpoint, such as `https://s3.eu-west-1.amazonaws.com`.
pub const S3_ENDPOINT_ENV: &str = "ROMEKA_S3_ENDPOINT";
/// The bucket.
pub const S3_BUCKET_ENV: &str = "ROMEKA_S3_BUCKET";
/// The region the signatures name. Defaults to `us-east-1`, which MinIO
/// accepts.
pub const S3_REGION_ENV: &str = "ROMEKA_S3_REGION";
/// The access key id.
pub const S3_ACCESS_KEY_ENV: &str = "ROMEKA_S3_ACCESS_KEY";
/// The secret key.
pub const S3_SECRET_KEY_ENV: &str = "ROMEKA_S3_SECRET_KEY";
/// Where plain URLs point instead of the bucket: a CDN or a public bucket
/// domain. Optional.
pub const S3_PUBLIC_BASE_ENV: &str = "ROMEKA_S3_PUBLIC_BASE";
/// Seconds a signed file URL stays valid. Defaults to 900.
pub const S3_URL_TTL_ENV: &str = "ROMEKA_S3_URL_TTL_SECS";

/// The region when none is set.
pub const DEFAULT_S3_REGION: &str = "us-east-1";

/// The longest signed URL SigV4 allows: seven days.
const MAX_URL_TTL_SECS: u64 = 7 * 24 * 60 * 60;

/// Where the process keeps its files.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StoreConfig {
    /// The local folder the caller names, served under `/files`.
    #[default]
    Local,
    /// A bucket.
    S3(Box<S3Config>),
}

impl StoreConfig {
    /// Read the store settings from `env`.
    ///
    /// # Errors
    /// A [`ConfigError::Invalid`] naming the variable when `ROMEKA_STORE_BACKEND` is
    /// neither `local` nor `s3`, when `s3` lacks a required variable, or when a
    /// value does not parse. A message never repeats the secret key.
    pub fn from_env(env: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let kind = env(STORE_ENV).unwrap_or_default();
        match kind.trim().to_ascii_lowercase().as_str() {
            "" | "local" => Ok(Self::Local),
            "s3" => s3_config(&env).map(|config| Self::S3(Box::new(config))),
            _ => Err(invalid(STORE_ENV, &kind, "expected local or s3")),
        }
    }
}

/// The process's store, as `ROMEKA_STORE_BACKEND` chooses it: the local folder at
/// `root`, or the bucket `ROMEKA_S3_*` names. This is the one place a backend
/// is chosen; every caller holds the trait object.
///
/// # Errors
/// A [`ConfigError`] naming the variable when the store settings are invalid.
pub fn file_store(root: impl Into<PathBuf>) -> Result<Arc<dyn Store>, ConfigError> {
    store_for(&StoreConfig::from_env(|key| std::env::var(key).ok())?, root)
}

/// The store `config` names; `root` is the local folder's.
///
/// # Errors
/// A [`ConfigError`] when the S3 endpoint or public base is not a usable URL.
pub fn store_for(
    config: &StoreConfig,
    root: impl Into<PathBuf>,
) -> Result<Arc<dyn Store>, ConfigError> {
    Ok(match config {
        StoreConfig::Local => local_store(root),
        StoreConfig::S3(s3) => {
            Arc::new(
                S3Store::new(S3Config::clone(s3)).map_err(|error| ConfigError::Invalid {
                    key: "ROMEKA_S3_ENDPOINT".to_string(),
                    value: String::new(),
                    reason: error.to_string(),
                })?,
            )
        }
    })
}

/// The local store at `root`, whatever the environment says: for tests and
/// for a router built without a configured store.
#[must_use]
pub fn local_store(root: impl Into<PathBuf>) -> Arc<dyn Store> {
    Arc::new(FileStore::new(root, FILES_URI))
}

fn s3_config(env: &impl Fn(&str) -> Option<String>) -> Result<S3Config, ConfigError> {
    let required = |key: &str| {
        env(key)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| invalid(key, "", "required when ROMEKA_STORE_BACKEND=s3"))
    };
    let endpoint_value = required(S3_ENDPOINT_ENV)?;
    let endpoint = endpoint_value
        .parse()
        .map_err(|error| invalid(S3_ENDPOINT_ENV, &endpoint_value, error))?;
    let secret_key = env(S3_SECRET_KEY_ENV)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            invalid(
                S3_SECRET_KEY_ENV,
                "",
                "required when ROMEKA_STORE_BACKEND=s3",
            )
        })?;
    // compose.yaml passes every variable, empty when unset.
    let url_ttl = match env(S3_URL_TTL_ENV).filter(|value| !value.trim().is_empty()) {
        None => DEFAULT_URL_TTL,
        Some(value) => {
            let seconds: u64 = value
                .trim()
                .parse()
                .map_err(|error| invalid(S3_URL_TTL_ENV, &value, error))?;
            if seconds == 0 || seconds > MAX_URL_TTL_SECS {
                return Err(invalid(
                    S3_URL_TTL_ENV,
                    &value,
                    "expected 1 to 604800 seconds",
                ));
            }
            Duration::from_secs(seconds)
        }
    };
    let config = S3Config {
        endpoint,
        bucket: required(S3_BUCKET_ENV)?,
        region: env(S3_REGION_ENV)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_S3_REGION.to_string()),
        access_key: required(S3_ACCESS_KEY_ENV)?,
        secret_key,
        public_base: env(S3_PUBLIC_BASE_ENV)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        url_ttl,
    };
    Ok(config)
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
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn s3_env(extra: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |key| {
            let base = [
                (STORE_ENV, "s3"),
                (S3_ENDPOINT_ENV, "http://minio:9000"),
                (S3_BUCKET_ENV, "romeka"),
                (S3_ACCESS_KEY_ENV, "AKIDEXAMPLE"),
                (S3_SECRET_KEY_ENV, "very-secret-value"),
            ];
            extra
                .iter()
                .chain(base.iter())
                .find(|(name, _)| *name == key)
                .map(|(_, value)| (*value).to_string())
        }
    }

    #[test]
    fn no_variable_means_the_local_store() {
        assert_eq!(StoreConfig::from_env(|_| None).unwrap(), StoreConfig::Local);
        let local = StoreConfig::from_env(|key| (key == STORE_ENV).then(|| "local".into()));
        assert_eq!(local.unwrap(), StoreConfig::Local);
    }

    #[test]
    fn s3_reads_every_setting_with_the_documented_defaults() {
        let StoreConfig::S3(config) = StoreConfig::from_env(s3_env(&[])).unwrap() else {
            panic!("s3");
        };
        assert_eq!(config.endpoint.as_str(), "http://minio:9000/");
        assert_eq!(config.bucket, "romeka");
        assert_eq!(config.region, "us-east-1");
        assert_eq!(config.access_key, "AKIDEXAMPLE");
        assert_eq!(config.secret_key, "very-secret-value");
        assert_eq!(config.public_base, None);
        assert_eq!(config.url_ttl, Duration::from_secs(4 * 60 * 60));

        let StoreConfig::S3(config) = StoreConfig::from_env(s3_env(&[
            (S3_REGION_ENV, "eu-west-1"),
            (S3_PUBLIC_BASE_ENV, "https://cdn.example.org"),
            (S3_URL_TTL_ENV, "300"),
        ]))
        .unwrap() else {
            panic!("s3");
        };
        assert_eq!(config.region, "eu-west-1");
        assert_eq!(
            config.public_base.as_deref(),
            Some("https://cdn.example.org")
        );
        assert_eq!(config.url_ttl, Duration::from_secs(300));
    }

    #[test]
    fn an_empty_optional_setting_is_the_default_as_compose_passes_it() {
        let StoreConfig::S3(config) = StoreConfig::from_env(s3_env(&[
            (S3_REGION_ENV, ""),
            (S3_PUBLIC_BASE_ENV, ""),
            (S3_URL_TTL_ENV, ""),
        ]))
        .unwrap() else {
            panic!("s3");
        };
        assert_eq!(config.region, "us-east-1");
        assert_eq!(config.public_base, None);
        assert_eq!(config.url_ttl, Duration::from_secs(4 * 60 * 60));
    }

    #[test]
    fn a_missing_or_bad_setting_names_its_variable_and_never_the_secret() {
        for (env, key) in [
            (s3_env(&[(S3_BUCKET_ENV, "")]), S3_BUCKET_ENV),
            (s3_env(&[(S3_ENDPOINT_ENV, "not a url")]), S3_ENDPOINT_ENV),
            (s3_env(&[(S3_URL_TTL_ENV, "0")]), S3_URL_TTL_ENV),
            (s3_env(&[(S3_URL_TTL_ENV, "604801")]), S3_URL_TTL_ENV),
            (s3_env(&[(STORE_ENV, "gcs")]), STORE_ENV),
        ] {
            let message = StoreConfig::from_env(env).unwrap_err().to_string();
            assert!(message.contains(key), "{message}");
            assert!(!message.contains("very-secret-value"), "{message}");
        }
        let debug = format!("{:?}", StoreConfig::from_env(s3_env(&[])).unwrap());
        assert!(!debug.contains("very-secret-value"), "{debug}");
    }
}
