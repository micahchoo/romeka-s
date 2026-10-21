//! `GET /api/romeka/files/allow`: whether the proxy may serve one stored file.
//!
//! Upstream serves every file under `/files` to anyone. Here Caddy's
//! `forward_auth` asks this route first, with the request's URI in
//! `X-Forwarded-Uri` and its cookies untouched. The answer is 204 to serve,
//! 403 to refuse, 404 when no media or asset owns the path. A 204 for a file
//! that is not public carries `X-Romeka-Private: 1`, and the proxy turns that
//! into `Cache-Control: private` and `Vary: Cookie`.
//!
//! The proxy serves the path it decoded itself, so this route accepts only the
//! exact shape the store writes, `<prefix>/<name>`, and refuses everything a
//! decoder or a path cleaner could read two ways.

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::Response;
use axum::routing::get;
use omeka_api::auth::Actor;
use omeka_api::errors::ApiError;
use omeka_db::repos::{AssetRepo, MediaFileOwner, MediaRepo};
use omeka_db::with_db;
use omeka_media::store::{Download, StorePrefix};

use crate::{AppState, actor_for, error_response};

/// The header Caddy copies from a 204 onto the file's answer.
pub(crate) const PRIVATE_HEADER: &str = "x-romeka-private";
/// The stored type, which the origin answers with instead of guessing from the
/// extension.
pub(crate) const CONTENT_TYPE_HEADER: &str = "x-romeka-content-type";
/// The `Content-Disposition` for an original, from its `o:source`.
pub(crate) const CONTENT_DISPOSITION_HEADER: &str = "x-romeka-content-disposition";

const FORWARDED_URI: &str = "x-forwarded-uri";
const FILES_PREFIX: &str = "/files/";
/// Every thumbnail is a JPEG, whatever the original (`TempFile::storeThumbnails`).
const THUMBNAIL_MEDIA_TYPE: &str = "image/jpeg";
/// The extension every stored thumbnail carries.
const THUMBNAIL_EXTENSION: &str = "jpg";

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/api/romeka/files/allow", get(allow))
}

/// A stored file named by its storage path, before any row is read.
#[derive(Debug, PartialEq, Eq)]
enum StoredFile<'a> {
    Original {
        storage_id: &'a str,
        extension: Option<&'a str>,
    },
    Thumbnail {
        storage_id: &'a str,
        extension: Option<&'a str>,
    },
    Asset {
        storage_id: &'a str,
        extension: Option<&'a str>,
    },
    /// `access/<storage id>.<suffix>`, a copy a browser plays or shows when it
    /// cannot open the original. It belongs to the media its storage id names
    /// and is served with the type its suffix states.
    Access {
        storage_id: &'a str,
        extension: Option<&'a str>,
    },
}

/// What a 204 tells the origin about the file.
#[derive(Debug, PartialEq, Eq)]
struct Answer {
    /// The file is visible only to a caller who may see its resource.
    private: bool,
    /// The type to answer with, when the row names one.
    content_type: Option<String>,
    /// The `Content-Disposition` of an original.
    disposition: Option<String>,
}

/// What the route answers for one file.
#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    Serve(Answer),
    Refused,
    Unknown,
}

async fn allow(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(forwarded) = headers
        .get(FORWARDED_URI)
        .and_then(|value| value.to_str().ok())
    else {
        return status(StatusCode::FORBIDDEN);
    };
    let Some((prefix, name)) = storage_path(forwarded) else {
        return status(StatusCode::FORBIDDEN);
    };
    let Some(file) = StoredFile::parse(&prefix, &name) else {
        return status(StatusCode::FORBIDDEN);
    };
    match verdict(&state, &file, forwarded, &headers).await {
        Ok(Verdict::Serve(answer)) => served(&answer),
        Ok(Verdict::Refused) => status(StatusCode::FORBIDDEN),
        Ok(Verdict::Unknown) => status(StatusCode::NOT_FOUND),
        Err(error) => error_response(error),
    }
}

fn served(answer: &Answer) -> Response {
    let mut builder = Response::builder().status(StatusCode::NO_CONTENT);
    if answer.private {
        builder = builder.header(PRIVATE_HEADER, "1");
    }
    if let Some(content_type) = &answer.content_type {
        builder = builder.header(CONTENT_TYPE_HEADER, content_type);
    }
    if let Some(disposition) = &answer.disposition {
        builder = builder.header(CONTENT_DISPOSITION_HEADER, disposition);
    }
    builder
        .body(Body::empty())
        .unwrap_or_else(|_| crate::empty_internal_error())
}

