//! PHP's rules for the request query: the numeric casts, and the encoding the
//! `Link` header carries.
//!
//! `ApiController::getList` builds the `Link` URLs with
//! `$this->url()->fromRoute(null, [], ['query' => $query], true)`, where
//! `$query` is the array PHP `parse_str` filled from the request. Laminas
//! assembles it in two steps:
//!
//! - `Laminas\Uri\Http::setQuery` runs `http_build_query` on the array, so a
//!   nested array becomes `name[index][field]=value` with every bracket
//!   percent-encoded, and replaces the `+` a space encodes to with `%20`.
//! - `Laminas\Uri\Uri::normalizeQuery` decodes and re-encodes the string,
//!   leaving the unreserved set plus `!$'()*,` and `:@/?` literal.
//!
//! This module rebuilds that array from the decoded request pairs and renders
//! it the same way, so a bracket parameter echoes as `id%5B0%5D=149` rather
//! than the `id[]=149` the port answered.
//!
//! [`super::params::Params::browse_pairs`] predates this module and echoes the
//! caller's raw pairs, a shape `crates/omeka-api/tests/property_boundary.rs`
//! pins. The `Link` header uses [`link_pairs`] instead, because a repeated name
//! has one value in the PHP array and an array parameter needs its indices.
//!
//! The numeric helpers mirror the casts the read surface reads a query
//! parameter through: `is_numeric` with `(int)` for `page`, `per_page`, `limit`
//! and `offset` ([`super::paging::Limit`]), and the string-to-int cast MySQL
//! applies to `buildBaseQuery`'s `id` list.

use super::params::Params;

/// One PHP query value: a scalar string, or a nested array.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Scalar(String),
    Array(Array),
}

/// A PHP array: ordered entries whose keys are the integer or string keys
/// `parse_str` builds. A list's keys are the decimal indices PHP assigns.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Array {
    entries: Vec<(String, Value)>,
}

impl Array {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    /// Set `key`, replacing the value in place when the key exists, which is
    /// what PHP `$query['key'] = ...` does to the array's order.
    pub fn set(&mut self, key: &str, value: Value) {
        match self.entries.iter_mut().find(|(name, _)| name == key) {
            Some((_, existing)) => *existing = value,
            None => self.entries.push((key.to_string(), value)),
        }
    }

    pub fn entries(&self) -> &[(String, Value)] {
        &self.entries
    }

    /// The array a nested write reaches: `name` becomes an empty array when it
    /// is absent, and stays its scalar when it is one (`parse_str` reports an
    /// error there and keeps the scalar).
    fn array_at(&mut self, key: &str) -> Option<&mut Array> {
        if self.get(key).is_none() {
            self.set(key, Value::Array(Array::default()));
        }
        match self.entries.iter_mut().find(|(name, _)| name == key) {
            Some((_, Value::Array(child))) => Some(child),
            _ => None,
        }
    }
}

/// The PHP array `parse_str` builds from decoded request pairs.
pub fn parse(pairs: &[(String, String)]) -> Array {
    let mut array = Array::default();
    for (key, value) in pairs {
        let (name, parts) = path(key);
        match parts.split_first() {
            None => array.set(&name, Value::Scalar(value.clone())),
            Some((head, rest)) => {
                let Some(child) = array.array_at(&name) else {
                    continue;
                };
                insert(child, head, rest, value.clone());
            }
        }
    }
    array
}

/// `http_build_query` over the array, then the `normalizeQuery` re-encoding:
/// one `(name, value)` pair per scalar, both still percent-encoded.
pub fn encode(array: &Array) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    push_pairs(array, "", &mut pairs);
    pairs
}

/// The query the `Link` header carries: the request pairs as the PHP array,
/// with the five parameters `SetBrowseDefaults` and `getList` write, rendered
/// as the Laminas URL helper renders them.
pub fn link_pairs(params: &Params, page: i64) -> Vec<(String, String)> {
    let mut query = parse(&params.decoded_pairs());
    let raw_sort_by = params.get("sort_by");
    let raw_sort_order = params.get("sort_order");
    let sort_by_default = matches!(raw_sort_by, None | Some(""));
    if sort_by_default {
        query.set("sort_by_default", Value::Scalar(String::new()));
    }
    if sort_by_default && matches!(raw_sort_order, None | Some("")) {
        query.set("sort_order_default", Value::Scalar(String::new()));
    }
    query.set(
        "sort_by",
        Value::Scalar(raw_sort_by.unwrap_or("id").to_string()),
    );
    query.set(
        "sort_order",
        Value::Scalar(raw_sort_order.unwrap_or("asc").to_string()),
    );
    query.set("page", Value::Scalar(page.to_string()));
    encode(&query)
}

/// The name and bracket parts of a request key: `property[0][type]` is
/// `("property", ["0", "type"])` and `id[]` is `("id", [""])`. A key with no
/// bracket is a scalar name and keeps no parts.
fn path(key: &str) -> (String, Vec<String>) {
    let Some((name, rest)) = key.split_once('[') else {
        return (key.to_string(), Vec::new());
    };
    let mut parts = Vec::new();
    let mut rest = rest;
    while let Some((part, tail)) = rest.split_once(']') {
        parts.push(part.to_string());
        match tail.strip_prefix('[') {
            Some(next) => rest = next,
            None => break,
        }
    }
    (name.to_string(), parts)
}

