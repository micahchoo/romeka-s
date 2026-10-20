//! [`S3Store`], the store on S3-compatible object storage (AWS S3, MinIO,
//! Cloudflare R2, Backblaze B2 and the like).
//!
//! Upstream has no object store in core; the `AmazonS3` module adds one. Here
//! the [`Store`] trait is the seam, so every caller works on either backend.
//!
//! Every request is a SigV4 presigned URL that `rusty-s3` builds and the
//! blocking ureq client sends. A presigned request signs the method, the host
//! and the query but not the body (`UNSIGNED-PAYLOAD`), so a part streams from
//! a reused buffer without hashing it first. The endpoint is the operator's,
//! so these requests skip the ingest URL guard: a MinIO on a private address
//! is the common case.
//!
//! # Stored objects
//!
//! The object key is the storage path, so `original/<id>.jpg` in the local tree
//! is the key `original/<id>.jpg` in the bucket. A put replaces the object in
//! one step; a reader sees the old object or the new one.

use std::fmt;
use std::io::Read;
use std::ops::Range;
use std::time::{Duration, SystemTime};

use rusty_s3::actions::{
    CompleteMultipartUpload, CreateMultipartUpload, ListObjectsV2, S3Action as _,
};
use rusty_s3::{Bucket, Credentials, UrlStyle};
use url::Url;

use super::s3_http::{Http, Sent};
use super::{Access, ListedFile, Store, StoreError, checked_path};

/// The smallest part S3 accepts, except the last: 5 MiB. A stream shorter than
/// one part goes up as one `PutObject`.
const MIN_PART_BYTES: u64 = 5 * 1024 * 1024;

/// The most parts one multipart upload may have.
const MAX_PARTS: u64 = 10_000;

/// How long the store's own presigned requests stay valid. Each is sent at
/// once, so this only has to cover one slow part.
const REQUEST_TTL: Duration = Duration::from_secs(60 * 60);

/// The settings of an [`S3Store`], from `ROMEKA_S3_*`.
#[derive(Clone, PartialEq, Eq)]
pub struct S3Config {
    /// The service endpoint, such as `https://s3.eu-west-1.amazonaws.com` or a
    /// MinIO at `http://minio:9000`. Buckets are addressed path style.
    pub endpoint: Url,
    /// The bucket that holds every stored file.
    pub bucket: String,
    /// The region the signature names; MinIO accepts `us-east-1`.
    pub region: String,
    /// The access key id.
    pub access_key: String,
    /// The secret key. [`fmt::Debug`] never prints it.
    pub secret_key: String,
    /// Where plain URLs point instead of the bucket, such as a CDN or a public
    /// bucket domain. Signed URLs always name the endpoint.
    pub public_base: Option<String>,
    /// How long a signed URL the API or the site renders stays valid.
    pub url_ttl: Duration,
}

impl fmt::Debug for S3Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("S3Config")
            .field("endpoint", &self.endpoint.as_str())
            .field("bucket", &self.bucket)
            .field("region", &self.region)
            .field("access_key", &self.access_key)
            .field("secret_key", &"<redacted>")
            .field("public_base", &self.public_base)
            .field("url_ttl", &self.url_ttl)
            .finish()
    }
}

/// Settings an [`S3Store`] cannot be built from.
#[derive(Debug, thiserror::Error)]
pub enum S3ConfigError {
    /// The endpoint is not an `http` or `https` URL with a host.
    #[error("the S3 endpoint {0} is not an http or https URL with a host")]
    Endpoint(String),
    /// The public base is not an `http` or `https` URL.
    #[error("the S3 public base {0} is not an http or https URL")]
    PublicBase(String),
}

/// A store on one bucket of an S3-compatible service.
pub struct S3Store {
    bucket: Bucket,
    credentials: Credentials,
    public_base: Option<Url>,
    url_ttl: Duration,
    http: Http,
}