/// The file's visibility, reading the caller only when the file is not public,
/// so a public file costs one indexed lookup.
async fn verdict(
    state: &AppState,
    file: &StoredFile<'_>,
    forwarded: &str,
    headers: &HeaderMap,
) -> Result<Verdict, ApiError> {
    let owner = match *file {
        StoredFile::Original {
            storage_id,
            extension,
        } => media_owner(state, storage_id)
            .await?
            .filter(|owner| owner.extension.as_deref() == extension),
        StoredFile::Thumbnail {
            storage_id,
            extension,
        } => media_owner(state, storage_id)
            .await?
            .filter(|_| extension == Some(THUMBNAIL_EXTENSION)),
        StoredFile::Access {
            storage_id,
            extension,
        } => media_owner(state, storage_id)
            .await?
            .filter(|_| access_media_type(extension).is_some()),
        StoredFile::Asset {
            storage_id,
            extension,
        } => {
            // Upstream assets carry no visibility: a site logo or a resource
            // thumbnail is public by construction.
            let asset = with_db!(&state.pool, |db| AssetRepo::new(db)
                .get_by_storage_id(storage_id)
                .await)
            .map_err(ApiError::internal)?;
            return Ok(match asset {
                Some(asset) if asset.extension.as_deref() == extension => Verdict::Serve(Answer {
                    private: false,
                    content_type: header_media_type(Some(&asset.media_type)),
                    disposition: None,
                }),
                _ => Verdict::Unknown,
            });
        }
    };
    let Some(owner) = owner else {
        return Ok(Verdict::Unknown);
    };
    let download = matches!(*file, StoredFile::Original { .. }).then(|| {
        Download::of(
            owner.media_type.as_deref(),
            owner.source.as_deref(),
            forwarded_name(forwarded),
        )
    });
    let served = match *file {
        StoredFile::Thumbnail { .. } => Some(THUMBNAIL_MEDIA_TYPE.to_string()),
        StoredFile::Access { extension, .. } => access_media_type(extension),
        _ => download
            .as_ref()
            .and_then(Download::served_type)
            .map(str::to_string),
    };
    let answer = Answer {
        private: !(owner.media_is_public && owner.item_is_public),
        content_type: header_media_type(served.as_deref()),
        disposition: download.as_ref().map(Download::disposition),
    };
    if !answer.private {
        return Ok(Verdict::Serve(answer));
    }
    // The key pair rides in the original query string, where the API reads it.
    let Ok(uri) = Uri::try_from(forwarded) else {
        return Ok(Verdict::Refused);
    };
    let actor = actor_for(state, &uri, headers).await?;
    Ok(if sees(&actor, &owner) {
        Verdict::Serve(answer)
    } else {
        Verdict::Refused
    })
}

/// The name in the forwarded path, after its last `/`, which is the stored
/// file name the disposition falls back to.
fn forwarded_name(forwarded: &str) -> &str {
    let path = forwarded
        .split_once('?')
        .map_or(forwarded, |(path, _query)| path);
    path.rsplit('/').next().unwrap_or_default()
}

/// The type an access copy with `extension` answers with. A copy is a JPEG
/// master, an MP4 or an M4A in this build; any other suffix is no copy the store
/// wrote.
fn access_media_type(extension: Option<&str>) -> Option<String> {
    match extension? {
        "jpg" => Some("image/jpeg".to_string()),
        "mp4" => Some("video/mp4".to_string()),
        "m4a" => Some("audio/mp4".to_string()),
        _ => None,
    }
}

/// A stored media type, when a header can carry it unchanged: `type/subtype`
/// of visible ASCII tokens. The type is a boundary value, so a value with a
/// space, a parameter or a control character is dropped and the origin falls
/// back to the extension.
fn header_media_type(value: Option<&str>) -> Option<String> {
    let value = value?;
    let (kind, subtype) = value.split_once('/')?;
    let token = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&byte))
    };
    (token(kind) && token(subtype)).then(|| value.to_string())
}

async fn media_owner(
    state: &AppState,
    storage_id: &str,
) -> Result<Option<MediaFileOwner>, ApiError> {
    with_db!(&state.pool, |db| MediaRepo::new(db)
        .file_owner(storage_id)
        .await)
    .map_err(ApiError::internal)
}

