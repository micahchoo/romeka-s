//! The one `multipart/form-data` shape the import routes take: a form with
//! exactly one file part. Other parts (plain fields) are ignored.

use axum::http::{HeaderMap, header};
use omeka_api::messages;

use super::wire::ImportError;

/// The file part's name and bytes.
///
/// # Errors
/// [`ImportError::BadRequest`] for a body that is not `multipart/form-data`
/// or holds no file part or more than one.
pub(super) fn one_file<'b>(
    headers: &HeaderMap,
    body: &'b [u8],
) -> Result<(String, &'b [u8]), ImportError> {
    let refused = || ImportError::BadRequest(messages::IMPORT_LIST_ONE_FILE.into());
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(refused)?;
    let (media_type, parameters) = content_type.split_once(';').ok_or_else(refused)?;
    if !media_type
        .trim()
        .eq_ignore_ascii_case("multipart/form-data")
    {
        return Err(refused());
    }
    let boundary = parameters
        .split(';')
        .find_map(|parameter| {
            let (name, value) = parameter.split_once('=')?;
            name.trim()
                .eq_ignore_ascii_case("boundary")
                .then(|| value.trim().trim_matches('"'))
        })
        .filter(|boundary| !boundary.is_empty())
        .ok_or_else(refused)?;
    let delimiter = format!("\r\n--{boundary}").into_bytes();
    let mut files = Vec::new();
    let first = format!("--{boundary}").into_bytes();
    let mut rest = body.strip_prefix(first.as_slice()).ok_or_else(refused)?;
    loop {
        if rest.starts_with(b"--") {
            break;
        }
        rest = rest.strip_prefix(b"\r\n").ok_or_else(refused)?;
        let end = find(rest, &delimiter).ok_or_else(refused)?;
        let part = &rest[..end];
        rest = &rest[end + delimiter.len()..];
        let split = find(part, b"\r\n\r\n").ok_or_else(refused)?;
        let head = String::from_utf8_lossy(&part[..split]);
        if let Some(name) = file_name(&head) {
            files.push((name, &part[split + 4..]));
        }
    }
    match files.len() {
        1 => Ok(files.remove(0)),
        _ => Err(refused()),
    }
}

/// The `filename` of a part's `Content-Disposition`, when it has one.
fn file_name(head: &str) -> Option<String> {
    let disposition = head.split("\r\n").find(|line| {
        line.split_once(':')
            .is_some_and(|(name, _)| name.trim().eq_ignore_ascii_case("content-disposition"))
    })?;
    disposition.split(';').find_map(|parameter| {
        let (name, value) = parameter.split_once('=')?;
        (name.trim() == "filename").then(|| value.trim().trim_matches('"').to_string())
    })
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use axum::http::HeaderValue;

    use super::*;

    fn headers(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_str(value).unwrap());
        headers
    }

    #[test]
    fn the_one_file_part_is_read_and_other_fields_are_ignored() {
        let body = b"--XyZ\r\nContent-Disposition: form-data; name=\"note\"\r\n\r\nhello\r\n--XyZ\r\nContent-Disposition: form-data; name=\"file\"; filename=\"list.csv\"\r\nContent-Type: text/csv\r\n\r\nId,Title\r\nA,\xe0\xa4\xa6\r\n\r\n--XyZ--\r\n";
        let (name, bytes) = one_file(&headers("multipart/form-data; boundary=XyZ"), body).unwrap();
        assert_eq!(name, "list.csv");
        assert_eq!(
            bytes, b"Id,Title\r\nA,\xe0\xa4\xa6\r\n",
            "the part's own line ends kept"
        );
    }

    #[test]
    fn a_form_with_no_file_or_two_files_or_no_boundary_is_refused() {
        let two = b"--b\r\nContent-Disposition: form-data; name=\"a\"; filename=\"a.csv\"\r\n\r\nx\r\n--b\r\nContent-Disposition: form-data; name=\"b\"; filename=\"b.csv\"\r\n\r\ny\r\n--b--";
        assert!(one_file(&headers("multipart/form-data; boundary=b"), two).is_err());
        let none = b"--b\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\nx\r\n--b--";
        assert!(one_file(&headers("multipart/form-data; boundary=b"), none).is_err());
        assert!(one_file(&headers("multipart/form-data"), none).is_err());
        assert!(one_file(&headers("application/json"), b"{}").is_err());
    }
}
