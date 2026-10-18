//! Generated coverage for the two boundary parsers: the query string
//! (`read::params`) and the request body (`write::body`).
//!
//! Every property asserts a typed outcome. A malformed body must come back as
//! `ApiError::BadRequest` with a 400 status, not as a panic or a wildcard
//! error, and query parsing must be total for any string a client can send.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use indexmap::IndexMap;
use omeka_api::errors::ApiError;
use omeka_api::read::Window;
use omeka_api::read::params::Params;
use omeka_api::write::body::Payload;
use proptest::prelude::*;
use serde_json::Value as Json;

/// The two sentences upstream `ApiController::jsonDecode` and
/// `ApiJsonStrategy` can put in a 400 body.
const JSON_MESSAGES: [&str; 2] = [
    "JSON: Syntax error",
    "JSON: Content must be an object or array.",
];

/// Characters that carry no escape sequence, so a generated query decodes to
/// itself and the expected result is exactly the string that was sent.
fn plain_name() -> BoxedStrategy<String> {
    prop::collection::vec(
        prop::sample::select(vec!['a', 'b', 'z', '0', '9', '_', '-', '.', ':', '/']),
        0..6,
    )
    .prop_map(|chars| chars.into_iter().collect())
    .boxed()
}

fn plain_value() -> BoxedStrategy<String> {
    prop::collection::vec(
        prop::sample::select(vec!['a', 'b', 'z', '0', '9', '_', '-', '.', ':', '/']),
        0..8,
    )
    .prop_map(|chars| chars.into_iter().collect())
    .boxed()
}

/// A `name=value` pair the browse-default plugin also writes.
const RESERVED: [&str; 5] = [
    "sort_by",
    "sort_order",
    "sort_by_default",
    "sort_order_default",
    "page",
];

/// A pair whose name the plugin never writes, so the plugin's own parameters
/// can be told apart from the caller's.
fn plain_foreign_pair() -> BoxedStrategy<(String, String)> {
    (plain_name(), plain_value())
        .prop_filter("the plugin writes this name itself", |(name, _)| {
            !RESERVED.contains(&name.as_str())
        })
        .boxed()
}

fn query_string(pairs: &[(String, String)]) -> String {
    pairs
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("&")
}

/// Text an arbitrary JSON string can hold.
fn text() -> BoxedStrategy<String> {
    prop_oneof![
        prop::collection::vec(any::<char>(), 0..24),
        prop::collection::vec(
            prop::sample::select(vec!['\\', '"', '\n', '\u{1}', 'é', '中']),
            0..8
        ),
    ]
    .prop_map(|chars| chars.into_iter().collect())
    .boxed()
}

/// An arbitrary JSON document. The depth bounds the recursion.
fn json_value(depth: u32) -> BoxedStrategy<Json> {
    let leaf = prop_oneof![
        Just(Json::Null),
        any::<bool>().prop_map(Json::Bool),
        any::<i64>().prop_map(|number| Json::Number(number.into())),
        text().prop_map(Json::String),
    ];
    if depth == 0 {
        leaf.boxed()
    } else {
        prop_oneof![
            leaf.clone(),
            prop::collection::vec(json_value(depth - 1), 0..3).prop_map(Json::Array),
            prop::collection::vec((text(), json_value(depth - 1)), 0..3)
                .prop_map(|members| Json::Object(members.into_iter().collect())),
        ]
        .boxed()
    }
}

/// Text drawn from the alphabet JSON is written in, so the parser sees
/// plausible near-misses rather than overwhelmingly non-JSON bytes.
fn json_like_text() -> BoxedStrategy<String> {
    prop::collection::vec(
        prop::sample::select(vec![
            '{', '}', '[', ']', '"', ':', ',', '0', '1', 'a', 'e', 'n', '\\', ' ', '-',
        ]),
        0..24,
    )
    .prop_map(|chars| chars.into_iter().collect())
    .boxed()
}

