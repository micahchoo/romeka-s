//! The tus request parts the upload routes read: the `Upload-Metadata`
//! header, and a request body as a blocking reader for the store.

use std::io::{self, Read};

use axum::body::{Body, Bytes};
use http_body_util::BodyExt;

/// The value of `key` in `Upload-Metadata`: comma-separated pairs of a key
/// and its value in base64.
pub(super) fn metadata_value(metadata: &str, key: &str) -> Option<String> {
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

/// A request body read as a blocking [`Read`]: a task forwards its frames,
/// and the reader waits for them on the blocking pool.
pub(super) struct BodyReader {
    frames: tokio::sync::mpsc::Receiver<io::Result<Bytes>>,
    held: Bytes,
}

impl BodyReader {
    pub(super) fn spawn(body: Body) -> Self {
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
        }
    }
}

impl Read for BodyReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        while self.held.is_empty() {
            match self.frames.blocking_recv() {
                Some(Ok(data)) => self.held = data,
                Some(Err(error)) => return Err(error),
                None => return Ok(0),
            }
        }
        let count = buffer.len().min(self.held.len());
        buffer[..count].copy_from_slice(&self.held.split_to(count));
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::{base64_decode, metadata_value};

    #[test]
    fn upload_metadata_names_the_file_in_base64() {
        assert_eq!(
            base64_decode("Qm94IDEvMDAwMS50aWY="),
            Some(b"Box 1/0001.tif".to_vec())
        );
        assert_eq!(
            base64_decode("Qm94IDEvMDAwMS50aWY"),
            Some(b"Box 1/0001.tif".to_vec())
        );
        assert_eq!(base64_decode("not base64!"), None);
        assert_eq!(
            metadata_value(
                "filetype aW1hZ2UvdGlmZg==,filename QcOxby5qcGc=",
                "filename"
            )
            .as_deref(),
            Some("Año.jpg")
        );
        assert_eq!(
            metadata_value("filetype aW1hZ2UvdGlmZg==", "filename"),
            None
        );
    }
}