impl fmt::Debug for S3Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("S3Store")
            .field("bucket", &self.bucket.base_url().as_str())
            .field("public_base", &self.public_base.as_ref().map(Url::as_str))
            .field("url_ttl", &self.url_ttl)
            .finish_non_exhaustive()
    }
}

impl S3Store {
    /// A store on `config.bucket`. Nothing is sent until the first call, so a
    /// wrong key or a missing bucket shows on the first read or write.
    ///
    /// # Errors
    /// [`S3ConfigError`] when the endpoint or the public base is not a usable
    /// URL.
    pub fn new(config: S3Config) -> Result<Self, S3ConfigError> {
        let endpoint = config.endpoint.to_string();
        let bucket = Bucket::new(
            config.endpoint,
            UrlStyle::Path,
            config.bucket,
            config.region,
        )
        .map_err(|_| S3ConfigError::Endpoint(endpoint))?;
        let public_base = config
            .public_base
            .map(|base| parse_public_base(&base).ok_or(S3ConfigError::PublicBase(base)))
            .transpose()?;
        Ok(Self {
            bucket,
            credentials: Credentials::new(config.access_key, config.secret_key),
            public_base,
            url_ttl: config.url_ttl,
            http: Http::new(),
        })
    }

    /// Create the bucket unless this account already owns it: for a first
    /// setup and for tests against a fresh MinIO.
    ///
    /// # Errors
    /// [`StoreError::Remote`] when the service refuses, for example because
    /// another account owns the name.
    pub fn create_bucket(&self) -> Result<(), StoreError> {
        let name = self.bucket.name().to_string();
        let url = self
            .bucket
            .create_bucket(&self.credentials)
            .sign(REQUEST_TTL);
        let sent = self.http.put(&name, "CreateBucket", &url, &[])?;
        if sent.status == 409 && sent.error_code() == "BucketAlreadyOwnedByYou" {
            return Ok(());
        }
        sent.expect(&name, "CreateBucket", &[200])
    }

    fn put_part(
        &self,
        key: &str,
        upload_id: &str,
        number: u16,
        bytes: &[u8],
    ) -> Result<String, StoreError> {
        let url = self
            .bucket
            .upload_part(Some(&self.credentials), key, number, upload_id)
            .sign(REQUEST_TTL);
        let sent = self.http.put(key, "UploadPart", &url, bytes)?;
        sent.expect(key, "UploadPart", &[200])?;
        sent.etag
            .ok_or_else(|| StoreError::remote(key, "UploadPart answered no ETag"))
    }

    /// Upload the parts of a stream whose first part is already in `buffer`.
    fn put_parts(
        &self,
        key: &str,
        upload_id: &str,
        reader: &mut dyn Read,
        buffer: &mut [u8],
        first: usize,
    ) -> Result<u64, StoreError> {
        let mut etags = Vec::new();
        let mut written = 0;
        let mut filled = first;
        while filled > 0 {
            let number = u16::try_from(etags.len() + 1)
                .ok()
                .filter(|number| u64::from(*number) <= MAX_PARTS)
                .ok_or_else(|| {
                    StoreError::remote(key, "the stream needs more than 10,000 parts")
                })?;
            etags.push(self.put_part(key, upload_id, number, &buffer[..filled])?);
            written += filled as u64;
            filled = fill(reader, buffer).map_err(|error| StoreError::reader(key, error))?;
        }
        let action = CompleteMultipartUpload::new(
            &self.bucket,
            Some(&self.credentials),
            key,
            upload_id,
            etags.iter().map(String::as_str),
        );
        let url = action.sign(REQUEST_TTL);
        let sent = self.http.post(
            key,
            "CompleteMultipartUpload",
            &url,
            action.body().as_bytes(),
        )?;
        sent.expect(key, "CompleteMultipartUpload", &[200])?;
        // S3 can answer 200 and report the failure in the body.
        if sent.body_text().contains("<Error>") {
            return Err(StoreError::remote(
                key,
                format!("CompleteMultipartUpload failed: {}", sent.error_code()),
            ));
        }
        Ok(written)
    }

