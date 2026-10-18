//! Generated coverage for the wire layer.
//!
//! `golden.rs` freezes fifteen fixed documents. These tests cover the space
//! around them. Every file under `verification/fixtures/api/` is read, parsed
//! as its wire type and printed back byte for byte, and a file with no
//! registered type fails the test instead of being skipped. A generated value
//! inserted into an item's value map must survive serialize, parse and
//! re-serialize unchanged.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use indexmap::IndexMap;
use omeka_api::jsonld;
use omeka_api::wire::api_resource::ApiResource;
use omeka_api::wire::asset::Asset;
use omeka_api::wire::data_type::DataType;
use omeka_api::wire::item::Item;
use omeka_api::wire::item_set::ItemSet;
use omeka_api::wire::job::Job;
use omeka_api::wire::media::Media;
use omeka_api::wire::module::Module;
use omeka_api::wire::property::Property;
use omeka_api::wire::resource_class::ResourceClass;
use omeka_api::wire::resource_template::ResourceTemplate;
use omeka_api::wire::site::Site;
use omeka_api::wire::user::User;
use omeka_api::wire::value_annotation::ValueAnnotation;
use omeka_api::wire::vocabulary::Vocabulary;
use omeka_api::wire::{
    LiteralType, ResourceValueType, UriType, Value, ValueLiteral, ValueMap, ValueResource, ValueUri,
};
use proptest::prelude::*;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// The frozen fixtures, as `golden.rs` reads them.
fn fixtures_dir() -> String {
    format!(
        "{}/../../verification/fixtures/api",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn read_fixture(name: &str) -> String {
    let path = format!("{}/{name}", fixtures_dir());
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("reading {path}: {err}"))
}

/// One document parses into `T` and prints back byte for byte.
fn round_trip<T: Serialize + DeserializeOwned>(name: &str) -> Result<(), String> {
    let frozen = read_fixture(name);
    let value: T = serde_json::from_str(&frozen)
        .map_err(|err| format!("{name} does not parse as its wire type: {err}"))?;
    let emitted = jsonld::to_string_pretty(&value).map_err(|err| format!("{name}: {err}"))?;
    if emitted == frozen.trim_end() {
        Ok(())
    } else {
        Err(format!("{name} did not round-trip"))
    }
}

/// The wire type owning each fixture file. A new file in the directory fails
/// the test below until it is listed here.
fn round_trip_fixture(name: &str) -> Result<(), String> {
    match name {
        "api_resource.json" => round_trip::<ApiResource>(name),
        "asset.json" => round_trip::<Asset>(name),
        "data_type.json" => round_trip::<DataType>(name),
        "item.json" => round_trip::<Item>(name),
        "item_set.json" => round_trip::<ItemSet>(name),
        "job.json" => round_trip::<Job>(name),
        "media.json" => round_trip::<Media>(name),
        "module.json" => round_trip::<Module>(name),
        "property.json" => round_trip::<Property>(name),
        "resource_class.json" => round_trip::<ResourceClass>(name),
        "resource_template.json" => round_trip::<ResourceTemplate>(name),
        "site.json" => round_trip::<Site>(name),
        "user.json" => round_trip::<User>(name),
        "value_annotation.json" => round_trip::<ValueAnnotation>(name),
        "vocabulary.json" => round_trip::<Vocabulary>(name),
        other => Err(format!("no wire type registered for fixture {other}")),
    }
}

#[test]
fn every_golden_fixture_survives_a_round_trip() {
    let mut names: Vec<String> = std::fs::read_dir(fixtures_dir())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".json"))
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no fixtures under {}", fixtures_dir());

    let failures: Vec<String> = names
        .iter()
        .filter_map(|name| round_trip_fixture(name).err())
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} fixtures failed: {failures:#?}",
        failures.len(),
        names.len()
    );
}

/// A term as a vocabulary stores it, so a generated key looks like one the
/// server emits and never collides with a member of the document itself.
fn term() -> BoxedStrategy<String> {
    prop::collection::vec(
        prop::sample::select(vec!['a', 'b', 'z', '0', '9', '_']),
        1..10,
    )
    .prop_map(|chars| format!("dcterms:{}", chars.into_iter().collect::<String>()))
    .boxed()
}

/// Text an arbitrary JSON string can hold, including the characters that
/// force an escape and multi-byte characters.
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

/// An annotation map, or none. The depth bounds the recursion through
/// `ValueResource::annotation`.
fn annotation(depth: u32) -> BoxedStrategy<Option<ValueMap>> {
    if depth == 0 {
        Just(None).boxed()
    } else {
        prop::option::of(value_map(depth - 1)).boxed()
    }
}