/// A request body: arbitrary bytes, JSON-like text, and valid JSON.
fn body_bytes() -> BoxedStrategy<Vec<u8>> {
    prop_oneof![
        prop::collection::vec(any::<u8>(), 0..256),
        json_like_text().prop_map(String::into_bytes),
        json_value(3).prop_map(|value| serde_json::to_vec(&value).unwrap()),
    ]
    .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// A query with no escapes in it splits into exactly the pairs that were
    /// sent, in order, and reads back last-value-wins like PHP `parse_str`.
    #[test]
    fn a_plain_query_reads_back_as_its_pairs(
        pairs in prop::collection::vec((plain_name(), plain_value()), 0..4)
    ) {
        let query = query_string(&pairs);
        let params = Params::parse(&query);
        prop_assert_eq!(params.raw_pairs().to_vec(), pairs.clone());
        let expected: IndexMap<String, String> = pairs.iter().cloned().collect();
        let actual: IndexMap<String, String> = params
            .pairs()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();
        prop_assert_eq!(actual, expected);
    }

    /// Query parsing is total: any string is accepted and the raw pairs are
    /// exactly the non-empty `&`-separated parts, each split on its first `=`.
    #[test]
    fn query_parsing_is_total(query in any::<String>()) {
        let params = Params::parse(&query);
        let expected: Vec<(String, String)> = query
            .split('&')
            .filter(|part| !part.is_empty())
            .map(|part| {
                let (name, value) = part.split_once('=').unwrap_or((part, ""));
                (name.to_string(), value.to_string())
            })
            .collect();
        prop_assert_eq!(params.raw_pairs().to_vec(), expected);
        prop_assert!(params.pairs().count() <= params.raw_pairs().len());
    }

    /// `browse_pairs` adds the five parameters `SetBrowseDefaults` writes,
    /// leaves the values the plugin computes, and keeps every other pair the
    /// caller sent, in order.
    #[test]
    fn browse_pairs_writes_the_set_browse_defaults(
        pairs in prop::collection::vec(plain_foreign_pair(), 0..4),
        page in any::<i64>(),
    ) {
        let query = query_string(&pairs);
        let params = Params::parse(&query);
        let got = params.browse_pairs(page);
        let last = |name: &str| {
            got.iter()
                .rev()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        };
        let count =
            |name: &str| got.iter().filter(|(key, _)| key == name).count();

        prop_assert_eq!(last("page"), Some(page.to_string()));
        prop_assert!(count("page") >= 1);

        let sent_sort_by = params.get("sort_by");
        let sent_sort_order = params.get("sort_order");
        prop_assert_eq!(
            last("sort_by"),
            Some(sent_sort_by.unwrap_or("id").to_string())
        );
        prop_assert_eq!(
            last("sort_order"),
            Some(sent_sort_order.unwrap_or("asc").to_string())
        );
        // The caller never sends a reserved name, so each `_default` parameter
        // is present exactly when the caller did not sort.
        prop_assert_eq!(count("sort_by_default") == 1, sent_sort_by.is_none());
        prop_assert_eq!(
            count("sort_order_default") == 1,
            sent_sort_by.is_none() && sent_sort_order.is_none()
        );

        let foreign = |key: &String| !RESERVED.contains(&key.as_str());
        let kept: Vec<&(String, String)> =
            got.iter().filter(|(key, _)| foreign(key)).collect();
        let sent: Vec<&(String, String)> =
            pairs.iter().filter(|(key, _)| foreign(key)).collect();
        prop_assert_eq!(kept, sent);
    }

    /// The list window is total for any page and per_page a client can send,
    /// and its offset is `per_page * (page - 1)` computed without overflow.
    #[test]
    fn window_from_params_is_total(page in any::<i64>(), per_page in any::<i64>()) {
        let query = format!("page={page}&per_page={per_page}");
        let window = Window::from_params(&Params::parse(&query));
        prop_assert!(window.page >= 1);
        prop_assert!(window.per_page >= 1);
        let exact = (window.per_page as u128) * (window.page as u128 - 1);
        let expected = i64::try_from(exact).unwrap_or(i64::MAX);
        prop_assert_eq!(window.offset, expected);
    }

    /// The page count is `ceil(total / per_page)` over the whole range.
    #[test]
    fn page_count_is_total(total in any::<i64>(), per_page in 1i64..=i64::MAX) {
        let window = Window { page: 1, per_page, offset: 0 };
        let expected = if total <= 0 {
            0
        } else {
            i64::try_from((total as u128).div_ceil(per_page as u128)).unwrap()
        };
        prop_assert_eq!(window.page_count(total), expected);
    }

    /// Any body is either an object or a typed 400. An accepted body keeps
    /// every member in the order it arrived.
    #[test]
    fn body_parsing_is_total_and_typed(bytes in body_bytes()) {
        match Payload::parse(&bytes) {
            Ok(payload) => {
                let value: Json = serde_json::from_slice(&bytes)
                    .expect("an accepted body must be JSON");
                let object = value.as_object().expect("an accepted body must be an object");
                let sent: Vec<String> = object.keys().cloned().collect();
                let read: Vec<String> =
                    payload.members().map(|(key, _)| key.clone()).collect();
                prop_assert_eq!(read, sent);
            }
            Err(err) => {
                prop_assert_eq!(err.status(), http::StatusCode::BAD_REQUEST);
                match err {
                    ApiError::BadRequest { message } => prop_assert!(
                        JSON_MESSAGES.contains(&message.as_str()),
                        "unexpected message {:?}",
                        message
                    ),
                    other => prop_assert!(false, "unexpected error {other:?}"),
                }
            }
        }
    }

    /// Every JSON object is accepted and keeps each member's value.
    #[test]
    fn every_json_object_parses(
        members in prop::collection::vec((text(), json_value(2)), 0..6)
    ) {
        let object: serde_json::Map<String, Json> = members.into_iter().collect();
        let bytes = serde_json::to_vec(&Json::Object(object.clone())).unwrap();
        let payload = Payload::parse(&bytes).unwrap();
        for (key, value) in &object {
            prop_assert_eq!(payload.get(key), Some(value));
        }
    }

    /// A generated body that is not an object is rejected with upstream's
    /// sentence for that case, never with a panic.
    #[test]
    fn a_non_object_body_is_rejected_with_the_upstream_message(
        value in json_value(2).prop_filter("not an object", |value| !value.is_object())
    ) {
        let bytes = serde_json::to_vec(&value).unwrap();
        let err = Payload::parse(&bytes).unwrap_err();
        match err {
            ApiError::BadRequest { message } => prop_assert_eq!(
                message,
                "JSON: Content must be an object or array."
            ),
            other => prop_assert!(false, "unexpected error {other:?}"),
        }
    }
}