/// Write one bracket part, then the rest of its chain.
fn insert(array: &mut Array, part: &str, rest: &[String], value: String) {
    let key = if part.is_empty() {
        next_index(array)
    } else {
        part.to_string()
    };
    match rest.split_first() {
        None => array.set(&key, Value::Scalar(value)),
        Some((head, tail)) => {
            let Some(child) = array.array_at(&key) else {
                return;
            };
            insert(child, head, tail, value);
        }
    }
}

/// The next free integer key, which is one past the largest integer key PHP
/// would have made an `int` out of.
fn next_index(array: &Array) -> String {
    let mut next = 0i64;
    for (key, _) in array.entries() {
        if let Some(index) = integer_key(key)
            && index >= next
        {
            next = index.saturating_add(1);
        }
    }
    next.to_string()
}

/// A PHP integer array key: decimal digits without a leading zero. `"01"` stays
/// a string key to PHP, so it is not one here either.
fn integer_key(key: &str) -> Option<i64> {
    if key.is_empty() || !key.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if key.len() > 1 && key.starts_with('0') {
        return None;
    }
    key.parse().ok()
}

fn push_pairs(array: &Array, prefix: &str, out: &mut Vec<(String, String)>) {
    for (key, value) in array.entries() {
        let name = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}[{key}]")
        };
        match value {
            Value::Scalar(text) => out.push((encode_component(&name), encode_component(text))),
            Value::Array(child) => push_pairs(child, &name, out),
        }
    }
}

/// The byte `Laminas\Uri\Uri::normalizeQuery` leaves literal.
fn is_literal(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'_' | b'-'
                | b'.'
                | b'~'
                | b'!'
                | b'$'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b','
                | b':'
                | b'@'
                | b'/'
                | b'?'
        )
}

/// One key or value as the `Link` query writes it: a space is `%20`, an
/// otherwise-unreserved byte stays, and every other byte is `%XX` uppercase.
fn encode_component(text: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if is_literal(byte) {
            out.push(byte as char);
        } else if byte == b' ' {
            out.push_str("%20");
        } else {
            out.push('%');
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    out
}

// ------------------------------------------------------------- numeric casts

/// PHP `is_numeric($raw)` followed by `(int)`: the truncated integer of a
/// numeric string, and `None` when the whole string is not numeric.
///
/// `is_numeric` accepts surrounding whitespace, an optional sign, digits with
/// an optional fraction, and an optional exponent. It rejects `inf` and `nan`,
/// which Rust's float parser accepts, so the shape is checked first.
pub(crate) fn numeric_int(raw: &str) -> Option<i64> {
    Some(php_int(numeric_value(raw)?))
}

/// PHP `is_numeric($raw)` as a float.
fn numeric_value(raw: &str) -> Option<f64> {
    let text = raw.trim();
    let bytes = text.as_bytes();
    let mut index = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        index += 1;
    }
    let mut digits = 0;
    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
        digits += 1;
    }
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        index += 1;
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        let mut exponent = 0;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
            exponent += 1;
        }
        if exponent == 0 {
            return None;
        }
    }
    if index != bytes.len() {
        return None;
    }
    text.parse::<f64>().ok()
}

/// PHP's `(int)` of a float: truncation toward zero, `0` for a value no integer
/// can hold (`NAN` and `INF` both cast to `0`), and the nearest bound otherwise.
fn php_int(value: f64) -> i64 {
    if !value.is_finite() {
        return 0;
    }
    value as i64
}

