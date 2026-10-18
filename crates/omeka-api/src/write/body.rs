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

use serde_json::{Map, Value, json};

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

    /// The `o:id` of a `{"o:id": N}` reference member when PHP `is_numeric`
    /// accepts it.
    ///
    /// Every upstream guard written `is_numeric($data['o:x']['o:id'])` reads a
    /// reference this way, `hydrateResourceClass`, `hydrateResourceTemplate`,
    /// `hydrateThumbnail` and `hydrateOwner` (`AbstractEntityAdapter`) among
    /// them. A number and a numeric string both name a row, so `12`, `"12"`,
    /// `" 12"`, `"12.0"` and `"1.2e1"` all name 12, while `"abc"`, `""`, `true`
    /// and `null` name none and clear the reference.
    pub fn numeric_ref(&self, key: &str) -> Option<&Value> {
        self.present_ref(key).filter(|value| is_numeric(value))
    }

    /// The `o:id` of a `{"o:id": N}` reference member when PHP `isset` accepts
    /// it, so the member is an object whose `o:id` is present and not null.
    ///
    /// `MediaAdapter::hydrate` (`o:item`), `SitePageAdapter` (`o:site`) and
    /// `SiteAdapter` (`o:page`, `o:site_permission`, `o:site_item_set`) look a
    /// reference up with no `is_numeric` guard, so a present non-numeric value
    /// reaches the lookup and comes back 404.
    pub fn present_ref(&self, key: &str) -> Option<&Value> {
        object_id(self.object.get(key)?)
    }

    /// The object's own `o:id` when PHP `isset` accepts it, for a member whose
    /// entries are the references (`o:page`, `o:site_permission`).
    pub fn id(&self) -> Option<&Value> {
        self.object.get("o:id").filter(|value| !value.is_null())
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

    /// The whole body as a JSON object, in body order.
    ///
    /// An upstream ingester reads `Request::getContent()` directly, so the
    /// media write hands it the body as it arrived rather than the value
    /// objects the shared hydrate reads.
    pub fn object(&self) -> &Map<String, Value> {
        &self.object
    }

    /// The body re-encoded the way PHP round-trips it: `php_json` over every
    /// member, in body order. This is the `data` an ingester that keeps the
    /// whole request stores (`Ingester\Html::ingest`).
    pub fn php_object(&self) -> Map<String, Value> {
        self.object
            .iter()
            .map(|(key, value)| (key.clone(), php_json(value)))
            .collect()
    }

    /// A member read the way a bare PHP `if ($value)` reads it: absent, `null`,
    /// `false`, `0`, an empty string, `"0"` and an empty array or object are
    /// all false.
    pub fn truthy(&self, key: &str) -> bool {
        self.object.get(key).is_some_and(|value| !is_empty(value))
    }
}

/// `isset($object['o:id'])`: the `o:id` of a JSON object, when it is present
/// and not null.
pub fn object_id(object: &Value) -> Option<&Value> {
    object
        .as_object()?
        .get("o:id")
        .filter(|value| !value.is_null())
}

/// A list entry that names a reference: the entry's `o:id` when the entry is an
/// object, otherwise the entry itself when PHP `is_numeric` accepts it.
///
/// `ItemAdapter::hydrate` reads the `o:item_set` and `o:site` lists this way,
/// and either shape reaches the same `findEntity`.
pub fn entry_ref(entry: &Value) -> Option<&Value> {
    match entry {
        Value::Object(_) => object_id(entry),
        value if is_numeric(value) => Some(value),
        _ => None,
    }
}

/// PHP `empty()`, for the one guard that uses it: a template property id that
/// is empty is skipped rather than looked up.
pub fn is_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Bool(flag) => !*flag,
        Value::Number(number) => number.as_f64() == Some(0.0),
        Value::String(text) => text.is_empty() || text == "0",
        Value::Array(items) => items.is_empty(),
        Value::Object(map) => map.is_empty(),
    }
}

/// PHP's loose `in_array` comparison between two reference values: two numeric
/// values are equal when their numbers are, and any other pair only when the
/// values are the same JSON.
pub fn same_reference(left: &Value, right: &Value) -> bool {
    match (is_numeric(left), is_numeric(right)) {
        (true, true) => mysql_number(left) == mysql_number(right),
        _ => left == right,
    }
}

