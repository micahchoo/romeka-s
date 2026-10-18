//! `multipart/form-data` request bodies (`ApiController::processPostData`).
//!
//! Upstream reads a multipart POST through the PSR-7 request: the `data` form
//! field carries the JSON content (`$request->getPost('data')`) and the `file`
//! field carries the upload (`$request->getFiles()`). [`Multipart`] is the port
//! of that split: the raw body plus the request's `Content-Type` in, the `data`
//! bytes and an optional upload out.
//!
//! Only the framing the two callers need is parsed. There may be several
//! `data` or `file` parts; the last of each wins, as PSR-7's array assignment
//! does. A body with no boundary, or a part the parser cannot delimit, yields
//! no `data`, which the caller then refuses as invalid JSON exactly as
//! `jsonDecode(null)` does.

use crate::errors::ApiError;

/// The two fields a multipart asset create reads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Multipart {
    /// The bytes of the `data` field, absent when the body carries none.
    pub data: Option<Vec<u8>>,
    /// The upload from the `file` field, absent when the body carries none.
    pub file: Option<UploadedFile>,
}

/// One uploaded file: the client file name and the bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadedFile {
    /// The `filename` of the `Content-Disposition` header, absent when the
    /// header carries none.
    pub filename: Option<String>,
    /// The part body.
    pub bytes: Vec<u8>,
}

impl Multipart {
    /// Parse a multipart body. A malformed body is not an error of its own:
    /// it simply carries no `data`, so the JSON check downstream produces the
    /// upstream 400.
    #[must_use]
    pub fn parse(content_type: &str, body: &[u8]) -> Self {
        let Some(boundary) = boundary(content_type) else {
            return Self::default();
        };
        let mut parsed = Self::default();
        for part in parts(body, &boundary) {
            let Some(part) = Part::parse(part) else {
                continue;
            };
            match part.name.as_deref() {
                Some("data") => parsed.data = Some(part.body),
                Some("file") => {
                    parsed.file = Some(UploadedFile {
                        filename: part.filename,
                        bytes: part.body,
                    });
                }
                _ => {}
            }
        }
        parsed
    }

    /// Parse a multipart body and its `data` field as a request payload.
    pub fn payload(&self) -> Result<crate::write::body::Payload, ApiError> {
        crate::write::body::Payload::parse(self.data.as_deref().unwrap_or_default())
    }
}

/// The `boundary` parameter of a `multipart/form-data` content type.
fn boundary(content_type: &str) -> Option<String> {
    for parameter in content_type.split(';').skip(1) {
        let Some((name, value)) = parameter.split_once('=') else {
            continue;
        };
        if !name.trim().eq_ignore_ascii_case("boundary") {
            continue;
        }
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|inner| inner.strip_suffix('"'))
            .unwrap_or(value);
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

/// The body of each part, between the boundary delimiter lines.
fn parts<'a>(body: &'a [u8], boundary: &str) -> Vec<&'a [u8]> {
    let delimiter = format!("--{boundary}");
    let delimiter = delimiter.as_bytes();
    let separator = format!("\r\n--{boundary}");
    let separator = separator.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(offset) = find(&body[from..], delimiter) {
        let after = from + offset + delimiter.len();
        // A trailing `--` closes the body; there is no part after it.
        if body[after..].starts_with(b"--") {
            break;
        }
        let content_start = if body[after..].starts_with(b"\r\n") {
            after + 2
        } else if body[after..].starts_with(b"\n") {
            after + 1
        } else {
            after
        };
        let Some(end) = find(&body[content_start..], separator) else {
            break;
        };
        out.push(&body[content_start..content_start + end]);
        from = content_start + end + 2;
    }
    out
}

/// One part: its `Content-Disposition` name and filename, and its body.
struct Part {
    name: Option<String>,
    filename: Option<String>,
    body: Vec<u8>,
}

impl Part {
    fn parse(part: &[u8]) -> Option<Self> {
        let header_end = find(part, b"\r\n\r\n")?;
        let headers = std::str::from_utf8(&part[..header_end]).ok()?;
        let body = part[header_end + 4..].to_vec();
        let mut name = None;
        let mut filename = None;
        for line in headers.split("\r\n") {
            let Some(value) = line
                .split_once(':')
                .filter(|(field, _)| field.eq_ignore_ascii_case("content-disposition"))
                .map(|(_, value)| value)
            else {
                continue;
            };
            for parameter in value.split(';') {
                let Some((key, value)) = parameter.split_once('=') else {
                    continue;
                };
                let value = value.trim().trim_matches('"').to_string();
                match key.trim() {
                    "name" => name = Some(value),
                    "filename" => filename = Some(value),
                    _ => {}
                }
            }
        }
        Some(Self {
            name,
            filename,
            body,
        })
    }
}

/// The first index of `needle` in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    const CONTENT_TYPE: &str = "multipart/form-data; boundary=----Test";

    fn body(parts: &[(&str, &str)]) -> Vec<u8> {
        let mut out = Vec::new();
        for (headers, content) in parts {
            out.extend_from_slice(format!("------Test\r\n{headers}\r\n\r\n").as_bytes());
            out.extend_from_slice(content.as_bytes());
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(b"------Test--\r\n");
        out
    }

    /// The `data` and `file` fields split out of one body, with the upload's
    /// client file name.
    #[test]
    fn parses_the_data_and_file_fields() {
        let raw = body(&[
            (
                "Content-Disposition: form-data; name=\"data\"",
                r#"{"o:name":"logo.png"}"#,
            ),
            (
                "Content-Disposition: form-data; name=\"file\"; filename=\"logo.png\"\r\nContent-Type: image/png",
                "PNGBYTES",
            ),
        ]);
        let parsed = Multipart::parse(CONTENT_TYPE, &raw);
        assert_eq!(
            parsed.data.as_deref(),
            Some(br#"{"o:name":"logo.png"}"#.as_slice())
        );
        let file = parsed.file.as_ref().expect("file");
        assert_eq!(file.filename.as_deref(), Some("logo.png"));
        assert_eq!(file.bytes, b"PNGBYTES");
        assert_eq!(
            parsed.payload().expect("payload").str_member("o:name"),
            Some("logo.png")
        );
    }

    /// A quoted boundary and an LF-only line ending both parse.
    #[test]
    fn parses_a_quoted_boundary() {
        let raw =
            b"--XYZ\r\nContent-Disposition: form-data; name=\"data\"\r\n\r\n{}\r\n--XYZ--\r\n";
        let parsed = Multipart::parse("multipart/form-data; boundary=\"XYZ\"", raw);
        assert_eq!(parsed.data.as_deref(), Some(b"{}".as_slice()));
        assert_eq!(parsed.file, None);
    }

    /// A body with no boundary carries no `data`, so the caller sees the
    /// upstream JSON syntax error.
    #[test]
    fn a_body_without_a_boundary_has_no_data() {
        let parsed = Multipart::parse("multipart/form-data", b"anything");
        assert_eq!(parsed, Multipart::default());
        assert!(parsed.payload().is_err());
    }
}
