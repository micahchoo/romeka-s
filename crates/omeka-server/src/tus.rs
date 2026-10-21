//! The parts of tus 1.0 that both upload route families read: the protocol
//! header and version, the request's numeric headers, and the lock that keeps
//! two requests for one upload apart.
//!
//! The protocol subset is Import's (`crates/omeka-server/src/imports/uploads.rs`,
//! `docs/import-v2.md` "Arrival"): `OPTIONS` names the protocol and the size
//! limit, `POST` creates from `Upload-Length` and `Upload-Metadata`, `HEAD`
//! answers the offset, `PATCH` appends at `Upload-Offset`, and `DELETE` removes
//! the upload. Each route family builds its own answers and its own refusal
//! document; this module holds only what must not differ between them.

use std::collections::HashMap;
use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex, PoisonError};

use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, header};
use http_body_util::BodyExt;

/// The protocol version both families speak.
pub(crate) const VERSION: &str = "1.0.0";

/// The only `Content-Type` a `PATCH` body may carry.
pub(crate) const OFFSET_TYPE: &str = "application/offset+octet-stream";

/// Add the `Tus-Resumable` header every answer carries.
pub(crate) fn header(builder: axum::http::response::Builder) -> axum::http::response::Builder {
    builder.header("tus-resumable", VERSION)
}

/// Whether the request names version 1.0.0; a request that does not is a 412.
pub(crate) fn speaks(headers: &HeaderMap) -> bool {
    headers
        .get("tus-resumable")
        .and_then(|value| value.to_str().ok())
        == Some(VERSION)
}

/// A `PATCH` body's `Content-Type` is the offset stream.
pub(crate) fn is_offset_type(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(';').next().unwrap_or_default().trim())
        == Some(OFFSET_TYPE)
}

/// A non-negative integer header, or `None` when it is absent, empty or not
/// all digits.
pub(crate) fn number(headers: &HeaderMap, name: &str) -> Option<u64> {
    let text = headers.get(name)?.to_str().ok()?.trim();
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// The value of `key` in `Upload-Metadata`: comma-separated pairs of a key and
/// its value in base64.
pub(crate) fn metadata_value(metadata: &str, key: &str) -> Option<String> {
    metadata.split(',').find_map(|pair| {
        let mut parts = pair.trim().splitn(2, ' ');
        if parts.next()? != key {
            return None;
        }
        String::from_utf8(base64_decode(parts.next()?.trim())?).ok()
    })
}

/// Standard base64, padded or not.
fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut buffer = 0_u32;
    let mut bits = 0;
    for byte in text.bytes().filter(|byte| *byte != b'=') {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((buffer >> bits) & 0xFF).ok()?);
        }
    }
    Some(out)
}

/// A request body read as a blocking [`Read`]: a task forwards its frames and
/// the reader waits for them on the blocking pool. It fails once the body
/// passes `cap` bytes and says so in the flag [`BodyReader::over`] returns, so
/// the caller can tell that failure from a dropped connection.
pub(crate) struct BodyReader {
    frames: tokio::sync::mpsc::Receiver<io::Result<Bytes>>,
    held: Bytes,
    cap: u64,
    taken: u64,
    over: Arc<AtomicBool>,
}

impl BodyReader {
    pub(crate) fn over(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.over)
    }

    pub(crate) fn spawn(body: Body, cap: u64) -> Self {
        let (sender, frames) = tokio::sync::mpsc::channel(4);
        tokio::spawn(async move {
            let mut body = body;
            while let Some(frame) = body.frame().await {
                let sent = match frame {
                    Ok(frame) => match frame.into_data() {
                        Ok(data) => sender.send(Ok(data)).await,
                        Err(_) => continue,
                    },
                    Err(error) => sender.send(Err(io::Error::other(error))).await,
                };
                if sent.is_err() {
                    return;
                }
            }
        });
        Self {
            frames,
            held: Bytes::new(),
            cap,
            taken: 0,
            over: Arc::default(),
        }
    }
}

impl Read for BodyReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        while self.held.is_empty() {
            match self.frames.blocking_recv() {
                Some(Ok(data)) => {
                    self.taken += data.len() as u64;
                    if self.taken > self.cap {
                        self.over.store(true, Ordering::Relaxed);
                        return Err(io::Error::other("past the chunk limit"));
                    }
                    self.held = data;
                }
                Some(Err(error)) => return Err(error),
                None => return Ok(0),
            }
        }
        let count = buffer.len().min(self.held.len());
        buffer[..count].copy_from_slice(&self.held.split_to(count));
        Ok(count)
    }
}

/// The locks that keep two requests for one upload apart. An entry goes when
/// its upload completes or is removed.
static LOCKS: LazyLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The lock of one upload, shared by every request that names it. `key` names
/// the route family before the upload, as `import/{importId}/{upload}` or
/// `user/{userId}/{upload}`, so an import's upload and a user's upload that
/// happen to share an id never share a lock.
pub(crate) fn lock_of(key: &str) -> Arc<tokio::sync::Mutex<()>> {
    let mut locks = LOCKS.lock().unwrap_or_else(PoisonError::into_inner);
    Arc::clone(locks.entry(key.to_string()).or_default())
}

pub(crate) fn forget_lock(key: &str) {
    LOCKS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(key);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn the_protocol_headers_are_read_as_the_tus_core_does() {
        let mut headers = HeaderMap::new();
        headers.insert("tus-resumable", "1.0.0".parse().unwrap());
        headers.insert("upload-offset", "12".parse().unwrap());
        assert!(speaks(&headers));
        assert_eq!(number(&headers, "upload-offset"), Some(12));
        headers.insert("upload-offset", "-1".parse().unwrap());
        assert_eq!(number(&headers, "upload-offset"), None);
        assert!(!speaks(&HeaderMap::new()));
    }

    #[tokio::test]
    async fn a_body_reader_fails_past_its_cap() {
        let mut reader = BodyReader::spawn(Body::from(vec![b'x'; 8]), 4);
        let over = reader.over();
        let failed = tokio::task::spawn_blocking(move || {
            let mut out = Vec::new();
            reader.read_to_end(&mut out).is_err()
        })
        .await
        .unwrap();
        assert!(failed);
        assert!(over.load(Ordering::Relaxed));
    }

    #[test]
    fn upload_metadata_names_the_file_in_base64() {
        assert_eq!(
            metadata_value(
                "filetype aW1hZ2UvanBlZw==,filename QcOxby5qcGc=",
                "filename"
            )
            .as_deref(),
            Some("Año.jpg")
        );
        assert_eq!(
            metadata_value("filetype aW1hZ2UvanBlZw==", "filename"),
            None
        );
    }
}