/// The offset of the largest page and page size a client can send saturates
/// instead of overflowing. Before the fix this panicked with "attempt to
/// multiply with overflow" in a debug build.
#[test]
fn a_huge_page_and_per_page_do_not_overflow_the_offset() {
    let params = Params::parse("page=9223372036854775807&per_page=9223372036854775807");
    let window = Window::from_params(&params);
    assert_eq!(window.page, i64::MAX);
    assert_eq!(window.per_page, i64::MAX);
    assert_eq!(window.offset, i64::MAX);
}

/// A page size of `i64::MAX` on a ten-row result is one page. Before the fix
/// `page_count` panicked with "attempt to add with overflow".
#[test]
fn a_huge_per_page_does_not_overflow_the_page_count() {
    let window = Window::from_params(&Params::parse("per_page=9223372036854775807"));
    assert_eq!(window.page_count(10), 1);
    assert_eq!(window.page_count(i64::MAX), 1);
    assert_eq!(window.page_count(0), 0);
}

#[test]
fn an_empty_object_body_parses() {
    let payload = Payload::parse(b"{}").unwrap();
    assert_eq!(payload.members().count(), 0);
}

/// Upstream `ApiController::jsonDecode`
/// (`application/src/Controller/ApiController.php:295`) accepts a JSON list,
/// because `json_decode($string, true)` returns an array for both shapes. This
/// port's `Payload` models an object only, so a list is a 400 whose message
/// names both shapes. The behaviour is frozen here and the divergence is in
/// the unit report.
#[test]
fn a_json_list_is_rejected_even_though_upstream_accepts_it() {
    let err = Payload::parse(br#"[{"o:id": 1}]"#).unwrap_err();
    assert!(matches!(&err, ApiError::BadRequest { .. }), "{err:?}");
    assert_eq!(err.status(), http::StatusCode::BAD_REQUEST);
}

/// The numerics behind a reference id: PHP `is_numeric`, MySQL's reading of the
/// string in a comparison with the id column, and PHP's `(int)` cast. The rows
/// are the grammar the HTTP table in `crates/omeka-server/tests/reference_ids.rs`
/// exercises end to end.
#[test]
fn reference_numbers_are_read_as_php_and_mysql_read_them() {
    use omeka_api::write::body::{cast_int, is_numeric, ref_row};

    let cases: [(&str, bool, Option<u64>, Option<i64>); 17] = [
        ("12", true, Some(12), Some(12)),
        (" 12", true, Some(12), Some(12)),
        ("12 ", true, Some(12), Some(12)),
        ("12.0", true, Some(12), Some(12)),
        ("12.5", true, None, Some(12)),
        ("12.", true, Some(12), Some(12)),
        (".5", true, None, Some(0)),
        ("1e3", true, Some(1000), Some(1000)),
        ("1.5e3", true, Some(1500), Some(1500)),
        ("-1", true, None, Some(-1)),
        ("0", true, None, Some(0)),
        ("0.0", true, None, Some(0)),
        ("1e", false, Some(1), None),
        ("12abc", false, Some(12), None),
        ("abc", false, None, None),
        ("", false, None, None),
        ("0x10", false, None, None),
    ];
    for (text, numeric, row, cast) in cases {
        let value = Json::String(text.to_string());
        assert_eq!(is_numeric(&value), numeric, "is_numeric({text:?})");
        assert_eq!(ref_row(&value), row, "ref_row({text:?})");
        assert_eq!(cast_int(&value), cast, "cast_int({text:?})");
    }
}

/// PHP's array-offset coercion, the key `o:primary_media` indexes the item's
/// media with: only a whole number and a canonical integer string match an id.
#[test]
fn array_keys_follow_php_array_offset_coercion() {
    use omeka_api::write::body::array_key;

    for (text, key) in [
        ("12", Some(12)),
        ("-12", Some(-12)),
        ("0", Some(0)),
        (" 12", None),
        ("12 ", None),
        ("12.0", None),
        ("+12", None),
        ("012", None),
        ("1e3", None),
        ("", None),
    ] {
        assert_eq!(
            array_key(&Json::String(text.to_string())),
            key,
            "array_key({text:?})"
        );
    }
    assert_eq!(array_key(&Json::from(12.5)), Some(12));
    assert_eq!(array_key(&Json::Bool(true)), Some(1));
    assert_eq!(array_key(&Json::Null), None);
}
