//! The HTTP side of [`super::S3Store`]: one blocking ureq agent that sends a
//! presigned URL and hands back the status, the `ETag` and the body.
//!
//! A failure names the storage path, the S3 action and the status, plus the
//! `<Code>` of an S3 error document. It never carries the URL, whose query
//! holds the access key id and a live signature.

use std::time::Duration;

use url::Url;

use super::StoreError;

/// How long a request waits to connect.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a request waits for the answer's head, which for `PutObject` and
/// `UploadPart` comes after the whole body is sent.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// How long reading one answer's body may take: a whole original of a few GB
/// on a slow link.
const BODY_TIMEOUT: Duration = Duration::from_secs(60 * 60);

/// The user agent, as the fetch client sends its own.
const USER_AGENT: &str = concat!("romeka-s/", env!("CARGO_PKG_VERSION"));

/// One agent, so the connection pool lives as long as the store.
pub(super) struct Http {
    agent: ureq::Agent,
}

/// An answer, read to its end.
pub(super) struct Sent {
    pub(super) status: u16,
    pub(super) etag: Option<String>,
    pub(super) body: Vec<u8>,
}

impl Http {
    pub(super) fn new() -> Self {
        let config = ureq::config::Config::builder()
            // A 404 from `HeadObject` is an answer, not a failure.
            .http_status_as_error(false)
            // The endpoint is configured; a proxy from the environment is not.
            .proxy(None)
            .user_agent(USER_AGENT)
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_recv_response(Some(RESPONSE_TIMEOUT))
            .timeout_recv_body(Some(BODY_TIMEOUT))
            .build();
        Self {
            agent: config.into(),
        }
    }

    pub(super) fn put(
        &self,
        key: &str,
        action: &str,
        url: &Url,
        body: &[u8],
    ) -> Result<Sent, StoreError> {
        read(key, action, self.agent.put(url.as_str()).send(body))
    }

    pub(super) fn post(
        &self,
        key: &str,
        action: &str,
        url: &Url,
        body: &[u8],
    ) -> Result<Sent, StoreError> {
        read(key, action, self.agent.post(url.as_str()).send(body))
    }

    pub(super) fn get(
        &self,
        key: &str,
        url: &Url,
        range: Option<&str>,
    ) -> Result<Sent, StoreError> {
        let request = self.agent.get(url.as_str());
        let request = match range {
            Some(range) => request.header("range", range),
            None => request,
        };
        read(key, "GetObject", request.call())
    }

    pub(super) fn head(&self, key: &str, url: &Url) -> Result<Sent, StoreError> {
        read(key, "HeadObject", self.agent.head(url.as_str()).call())
    }

    pub(super) fn delete(&self, key: &str, action: &str, url: &Url) -> Result<Sent, StoreError> {
        read(key, action, self.agent.delete(url.as_str()).call())
    }
}

impl Sent {
    /// `Ok` when the status is one of `ok`, otherwise the failure.
    pub(super) fn expect(&self, key: &str, action: &str, ok: &[u16]) -> Result<(), StoreError> {
        if ok.contains(&self.status) {
            Ok(())
        } else {
            Err(self.failure(key, action))
        }
    }

    pub(super) fn failure(&self, key: &str, action: &str) -> StoreError {
        let code = self.error_code();
        let detail = if code.is_empty() {
            String::new()
        } else {
            format!(" ({code})")
        };
        StoreError::remote(key, format!("{action} answered {}{detail}", self.status))
    }

    pub(super) fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// The `<Code>` of an S3 error document, such as `AccessDenied` or
    /// `NoSuchBucket`; empty when the body has none.
    pub(super) fn error_code(&self) -> String {
        let text = self.body_text();
        text.split_once("<Code>")
            .and_then(|(_, rest)| rest.split_once("</Code>"))
            .map(|(code, _)| code.chars().take(64).collect())
            .unwrap_or_default()
    }
}

fn read(
    key: &str,
    action: &str,
    answer: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> Result<Sent, StoreError> {
    let mut response = answer
        .map_err(|error| StoreError::remote(key, format!("{action}: {}", error_kind(&error))))?;
    let status = response.status().as_u16();
    let etag = response
        .headers()
        .get("etag")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body = response
        .body_mut()
        .with_config()
        .limit(u64::MAX)
        .read_to_vec()
        .map_err(|error| {
            StoreError::remote(key, format!("{action} body: {}", error_kind(&error)))
        })?;
    Ok(Sent { status, etag, body })
}

/// A transport error without its URL.
fn error_kind(error: &ureq::Error) -> String {
    match error {
        ureq::Error::Io(io) => io.to_string(),
        ureq::Error::Timeout(timeout) => format!("timed out ({timeout})"),
        ureq::Error::HostNotFound => "host not found".to_string(),
        ureq::Error::ConnectionFailed => "connection failed".to_string(),
        ureq::Error::Tls(message) => format!("TLS: {message}"),
        _ => "request failed".to_string(),
    }
}