    fn head(&self, key: &str) -> Result<Sent, StoreError> {
        let url = self
            .bucket
            .head_object(Some(&self.credentials), key)
            .sign(REQUEST_TTL);
        self.http.head(key, &url)
    }
}

impl Store for S3Store {
    fn put(&self, bytes: &[u8], storage_path: &str) -> Result<(), StoreError> {
        let key = checked_path(storage_path)?;
        let url = self
            .bucket
            .put_object(Some(&self.credentials), key)
            .sign(REQUEST_TTL);
        self.http
            .put(key, "PutObject", &url, bytes)?
            .expect(key, "PutObject", &[200])
    }

    /// A stream longer than one part goes up as a multipart upload, one part
    /// buffered at a time. A failed upload is aborted, so the object at the
    /// path stays what it was.
    fn put_stream(
        &self,
        reader: &mut dyn Read,
        size_hint: Option<u64>,
        storage_path: &str,
    ) -> Result<u64, StoreError> {
        let key = checked_path(storage_path)?;
        let mut buffer = vec![0; part_bytes(size_hint)];
        let first = fill(reader, &mut buffer).map_err(|error| StoreError::reader(key, error))?;
        if first < buffer.len() {
            self.put(&buffer[..first], key)?;
            return Ok(first as u64);
        }
        let url = self
            .bucket
            .create_multipart_upload(Some(&self.credentials), key)
            .sign(REQUEST_TTL);
        let sent = self.http.post(key, "CreateMultipartUpload", &url, &[])?;
        sent.expect(key, "CreateMultipartUpload", &[200])?;
        let created =
            CreateMultipartUpload::parse_response(&sent.body_text()).map_err(|error| {
                StoreError::remote(key, format!("CreateMultipartUpload answer: {error}"))
            })?;
        let upload_id = created.upload_id();
        let result = self.put_parts(key, upload_id, reader, &mut buffer, first);
        if result.is_err() {
            let url = self
                .bucket
                .abort_multipart_upload(Some(&self.credentials), key, upload_id)
                .sign(REQUEST_TTL);
            // The upload already failed; a failed abort leaves parts that the
            // bucket's lifecycle rule for incomplete uploads removes.
            let _ = self.http.delete(key, "AbortMultipartUpload", &url);
        }
        result
    }

    fn read(&self, storage_path: &str) -> Result<Vec<u8>, StoreError> {
        let key = checked_path(storage_path)?;
        let url = self
            .bucket
            .get_object(Some(&self.credentials), key)
            .sign(REQUEST_TTL);
        let sent = self.http.get(key, &url, None)?;
        sent.expect(key, "GetObject", &[200])?;
        Ok(sent.body)
    }

    fn read_range(&self, storage_path: &str, range: Range<u64>) -> Result<Vec<u8>, StoreError> {
        let key = checked_path(storage_path)?;
        if range.start >= range.end {
            self.head(key)?.expect(key, "HeadObject", &[200])?;
            return Ok(Vec::new());
        }
        let url = self
            .bucket
            .get_object(Some(&self.credentials), key)
            .sign(REQUEST_TTL);
        let header = format!("bytes={}-{}", range.start, range.end - 1);
        let sent = self.http.get(key, &url, Some(&header))?;
        match sent.status {
            206 => Ok(sent.body),
            // A range that starts at or past the end of an existing object.
            416 => Ok(Vec::new()),
            // A server that ignores `Range` answers the whole object.
            200 => Ok(clip(sent.body, range)),
            _ => Err(sent.failure(key, "GetObject")),
        }
    }

    /// S3 answers 204 whether or not the object existed, so a `HeadObject`
    /// first tells the two apart.
    fn delete(&self, storage_path: &str) -> Result<bool, StoreError> {
        let key = checked_path(storage_path)?;
        if !self.exists(key)? {
            return Ok(false);
        }
        let url = self
            .bucket
            .delete_object(Some(&self.credentials), key)
            .sign(REQUEST_TTL);
        self.http
            .delete(key, "DeleteObject", &url)?
            .expect(key, "DeleteObject", &[200, 204])?;
        Ok(true)
    }