fn value_map(depth: u32) -> BoxedStrategy<ValueMap> {
    prop::collection::vec((term(), prop::collection::vec(value(depth), 1..3)), 1..3)
        .prop_map(|pairs| pairs.into_iter().collect::<IndexMap<String, Vec<Value>>>())
        .boxed()
}

fn value(depth: u32) -> BoxedStrategy<Value> {
    prop_oneof![
        value_literal(depth).prop_map(Value::Literal),
        value_uri(depth).prop_map(Value::Uri),
        value_resource(depth).prop_map(Value::Resource),
    ]
    .boxed()
}

fn value_literal(depth: u32) -> BoxedStrategy<ValueLiteral> {
    (
        Just(LiteralType),
        any::<u64>(),
        text(),
        any::<bool>(),
        annotation(depth),
        text(),
        prop::option::of(text()),
    )
        .prop_map(
            |(data_type, property_id, property_label, is_public, annotation, value, language)| {
                ValueLiteral {
                    data_type,
                    property_id,
                    property_label,
                    is_public,
                    annotation,
                    value,
                    language,
                }
            },
        )
        .boxed()
}

fn value_uri(depth: u32) -> BoxedStrategy<ValueUri> {
    (
        Just(UriType),
        any::<u64>(),
        text(),
        any::<bool>(),
        annotation(depth),
        text(),
        prop::option::of(text()),
        prop::option::of(text()),
    )
        .prop_map(
            |(data_type, property_id, property_label, is_public, annotation, id, label, lang)| {
                ValueUri {
                    data_type,
                    property_id,
                    property_label,
                    is_public,
                    annotation,
                    id,
                    label,
                    lang,
                }
            },
        )
        .boxed()
}

fn value_resource(depth: u32) -> BoxedStrategy<ValueResource> {
    (
        (
            prop::sample::select(vec![
                ResourceValueType::All,
                ResourceValueType::Item,
                ResourceValueType::ItemSet,
                ResourceValueType::Media,
            ]),
            any::<u64>(),
            text(),
            any::<bool>(),
            annotation(depth),
        ),
        (text(), any::<u64>(), text()),
        (
            prop::option::of(text()),
            prop::option::of(text()),
            prop::option::of(text()),
            prop::option::of(text()),
            prop::option::of(text()),
        ),
    )
        .prop_map(
            |(
                (data_type, property_id, property_label, is_public, annotation),
                (id, value_resource_id, value_resource_name),
                (url, display_title, thumbnail_url, thumbnail_title, thumbnail_type),
            )| ValueResource {
                data_type,
                property_id,
                property_label,
                is_public,
                annotation,
                id,
                value_resource_id,
                value_resource_name,
                url,
                display_title,
                thumbnail_url,
                thumbnail_title,
                thumbnail_type,
            },
        )
        .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// A generated value list added to the frozen item document survives
    /// serialize, parse and re-serialize unchanged, values and order alike.
    #[test]
    fn a_generated_value_list_in_an_item_round_trips(values in value_map(2)) {
        let mut item: Item = serde_json::from_str(&read_fixture("item.json")).unwrap();
        for (term, list) in values {
            item.values.insert(term, list);
        }
        let emitted = jsonld::to_string_pretty(&item).unwrap();
        let parsed: Item = serde_json::from_str(&emitted).unwrap();
        prop_assert_eq!(&parsed.values, &item.values, "the value map changed");
        prop_assert_eq!(jsonld::to_string_pretty(&parsed).unwrap(), emitted);
    }

    /// Each generated value survives a round trip as itself, so no `Value`
    /// variant swallows one of its neighbours.
    #[test]
    fn a_generated_value_round_trips_as_its_own_variant(v in value(2)) {
        let emitted = serde_json::to_string(&v).unwrap();
        let parsed: Value = serde_json::from_str(&emitted).unwrap();
        prop_assert_eq!(&parsed, &v);
        prop_assert_eq!(serde_json::to_string(&parsed).unwrap(), emitted);
    }

    /// The compact and pretty forms of one document parse to the same value.
    #[test]
    fn both_serializer_forms_agree(values in value_map(2)) {
        let mut item: Item = serde_json::from_str(&read_fixture("item.json")).unwrap();
        item.values = values;
        let compact = jsonld::to_string(&item).unwrap();
        let pretty = jsonld::to_string_pretty(&item).unwrap();
        let from_compact: Item = serde_json::from_str(&compact).unwrap();
        let from_pretty: Item = serde_json::from_str(&pretty).unwrap();
        prop_assert_eq!(from_compact, from_pretty);
    }
}
