//! Micro-benches for value and language parsing and validation.
//!
//! Covers `crates/omeka-core/src/value.rs`: the language tag rule
//! (`Omeka.langIsValid`), literal and URI validity, the typed constructors, and
//! the value object parse and serialise paths the API calls.
//!
//! Run with `cargo bench -p omeka-core --bench value_rules`. The numbers, their
//! limiters and the checks that did not apply are in `crates/omeka-core/README.md`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use omeka_core::ids::{PropertyId, ResourceId};
use omeka_core::value::{
    LangTag, ResourceDataType, Value, ValueData, ValueObject, lang_tag_is_valid, literal_is_valid,
    uri_is_valid,
};

/// A spread across the branches of `lang_tag_is_valid`: a bare language, a
/// region, a script and region, a full extension chain, and one rejection.
const LANG_TAGS: [(&str, &str); 5] = [
    ("en", "en"),
    ("en-US", "en-US"),
    ("sr-Latn-RS", "sr-Latn-RS"),
    ("en-US-u-ca-gregory", "en-US-u-ca-gregory"),
    ("english-invalid", "english"),
];

/// One URI per branch of `uri_is_valid`.
const URIS: [(&str, &str); 4] = [
    ("clean", "https://example.org/files/logo-iiif-34x30.png"),
    ("whitespace", "  https://example.org/a\tb\r\n"),
    ("javascript", "javascript:alert(1)"),
    ("control", "https://example.org/\u{1}"),
];

/// One value object of each shape, in the wire form the API reads.
fn object_json(shape: &str) -> serde_json::Value {
    match shape {
        "literal" => serde_json::json!({
            "type": "literal",
            "property_id": 2,
            "@value": "A resource title",
            "@language": "en-US",
            "is_public": true,
        }),
        "uri" => serde_json::json!({
            "type": "uri",
            "property_id": 3,
            "@id": "https://example.org/files/logo-iiif-34x30.png",
            "o:label": "A label",
            "is_public": true,
        }),
        _ => serde_json::json!({
            "type": "resource:item",
            "property_id": 4,
            "value_resource_id": 99,
            "is_public": true,
        }),
    }
}

/// A list body of 25 value objects, the shape a 25-row item list carries.
fn list_body() -> String {
    let values: Vec<serde_json::Value> = (0..25)
        .map(|index| {
            let mut value = object_json("literal");
            value["property_id"] = serde_json::json!(index % 7 + 1);
            value
        })
        .collect();
    serde_json::to_string(&values).unwrap()
}

fn value(data: ValueData) -> Value {
    Value::new(ResourceId::new(1), PropertyId::new(2), data)
}

fn value_rules(c: &mut Criterion) {
    // Run once, outside the timed region: a fixture that stopped parsing would
    // otherwise leave a bench quietly timing a rejection path.
    assert!(LangTag::parse("en-US-u-ca-gregory").is_ok());
    assert!(LangTag::parse("english").is_err());
    assert!(literal_is_valid("A resource title"));
    assert!(!literal_is_valid("   "));
    assert!(uri_is_valid(
        "https://example.org/files/logo-iiif-34x30.png"
    ));
    assert!(uri_is_valid("  https://example.org/a\tb\r\n"));
    assert!(!uri_is_valid("javascript:alert(1)"));
    assert!(!uri_is_valid("https://example.org/\u{1}"));
    assert!(ValueData::literal("A resource title").is_ok());
    assert!(ValueData::uri("https://example.org/a").is_ok());
    for shape in ["literal", "uri", "resource"] {
        assert!(ValueObject::parse(&object_json(shape)).is_ok(), "{shape}");
    }
    let body = list_body();
    assert!(serde_json::from_str::<Vec<ValueObject>>(&body).is_ok());

    let mut group = c.benchmark_group("core/value_rules");
    group
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(50);

    for (name, tag) in LANG_TAGS {
        group.bench_function(format!("lang_tag_is_valid/{name}"), |b| {
            b.iter(|| lang_tag_is_valid(black_box(tag)))
        });
    }
    group.bench_function("lang_tag_parse/valid", |b| {
        b.iter(|| LangTag::parse(black_box("en-US-u-ca-gregory")))
    });
    group.bench_function("lang_tag_parse/invalid", |b| {
        b.iter(|| LangTag::parse(black_box("english")))
    });
    group.bench_function("literal_is_valid/clean", |b| {
        b.iter(|| literal_is_valid(black_box("A resource title")))
    });
    group.bench_function("literal_is_valid/whitespace_only", |b| {
        b.iter(|| literal_is_valid(black_box("   ")))
    });
    for (name, uri) in URIS {
        group.bench_function(format!("uri_is_valid/{name}"), |b| {
            b.iter(|| uri_is_valid(black_box(uri)))
        });
    }

    group.bench_function("value_data_literal", |b| {
        b.iter(|| ValueData::literal(black_box("A resource title")))
    });
    group.bench_function("value_data_uri", |b| {
        b.iter(|| ValueData::uri(black_box("https://example.org/a")))
    });

    for shape in ["literal", "uri", "resource"] {
        let json = object_json(shape);
        group.bench_function(format!("value_object_parse/{shape}"), |b| {
            b.iter(|| ValueObject::parse(black_box(&json)))
        });
    }
    let body = list_body();
    group.bench_function("value_object_parse/list_of_25", |b| {
        b.iter(|| serde_json::from_str::<Vec<ValueObject>>(black_box(&body)))
    });

    let literal = value(ValueData::literal("A resource title").unwrap())
        .with_lang(LangTag::parse("en-US").unwrap());
    let uri = value(
        ValueData::uri_with_label("https://example.org/a", Some("A label".to_string())).unwrap(),
    );
    let resource = value(ValueData::resource(
        ResourceId::new(99),
        ResourceDataType::Item,
    ));
    for (name, item) in [
        ("literal", &literal),
        ("uri", &uri),
        ("resource", &resource),
    ] {
        group.bench_function(format!("value_to_object/{name}"), |b| {
            b.iter(|| black_box(item).to_object())
        });
    }
    group.bench_function("value_to_json/literal", |b| {
        b.iter(|| serde_json::to_value(black_box(&literal).to_object()))
    });

    group.finish();
}

criterion_group!(benches, value_rules);
criterion_main!(benches);
