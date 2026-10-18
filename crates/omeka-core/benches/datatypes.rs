//! Micro-benches for the data type registry.
//!
//! Covers `crates/omeka-core/src/datatypes.rs`: the name lookup, the six
//! registered types' validity and hydration, the JSON-LD, string and fulltext
//! output, conversion, and the extract lookup.
//!
//! Run with `cargo bench -p omeka-core --bench datatypes`. The numbers, their
//! limiters and the checks that did not apply are in `crates/omeka-core/README.md`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use omeka_core::datatypes::{DataType, Registry};
use omeka_core::ids::{PropertyId, ResourceId};
use omeka_core::value::{LangTag, ResourceDataType, Value, ValueData, ValueObject};

fn value(data: ValueData) -> Value {
    Value::new(ResourceId::new(1), PropertyId::new(2), data)
}

fn object(json: serde_json::Value) -> ValueObject {
    ValueObject::parse(&json).unwrap()
}

fn datatypes(c: &mut Criterion) {
    let registry = Registry::core();
    let literal_object = object(serde_json::json!({
        "type": "literal","property_id": 2,"@value": "A resource title","@language": "en-US",
    }));
    let uri_object = object(serde_json::json!({
        "type": "uri","property_id": 3,"@id": "https://example.org/a","o:label": "A label",
    }));
    let resource_object = object(serde_json::json!({
        "type": "resource:item","property_id": 4,"value_resource_id": 99,
    }));
    let empty_literal_object = object(serde_json::json!({
        "type": "literal","property_id": 2,"@value": "   ",
    }));
    let literal = value(ValueData::literal("A resource title").unwrap())
        .with_lang(LangTag::parse("en-US").unwrap());
    let uri = value(
        ValueData::uri_with_label("https://example.org/a", Some("A label".to_string())).unwrap(),
    );
    let resource = value(ValueData::resource(
        ResourceId::new(99),
        ResourceDataType::Item,
    ));

    // Run once, outside the timed region: a case that started failing would
    // otherwise be timed as an error path.
    assert_eq!(registry.iter().count(), 6);
    assert!(registry.get("literal").is_some());
    assert!(registry.get("missing").is_none());
    assert!(registry.is_registered("literal"));
    assert!(!registry.is_registered("missing"));
    assert!(registry.hydrate(&literal_object).is_ok());
    assert!(registry.hydrate(&uri_object).is_ok());
    assert!(registry.hydrate(&resource_object).is_ok());
    assert!(registry.hydrate(&empty_literal_object).is_err());
    assert!(registry.get("literal").unwrap().json_ld(&literal).is_some());
    assert!(
        registry
            .get("resource:item")
            .unwrap()
            .json_ld(&resource)
            .is_none()
    );
    assert_eq!(registry.get_for_extract(&literal.data).name(), "literal");
    assert_eq!(
        registry.get("uri").unwrap().fulltext(&uri).as_deref(),
        Some("https://example.org/a A label")
    );
    assert!(registry.convert(
        &mut value(ValueData::uri("https://example.org/a").unwrap()),
        "literal"
    ));
    assert!(registry.convert(&mut value(ValueData::literal("just text").unwrap()), "uri"));
    assert!(!registry.convert(&mut resource.clone(), "resource:item"));

    let mut group = c.benchmark_group("core/datatypes");
    group
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(50);

    for name in ["literal", "resource:itemset", "missing"] {
        group.bench_function(format!("get/{name}"), |b| {
            b.iter(|| registry.get(black_box(name)))
        });
        group.bench_function(format!("is_registered/{name}"), |b| {
            b.iter(|| registry.is_registered(black_box(name)))
        });
    }

    for (name, item) in [
        ("literal", &literal_object),
        ("uri", &uri_object),
        ("resource_item", &resource_object),
        ("invalid_literal", &empty_literal_object),
    ] {
        group.bench_function(format!("hydrate/{name}"), |b| {
            b.iter(|| registry.hydrate(black_box(item)))
        });
    }

    for (name, data_type, item) in [("literal", "literal", &literal), ("uri", "uri", &uri)] {
        group.bench_function(format!("json_ld/{name}"), |b| {
            b.iter(|| registry.get(data_type).unwrap().json_ld(black_box(item)))
        });
        group.bench_function(format!("to_string/{name}"), |b| {
            b.iter(|| registry.get(data_type).unwrap().to_string(black_box(item)))
        });
    }
    group.bench_function("fulltext/uri", |b| {
        b.iter(|| registry.get("uri").unwrap().fulltext(black_box(&uri)))
    });

    group.bench_function("convert/literal_from_uri", |b| {
        b.iter_batched(
            || value(ValueData::uri("https://example.org/a").unwrap()),
            |mut item| {
                let converted = registry.convert(&mut item, "literal");
                black_box((converted, item))
            },
            criterion::BatchSize::SmallInput,
        )
    });
    group.bench_function("convert/uri_from_literal", |b| {
        b.iter_batched(
            || value(ValueData::literal("just text").unwrap()),
            |mut item| {
                let converted = registry.convert(&mut item, "uri");
                black_box((converted, item))
            },
            criterion::BatchSize::SmallInput,
        )
    });
    group.bench_function("convert/resource_noop", |b| {
        b.iter_batched(
            || resource.clone(),
            |mut item| {
                let converted = registry.convert(&mut item, "resource:item");
                black_box((converted, item))
            },
            criterion::BatchSize::SmallInput,
        )
    });

    for (name, data) in [
        ("literal", literal.data.clone()),
        ("resource_item", resource.data.clone()),
    ] {
        group.bench_function(format!("get_for_extract/{name}"), |b| {
            b.iter(|| registry.get_for_extract(black_box(&data)))
        });
    }

    group.bench_function("scan/names", |b| {
        b.iter(|| {
            registry
                .iter()
                .map(DataType::name)
                .collect::<Vec<&'static str>>()
        })
    });
    group.bench_function("scan/value_annotating", |b| {
        b.iter(|| {
            registry
                .iter()
                .filter(|data_type| data_type.value_annotating())
                .map(DataType::name)
                .collect::<Vec<&'static str>>()
        })
    });

    group.finish();
}

criterion_group!(benches, datatypes);
criterion_main!(benches);