    fn exists(&self, storage_path: &str) -> Result<bool, StoreError> {
        let key = checked_path(storage_path)?;
        let sent = self.head(key)?;
        match sent.status {
            200 => Ok(true),
            404 => Ok(false),
            _ => Err(sent.failure(key, "HeadObject")),
        }
    }

    /// `ListObjectsV2` with `/` as the delimiter, so nested folders come back
    /// as common prefixes and are left out, page after page.
    fn list(&self, prefix: &str) -> Result<Vec<ListedFile>, StoreError> {
        let prefix = checked_path(prefix.trim_end_matches('/'))?;
        let folder = format!("{prefix}/");
        let mut listed = Vec::new();
        let mut token: Option<String> = None;
        loop {
            let mut action = ListObjectsV2::new(&self.bucket, Some(&self.credentials));
            action.with_prefix(folder.as_str());
            action.with_delimiter("/");
            if let Some(token) = &token {
                action.with_continuation_token(token.as_str());
            }
            let url = action.sign(REQUEST_TTL);
            let sent = self.http.get(prefix, &url, None)?;
            sent.expect(prefix, "ListObjectsV2", &[200])?;
            let page = ListObjectsV2::parse_response(&sent.body_text()).map_err(|error| {
                StoreError::remote(prefix, format!("ListObjectsV2 answer: {error}"))
            })?;
            for object in page.contents {
                if object.key.len() == folder.len() {
                    continue;
                }
                listed.push(ListedFile {
                    modified: modified(&object.last_modified),
                    storage_path: object.key,
                });
            }
            match page.next_continuation_token {
                Some(next) => token = Some(next),
                None => break,
            }
        }
        listed.sort_by(|a, b| a.storage_path.cmp(&b.storage_path));
        Ok(listed)
    }

    /// A plain URL names the public base when one is set, otherwise the object
    /// in the bucket, which answers only when the bucket policy lets anyone
    /// read that prefix. A signed URL always names the endpoint.
    fn url(&self, storage_path: &str, access: Access) -> String {
        match access {
            Access::Public => match &self.public_base {
                Some(base) => {
                    let mut url = base.clone();
                    if let Ok(mut segments) = url.path_segments_mut() {
                        segments.pop_if_empty().extend(storage_path.split('/'));
                    }
                    url.into()
                }
                None => self
                    .bucket
                    .object_url(storage_path)
                    .map(Into::into)
                    .unwrap_or_default(),
            },
            Access::Signed { ttl } => self
                .bucket
                .get_object(Some(&self.credentials), storage_path)
                .sign(ttl)
                .into(),
        }
    }

    fn url_ttl(&self) -> Duration {
        self.url_ttl
    }
}

/// The part size for a stream of about `size_hint` bytes: 5 MiB, or larger
/// when 10,000 parts of 5 MiB would not hold the stream.
fn part_bytes(size_hint: Option<u64>) -> usize {
    let needed = size_hint.map_or(0, |size| size.div_ceil(MAX_PARTS));
    let mib = 1024 * 1024;
    let bytes = needed.max(MIN_PART_BYTES).div_ceil(mib) * mib;
    usize::try_from(bytes).unwrap_or(usize::MAX)
}

/// Read until `buffer` is full or the reader ends; the count read.
fn fill(reader: &mut dyn Read, buffer: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
}

fn clip(mut body: Vec<u8>, range: Range<u64>) -> Vec<u8> {
    let len = body.len() as u64;
    let start = usize::try_from(range.start.min(len)).unwrap_or(usize::MAX);
    let end = usize::try_from(range.end.min(len)).unwrap_or(usize::MAX);
    body.truncate(end);
    body.drain(..start);
    body
}