/// A value as PHP interpolates it into a message string: a string as itself, a
/// bool as `1` or the empty string, and a number as JSON prints it.
pub fn php_display(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Bool(true) => "1".to_string(),
        Value::Bool(false) => String::new(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// PHP's array-offset coercion, the key `$itemMedia[$id]` uses: a number
/// truncates toward zero, a bool is `1` or `0`, and a string is an int only
/// when it is a canonical decimal integer, so `"12"` matches the key 12 while
/// `" 12"`, `"12.0"`, `"+12"` and `"012"` stay string keys and match nothing.
pub fn array_key(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|float| float.trunc() as i64)),
        Value::Bool(flag) => Some(i64::from(*flag)),
        Value::String(text) => canonical_int(text),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

/// A string PHP converts to an integer array key: an optional `-`, at least one
/// digit, and no leading zero unless the value is `0`.
fn canonical_int(text: &str) -> Option<i64> {
    let digits = text.strip_prefix('-').unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if digits.len() > 1 && digits.starts_with('0') {
        return None;
    }
    text.parse::<i64>().ok()
}

/// PHP `is_numeric`: a number, or a string PHP reads whole as one. A bool, an
/// array, an object and `null` are not numeric.
pub fn is_numeric(value: &Value) -> bool {
    match value {
        Value::Number(_) => true,
        Value::String(text) => {
            let whole = text.trim_end_matches(php_space);
            leading_number(text).is_some_and(|(_, used)| used == whole.len())
        }
        _ => false,
    }
}

/// The number MySQL compares against a numeric column when the column is
/// equated with `value`: a number as it is, a bool as `1` or `0`, and a string
/// as its leading number.
fn mysql_number(value: &Value) -> f64 {
    match value {
        Value::Number(number) => number.as_f64().unwrap_or(0.0),
        Value::Bool(true) => 1.0,
        Value::Bool(false) => 0.0,
        Value::String(text) => leading_number(text).map(|(value, _)| value).unwrap_or(0.0),
        _ => 0.0,
    }
}

/// The stored id a reference names, or `None` when the comparison matches no
/// row: `0`, a negative, a fraction, a value past the id range, or a string
/// with no leading number. Ids start at 1, so `0` matches nothing.
pub fn ref_row(value: &Value) -> Option<u64> {
    let number = mysql_number(value);
    let whole = number.is_finite() && number >= 1.0 && number.fract() == 0.0;
    (whole && number <= u64::MAX as f64).then_some(number as u64)
}

/// `(int)` of a value PHP reads as a number, truncated toward zero, as
/// `AbstractEntityAdapter::hydrateOwner` casts the id the body named.
pub fn cast_int(value: &Value) -> Option<i64> {
    let number = match value {
        Value::Number(number) => number.as_f64()?,
        Value::String(_) if is_numeric(value) => leading_number(value.as_str()?).map(|(n, _)| n)?,
        _ => return None,
    };
    Some(number.trunc() as i64)
}

/// The criteria `AbstractEntityAdapter::findEntity` prints in its 404, which is
/// `json_encode(['id' => $value])` over the value the body carried.
pub fn ref_criteria(value: &Value) -> String {
    json!({ "id": value }).to_string()
}

/// Whether a character is one of the six whitespace bytes PHP's `is_numeric`
/// and MySQL's string-to-number conversion skip.
fn php_space(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\n' | '\r' | '\u{b}' | '\u{c}')
}

/// The longest number at the start of `text`, with the byte offset it ends at.
///
/// MySQL reads a string compared with a numeric column this way, so `"1abc"`
/// is `1` and `"abc"` is `0`. PHP `is_numeric` reads the same grammar and then
/// requires the whole string.
fn leading_number(text: &str) -> Option<(f64, usize)> {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() && matches!(bytes[index], b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
    {
        index += 1;
    }
    let start = index;
    if index < bytes.len() && matches!(bytes[index], b'+' | b'-') {
        index += 1;
    }
    let integer_start = index;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    let integer_digits = index - integer_start;
    let mut fraction_digits = 0;
    if index < bytes.len() && bytes[index] == b'.' {
        index += 1;
        let fraction_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        fraction_digits = index - fraction_start;
    }
    if integer_digits == 0 && fraction_digits == 0 {
        return None;
    }
    let mut end = index;
    if index < bytes.len() && matches!(bytes[index], b'e' | b'E') {
        let mut exponent = index + 1;
        if exponent < bytes.len() && matches!(bytes[exponent], b'+' | b'-') {
            exponent += 1;
        }
        let exponent_start = exponent;
        while exponent < bytes.len() && bytes[exponent].is_ascii_digit() {
            exponent += 1;
        }
        if exponent > exponent_start {
            end = exponent;
        }
    }
    // The slice drops the leading whitespace. Rust parses `12.`, `.5` and
    // `+12` the way MySQL and PHP read them.
    let value = text[start..end].parse::<f64>().ok()?;
    Some((value, end))
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
