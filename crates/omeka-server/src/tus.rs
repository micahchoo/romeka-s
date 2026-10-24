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
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

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

/// How long a body may send nothing before its reader fails. Without it a
/// client that stalls mid-chunk holds a blocking thread and its upload's lock
/// for as long as the connection stays open.
const FRAME_IDLE: Duration = Duration::from_secs(30);

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
        Self::spawn_idle(body, cap, FRAME_IDLE)
    }

    fn spawn_idle(body: Body, cap: u64, idle: Duration) -> Self {
        let (sender, frames) = tokio::sync::mpsc::channel(4);
        tokio::spawn(async move {
            let mut body = body;
            loop {
                let Ok(frame) = tokio::time::timeout(idle, body.frame()).await else {
                    let _ = sender
                        .send(Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "the client sent nothing",
                        )))
                        .await;
                    return;
                };
                let Some(frame) = frame else { return };
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

/// The locks that keep two requests for one upload apart. An entry goes once no
/// request holds or waits for it, so an upload that is abandoned, swept or
/// expired leaves nothing behind.
static LOCKS: LazyLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn locks() -> MutexGuard<'static, HashMap<String, Arc<tokio::sync::Mutex<()>>>> {
    LOCKS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The lock of one upload, held until dropped. `key` names the route family
/// before the upload, as `import/{importId}/{upload}` or `user/{userId}/{upload}`,
/// so an import's upload and a user's upload that happen to share an id never
/// share a lock.
struct Held {
    key: String,
    guard: Option<tokio::sync::OwnedMutexGuard<()>>,
}

impl Held {
    async fn take(key: String) -> Self {
        let lock = {
            let mut locks = locks();
            // Every clone is made under this mutex, so a count of one is the map's
            // own: no request holds the entry or waits for it. This also clears an
            // entry whose last waiter was cancelled before it got the lock.
            locks.retain(|_, lock| Arc::strong_count(lock) > 1);
            Arc::clone(locks.entry(key.clone()).or_default())
        };
        let guard = lock.lock_owned().await;
        Self {
            key,
            guard: Some(guard),
        }
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        drop(self.guard.take());
        let mut locks = locks();
        if locks
            .get(&self.key)
            .is_some_and(|lock| Arc::strong_count(lock) == 1)
        {
            locks.remove(&self.key);
        }
    }
}

/// Run `work` under the lock of upload `key`, in a task of its own. A request
/// that ends early (the request timeout, a dropped connection) then neither
/// stops a write half way nor lets the next request for the upload in before
/// the write and its offset update end. `None` when the task panicked.
pub(crate) async fn locked<F>(key: String, work: F) -> Option<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    tokio::spawn(async move {
        let _held = Held::take(key).await;
        work.await
    })
    .await
    .ok()
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

    fn entries(key: &str) -> usize {
        locks().keys().filter(|held| held.as_str() == key).count()
    }

    #[tokio::test]
    async fn an_upload_lock_goes_once_no_request_holds_it() {
        let key = "test/lock-goes".to_string();
        assert_eq!(locked(key.clone(), async { 7 }).await, Some(7));
        assert_eq!(entries(&key), 0);
    }

    #[tokio::test]
    async fn a_timed_out_request_keeps_the_lock_until_its_work_ends() {
        let key = "test/lock-outlives".to_string();
        let (release, released) = tokio::sync::oneshot::channel::<()>();
        let first = tokio::time::timeout(
            std::time::Duration::from_millis(20),
            locked(key.clone(), async move {
                let _ = released.await;
                "first"
            }),
        )
        .await;
        assert!(first.is_err(), "the request gave up before its work ended");
        assert_eq!(entries(&key), 1, "the abandoned work still holds the lock");
        let second = tokio::spawn(locked(key.clone(), async { "second" }));
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        assert!(!second.is_finished(), "the next request waits for the work");
        release.send(()).unwrap();
        assert_eq!(second.await.unwrap(), Some("second"));
        assert_eq!(entries(&key), 0);
    }

    #[tokio::test]
    async fn a_body_that_stalls_fails_its_reader() {
        let (sender, body) = mpsc_body();
        sender.send(Bytes::from_static(b"part")).await.unwrap();
        let mut reader = BodyReader::spawn_idle(body, 1024, Duration::from_millis(50));
        let failed = tokio::task::spawn_blocking(move || {
            let mut out = Vec::new();
            let read = reader.read_to_end(&mut out).map_err(|error| error.kind());
            (out, read)
        });
        let (out, read) = failed.await.unwrap();
        assert_eq!(out, b"part");
        assert_eq!(read, Err(io::ErrorKind::TimedOut));
        drop(sender);
    }

    /// A body that yields nothing until its sender sends or drops.
    fn mpsc_body() -> (tokio::sync::mpsc::Sender<Bytes>, Body) {
        let (sender, receiver) = tokio::sync::mpsc::channel::<Bytes>(1);
        let stream = futures_util::stream::unfold(receiver, |mut receiver| async move {
            let data = receiver.recv().await?;
            Some((Ok::<_, io::Error>(data), receiver))
        });
        (sender, Body::from_stream(stream))
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