/// PHP's `(int)` of a string: the value of its leading numeric prefix, and `0`
/// when there is none (`(int) 'abc'` and `(int) ''` are both `0`).
///
/// MySQL compares an integer column to a string the same way, which is how
/// `buildBaseQuery`'s `id` list reaches a row.
pub fn php_int_cast(raw: &str) -> i64 {
    if let Some(value) = numeric_value(raw) {
        return php_int(value);
    }
    let text = raw.trim_start();
    let (negative, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let Ok(value) = digits.parse::<f64>() else {
        return 0;
    };
    if negative {
        -php_int(value)
    } else {
        php_int(value)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn encoded(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();
        encode(&parse(&pairs))
    }

    /// The `x_ids_filter` shape: `id[]=149&id[]=150` becomes the indexed form
    /// with both brackets encoded.
    #[test]
    fn a_repeated_bracket_name_gets_zero_based_indices() {
        assert_eq!(
            encoded(&[("id[]", "149"), ("id[]", "150")]),
            vec![
                ("id%5B0%5D".to_string(), "149".to_string()),
                ("id%5B1%5D".to_string(), "150".to_string()),
            ]
        );
    }

    /// The `x_prop_search_eq` shape: nested brackets and a value with spaces.
    #[test]
    fn a_nested_bracket_name_keeps_every_part() {
        assert_eq!(
            encoded(&[
                ("property[0][property]", "1"),
                ("property[0][type]", "eq"),
                ("property[0][text]", "URI value item updated"),
            ]),
            vec![
                ("property%5B0%5D%5Bproperty%5D".to_string(), "1".to_string()),
                ("property%5B0%5D%5Btype%5D".to_string(), "eq".to_string()),
                (
                    "property%5B0%5D%5Btext%5D".to_string(),
                    "URI%20value%20item%20updated".to_string()
                ),
            ]
        );
    }

    /// A scalar value keeps `!$'()*,:@/?` and the unreserved set literal, and
    /// encodes the rest, which is what `normalizeQuery` leaves.
    #[test]
    fn a_value_keeps_the_characters_laminas_leaves_literal() {
        let (_, value) = encoded(&[("a", "b:c/d?e@f!g$h'i(j)k*l,m~n_o-p.q&r=s+t#u%v")]).remove(0);
        assert_eq!(value, "b:c/d?e@f!g$h'i(j)k*l,m~n_o-p.q%26r%3Ds%2Bt%23u%25v");
    }

    /// `http_build_query` adds an already-ordered list in place, so a repeated
    /// family keeps the position of its first element and the scalar that
    /// follows a list keeps its own.
    #[test]
    fn entries_keep_the_position_of_their_first_key() {
        assert_eq!(
            encoded(&[("a[]", "1"), ("b", "2"), ("a[]", "3")]),
            vec![
                ("a%5B0%5D".to_string(), "1".to_string()),
                ("a%5B1%5D".to_string(), "3".to_string()),
                ("b".to_string(), "2".to_string()),
            ]
        );
    }

    /// An explicit index is kept as written, and an out-of-order index keeps the
    /// arrival order.
    #[test]
    fn an_explicit_index_is_kept() {
        assert_eq!(
            encoded(&[("a[1]", "x"), ("a[0]", "y")]),
            vec![
                ("a%5B1%5D".to_string(), "x".to_string()),
                ("a%5B0%5D".to_string(), "y".to_string()),
            ]
        );
    }

    /// The five browse-default parameters are appended after the caller's, and
    /// the caller's `page` keeps its place.
    #[test]
    fn browse_defaults_are_appended_after_the_caller_pairs() {
        let params = Params::parse("item_set_id=1&page=abc&per_page=2");
        assert_eq!(
            link_pairs(&params, 1),
            vec![
                ("item_set_id".to_string(), "1".to_string()),
                ("page".to_string(), "1".to_string()),
                ("per_page".to_string(), "2".to_string()),
                ("sort_by_default".to_string(), String::new()),
                ("sort_order_default".to_string(), String::new()),
                ("sort_by".to_string(), "id".to_string()),
                ("sort_order".to_string(), "asc".to_string()),
            ]
        );
    }

    /// A caller that sorts leaves both `_default` parameters unset, which
    /// `http_build_query` drops when they hold null.
    #[test]
    fn an_explicit_sort_leaves_no_default_parameters() {
        let params = Params::parse("sort_by=id&sort_order=desc");
        assert_eq!(
            link_pairs(&params, 2),
            vec![
                ("sort_by".to_string(), "id".to_string()),
                ("sort_order".to_string(), "desc".to_string()),
                ("page".to_string(), "2".to_string()),
            ]
        );
    }

    /// A key the request did not send is appended, and one it did keeps its
    /// place.
    #[test]
    fn a_present_default_parameter_is_overwritten_in_place() {
        let params = Params::parse("sort_by_default=x&page=3");
        assert_eq!(
            link_pairs(&params, 1),
            vec![
                ("sort_by_default".to_string(), String::new()),
                ("page".to_string(), "1".to_string()),
                ("sort_order_default".to_string(), String::new()),
                ("sort_by".to_string(), "id".to_string()),
                ("sort_order".to_string(), "asc".to_string()),
            ]
        );
    }

    #[test]
    fn numeric_int_reads_a_php_numeric_string() {
        assert_eq!(numeric_int("2"), Some(2));
        assert_eq!(numeric_int(" 2 "), Some(2));
        assert_eq!(numeric_int("-3"), Some(-3));
        assert_eq!(numeric_int("1.9"), Some(1));
        assert_eq!(numeric_int("1e3"), Some(1000));
        assert_eq!(numeric_int("abc"), None);
        assert_eq!(numeric_int(""), None);
        assert_eq!(numeric_int("inf"), None);
        assert_eq!(numeric_int("1x"), None);
    }

    #[test]
    fn php_int_cast_takes_the_leading_numeric_prefix() {
        assert_eq!(php_int_cast("12"), 12);
        assert_eq!(php_int_cast("12abc"), 12);
        assert_eq!(php_int_cast("  -3 x"), -3);
        assert_eq!(php_int_cast("1.9"), 1);
        assert_eq!(php_int_cast("1e3"), 1000);
        assert_eq!(php_int_cast("abc"), 0);
        assert_eq!(php_int_cast(""), 0);
    }
}
