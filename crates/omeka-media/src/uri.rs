//! A minimal HTTP URI parser.
//!
//! The media ingesters and renderers use `Laminas\Uri\Http` for one narrow job:
//! check that a string is an absolute `http`/`https` URI, then read the host,
//! the path and the query parameters. This module ports exactly that surface.
//! It deliberately does not normalise the URI, because upstream keeps the raw
//! source string (for example `Youtube` stores `o:source` unchanged) and a
//! normalising parser would change the stored value.

/// A parsed absolute HTTP URI (`Laminas\Uri\Http`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpUri {
    raw: String,
    scheme: String,
    host: String,
    path: String,
    query: Vec<(String, String)>,
}

impl HttpUri {
    /// Parse a string as an absolute HTTP URI.
    ///
    /// Returns `None` when the scheme is not `http`/`https` or the host is
    /// empty, which is exactly `Laminas\Uri\Http::isValid()` combined with
    /// `isAbsolute()` as the ingesters use it
    /// (`Media/Ingester/Url.php:62`, `Youtube.php:44`, `IIIF.php:54`).
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let (scheme, rest) = raw.split_once("://")?;
        let scheme = scheme.to_ascii_lowercase();
        if scheme != "http" && scheme != "https" {
            return None;
        }
        let (authority, tail) = match rest.find(['/', '?', '#']) {
            Some(pos) => (&rest[..pos], &rest[pos..]),
            None => (rest, ""),
        };
        if authority.is_empty() {
            return None;
        }
        let host = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host)
            .split(':')
            .next()
            .unwrap_or("");
        if host.is_empty() {
            return None;
        }
        let (path, query) = split_path_query(tail);
        Some(Self {
            raw: raw.to_string(),
            scheme,
            host: host.to_string(),
            path,
            query: parse_query(query),
        })
    }

    /// The original string, unchanged.
    #[must_use]
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// The scheme, lowercased.
    #[must_use]
    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    /// The host, without userinfo or port.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The path, empty when the URI has none.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The decoded query parameters, in order.
    #[must_use]
    pub fn query_pairs(&self) -> &[(String, String)] {
        &self.query
    }

    /// The first value for a query key, like `getQueryAsArray()[$key]`.
    #[must_use]
    pub fn query_value(&self, key: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }

    /// Rebuild the URI string with a replacement query, like
    /// `setQuery()` followed by the object's `__toString()`.
    #[must_use]
    pub fn with_query(&self, query: &[(String, String)]) -> String {
        let base = format!("{}://{}{}", self.scheme, self.host, self.path);
        if query.is_empty() {
            return base;
        }
        let encoded: Vec<String> = query
            .iter()
            .map(|(key, value)| {
                format!(
                    "{}={}",
                    crate::text::urlencode(key),
                    crate::text::urlencode(value)
                )
            })
            .collect();
        format!("{base}?{}", encoded.join("&"))
    }
}

fn split_path_query(tail: &str) -> (String, &str) {
    let without_fragment = tail.split('#').next().unwrap_or("");
    match without_fragment.split_once('?') {
        Some((path, query)) => (path.to_string(), query),
        None => (without_fragment.to_string(), ""),
    }
}

fn parse_query(query: &str) -> Vec<(String, String)> {
    if query.is_empty() {
        return Vec::new();
    }
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((key, value)) => (percent_decode(key), percent_decode(value)),
            None => (percent_decode(pair), String::new()),
        })
        .collect()
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(byte) => {
                        out.push(byte);
                        index += 3;
                    }
                    Err(_) => {
                        out.push(b'%');
                        index += 1;
                    }
                }
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The YouTube ingester reads host, path and the `v` parameter
    /// (`Media/Ingester/Youtube.php:45`).
    #[test]
    fn parses_watch_url_like_laminas_http_uri() {
        let uri = HttpUri::parse("https://www.youtube.com/watch?v=abc123&t=5").expect("valid");
        assert_eq!(uri.host(), "www.youtube.com");
        assert_eq!(uri.path(), "/watch");
        assert_eq!(uri.query_value("v"), Some("abc123"));
        assert_eq!(uri.query_value("t"), Some("5"));
    }

    /// A short `youtu.be` URL keeps the id in the path.
    #[test]
    fn parses_short_url_path() {
        let uri = HttpUri::parse("https://youtu.be/abc123").expect("valid");
        assert_eq!(uri.host(), "youtu.be");
        assert_eq!(uri.path(), "/abc123");
        assert_eq!(uri.query_pairs(), &[]);
    }

    /// `isValid()` is false without an http(s) scheme or without a host.
    #[test]
    fn rejects_non_http_and_relative_strings() {
        assert!(HttpUri::parse("ftp://example.org/a").is_none());
        assert!(HttpUri::parse("example.org/a").is_none());
        assert!(HttpUri::parse("https:///a").is_none());
        assert!(HttpUri::parse("").is_none());
    }

    /// `setQuery` plus `__toString` for the YouTube embed URL.
    #[test]
    fn with_query_rebuilds_the_url() {
        let uri = HttpUri::parse("https://www.youtube.com/embed/abc").expect("valid");
        assert_eq!(uri.with_query(&[]), "https://www.youtube.com/embed/abc");
        let query = vec![
            ("start".to_string(), "30".to_string()),
            ("end".to_string(), "60".to_string()),
        ];
        assert_eq!(
            uri.with_query(&query),
            "https://www.youtube.com/embed/abc?start=30&end=60"
        );
    }

    /// Percent-decoding in query values, as `getQueryAsArray` does.
    #[test]
    fn decodes_percent_escapes_in_query() {
        let uri = HttpUri::parse("https://example.org/a?q=a%20b+c").expect("valid");
        assert_eq!(uri.query_value("q"), Some("a b c"));
    }
}
