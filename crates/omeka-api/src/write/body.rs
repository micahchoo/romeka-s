//! Request body parsing at the write boundary.
//!
//! `ApiController::jsonDecode` calls `json_decode` with error checking and
//! rejects a scalar with `InvalidJsonException`; `ApiJsonStrategy` maps that to
//! 400. `processPostData` hands the decoded array to `create` or `update`. This
//! module is that boundary: raw bytes in, a JSON object out, a typed error for
//! anything else.
//!
//! PHP `json_decode($string, true)` decodes a JSON object as an associative
//! array and a JSON list as a list. Re-encoding turns an empty associative
//! array into `[]` and a non-empty one into `{}`, so a stored `data` member
//! that arrived as `{}` comes back as `[]`. [`php_json`] reproduces that.

use serde_json::{Map, Value};

use crate::errors::ApiError;
use crate::write::multipart::{Multipart, UploadedFile};

/// A create body: JSON, or the `data` and `file` fields of a multipart
/// request (`ApiController::processPostData`).
///
/// Only `POST` reads a file, so only `Api::create` takes this shape; `update`
/// keeps the JSON-only path.
#[derive(Debug, Clone, PartialEq)]
pub enum RequestBody {
    /// A JSON body, the `application/json` path.
    Json(Payload),
    /// A `multipart/form-data` body, split into its `data` payload and its
    /// optional upload.
    Multipart {
        payload: Payload,
        file: Option<UploadedFile>,
    },
}

impl RequestBody {
    /// Parse a request body at the boundary. A `multipart/form-data` content
    /// type selects the multipart path, anything else the JSON path, exactly
    /// as `ApiController::processPostData` matches the header.
    pub fn parse(content_type: Option<&str>, bytes: &[u8]) -> Result<Self, ApiError> {
        match content_type {
            Some(value) if is_multipart(value) => {
                let multipart = Multipart::parse(value, bytes);
                Ok(Self::Multipart {
                    payload: multipart.payload()?,
                    file: multipart.file,
                })
            }
            _ => Ok(Self::Json(Payload::parse(bytes)?)),
        }
    }
}

/// Whether a content type selects the multipart path.
fn is_multipart(content_type: &str) -> bool {
    content_type.split(';').next().is_some_and(|media_type| {
        media_type
            .trim()
            .eq_ignore_ascii_case("multipart/form-data")
    })
}

/// A parsed JSON object body.
#[derive(Debug, Clone, PartialEq)]
pub struct Payload {
    object: Map<String, Value>,
}

impl Payload {
    /// Parse a request body. Upstream's two 400 messages are reproduced.
    pub fn parse(bytes: &[u8]) -> Result<Self, ApiError> {
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|_| ApiError::bad_request("JSON: Syntax error"))?;
        match value {
            Value::Object(object) => Ok(Self { object }),
            _ => Err(ApiError::bad_request(
                "JSON: Content must be an object or array.",
            )),
        }
    }

    /// Build a payload from an already-decoded JSON object, for a nested body
    /// the API hydrates the way it hydrates a request (a value annotation).
    pub fn from_object(object: Map<String, Value>) -> Self {
        Self { object }
    }

    pub fn has(&self, key: &str) -> bool {
        self.object.contains_key(key)
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.object.get(key)
    }

    /// A member as a string, `None` when absent or not a string.
    pub fn str_member(&self, key: &str) -> Option<&str> {
        self.object.get(key).and_then(Value::as_str)
    }

    /// A member as a bool, falling back to `default` when absent or not a bool.
    pub fn bool_member(&self, key: &str, default: bool) -> bool {
        self.object
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(default)
    }

    /// The `o:id` of a `{"o:id": N}` reference member.
    pub fn ref_id(&self, key: &str) -> Option<u64> {
        self.object.get(key)?.get("o:id").and_then(Value::as_u64)
    }

    /// An integer member, `None` when absent or not an integer.
    pub fn int_member(&self, key: &str) -> Option<i64> {
        self.object.get(key).and_then(Value::as_i64)
    }

    /// Whether a member is a JSON list or a JSON object. PHP `json_decode($s,
    /// true)` turns both into an array, so `is_array` accepts either.
    pub fn is_array_like(&self, key: &str) -> bool {
        matches!(
            self.object.get(key),
            Some(Value::Array(_) | Value::Object(_))
        )
    }

    /// The object entries of a list member (`o:class`, `o:property`,
    /// `o:resource_template_property`). PHP decodes a JSON list and a JSON
    /// object both as arrays and `foreach` walks either, so both shapes yield
    /// their entries, in order. A non-object entry is skipped.
    pub fn entries(&self, key: &str) -> Vec<Self> {
        let Some(value) = self.object.get(key) else {
            return Vec::new();
        };
        let values: Vec<&Value> = match value {
            Value::Array(items) => items.iter().collect(),
            Value::Object(map) => map.values().collect(),
            _ => Vec::new(),
        };
        values
            .into_iter()
            .filter_map(|value| match value {
                Value::Object(map) => Some(Self {
                    object: map.clone(),
                }),
                _ => None,
            })
            .collect()
    }

    /// Every top-level member, in body order.
    pub fn members(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.object.iter()
    }
}

/// Decode a stored JSON value the way `json_decode(..., true)` then
/// `json_encode` round-trips it: an empty object becomes an empty array.
/// Non-empty objects and arrays are unchanged, recursively.
pub fn php_json(value: &Value) -> Value {
    match value {
        Value::Object(map) if map.is_empty() => Value::Array(Vec::new()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), php_json(value)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(php_json).collect()),
        other => other.clone(),
    }
}

/// The upstream message `ApiController::checkContentType` throws for a write
/// method whose `Content-Type` is not JSON or multipart.
pub fn content_type_error(media_type: Option<&str>) -> ApiError {
    ApiError::UnsupportedMediaType {
        message: format!(
            "Invalid Content-Type header. Expecting \"application/json\", got \"{}\".",
            media_type.unwrap_or("none")
        ),
    }
}