/// A media file is visible when the caller sees the media and its item, by the
/// rule the API applies to each (`ResourceVisibilityFilter`).
fn sees(actor: &Actor, owner: &MediaFileOwner) -> bool {
    actor.sees_resource(owner.media_is_public, owner.media_owner_id)
        && actor.sees_resource(owner.item_is_public, owner.item_owner_id)
}

fn status(code: StatusCode) -> Response {
    Response::builder()
        .status(code)
        .body(Body::empty())
        .unwrap_or_else(|_| crate::empty_internal_error())
}

impl<'a> StoredFile<'a> {
    /// The file a decoded `<prefix>/<name>` names, `None` for a prefix the
    /// store never writes.
    fn parse(prefix: &str, name: &'a str) -> Option<Self> {
        let prefix = StorePrefix::ALL
            .into_iter()
            .find(|known| known.as_str() == prefix)?;
        let (storage_id, extension) = match name.rsplit_once('.') {
            Some((storage_id, extension)) => (storage_id, Some(extension)),
            None => (name, None),
        };
        if storage_id.is_empty() {
            return None;
        }
        Some(match prefix {
            StorePrefix::Original => Self::Original {
                storage_id,
                extension,
            },
            StorePrefix::Asset => Self::Asset {
                storage_id,
                extension,
            },
            StorePrefix::Thumbnail(_) => Self::Thumbnail {
                storage_id,
                extension,
            },
            StorePrefix::Access => Self::Access {
                storage_id,
                extension,
            },
        })
    }
}

/// The decoded `(prefix, name)` of a `/files/<prefix>/<name>` URI, or `None`
/// for any other shape.
///
/// Each segment is percent-decoded once, on its own, so an encoded `/` cannot
/// join or split segments. A decoded segment that is empty, `.` or `..`, or
/// that holds a `/`, `\`, `%`, or a control character is refused: the first
/// three are traversal or a path a cleaner rewrites, a `%` left after one
/// decode is a second layer of encoding, and no storage name holds any of them.
fn storage_path(forwarded: &str) -> Option<(String, String)> {
    let path = forwarded
        .split_once('?')
        .map_or(forwarded, |(path, _query)| path);
    let rest = path.strip_prefix(FILES_PREFIX)?;
    let mut segments = rest.split('/').map(decode_segment);
    let prefix = segments.next()??;
    let name = segments.next()??;
    if segments.next().is_some() {
        return None;
    }
    Some((prefix, name))
}

fn decode_segment(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = raw.get(index + 1..index + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    let segment = String::from_utf8(out).ok()?;
    let illegal = segment.is_empty()
        || segment == "."
        || segment == ".."
        || segment
            .chars()
            .any(|c| matches!(c, '/' | '\\' | '%') || c.is_control());
    (!illegal).then_some(segment)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_segment_decodes_once() {
        assert_eq!(decode_segment("a%62c").as_deref(), Some("abc"));
        assert_eq!(decode_segment("%252e"), None, "a second layer is refused");
        assert_eq!(decode_segment("%2"), None, "a truncated escape is refused");
        assert_eq!(decode_segment("%zz"), None);
    }

    #[test]
    fn a_name_splits_at_its_last_dot() {
        assert_eq!(
            StoredFile::parse("original", "a.b.png"),
            Some(StoredFile::Original {
                storage_id: "a.b",
                extension: Some("png"),
            })
        );
        assert_eq!(StoredFile::parse("original", ".png"), None);
        assert_eq!(StoredFile::parse("thumbnails", "abc.jpg"), None);
    }

    /// An access copy is a prefix the store writes, and its suffix names the
    /// type the origin answers with. Any other suffix is no copy.
    #[test]
    fn an_access_copy_names_its_type_by_its_suffix() {
        assert_eq!(
            StoredFile::parse("access", "abc123.mp4"),
            Some(StoredFile::Access {
                storage_id: "abc123",
                extension: Some("mp4"),
            })
        );
        assert_eq!(access_media_type(Some("mp4")).as_deref(), Some("video/mp4"));
        assert_eq!(access_media_type(Some("m4a")).as_deref(), Some("audio/mp4"));
        assert_eq!(
            access_media_type(Some("jpg")).as_deref(),
            Some("image/jpeg")
        );
        assert_eq!(access_media_type(Some("avi")), None);
        assert_eq!(access_media_type(None), None);
    }
}