/// `LastModified` is ISO 8601 in UTC. A value that does not parse reads as the
/// epoch, so a caller that sweeps old files never skips a file for it.
fn modified(value: &str) -> SystemTime {
    chrono::DateTime::parse_from_rfc3339(value).map_or(SystemTime::UNIX_EPOCH, SystemTime::from)
}

fn parse_public_base(value: &str) -> Option<Url> {
    let url = Url::parse(value).ok()?;
    (matches!(url.scheme(), "http" | "https") && url.host_str().is_some()).then_some(url)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn config() -> S3Config {
        S3Config {
            endpoint: "http://127.0.0.1:9000".parse().unwrap(),
            bucket: "romeka".to_string(),
            region: "us-east-1".to_string(),
            access_key: "AKIDEXAMPLE".to_string(),
            secret_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".to_string(),
            public_base: None,
            url_ttl: Duration::from_secs(900),
        }
    }

    #[test]
    fn debug_output_never_carries_the_secret_key() {
        let config = config();
        let printed = format!("{config:?} {:?}", S3Store::new(config.clone()).unwrap());
        assert!(!printed.contains(&config.secret_key), "{printed}");
        assert!(printed.contains("<redacted>"), "{printed}");
    }

    #[test]
    fn a_plain_url_names_the_object_in_the_bucket() {
        let store = S3Store::new(config()).unwrap();
        assert_eq!(
            store.url("square/abc.jpg", Access::Public),
            "http://127.0.0.1:9000/romeka/square/abc.jpg"
        );
    }

    #[test]
    fn a_signed_url_carries_its_ttl_and_a_signature_but_not_the_secret() {
        let store = S3Store::new(config()).unwrap();
        let url = store.url(
            "original/abc.jpg",
            Access::Signed {
                ttl: Duration::from_secs(900),
            },
        );
        assert!(
            url.starts_with("http://127.0.0.1:9000/romeka/original/abc.jpg?"),
            "{url}"
        );
        assert!(url.contains("X-Amz-Expires=900"), "{url}");
        assert!(url.contains("X-Amz-Signature="), "{url}");
        assert!(!url.contains("wJalrXUtnFEMI"), "{url}");
    }

    #[test]
    fn a_public_base_keeps_its_path_and_encodes_the_storage_path() {
        let mut config = config();
        config.public_base = Some("https://cdn.example.org/files".to_string());
        let store = S3Store::new(config).unwrap();
        assert_eq!(
            store.url("asset/a b#c.png", Access::Public),
            "https://cdn.example.org/files/asset/a%20b%23c.png"
        );
    }

    #[test]
    fn unusable_urls_are_refused_at_construction() {
        let mut bad_base = config();
        bad_base.public_base = Some("cdn.example.org".to_string());
        assert!(matches!(
            S3Store::new(bad_base),
            Err(S3ConfigError::PublicBase(_))
        ));
        let mut bad_endpoint = config();
        bad_endpoint.endpoint = "ftp://example.org".parse().unwrap();
        assert!(matches!(
            S3Store::new(bad_endpoint),
            Err(S3ConfigError::Endpoint(_))
        ));
    }

    #[test]
    fn parts_are_5_mib_until_10000_of_them_would_not_hold_the_stream() {
        assert_eq!(part_bytes(None), 5 * 1024 * 1024);
        assert_eq!(part_bytes(Some(50 * 1024 * 1024)), 5 * 1024 * 1024);
        let hundred_gib = 100 * 1024 * 1024 * 1024;
        let part = part_bytes(Some(hundred_gib)) as u64;
        assert!(part * MAX_PARTS >= hundred_gib && part.is_multiple_of(1024 * 1024));
    }

    #[test]
    fn a_whole_body_is_clipped_to_the_range() {
        assert_eq!(clip(b"0123456789".to_vec(), 2..5), b"234");
        assert_eq!(clip(b"0123456789".to_vec(), 7..100), b"789");
        assert!(clip(b"0123".to_vec(), 9..12).is_empty());
    }
}
